//! Streaming index of a `.unitypackage` (gzip tar of `GUID/{asset,asset.meta,pathname,preview.png}`).
//!
//! One pass keeps small Unity YAML text and metadata in memory. Big binary assets (FBX, textures)
//! are only measured, unless the caller asks to keep them because the source cannot be read twice.
use crate::{Guid, Result};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{self, Read},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

/// Animation and import-cache YAML is large and never needed to decide what to show.
const SKIPPED_YAML_CLASSES: &[u32] = &[28, 43, 74, 91, 206, 221, 1101, 1102, 1107];
const SNIFF_BYTES: usize = 4096;
const UTF8_BOM: &[u8] = &[0xef, 0xbb, 0xbf];

#[derive(Clone, Debug)]
pub struct IndexOptions {
    /// Unity YAML and `.meta` files larger than this are measured but not kept.
    pub max_text_bytes: u64,
    /// Keep FBX and image bytes during the index pass (for sources that cannot be read twice).
    pub keep_binaries: bool,
    /// Upper bound of kept binary bytes; beyond it binaries are dropped and an issue is recorded.
    pub max_binary_bytes: u64,
}

impl Default for IndexOptions {
    fn default() -> Self {
        IndexOptions { max_text_bytes: 8 << 20, keep_binaries: false, max_binary_bytes: 1 << 30 }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetKind {
    Folder,
    Yaml,
    Shader,
    Fbx,
    Png,
    Jpeg,
    Psd,
    Other,
}

#[derive(Clone, Debug)]
pub struct Entry {
    pub guid: Guid,
    pub pathname: Option<String>,
    /// Size of `asset`; `None` for folders.
    pub size: Option<u64>,
    pub kind: AssetKind,
    pub has_meta: bool,
    pub has_preview: bool,
    /// The asset when it is Unity YAML worth keeping (prefab, material, ...).
    pub text: Option<Arc<str>>,
    pub meta: Option<Arc<str>>,
    /// `Shader "name"` of a `.shader` asset.
    pub shader_name: Option<String>,
    /// Asset bytes kept during the index pass (`IndexOptions::keep_binaries`).
    pub bytes: Option<Arc<[u8]>>,
    /// `pathname` has `..`, a root or a drive letter.
    pub unsafe_path: bool,
}

impl Entry {
    /// Lowercase extension of `pathname`, without the dot.
    pub fn extension(&self) -> Option<String> {
        let name = self.file_name()?;
        let (_, ext) = name.rsplit_once('.')?;
        Some(ext.to_ascii_lowercase())
    }

    /// File name of `pathname` without the extension.
    pub fn stem(&self) -> Option<&str> {
        let name = self.file_name()?;
        Some(name.rsplit_once('.').map_or(name, |(stem, _)| stem))
    }

    pub fn file_name(&self) -> Option<&str> {
        self.pathname.as_deref()?.rsplit(['/', '\\']).next()
    }

    /// A file outside a unitypackage, addressed by a GUID derived from its path.
    pub(crate) fn loose(pathname: String, size: u64, kind: AssetKind, bytes: Option<Arc<[u8]>>) -> Entry {
        let mut e = Entry::new(Guid::from_path(&pathname));
        e.pathname = Some(pathname);
        e.size = Some(size);
        e.kind = kind;
        e.bytes = bytes;
        e
    }

    fn new(guid: Guid) -> Entry {
        Entry {
            guid,
            pathname: None,
            size: None,
            kind: AssetKind::Folder,
            has_meta: false,
            has_preview: false,
            text: None,
            meta: None,
            shader_name: None,
            bytes: None,
            unsafe_path: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PackageIssue {
    NonGuidFolder { name: String },
    UnsafePath { guid: Guid, pathname: String },
    DuplicateEntry { guid: Guid, file: String },
    MissingPathname { guid: Guid },
    Truncated { entries: u64, message: String },
    BinaryBudgetExceeded { limit: u64 },
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize)]
pub struct ReadStats {
    /// How many times the package was read from the start.
    pub passes: u32,
    /// Compressed bytes read over all passes.
    pub bytes: u64,
}

#[derive(Clone, Debug, Default)]
pub struct PackageIndex {
    entries: BTreeMap<Guid, Entry>,
    pub issues: Vec<PackageIssue>,
    pub stats: ReadStats,
}

impl PackageIndex {
    pub fn read(reader: impl Read, opts: &IndexOptions) -> Result<PackageIndex> {
        let mut index = PackageIndex::default();
        let counted = Arc::new(AtomicU64::new(0));
        let mut archive = open_tar(reader, counted.clone())?;
        let mut seen_files: BTreeSet<(Guid, String)> = BTreeSet::new();
        let mut bad_folders: BTreeSet<String> = BTreeSet::new();
        let mut binary_bytes = 0u64;
        let mut entries_read = 0u64;
        let iter = archive.entries().map_err(|e| crate::Error::NotUnityPackage(e.to_string()))?;
        for item in iter {
            let mut entry = match item {
                Ok(e) => e,
                Err(e) => {
                    index.truncated(entries_read, e)?;
                    break;
                }
            };
            entries_read += 1;
            if !entry.header().entry_type().is_file() {
                continue;
            }
            let name = String::from_utf8_lossy(&entry.path_bytes()).into_owned();
            let Some((guid, file)) = split_entry_name(&name, &mut bad_folders, &mut index.issues) else { continue };
            if !seen_files.insert((guid, file.to_string())) {
                index.issues.push(PackageIssue::DuplicateEntry { guid, file: file.to_string() });
                continue;
            }
            let size = entry.header().size().unwrap_or(0);
            let e = index.entries.entry(guid).or_insert_with(|| Entry::new(guid));
            let result = match file {
                "pathname" => read_limited(&mut entry, 64 << 10).map(|bytes| {
                    let text = String::from_utf8_lossy(&bytes);
                    let first = text.lines().next().unwrap_or("").trim_end_matches('\r').to_string();
                    e.unsafe_path = is_unsafe_path(&first);
                    if e.unsafe_path {
                        index.issues.push(PackageIssue::UnsafePath { guid, pathname: first.clone() });
                    }
                    e.pathname = Some(first);
                }),
                "asset.meta" => {
                    e.has_meta = true;
                    if size <= opts.max_text_bytes {
                        read_limited(&mut entry, size as usize).map(|b| e.meta = Some(Arc::from(String::from_utf8_lossy(&b).as_ref())))
                    } else {
                        Ok(())
                    }
                }
                "preview.png" => {
                    e.has_preview = true;
                    Ok(())
                }
                "asset" => {
                    e.size = Some(size);
                    read_asset(&mut entry, size, e, opts, &mut binary_bytes, &mut index.issues)
                }
                _ => Ok(()),
            };
            if let Err(err) = result {
                index.truncated(entries_read, err)?;
                break;
            }
        }
        for e in index.entries.values_mut() {
            if e.size.is_some() && e.pathname.is_none() {
                index.issues.push(PackageIssue::MissingPathname { guid: e.guid });
            }
            if e.kind == AssetKind::Other
                && let Some(kind) = e.extension().and_then(|x| kind_from_extension(&x))
            {
                e.kind = kind;
            }
        }
        index.stats = ReadStats { passes: 1, bytes: counted.load(Ordering::Relaxed) };
        Ok(index)
    }

    /// Add the entries of `other` whose GUIDs this index does not have. Returns their GUIDs.
    pub fn merge_missing(&mut self, other: &PackageIndex) -> BTreeSet<Guid> {
        let mut added = BTreeSet::new();
        for e in other.entries.values() {
            if !self.entries.contains_key(&e.guid) {
                self.entries.insert(e.guid, e.clone());
                added.insert(e.guid);
            }
        }
        added
    }

    /// Add one entry, unless an entry with its GUID exists.
    pub fn insert_missing(&mut self, entry: Entry) -> bool {
        if self.entries.contains_key(&entry.guid) {
            return false;
        }
        self.entries.insert(entry.guid, entry);
        true
    }

    pub(crate) fn from_loose(entries: Vec<Entry>) -> PackageIndex {
        PackageIndex { entries: entries.into_iter().map(|e| (e.guid, e)).collect(), issues: Vec::new(), stats: ReadStats { passes: 1, bytes: 0 } }
    }

    fn truncated(&mut self, entries: u64, err: io::Error) -> Result<()> {
        if entries == 0 && self.entries.is_empty() {
            return Err(crate::Error::NotUnityPackage(err.to_string()));
        }
        self.issues.push(PackageIssue::Truncated { entries, message: err.to_string() });
        Ok(())
    }

    pub fn get(&self, guid: &Guid) -> Option<&Entry> {
        self.entries.get(guid)
    }

    pub fn entries(&self) -> impl Iterator<Item = &Entry> {
        self.entries.values()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Find an entry by its `pathname`, ignoring case and `\` vs `/`.
    pub fn by_path(&self, pathname: &str) -> Option<&Entry> {
        let want = normalize_path(pathname);
        self.entries.values().find(|e| e.pathname.as_deref().is_some_and(|p| normalize_path(p) == want))
    }

    /// Read the package again and return the bytes of the wanted assets.
    /// Assets kept during the index pass are returned without reading.
    pub fn read_assets(&mut self, reader: impl Read, wanted: &BTreeSet<Guid>) -> Result<BTreeMap<Guid, Arc<[u8]>>> {
        let mut out = BTreeMap::new();
        let mut missing = BTreeSet::new();
        for guid in wanted {
            match self.entries.get(guid) {
                Some(Entry { bytes: Some(b), .. }) => {
                    out.insert(*guid, b.clone());
                }
                Some(Entry { size: Some(_), .. }) => {
                    missing.insert(*guid);
                }
                _ => {}
            }
        }
        if missing.is_empty() {
            return Ok(out);
        }
        let counted = Arc::new(AtomicU64::new(0));
        let mut archive = open_tar(reader, counted.clone())?;
        let mut ignored = BTreeSet::new();
        let mut issues = Vec::new();
        for item in archive.entries()? {
            let mut entry = item?;
            let name = String::from_utf8_lossy(&entry.path_bytes()).into_owned();
            let Some((guid, "asset")) = split_entry_name(&name, &mut ignored, &mut issues) else { continue };
            if !missing.remove(&guid) {
                continue;
            }
            let mut bytes = Vec::with_capacity(entry.header().size().unwrap_or(0) as usize);
            entry.read_to_end(&mut bytes)?;
            out.insert(guid, Arc::from(bytes));
            if missing.is_empty() {
                break;
            }
        }
        self.stats.passes += 1;
        self.stats.bytes += counted.load(Ordering::Relaxed);
        Ok(out)
    }
}

struct Counting<R> {
    inner: R,
    count: Arc<AtomicU64>,
}

impl<R: Read> Read for Counting<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = self.inner.read(buf)?;
        self.count.fetch_add(n as u64, Ordering::Relaxed);
        Ok(n)
    }
}

type TarReader<R> = tar::Archive<flate2::read::GzDecoder<io::Chain<io::Cursor<[u8; 2]>, Counting<R>>>>;

fn open_tar<R: Read>(reader: R, count: Arc<AtomicU64>) -> Result<TarReader<R>> {
    let mut counting = Counting { inner: reader, count };
    let mut magic = [0u8; 2];
    counting.read_exact(&mut magic).map_err(|e| crate::Error::NotUnityPackage(e.to_string()))?;
    if magic != [0x1f, 0x8b] {
        return Err(crate::Error::NotUnityPackage("missing gzip header".into()));
    }
    Ok(tar::Archive::new(flate2::read::GzDecoder::new(io::Cursor::new(magic).chain(counting))))
}

/// `GUID/file` with an optional `./`; anything else is skipped, non-GUID folders are reported once.
fn split_entry_name<'a>(name: &'a str, bad: &mut BTreeSet<String>, issues: &mut Vec<PackageIssue>) -> Option<(Guid, &'a str)> {
    let name = name.strip_prefix("./").unwrap_or(name);
    let (top, file) = name.split_once('/')?;
    let Some(guid) = Guid::parse(top) else {
        if !top.is_empty() && bad.insert(top.to_string()) {
            issues.push(PackageIssue::NonGuidFolder { name: top.to_string() });
        }
        return None;
    };
    (!file.contains('/')).then_some((guid, file))
}

fn read_limited(r: &mut impl Read, limit: usize) -> io::Result<Vec<u8>> {
    let mut buf = Vec::with_capacity(limit.min(1 << 20));
    r.take(limit as u64).read_to_end(&mut buf)?;
    Ok(buf)
}

fn read_asset(
    r: &mut impl Read,
    size: u64,
    e: &mut Entry,
    opts: &IndexOptions,
    binary_bytes: &mut u64,
    issues: &mut Vec<PackageIssue>,
) -> io::Result<()> {
    let mut head = read_limited(r, SNIFF_BYTES)?;
    e.kind = sniff(&head);
    match e.kind {
        AssetKind::Yaml if size <= opts.max_text_bytes && !yaml_class(&head).is_some_and(|c| SKIPPED_YAML_CLASSES.contains(&c)) => {
            r.read_to_end(&mut head)?;
            e.text = Some(Arc::from(String::from_utf8_lossy(&head).as_ref()));
        }
        AssetKind::Shader => e.shader_name = shader_name(&String::from_utf8_lossy(&head)),
        AssetKind::Fbx | AssetKind::Png | AssetKind::Jpeg | AssetKind::Psd if opts.keep_binaries => {
            if *binary_bytes + size <= opts.max_binary_bytes {
                r.read_to_end(&mut head)?;
                *binary_bytes += size;
                e.bytes = Some(Arc::from(head));
            } else if !issues.iter().any(|i| matches!(i, PackageIssue::BinaryBudgetExceeded { .. })) {
                issues.push(PackageIssue::BinaryBudgetExceeded { limit: opts.max_binary_bytes });
            }
        }
        _ => {}
    }
    Ok(())
}

pub(crate) fn sniff(head: &[u8]) -> AssetKind {
    let text = head.strip_prefix(UTF8_BOM).unwrap_or(head);
    if head.starts_with(b"Kaydara FBX Binary") || text.starts_with(b"; FBX") {
        AssetKind::Fbx
    } else if head.starts_with(&[0x89, b'P', b'N', b'G']) {
        AssetKind::Png
    } else if head.starts_with(&[0xff, 0xd8, 0xff]) {
        AssetKind::Jpeg
    } else if head.starts_with(b"8BPS") {
        AssetKind::Psd
    } else if text.starts_with(b"%YAML") {
        AssetKind::Yaml
    } else if !head.contains(&0) && shader_name(&String::from_utf8_lossy(text)).is_some() {
        AssetKind::Shader
    } else {
        AssetKind::Other
    }
}

/// Class ID of the first `--- !u!N &id` document.
fn yaml_class(head: &[u8]) -> Option<u32> {
    let text = String::from_utf8_lossy(head);
    let rest = &text[text.find("--- !u!")? + 7..];
    let end = rest.find(|c: char| !c.is_ascii_digit())?;
    rest[..end].parse().ok()
}

/// `Shader "name"` as the first statement, after comments and blank lines.
pub(crate) fn shader_name(text: &str) -> Option<String> {
    let mut in_block = false;
    for line in text.lines() {
        let mut line = line.trim();
        if in_block {
            let Some(end) = line.find("*/") else { continue };
            line = line[end + 2..].trim();
            in_block = false;
        }
        if line.starts_with("/*") {
            in_block = !line.contains("*/");
            continue;
        }
        if line.is_empty() || line.starts_with("//") {
            continue;
        }
        let rest = line.strip_prefix("Shader")?.trim_start().strip_prefix('"')?;
        return Some(rest[..rest.find('"')?].to_string());
    }
    None
}

pub(crate) fn kind_from_extension(ext: &str) -> Option<AssetKind> {
    Some(match ext {
        "fbx" => AssetKind::Fbx,
        "png" => AssetKind::Png,
        "jpg" | "jpeg" => AssetKind::Jpeg,
        "psd" => AssetKind::Psd,
        "shader" => AssetKind::Shader,
        _ => return None,
    })
}

fn is_unsafe_path(p: &str) -> bool {
    let b = p.as_bytes();
    p.starts_with(['/', '\\']) || (b.len() >= 2 && b[1] == b':' && b[0].is_ascii_alphabetic()) || p.split(['/', '\\']).any(|c| c == "..")
}

pub(crate) fn normalize_path(p: &str) -> String {
    p.replace('\\', "/").to_lowercase()
}

#[cfg(test)]
mod tests;
