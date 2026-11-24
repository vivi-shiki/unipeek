//! Open any supported input (unitypackage, zip, folder, single FBX) as a `PackageIndex`.
//!
//! Zips and folders without a `.unitypackage` become a "loose" index: each FBX or image gets a
//! GUID derived from its path, so planning and binding work the same way for every container.
use crate::{Guid, IndexOptions, Input, PackageIndex, Result};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

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
    #[allow(dead_code)]
    origin: (),
}

impl Package {
    /// Open an input and index the package it points at.
    /// Returns the inner path left for `Plan::new` (a prefab or model inside the package).
    pub fn open(_input: Input, _opts: &IndexOptions) -> Result<(Package, Option<String>)> {
        todo!()
    }

    /// Bytes of FBX and texture assets, reading the input again when needed.
    pub fn read_assets(&mut self, _wanted: &BTreeSet<Guid>) -> Result<BTreeMap<Guid, Arc<[u8]>>> {
        todo!()
    }
}

#[cfg(test)]
mod tests;
