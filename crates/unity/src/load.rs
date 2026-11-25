//! Open any supported input (unitypackage, zip, 7z, folder, Unity project, single FBX) as a `PackageIndex`.
//!
//! Archives and folders without a `.unitypackage` become a "loose" index. A file with a `.meta`
//! next to it (an unpacked Unity project) keeps the GUID from its `.meta`; any other FBX or image
//! gets a GUID derived from its path, so planning and binding work the same for every container.
use crate::{
    AssetKind, Entry, Error, Guid, IndexOptions, Input, PackageIndex, Result, Source,
    archive::{Archive, ArchiveData, Format, Member},
    package::{kind_from_extension, normalize_path, sniff},
    source::is_container,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{self, Read},
    path::{Path, PathBuf},
    sync::Arc,
};

/// Folders deeper than this are not searched; real item folders are shallow.
const MAX_DEPTH: usize = 8;
/// Unity projects nest deeper (`Assets/Shop/Item/Variant/Textures/...`).
const PROJECT_DEPTH: usize = 16;
/// Archives inside archives are followed this many levels deep.
const MAX_NESTING: usize = 3;
/// A `.meta` larger than this is not a Unity `.meta`.
const MAX_META: u64 = 1 << 20;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Container {
    UnityPackage,
    Zip,
    SevenZ,
    Folder,
    /// A Unity project folder (or archive of one): `Assets/` and `Packages/` with their `.meta` files.
    Project,
    Model,
    /// glTF, GLB or VRM, read by the rendering crate.
    Gltf,
}

pub struct Package {
    pub index: PackageIndex,
    pub container: Container,
    /// The `.unitypackage` (or nested archive chain, joined with `#`) picked inside an archive or folder.
    pub member: Option<String>,
    origin: Origin,
    /// Packages merged in through a `Resolver`, with the GUIDs each one supplied.
    deps: Vec<(String, Package, BTreeSet<Guid>)>,
}

enum Origin {
    /// A unitypackage source. One-shot readers were indexed with their binaries kept.
    Package(Source),
    Member { archive: Archive, member: Member },
    /// Loose files whose bytes were kept while indexing.
    Kept,
    Files(BTreeMap<Guid, PathBuf>),
}

impl Package {
    /// Open an input and index the package it points at.
    /// Returns the inner path left for `Plan::new` (a prefab or model inside the package).
    pub fn open(input: Input, opts: &IndexOptions) -> Result<(Package, Option<String>)> {
        open_nested(input, opts, 0)
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
            Origin::Member { archive, member } => {
                let index = &mut self.index;
                archive.with_member(member, |r| index.read_assets(r, wanted))
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

    fn new(index: PackageIndex, container: Container, member: Option<String>, origin: Origin) -> Package {
        Package { index, container, member, origin, deps: Vec::new() }
    }
}

fn open_nested(input: Input, opts: &IndexOptions, depth: usize) -> Result<(Package, Option<String>)> {
    let Input { source, inner } = input;
    let mut inner = inner.into_iter();
    if let Source::Path(p) = &source
        && p.is_dir()
    {
        return open_folder(p, &mut inner, opts, depth);
    }
    let (head, source) = peek(source)?;
    match detect(&head, &source) {
        Some(Detected::UnityPackage) => {
            let (index, source) = index_source(source, opts)?;
            Ok((Package::new(index, Container::UnityPackage, None, Origin::Package(source)), inner.next()))
        }
        Some(Detected::Archive(format)) => {
            let data = match source {
                Source::Path(p) => ArchiveData::Path(p),
                Source::Bytes(b) => ArchiveData::Bytes(b),
                mut other => {
                    let mut bytes = Vec::new();
                    other.open()?.read_to_end(&mut bytes)?;
                    ArchiveData::Bytes(Arc::from(bytes))
                }
            };
            open_archive(Archive { data, format }, &mut inner, opts, depth)
        }
        Some(Detected::Model) => Ok((open_model(source, opts)?, inner.next())),
        None => Err(Error::Unsupported(format!("{} is not a unitypackage, zip, 7z, folder or FBX", source.describe()))),
    }
}

enum Detected {
    UnityPackage,
    Archive(Format),
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
        return Some(Detected::Archive(Format::Zip));
    }
    if head.starts_with(b"7z\xbc\xaf\x27\x1c") {
        return Some(Detected::Archive(Format::SevenZ));
    }
    if sniff(head) == AssetKind::Fbx {
        return Some(Detected::Model);
    }
    let Source::Path(p) = source else { return None };
    match p.extension()?.to_str()?.to_ascii_lowercase().as_str() {
        "unitypackage" => Some(Detected::UnityPackage),
        "zip" => Some(Detected::Archive(Format::Zip)),
        "7z" => Some(Detected::Archive(Format::SevenZ)),
        "fbx" => Some(Detected::Model),
        _ => None,
    }
}

fn index_source(mut source: Source, opts: &IndexOptions) -> Result<(PackageIndex, Source)> {
    let opts = if source.reopenable() { opts.clone() } else { IndexOptions { keep_binaries: true, ..opts.clone() } };
    let index = PackageIndex::read(source.open()?, &opts)?;
    Ok((index, source))
}

fn is_archive(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.ends_with(".zip") || lower.ends_with(".7z")
}

fn is_unitypackage(name: &str) -> bool {
    name.to_ascii_lowercase().ends_with(".unitypackage")
}

fn open_archive(archive: Archive, inner: &mut impl Iterator<Item = String>, opts: &IndexOptions, depth: usize) -> Result<(Package, Option<String>)> {
    let container = if archive.format == Format::Zip { Container::Zip } else { Container::SevenZ };
    let members = archive.members()?;
    let first = inner.next();
    let named = first.as_deref().filter(|s| is_container(s));
    let chosen = match named {
        Some(seg) => {
            let want = normalize_path(seg);
            Some(members.iter().find(|m| normalize_path(&m.name) == want).cloned().ok_or_else(|| Error::NotFound(format!("{seg} is not in the archive")))?)
        }
        None => {
            let largest = |pick: &dyn Fn(&str) -> bool| members.iter().filter(|m| pick(&m.name) && !in_library(&m.name)).max_by_key(|m| m.size).cloned();
            // A shop zip holds the unitypackage; a zip of zips holds it one level down. Loose
            // models win over nested archives so a project archive is read as a project.
            largest(&is_unitypackage).or_else(|| if members.iter().any(|m| loose_kind(&m.name) == Some(AssetKind::Fbx)) { None } else { largest(&is_archive) })
        }
    };
    let rest = if named.is_some() { inner.next() } else { first };
    if let Some(member) = chosen {
        if is_unitypackage(&member.name) {
            let index = archive.with_member(&member, |r| PackageIndex::read(r, &IndexOptions { keep_binaries: false, ..opts.clone() }))?;
            let name = member.name.clone();
            return Ok((Package::new(index, container, Some(name), Origin::Member { archive, member }), rest));
        }
        if !is_archive(&member.name) {
            return Err(Error::Unsupported(format!("{} is not a unitypackage or archive", member.name)));
        }
        if depth + 1 >= MAX_NESTING {
            return Err(Error::Unsupported(format!("{} is nested more than {MAX_NESTING} archives deep", member.name)));
        }
        if member.size > opts.max_binary_bytes {
            return Err(Error::Unsupported(format!("nested archive {} is larger than {} bytes", member.name, opts.max_binary_bytes)));
        }
        let bytes = archive.with_member(&member, |r| {
            let mut b = Vec::with_capacity(member.size as usize);
            r.take(opts.max_binary_bytes).read_to_end(&mut b)?;
            Ok(b)
        })?;
        let mut nested = Input::new(Source::Bytes(Arc::from(bytes)));
        nested.inner = rest.into_iter().chain(inner.by_ref()).collect();
        let (mut pkg, left) = open_nested(nested, opts, depth + 1)?;
        pkg.member = Some(match pkg.member.take() {
            Some(m) => format!("{}#{m}", member.name),
            None => member.name.clone(),
        });
        return Ok((pkg, left));
    }

    // Loose files: keep `.meta`, Unity YAML and, within the budget, models and images.
    let mut files = Vec::new();
    let mut kept = 0u64;
    let wanted = |m: &Member| !in_library(&m.name) && (is_meta(&m.name) || loose_kind(&m.name).is_some());
    archive.each(wanted, |m, r| {
        let keep = match loose_kind(&m.name) {
            _ if is_meta(&m.name) => m.size <= MAX_META,
            Some(AssetKind::Yaml | AssetKind::Shader) => m.size <= opts.max_text_bytes,
            Some(_) => kept + m.size <= opts.max_binary_bytes,
            None => false,
        };
        let mut bytes = Vec::new();
        if keep {
            r.read_to_end(&mut bytes)?;
            if !is_meta(&m.name) && matches!(loose_kind(&m.name), Some(AssetKind::Fbx | AssetKind::Png | AssetKind::Jpeg | AssetKind::Psd)) {
                kept += m.size;
            }
        }
        files.push(LooseFile { rel: m.name.clone(), size: m.size, bytes: keep.then_some(bytes), path: None });
        Ok(())
    })?;
    let (index, _, project) = loose_index(files);
    let container = if project { Container::Project } else { container };
    Ok((Package::new(index, container, None, Origin::Kept), rest))
}

fn open_folder(dir: &Path, inner: &mut impl Iterator<Item = String>, opts: &IndexOptions, depth: usize) -> Result<(Package, Option<String>)> {
    if dir.join("Assets").is_dir() && (dir.join("ProjectSettings").is_dir() || dir.join("Packages").is_dir()) {
        // A Unity project: only `Assets/` and `Packages/` hold assets; `Library/` is a cache.
        let mut files = Vec::new();
        for top in ["Assets", "Packages"] {
            files.extend(walk_limited(&dir.join(top), PROJECT_DEPTH).into_iter().map(|(rel, p, s)| (format!("{top}/{rel}"), p, s)));
        }
        let (index, map, _) = loose_index(read_loose(files, opts));
        return Ok((Package::new(index, Container::Project, None, Origin::Files(map)), inner.next()));
    }
    let files = walk_limited(dir, MAX_DEPTH);
    let packages = files.iter().filter(|(rel, _, _)| is_unitypackage(rel));
    if let Some((rel, path, _)) = packages.max_by_key(|(_, _, size)| *size) {
        let (index, source) = index_source(Source::Path(path.clone()), opts)?;
        return Ok((Package::new(index, Container::Folder, Some(rel.clone()), Origin::Package(source)), inner.next()));
    }
    let has_models = files.iter().any(|(rel, _, _)| loose_kind(rel) == Some(AssetKind::Fbx));
    if !has_models && let Some((rel, path, _)) = files.iter().filter(|(rel, _, _)| is_archive(rel)).max_by_key(|(_, _, size)| *size) {
        let mut nested = Input::new(Source::Path(path.clone()));
        nested.inner = inner.collect();
        let (mut pkg, left) = open_nested(nested, opts, depth)?;
        pkg.member = Some(match pkg.member.take() {
            Some(m) => format!("{rel}#{m}"),
            None => rel.clone(),
        });
        return Ok((pkg, left));
    }
    let (index, map, project) = loose_index(read_loose(files, opts));
    let container = if project { Container::Project } else { Container::Folder };
    Ok((Package::new(index, container, None, Origin::Files(map)), inner.next()))
}

fn open_model(source: Source, opts: &IndexOptions) -> Result<Package> {
    match source {
        Source::Path(path) => {
            let dir = path.parent().map(Path::to_path_buf).unwrap_or_default();
            let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "model.fbx".into());
            let size = std::fs::metadata(&path)?.len();
            // Textures sit next to the model or in a subfolder such as `Textures/`.
            let mut files: Vec<_> = walk_limited(&dir, 3)
                .into_iter()
                .filter(|(_, p, _)| p != &path && loose_kind(&p.to_string_lossy()).is_some_and(|k| matches!(k, AssetKind::Png | AssetKind::Jpeg | AssetKind::Psd)))
                .collect();
            files.push((name, path, size));
            let (index, map, _) = loose_index(read_loose(files, opts));
            Ok(Package::new(index, Container::Model, None, Origin::Files(map)))
        }
        mut other => {
            let mut bytes = Vec::new();
            other.open()?.take(opts.max_binary_bytes).read_to_end(&mut bytes)?;
            let entry = Entry::loose("model.fbx".into(), bytes.len() as u64, AssetKind::Fbx, Some(Arc::from(bytes)));
            Ok(Package::new(PackageIndex::from_loose(vec![entry]), Container::Model, None, Origin::Kept))
        }
    }
}

/// A file of a folder or archive. `bytes` is what was read while listing (`.meta`, Unity YAML,
/// kept binaries); `path` is where a folder's file can be read later.
struct LooseFile {
    rel: String,
    size: u64,
    bytes: Option<Vec<u8>>,
    path: Option<PathBuf>,
}

/// Folder files: read `.meta` files and Unity YAML now, models and images when they are needed.
fn read_loose(files: Vec<(String, PathBuf, u64)>, opts: &IndexOptions) -> Vec<LooseFile> {
    files
        .into_iter()
        .filter(|(rel, _, _)| is_meta(rel) || loose_kind(rel).is_some())
        .map(|(rel, path, size)| {
            let small = if is_meta(&rel) { size <= MAX_META } else { matches!(loose_kind(&rel), Some(AssetKind::Yaml | AssetKind::Shader)) && size <= opts.max_text_bytes };
            let bytes = if small { std::fs::read(&path).ok() } else { None };
            LooseFile { rel, size, bytes, path: Some(path) }
        })
        .collect()
}

/// Build an index from loose files. Returns it, where to read each file, and whether `.meta`
/// files gave GUIDs (an unpacked Unity project).
fn loose_index(files: Vec<LooseFile>) -> (PackageIndex, BTreeMap<Guid, PathBuf>, bool) {
    let mut metas: BTreeMap<String, String> = BTreeMap::new();
    let mut assets = Vec::new();
    for f in files {
        match f.rel.strip_suffix(".meta") {
            Some(asset) => {
                if let Some(b) = &f.bytes {
                    metas.insert(normalize_path(asset), String::from_utf8_lossy(b).into_owned());
                }
            }
            None => assets.push(f),
        }
    }
    let mut entries = Vec::new();
    let mut paths = BTreeMap::new();
    let mut from_meta = false;
    for f in assets {
        let Some(by_name) = loose_kind(&f.rel) else { continue };
        let kind = match f.bytes.as_deref().map(sniff) {
            Some(AssetKind::Other) | None => by_name,
            Some(sniffed) => sniffed,
        };
        let meta = metas.get(&normalize_path(&f.rel));
        let guid = meta.and_then(|m| crate::meta::Meta::parse(m).guid);
        let mut e = Entry::loose(project_path(&f.rel), f.size, kind, None);
        if let Some(g) = guid {
            e.guid = g;
            from_meta = true;
        }
        e.meta = meta.map(|m| Arc::from(m.as_str()));
        e.has_meta = meta.is_some();
        match (kind, f.bytes) {
            (AssetKind::Yaml, Some(b)) => e.text = Some(Arc::from(String::from_utf8_lossy(&b).as_ref())),
            (AssetKind::Shader, Some(b)) => e.shader_name = crate::package::shader_name(&String::from_utf8_lossy(&b)),
            (_, Some(b)) if !b.is_empty() => e.bytes = Some(Arc::from(b)),
            _ => {}
        }
        if let Some(p) = f.path {
            paths.insert(e.guid, p);
        }
        entries.push(e);
    }
    (PackageIndex::from_loose(entries), paths, from_meta)
}

/// `Assets/...` or `Packages/...` when the path is inside a Unity project, the path as is otherwise.
fn project_path(rel: &str) -> String {
    let parts: Vec<&str> = rel.split('/').collect();
    match parts.iter().position(|p| *p == "Assets" || *p == "Packages") {
        Some(i) if i > 0 => parts[i..].join("/"),
        _ => rel.to_string(),
    }
}

fn is_meta(path: &str) -> bool {
    path.to_ascii_lowercase().ends_with(".meta")
}

fn in_library(path: &str) -> bool {
    let normalized = normalize_path(path);
    normalized.starts_with("library/") || normalized.contains("/library/")
}

/// Kind of a loose file worth indexing, by extension. Files under Unity's `Library/` are skipped.
fn loose_kind(path: &str) -> Option<AssetKind> {
    if in_library(path) {
        return None;
    }
    let normalized = normalize_path(path);
    let ext = normalized.rsplit_once('.')?.1;
    match ext {
        "prefab" | "mat" => Some(AssetKind::Yaml),
        _ => kind_from_extension(ext).filter(|k| matches!(k, AssetKind::Fbx | AssetKind::Png | AssetKind::Jpeg | AssetKind::Psd | AssetKind::Shader)),
    }
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
