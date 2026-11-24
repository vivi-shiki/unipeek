//! `.meta` files: the importer settings unipeek needs (FBX material remapping, texture type).
use crate::{
    Guid,
    yaml::{self, Node, ObjRef},
};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Meta {
    pub guid: Option<Guid>,
    /// `ModelImporter`, `TextureImporter`, `NativeFormatImporter`, ...
    pub importer: Option<String>,
    /// FBX material name → `.mat` reference, from `externalObjects`.
    pub external_materials: Vec<(String, ObjRef)>,
    /// `ModelImporter.meshes.globalScale`.
    pub global_scale: Option<f32>,
    /// `TextureImporter.textureType == 1`.
    pub normal_map: bool,
}

impl Meta {
    pub fn parse(_text: &str) -> Meta {
        todo!()
    }

    /// The `.mat` that replaces the FBX material `name`, if remapped.
    pub fn material_for(&self, _name: &str) -> Option<Guid> {
        todo!()
    }
}

#[allow(dead_code)]
fn unused(_: &Node) {
    let _ = yaml::parse;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::model_meta;

    const MODEL: &str = "7078d86f15dd32d4bb4540ac2ff9e0ac";
    const BODY: &str = "c5898e34be6905c45a38f3f063d0172d";
    const HAIR: &str = "00f0f13e2177deb409cf3f9658d0fb00";

    #[test]
    fn model_meta_maps_material_names() {
        let m = Meta::parse(&model_meta(MODEL, &[("Body", BODY), ("Hair", HAIR)]));
        assert_eq!(m.guid, Guid::parse(MODEL));
        assert_eq!(m.importer.as_deref(), Some("ModelImporter"));
        assert_eq!(m.external_materials.len(), 2);
        assert_eq!(m.material_for("Hair"), Guid::parse(HAIR));
        assert_eq!(m.material_for("Body"), Guid::parse(BODY));
        assert_eq!(m.material_for("Skirt"), None);
        assert_eq!(m.global_scale, Some(1.0));
    }

    #[test]
    fn only_material_remaps_count() {
        let text = format!(
            "fileFormatVersion: 2\nguid: {MODEL}\nModelImporter:\n  externalObjects:\n  - first:\n      type: UnityEngine:Avatar\n      assembly: UnityEngine.CoreModule\n      name: Body\n    second: {{fileID: 9000000, guid: {HAIR}, type: 3}}\n"
        );
        let m = Meta::parse(&text);
        assert!(m.external_materials.is_empty());
        assert_eq!(m.material_for("Body"), None);
    }

    #[test]
    fn texture_meta_detects_normal_maps() {
        let text = format!("fileFormatVersion: 2\nguid: {HAIR}\nTextureImporter:\n  serializedVersion: 12\n  textureType: 1\n");
        let m = Meta::parse(&text);
        assert_eq!(m.importer.as_deref(), Some("TextureImporter"));
        assert!(m.normal_map);
        let plain = Meta::parse(&format!("guid: {HAIR}\nTextureImporter:\n  textureType: 0\n"));
        assert!(!plain.normal_map);
    }

    #[test]
    fn empty_and_broken_meta() {
        assert_eq!(Meta::parse(""), Meta::default());
        let m = Meta::parse("guid: not-a-guid\nModelImporter:\n  externalObjects: {}\n");
        assert_eq!(m.guid, None);
        assert!(m.external_materials.is_empty());
    }
}
