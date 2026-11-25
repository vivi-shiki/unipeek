//! Open any supported input (unitypackage, zip, folder, single FBX) as a `PackageIndex`.
//!
//! Zips and folders without a `.unitypackage` become a "loose" index: each FBX or image gets a
//! GUID derived from its path, so planning and binding work the same way for every container.
use crate::{
    AssetKind, Entry, Error, Guid, IndexOptions, Input, PackageIndex, Result, Source,
    package::{kind_from_extension, normalize_path, sniff},
    source::is_container,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{self, Read, Seek},
    path::{Path, PathBuf},
    sync::Arc,
};

/// Folders deeper than this are not searched; real item folders are shallow.
const MAX_DEPTH: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Container {
    UnityPackage,
    Zip,
    Folder,
    Model,
}

pub struct Package {
    pub index: PackageIndex,
    pub container: Container,
    /// The `.unitypackage` picked inside a zip or folder.
    pub member: Option<String>,
    origin: Origin,
    /// Packages merged in through a `Resolver`, with the GUIDs each one supplied.
    deps: Vec<(String, Package, BTreeSet<Guid>)>,
}

enum Origin {
    /// A unitypackage source. One-shot readers were indexed with their binaries kept.
    Package(Source),
    ZipMember { zip: ZipData, index: usize },
    /// Loose files whose bytes were kept while indexing.
    Kept,
    Files(BTreeMap<Guid, PathBuf>),
}

#[derive(Clone)]
enum ZipData {
    Path(PathBuf),
    Bytes(Arc<[u8]>),
}

trait ReadSeek: Read + Seek + Send {}
impl<T: Read + Seek + Send> ReadSeek for T {}

impl ZipData {
    fn archive(&self) -> Result<zip::ZipArchive<Box<dyn ReadSeek>>> {
        let reader: Box<dyn ReadSeek> = match self {
            ZipData::Path(p) => Box::new(io::BufReader::new(std::fs::File::open(p)?)),
            ZipData::Bytes(b) => Box::new(io::Cursor::new(b.clone())),
        };
        zip::ZipArchive::new(reader).map_err(|e| Error::Unsupported(format!("unreadable zip: {e}")))
    }
}

impl Package {
    /// Open an input and index the package it points at.
    /// Returns the inner path left for `Plan::new` (a prefab or model inside the package).
    pub fn open(input: Input, opts: &IndexOptions) -> Result<(Package, Option<String>)> {
        let Input { source, inner } = input;
        let mut inner = inner.into_iter();
        if let Source::Path(p) = &source
            && p.is_dir()
        {
            return Ok((open_folder(p, opts)?, inner.next()));
        }
        let (head, source) = peek(source)?;
        match detect(&head, &source) {
            Some(Detected::UnityPackage) => {
                let index = index_source(source, opts)?;
                Ok((Package { index: index.0, container: Container::UnityPackage, member: None, origin: Origin::Package(index.1), deps: Vec::new() }, inner.next()))
            }
            Some(Detected::Zip) => {
                let zip = match source {
                    Source::Path(p) => ZipData::Path(p),
                    Source::Bytes(b) => ZipData::Bytes(b),
                    mut other => {
                        let mut bytes = Vec::new();
                        other.open()?.read_to_end(&mut bytes)?;
                        ZipData::Bytes(Arc::from(bytes))
                    }
                };
                open_zip(zip, &mut inner, opts)
            }
            Some(Detected::Model) => Ok((open_model(source, opts)?, inner.next())),
            None => Err(Error::Unsupported(format!("{} is not a unitypackage, zip, folder or FBX", source.describe()))),
        }
    }

    /// Bytes of FBX and texture assets, reading the input again when needed. Assets that came from
    /// merged dependencies are read from their own packages, each at most once.
    pub fn read_assets(&mut self, wanted: &BTreeSet<Guid>) -> Result<BTreeMap<Guid, Arc<[u8]>>> {
        let mut own = wanted.clone();
        let mut out = BTreeMap::new();
        for (_, dep, supplied) in &mut self.deps {
            let theirs: BTreeSet<Guid> = own.intersection(supplied).copied().collect();
            if theirs.is_empty() {
                continue;
            }
            own.retain(|g| !theirs.contains(g));
            let before = dep.index.stats;
            out.extend(dep.read_assets(&theirs)?);
            self.index.stats.passes += dep.index.stats.passes - before.passes;
            self.index.stats.bytes += dep.index.stats.bytes - before.bytes;
        }
        out.extend(self.read_own(&own)?);
        Ok(out)
    }

    /// Merge the index of `dep` (found through a `Resolver` at `location`): its entries that this
    /// package lacks become visible to planning, and their bytes are read from `dep`.
    /// Returns how many entries were added.
    pub fn add_dependency(&mut self, location: String, dep: Package) -> usize {
        let added = self.index.merge_missing(&dep.index);
        let count = added.len();
        self.index.stats.passes += dep.index.stats.passes;
        self.index.stats.bytes += dep.index.stats.bytes;
        if count > 0 {
            self.deps.push((location, dep, added));
        }
        count
    }

    /// Where the merged dependencies came from, in the order they were added.
    pub fn dependencies(&self) -> impl Iterator<Item = &str> {
        self.deps.iter().map(|(l, _, _)| l.as_str())
    }

    fn read_own(&mut self, wanted: &BTreeSet<Guid>) -> Result<BTreeMap<Guid, Arc<[u8]>>> {
        // Files a resolver supplied keep their bytes in the index, whatever this package's origin.
        let mut kept = BTreeMap::new();
        for g in wanted {
            if let Some(b) = self.index.get(g).and_then(|e| e.bytes.clone()) {
                kept.insert(*g, b);
            }
        }
        let wanted: BTreeSet<Guid> = wanted.iter().filter(|g| !kept.contains_key(g)).copied().collect();
        if wanted.is_empty() {
            return Ok(kept);
        }
        let mut out = self.read_origin(&wanted)?;
        out.extend(kept);
        Ok(out)
    }

    fn read_origin(&mut self, wanted: &BTreeSet<Guid>) -> Result<BTreeMap<Guid, Arc<[u8]>>> {
        match &mut self.origin {
            Origin::Package(source) if source.reopenable() => self.index.read_assets(source.open()?, wanted),
            Origin::Package(_) | Origin::Kept => self.index.read_assets(io::empty(), wanted),
            Origin::ZipMember { zip, index } => {
                let mut archive = zip.archive()?;
                let member = archive.by_index(*index).map_err(|e| Error::Unsupported(format!("zip member: {e}")))?;
                self.index.read_assets(member, wanted)
            }
            Origin::Files(files) => {
                let mut out = BTreeMap::new();
                for guid in wanted {
                    if let Some(path) = files.get(guid) {
                        out.insert(*guid, Arc::from(std::fs::read(path)?));
                    }
                }
                Ok(out)
            }
        }
    }
}

enum Detected {
    UnityPackage,
    Zip,
    Model,
}

/// Read the first bytes of a source without losing them.
fn peek(source: Source) -> Result<(Vec<u8>, Source)> {
    let mut head = Vec::with_capacity(32);
    match source {
        Source::Path(p) => {
            std::fs::File::open(&p)?.take(32).read_to_end(&mut head)?;
            Ok((head, Source::Path(p)))
        }
        Source::Bytes(b) => Ok((b[..b.len().min(32)].to_vec(), Source::Bytes(b))),
        Source::Reader(mut r) => {
            (&mut r).take(32).read_to_end(&mut head)?;
            let rest: Box<dyn Read + Send> = Box::new(io::Cursor::new(head.clone()).chain(r));
            Ok((head, Source::Reader(rest)))
        }
        Source::Consumed => Err(Error::InvalidInput("the reader of this source was already consumed".into())),
    }
}

fn detect(head: &[u8], source: &Source) -> Option<Detected> {
    if head.starts_with(&[0x1f, 0x8b]) {
        return Some(Detected::UnityPackage);
    }
    if head.starts_with(b"PK\x03\x04") || head.starts_with(b"PK\x05\x06") {
        return Some(Detected::Zip);
    }
    if sniff(head) == AssetKind::Fbx {
        return Some(Detected::Model);
    }
    let Source::Path(p) = source else { return None };
    match p.extension()?.to_str()?.to_ascii_lowercase().as_str() {
        "unitypackage" => Some(Detected::UnityPackage),
        "zip" => Some(Detected::Zip),
        "fbx" => Some(Detected::Model),
        _ => None,
    }
}

fn index_source(mut source: Source, opts: &IndexOptions) -> Result<(PackageIndex, Source)> {
    let opts = if source.reopenable() { opts.clone() } else { IndexOptions { keep_binaries: true, ..opts.clone() } };
    let index = PackageIndex::read(source.open()?, &opts)?;
    Ok((index, source))
}

fn open_zip(zip: ZipData, inner: &mut impl Iterator<Item = String>, opts: &IndexOptions) -> Result<(Package, Option<String>)> {
    let mut archive = zip.archive()?;
    let mut members = Vec::new();
    for i in 0..archive.len() {
        let Ok(f) = archive.by_index_raw(i) else { continue };
        if !f.is_dir() {
            members.push((i, decode_name(f.name_raw()), f.size()));
        }
    }
    let first = inner.next();
    let chosen = match &first {
        Some(seg) if is_container(seg) => {
            let want = normalize_path(seg);
            let found = members.iter().find(|(_, n, _)| normalize_path(n) == want).ok_or_else(|| Error::NotFound(format!("{seg} is not in the zip")))?;
            if !found.1.to_ascii_lowercase().ends_with(".unitypackage") {
                return Err(Error::Unsupported(format!("nested zip {seg}")));
            }
            Some(found.clone())
        }
        _ => members.iter().filter(|(_, n, _)| n.to_ascii_lowercase().ends_with(".unitypackage")).max_by_key(|(_, _, size)| *size).cloned(),
    };
    if let Some((i, name, _)) = chosen {
        let member = archive.by_index(i).map_err(|e| Error::Unsupported(format!("zip member {name}: {e}")))?;
        let opts = IndexOptions { keep_binaries: false, ..opts.clone() };
        let index = PackageIndex::read(member, &opts)?;
        let next = if first.as_deref().is_some_and(is_container) { inner.next() } else { first };
        let pkg = Package { index, container: Container::Zip, member: Some(name), origin: Origin::ZipMember { zip, index: i }, deps: Vec::new() };
        return Ok((pkg, next));
    }
    let mut entries = Vec::new();
    let mut kept = 0u64;
    for (i, name, size) in members {
        let Some(kind) = loose_kind(&name) else { continue };
        let mut bytes = Vec::new();
        if kept + size <= opts.max_binary_bytes {
            let mut f = archive.by_index(i).map_err(|e| Error::Unsupported(format!("zip member {name}: {e}")))?;
            f.read_to_end(&mut bytes)?;
            kept += size;
        }
        let kind = match sniff(&bytes) {
            AssetKind::Other => kind,
            sniffed => sniffed,
        };
        entries.push(Entry::loose(name, size, kind, (!bytes.is_empty()).then(|| Arc::from(bytes))));
    }
    let pkg = Package { index: PackageIndex::from_loose(entries), container: Container::Zip, member: None, origin: Origin::Kept, deps: Vec::new() };
    Ok((pkg, first))
}

fn open_folder(dir: &Path, opts: &IndexOptions) -> Result<Package> {
    let files = walk(dir);
    let packages = files.iter().filter(|(rel, _, _)| rel.to_ascii_lowercase().ends_with(".unitypackage"));
    if let Some((rel, path, _)) = packages.max_by_key(|(_, _, size)| *size) {
        let (index, source) = index_source(Source::Path(path.clone()), opts)?;
        return Ok(Package { index, container: Container::Folder, member: Some(rel.clone()), origin: Origin::Package(source), deps: Vec::new() });
    }
    let (index, map) = loose_files(files);
    Ok(Package { index, container: Container::Folder, member: None, origin: Origin::Files(map), deps: Vec::new() })
}

fn open_model(source: Source, opts: &IndexOptions) -> Result<Package> {
    match source {
        Source::Path(path) => {
            let dir = path.parent().map(Path::to_path_buf).unwrap_or_default();
            let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "model.fbx".into());
            let size = std::fs::metadata(&path)?.len();
            // Textures sit next to the model or in a subfolder such as `Textures/`.
            let mut files: Vec<_> = walk_limited(&dir, 3).into_iter().filter(|(_, p, _)| p != &path && loose_kind(&p.to_string_lossy()).is_some_and(|k| k != AssetKind::Fbx)).collect();
            files.push((name, path, size));
            let (index, map) = loose_files(files);
            Ok(Package { index, container: Container::Model, member: None, origin: Origin::Files(map), deps: Vec::new() })
        }
        mut other => {
            let mut bytes = Vec::new();
            other.open()?.take(opts.max_binary_bytes).read_to_end(&mut bytes)?;
            let entry = Entry::loose("model.fbx".into(), bytes.len() as u64, AssetKind::Fbx, Some(Arc::from(bytes)));
            Ok(Package { index: PackageIndex::from_loose(vec![entry]), container: Container::Model, member: None, origin: Origin::Kept, deps: Vec::new() })
        }
    }
}

fn loose_files(files: Vec<(String, PathBuf, u64)>) -> (PackageIndex, BTreeMap<Guid, PathBuf>) {
    let mut entries = Vec::new();
    let mut map = BTreeMap::new();
    for (rel, path, size) in files {
        let Some(kind) = loose_kind(&rel) else { continue };
        let e = Entry::loose(rel, size, kind, None);
        map.insert(e.guid, path);
        entries.push(e);
    }
    (PackageIndex::from_loose(entries), map)
}

/// Kind of a loose file worth indexing, by extension. Files under Unity's `Library/` are skipped.
fn loose_kind(path: &str) -> Option<AssetKind> {
    let normalized = normalize_path(path);
    if normalized.starts_with("library/") || normalized.contains("/library/") {
        return None;
    }
    let ext = normalized.rsplit_once('.')?.1;
    kind_from_extension(ext).filter(|k| matches!(k, AssetKind::Fbx | AssetKind::Png | AssetKind::Jpeg | AssetKind::Psd))
}

/// Zip names are UTF-8 when valid, otherwise Shift-JIS (Japanese tools do not set the UTF-8 flag).
fn decode_name(raw: &[u8]) -> String {
    let name = match std::str::from_utf8(raw) {
        Ok(s) => s.to_string(),
        Err(_) => encoding_rs::SHIFT_JIS.decode(raw).0.into_owned(),
    };
    name.replace('\\', "/")
}

fn walk(dir: &Path) -> Vec<(String, PathBuf, u64)> {
    walk_limited(dir, MAX_DEPTH)
}

/// Files under `dir` as (relative path with `/`, full path, size), skipping `Library/` and dot folders.
fn walk_limited(dir: &Path, depth: usize) -> Vec<(String, PathBuf, u64)> {
    let mut out = Vec::new();
    let mut stack = vec![(dir.to_path_buf(), String::new(), 0usize)];
    while let Some((path, rel, d)) = stack.pop() {
        let Ok(read) = std::fs::read_dir(&path) else { continue };
        let mut items: Vec<_> = read.flatten().collect();
        items.sort_by_key(|e| e.file_name());
        for item in items {
            let name = item.file_name().to_string_lossy().into_owned();
            let child_rel = if rel.is_empty() { name.clone() } else { format!("{rel}/{name}") };
            let Ok(ft) = item.file_type() else { continue };
            if ft.is_dir() {
                if d < depth && !name.starts_with('.') && !name.eq_ignore_ascii_case("Library") {
                    stack.push((item.path(), child_rel, d + 1));
                }
            } else if ft.is_file() {
                let size = item.metadata().map(|m| m.len()).unwrap_or(0);
                out.push((child_rel, item.path(), size));
            }
        }
    }
    out.sort();
    out
}

#[cfg(test)]
mod tests;
