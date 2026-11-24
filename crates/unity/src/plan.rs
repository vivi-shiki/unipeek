//! Decide what to show from a package: the main prefab, its models, and the material of every
//! FBX material slot. Everything here is GPU-free; the renderer only follows the plan.
use crate::{
    Error, Guid, PackageIndex, Result,
    material::Look,
    prefab::Usage,
    shader::ShaderInfo,
};

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct Candidate {
    pub guid: Guid,
    pub path: String,
    /// Models in this package the prefab uses, directly or through nested prefabs.
    pub models: usize,
    pub variant: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct PlannedModel {
    pub guid: Guid,
    pub path: String,
    pub size: Option<u64>,
}

/// A GUID the plan needs that is not in the package.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize)]
pub struct MissingRef {
    pub guid: Guid,
    pub usage: Usage,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize)]
pub struct Plan {
    /// The prefab shown; `None` when the package has no usable prefab and a model is shown alone.
    pub prefab: Option<Candidate>,
    /// Prefabs that use a model, best first.
    pub candidates: Vec<Candidate>,
    pub models: Vec<PlannedModel>,
    /// Materials the prefab assigns (renderers, overrides), used to bind FBX slots by name.
    pub prefab_materials: Vec<Guid>,
    pub missing: Vec<MissingRef>,
    /// `m_Materials` overrides on model instances; their target renderer cannot be resolved yet.
    pub material_overrides: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BindSource {
    /// The FBX `.meta` remaps the slot (`externalObjects`).
    ExternalObject,
    /// A material the prefab assigns has the slot's name.
    PrefabMaterial,
    /// A material in the package has the slot's name.
    NameMatch,
    /// No material; a texture whose name ends with the slot's name.
    TextureName,
    Unbound,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct TextureRef {
    pub guid: Guid,
    pub path: Option<String>,
    pub in_package: bool,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct Binding {
    pub source: BindSource,
    pub material: Option<Guid>,
    pub material_name: Option<String>,
    pub shader: Option<ShaderInfo>,
    pub look: Look,
    pub main_texture: Option<TextureRef>,
}

impl Plan {
    /// Plan the package. `inner` selects a prefab or model by its path inside the package.
    pub fn new(_index: &PackageIndex, _inner: Option<&str>) -> Result<Plan> {
        todo!()
    }

    /// Material and texture for the FBX material slot `slot` of `model`.
    pub fn bind(&self, _index: &PackageIndex, _model: Guid, _slot: &str) -> Binding {
        todo!()
    }
}

#[allow(dead_code)]
fn unused() -> Error {
    Error::NotFound(String::new())
}

#[cfg(test)]
mod tests;
