//! Prefabs: which models, renderers and materials a prefab uses, and which GUIDs it points at.
use crate::{
    Guid,
    yaml::{self, Node, ObjRef},
};

#[derive(Clone, Debug, PartialEq)]
pub struct GameObject {
    pub file_id: i64,
    pub name: String,
    pub active: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Renderer {
    pub file_id: i64,
    pub skinned: bool,
    pub game_object: i64,
    pub enabled: bool,
    pub mesh: Option<ObjRef>,
    pub materials: Vec<ObjRef>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Modification {
    pub target: ObjRef,
    pub property: String,
    pub value: Option<String>,
    pub reference: Option<ObjRef>,
}

/// A `PrefabInstance`: a nested prefab or a model placed in this prefab.
#[derive(Clone, Debug, PartialEq)]
pub struct Instance {
    pub file_id: i64,
    pub source: ObjRef,
    /// `m_TransformParent`; 0 for the root of a prefab variant.
    pub parent: i64,
    pub modifications: Vec<Modification>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Usage {
    /// A nested prefab or model (`m_SourcePrefab`, modification targets).
    Prefab,
    Mesh,
    Material,
    Texture,
    Shader,
    Script,
    Other,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Prefab {
    pub objects: Vec<GameObject>,
    pub renderers: Vec<Renderer>,
    pub instances: Vec<Instance>,
    /// `MeshFilter` components: game object → mesh.
    pub mesh_filters: Vec<(i64, ObjRef)>,
    /// Every GUID referenced from this file, with how it is used. Sorted and unique.
    pub references: Vec<(Guid, Usage)>,
}

impl Prefab {
    pub fn parse(_text: &str) -> Prefab {
        todo!()
    }

    /// A prefab variant has a root `PrefabInstance` without a parent transform.
    pub fn is_variant(&self) -> bool {
        todo!()
    }

    /// Materials assigned by renderers and by `m_Materials` modifications.
    pub fn material_refs(&self) -> Vec<Guid> {
        todo!()
    }

    /// Modifications that replace a material slot (`m_Materials.Array.data[i]`).
    pub fn material_overrides(&self) -> impl Iterator<Item = (&Instance, &Modification)> {
        self.instances.iter().flat_map(|i| i.modifications.iter().map(move |m| (i, m))).filter(|(_, m)| m.property.starts_with("m_Materials.Array.data["))
    }
}

#[allow(dead_code)]
fn unused(_: &Node) {
    let _ = yaml::parse_documents;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{prefab_instancing, prefab_with_renderer};

    const MODEL: &str = "7078d86f15dd32d4bb4540ac2ff9e0ac";
    const MAT_A: &str = "c5898e34be6905c45a38f3f063d0172d";
    const MAT_B: &str = "fd7c89bf2761e894eaf746251d625bab";
    const SCRIPT: &str = "5f3a8e3ff2bbb2a46b9cbd0cec5d2c6d";

    fn g(s: &str) -> Guid {
        Guid::parse(s).unwrap()
    }

    #[test]
    fn instance_of_a_model_with_material_override() {
        let p = Prefab::parse(&prefab_instancing(MODEL, &[(-7511558181221131132, 1, MAT_B)]));
        assert_eq!(p.instances.len(), 1);
        let inst = &p.instances[0];
        assert_eq!(inst.source.guid, Some(g(MODEL)));
        assert_eq!(inst.parent, 0);
        assert_eq!(inst.modifications.len(), 2);
        assert_eq!(inst.modifications[0].property, "m_Name");
        assert_eq!(inst.modifications[0].value.as_deref(), Some("Root"));
        assert!(p.is_variant());
        let overrides: Vec<_> = p.material_overrides().collect();
        assert_eq!(overrides.len(), 1);
        assert_eq!(overrides[0].1.reference.unwrap().guid, Some(g(MAT_B)));
        assert_eq!(p.material_refs(), vec![g(MAT_B)]);
        assert!(p.references.contains(&(g(MODEL), Usage::Prefab)));
        assert!(p.references.contains(&(g(MAT_B), Usage::Material)));
    }

    #[test]
    fn renderer_with_mesh_and_materials() {
        let p = Prefab::parse(&prefab_with_renderer("Body", MODEL, &[MAT_A, MAT_B]));
        assert_eq!(p.objects, vec![GameObject { file_id: 10, name: "Body".into(), active: true }]);
        assert_eq!(p.renderers.len(), 1);
        let r = &p.renderers[0];
        assert!(r.skinned && r.enabled);
        assert_eq!(r.game_object, 10);
        assert_eq!(r.mesh.unwrap().guid, Some(g(MODEL)));
        assert_eq!(r.materials.iter().map(|m| m.guid.unwrap()).collect::<Vec<_>>(), vec![g(MAT_A), g(MAT_B)]);
        assert!(!p.is_variant());
        assert_eq!(p.material_refs(), vec![g(MAT_A), g(MAT_B)]);
        assert!(p.references.contains(&(g(MODEL), Usage::Mesh)));
    }

    #[test]
    fn scripts_and_mesh_filters_are_classified() {
        let text = format!(
            "%YAML 1.1\n--- !u!1 &1\nGameObject:\n  m_Name: Prop\n  m_IsActive: 1\n--- !u!33 &2\nMeshFilter:\n  m_GameObject: {{fileID: 1}}\n  m_Mesh: {{fileID: 4300000, guid: {MODEL}, type: 3}}\n--- !u!23 &3\nMeshRenderer:\n  m_GameObject: {{fileID: 1}}\n  m_Enabled: 0\n  m_Materials:\n  - {{fileID: 2100000, guid: {MAT_A}, type: 2}}\n--- !u!114 &4\nMonoBehaviour:\n  m_GameObject: {{fileID: 1}}\n  m_Script: {{fileID: 11500000, guid: {SCRIPT}, type: 3}}\n"
        );
        let p = Prefab::parse(&text);
        assert_eq!(p.mesh_filters, vec![(1, ObjRef { file_id: 4300000, guid: Some(g(MODEL)), kind: Some(3) })]);
        assert_eq!(p.renderers.len(), 1);
        assert!(!p.renderers[0].skinned && !p.renderers[0].enabled);
        assert!(p.renderers[0].mesh.is_none());
        assert!(p.references.contains(&(g(SCRIPT), Usage::Script)));
        assert!(p.references.contains(&(g(MODEL), Usage::Mesh)));
        let unique: std::collections::BTreeSet<_> = p.references.iter().collect();
        assert_eq!(unique.len(), p.references.len());
    }

    #[test]
    fn inactive_objects_and_empty_text() {
        let text = "--- !u!1 &5\nGameObject:\n  m_Name: Hidden\n  m_IsActive: 0\n";
        assert!(!Prefab::parse(text).objects[0].active);
        assert_eq!(Prefab::parse(""), Prefab::default());
    }
}
