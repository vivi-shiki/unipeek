//! Streaming index of a `.unitypackage` (gzip tar of `GUID/{asset,asset.meta,pathname,preview.png}`).
//!
//! One pass keeps small Unity YAML text and metadata in memory. Big binary assets (FBX, textures)
//! are only measured, unless the caller asks to keep them because the source cannot be read twice.
use crate::{Guid, Result};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::Read,
    sync::Arc,
};

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
        todo!()
    }

    /// File name of `pathname` without the extension.
    pub fn stem(&self) -> Option<&str> {
        todo!()
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
    pub fn read(_reader: impl Read, _opts: &IndexOptions) -> Result<PackageIndex> {
        todo!()
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
    pub fn by_path(&self, _pathname: &str) -> Option<&Entry> {
        todo!()
    }

    /// Read the package again and return the bytes of the wanted assets.
    /// Assets kept during the index pass are returned without reading.
    pub fn read_assets(&mut self, _reader: impl Read, _wanted: &BTreeSet<Guid>) -> Result<BTreeMap<Guid, Arc<[u8]>>> {
        todo!()
    }
}

#[cfg(test)]
mod tests;
