//! Cross-package dependencies. An outfit's prefab points at its base avatar, a material or a
//! texture by GUID; the host knows which package holds that GUID and says so through a `Resolver`.
//! unipeek groups the answers by package and reads each package once.
use crate::{AssetKind, Entry, Error, Guid, IndexOptions, Input, Package, Result, Source};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::Read,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

/// Opens a package that has no path of its own (a zip member, a cold-archived file).
pub type OpenReader = Arc<dyn Fn() -> std::io::Result<Box<dyn Read + Send>> + Send + Sync>;

/// Where a GUID can be found.
#[derive(Clone)]
pub enum Located {
    /// In a package at `location`: a path, optionally with `#member` for a package inside a zip.
    Package { location: String, guid: Guid },
    /// In a package read from a stream; `name` identifies it for grouping and reports.
    Reader { name: String, open: OpenReader, guid: Guid },
    /// A loose file: an FBX, an image, or Unity YAML (`.prefab`, `.mat`).
    File(PathBuf),
}

impl std::fmt::Debug for Located {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Located::Package { location, guid } => write!(f, "Package({location}, {guid})"),
            Located::Reader { name, guid, .. } => write!(f, "Reader({name}, {guid})"),
            Located::File(p) => write!(f, "File({})", p.display()),
        }
    }
}

impl Located {
    fn key(&self) -> &str {
        match self {
            Located::Package { location, .. } => location,
            Located::Reader { name, .. } => name,
            Located::File(_) => "",
        }
    }
}

/// Implemented by the host: given GUIDs a package needs but does not contain, say where they are.
/// Called with all GUIDs at once so the host can batch its lookup. Unknown GUIDs are left out.
pub trait Resolver: Send + Sync {
    fn locate(&self, guids: &[Guid]) -> Vec<(Guid, Located)>;
}

/// A `Resolver` over a list of packages, zips and folders, for the command line's `--with`.
/// Each source is indexed on first use and only once.
pub struct Sources {
    sources: Vec<(String, Mutex<Option<BTreeSet<Guid>>>)>,
    opts: IndexOptions,
}

impl Sources {
    /// `locations` are packages, zips or folders. A folder holding packages or zips counts as each
    /// of them (up to three levels deep); any other folder is one source.
    pub fn new(locations: &[String], opts: IndexOptions) -> Sources {
        let mut sources = Vec::new();
        for l in locations {
            let path = Path::new(l);
            let inside = if path.is_dir() { containers_in(path, 3) } else { Vec::new() };
            if inside.is_empty() {
                sources.push(l.clone());
            } else {
                sources.extend(inside.into_iter().map(|p| p.display().to_string()));
            }
        }
        Sources { sources: sources.into_iter().map(|s| (s, Mutex::new(None))).collect(), opts }
    }

    pub fn locations(&self) -> impl Iterator<Item = &str> {
        self.sources.iter().map(|(s, _)| s.as_str())
    }

    fn guids(&self, i: usize) -> BTreeSet<Guid> {
        let (location, cache) = &self.sources[i];
        let mut cache = cache.lock().unwrap_or_else(|e| e.into_inner());
        cache
            .get_or_insert_with(|| match Package::open(Input::from_location(location), &self.opts) {
                Ok((pkg, _)) => pkg.index.entries().filter(|e| e.size.is_some()).map(|e| e.guid).collect(),
                Err(_) => BTreeSet::new(),
            })
            .clone()
    }
}

impl Resolver for Sources {
    fn locate(&self, guids: &[Guid]) -> Vec<(Guid, Located)> {
        let mut left: BTreeSet<Guid> = guids.iter().copied().collect();
        let mut out = Vec::new();
        for i in 0..self.sources.len() {
            if left.is_empty() {
                break;
            }
            let have = self.guids(i);
            let found: Vec<Guid> = left.intersection(&have).copied().collect();
            for g in found {
                left.remove(&g);
                out.push((g, Located::Package { location: self.sources[i].0.clone(), guid: g }));
            }
        }
        out
    }
}

fn containers_in(dir: &Path, depth: usize) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(read) = std::fs::read_dir(dir) else { return out };
    let mut items: Vec<_> = read.flatten().collect();
    items.sort_by_key(|e| e.file_name());
    for item in items {
        let p = item.path();
        let name = item.file_name().to_string_lossy().to_ascii_lowercase();
        if p.is_dir() {
            if depth > 0 && !name.starts_with('.') && name != "library" {
                out.extend(containers_in(&p, depth - 1));
            }
        } else if name.ends_with(".unitypackage") || name.ends_with(".zip") {
            out.push(p);
        }
    }
    out
}

impl Package {
    /// Ask `resolver` for `guids` and merge what it finds. Packages are opened once each, even when
    /// they hold many of the GUIDs. Returns the GUIDs that were found and added.
    pub fn resolve(&mut self, resolver: &dyn Resolver, guids: &[Guid], opts: &IndexOptions) -> Result<BTreeSet<Guid>> {
        if guids.is_empty() {
            return Ok(BTreeSet::new());
        }
        let mut by_package: BTreeMap<String, Vec<(Guid, Located)>> = BTreeMap::new();
        let mut added = BTreeSet::new();
        for (guid, located) in resolver.locate(guids) {
            if self.index.get(&guid).is_some_and(|e| e.size.is_some()) {
                continue;
            }
            match located {
                Located::File(path) => {
                    if let Some(entry) = loose_entry(guid, &path)
                        && self.index.insert_missing(entry)
                    {
                        added.insert(guid);
                    }
                }
                other => by_package.entry(other.key().to_string()).or_default().push((guid, other)),
            }
        }
        for (key, wanted) in by_package {
            let source = match &wanted[0].1 {
                Located::Package { location, .. } => Input::from_location(location),
                Located::Reader { open, .. } => Input::new(Source::Reader(open().map_err(Error::Io)?)),
                Located::File(_) => continue,
            };
            // A dependency that cannot be read stays missing; it never fails this package.
            let Ok((dep, _)) = Package::open(source, opts) else { continue };
            let found: Vec<Guid> = wanted.iter().map(|(g, _)| *g).filter(|g| dep.index.get(g).is_some()).collect();
            if self.add_dependency(key, dep) > 0 {
                added.extend(found);
            }
        }
        Ok(added)
    }
}

/// An entry for a loose file found by a resolver, keeping its bytes (text for Unity YAML).
fn loose_entry(guid: Guid, path: &Path) -> Option<Entry> {
    let bytes = std::fs::read(path).ok()?;
    let name = path.to_string_lossy().replace('\\', "/");
    let kind = match crate::package::sniff(&bytes) {
        AssetKind::Other => name.rsplit_once('.').and_then(|(_, x)| crate::package::kind_from_extension(&x.to_ascii_lowercase())).unwrap_or(AssetKind::Other),
        k => k,
    };
    let mut e = Entry::loose(name, bytes.len() as u64, kind, None);
    e.guid = guid;
    if kind == AssetKind::Yaml {
        e.text = Some(Arc::from(String::from_utf8_lossy(&bytes).as_ref()));
    } else {
        e.bytes = Some(Arc::from(bytes));
    }
    Some(e)
}
