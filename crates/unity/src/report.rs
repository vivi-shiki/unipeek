//! What unipeek found and why it had to degrade. Degrading is never an error.
use crate::{
    PackageIndex, PackageIssue, Plan, ReadStats,
    load::Container,
    plan::{BindSource, Binding, MissingRef},
    shader::{RenderMode, ShaderFamily},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Ok,
    Degraded,
    Unsupported,
    Error,
}

impl Status {
    /// Process exit code for the command line: 0 ok, 1 degraded, 2 unsupported, 3 error.
    pub fn exit_code(self) -> i32 {
        match self {
            Status::Ok => 0,
            Status::Degraded => 1,
            Status::Unsupported => 2,
            Status::Error => 3,
        }
    }
}

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Degrade {
    /// No prefab and no model in the input.
    NothingToShow,
    /// A nested prefab or mesh the prefab uses is not in the package (often the base avatar).
    MissingDependency { guid: crate::Guid, usage: crate::prefab::Usage },
    /// An FBX material slot got no material and no texture: drawn white.
    MissingMaterial { model: String, slot: String },
    UnknownShader { material: String, shader: Option<String> },
    MissingTexture { material: String, guid: crate::Guid },
    TextureUnreadable { path: String, reason: String },
    ModelUnreadable { path: String, reason: String },
    /// `m_Materials` overrides on model instances were not applied.
    MaterialOverridesIgnored { count: usize },
    Truncated { message: String },
}

#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
pub struct ModelReport {
    pub path: String,
    pub triangles: Option<u64>,
    pub bones: Option<u64>,
    pub blend_shapes: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct MaterialReport {
    pub model: String,
    pub slot: String,
    pub source: BindSource,
    pub material: Option<String>,
    pub shader: Option<String>,
    pub family: Option<ShaderFamily>,
    pub mode: RenderMode,
    pub texture: Option<String>,
    pub texture_size: Option<[u32; 2]>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct Report {
    pub input: String,
    pub container: Container,
    /// The `.unitypackage` picked inside a zip or folder.
    pub member: Option<String>,
    pub prefab: Option<String>,
    pub candidates: Vec<String>,
    pub models: Vec<ModelReport>,
    pub materials: Vec<MaterialReport>,
    pub missing: Vec<MissingRef>,
    pub degrades: Vec<Degrade>,
    pub issues: Vec<PackageIssue>,
    pub read: ReadStats,
    pub status: Status,
}

impl Report {
    pub fn new(_input: String, _container: Container, _member: Option<String>, _index: &PackageIndex, _plan: &Plan) -> Report {
        todo!()
    }

    /// Record how one FBX material slot was bound, with the degrades it implies.
    pub fn add_binding(&mut self, _model: &str, _slot: &str, _binding: &Binding) {
        todo!()
    }

    pub fn degrade(&mut self, _d: Degrade) {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Guid, IndexOptions, Input, Source,
        load::Package,
        prefab::Usage,
        testing::{FBX_BINARY_MAGIC, PNG_MAGIC, PackageBuilder, material_yaml, model_meta, prefab_instancing},
    };
    use std::sync::Arc;

    const MODEL: &str = "7078d86f15dd32d4bb4540ac2ff9e0ac";
    const PREFAB: &str = "00715af155a8b554aa5d5ea437e05c8f";
    const MAT: &str = "c5898e34be6905c45a38f3f063d0172d";
    const ODD_MAT: &str = "00f0f13e2177deb409cf3f9658d0fb00";
    const TEX: &str = "4bbb1657292f0d74dab36e002aa5696d";
    const GONE_TEX: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const BASE: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    const LIL: &str = "df12117ecd77c31469c224178886498e";
    const STRANGE: &str = "dddddddddddddddddddddddddddddddd";

    fn open(b: PackageBuilder) -> (Package, Plan) {
        let (pkg, inner) = Package::open(Input::new(Source::Bytes(Arc::from(b.build()))), &IndexOptions::default()).unwrap();
        let plan = Plan::new(&pkg.index, inner.as_deref()).unwrap();
        (pkg, plan)
    }

    fn fbx() -> Vec<u8> {
        let mut v = FBX_BINARY_MAGIC.to_vec();
        v.resize(100, 0);
        v
    }

    fn package(prefab: String) -> PackageBuilder {
        PackageBuilder::new()
            .asset_with_meta(MODEL, "Assets/A/Avatar.fbx", fbx(), model_meta(MODEL, &[("Body", MAT), ("Odd", ODD_MAT)]))
            .asset(MAT, "Assets/A/Body.mat", material_yaml("Body", LIL, 4800000, Some(TEX), -1))
            .asset(ODD_MAT, "Assets/A/Odd.mat", material_yaml("Odd", STRANGE, 4800000, Some(GONE_TEX), -1))
            .asset(TEX, "Assets/A/Body.png", PNG_MAGIC.to_vec())
            .asset(PREFAB, "Assets/A/Avatar.prefab", prefab)
    }

    #[test]
    fn clean_package_is_ok() {
        let (pkg, plan) = open(package(prefab_instancing(MODEL, &[])));
        let mut r = Report::new("Avatar.unitypackage".into(), pkg.container, None, &pkg.index, &plan);
        assert_eq!(r.prefab.as_deref(), Some("Assets/A/Avatar.prefab"));
        assert_eq!(r.models, vec![ModelReport { path: "Assets/A/Avatar.fbx".into(), ..Default::default() }]);
        assert_eq!(r.status, Status::Ok);
        let b = plan.bind(&pkg.index, plan.models[0].guid, "Body");
        r.add_binding("Assets/A/Avatar.fbx", "Body", &b);
        assert_eq!(r.status, Status::Ok);
        assert_eq!(r.materials.len(), 1);
        let m = &r.materials[0];
        assert_eq!((m.material.as_deref(), m.family, m.texture.as_deref()), (Some("Body"), Some(ShaderFamily::LilToon), Some("Assets/A/Body.png")));
        assert_eq!(r.read.passes, 1);
    }

    #[test]
    fn bindings_add_degrades_and_missing_textures() {
        let (pkg, plan) = open(package(prefab_instancing(MODEL, &[(-1, 0, MAT)])));
        let mut r = Report::new("x".into(), pkg.container, None, &pkg.index, &plan);
        assert!(r.degrades.contains(&Degrade::MaterialOverridesIgnored { count: 1 }));
        let model = plan.models[0].guid;
        r.add_binding("Assets/A/Avatar.fbx", "Odd", &plan.bind(&pkg.index, model, "Odd"));
        r.add_binding("Assets/A/Avatar.fbx", "Eyes", &plan.bind(&pkg.index, model, "Eyes"));
        assert!(r.degrades.contains(&Degrade::UnknownShader { material: "Odd".into(), shader: None }));
        assert!(r.degrades.contains(&Degrade::MissingTexture { material: "Odd".into(), guid: Guid::parse(GONE_TEX).unwrap() }));
        assert!(r.degrades.contains(&Degrade::MissingMaterial { model: "Assets/A/Avatar.fbx".into(), slot: "Eyes".into() }));
        assert!(r.missing.contains(&MissingRef { guid: Guid::parse(GONE_TEX).unwrap(), usage: Usage::Texture }));
        assert_eq!(r.status, Status::Degraded);
        let before = r.degrades.len();
        r.add_binding("Assets/A/Avatar.fbx", "Odd", &plan.bind(&pkg.index, model, "Odd"));
        assert_eq!(r.degrades.len(), before, "degrades are not repeated");
    }

    #[test]
    fn missing_base_avatar_degrades_but_scripts_do_not() {
        let prefab = prefab_instancing(MODEL, &[]) + &prefab_instancing(BASE, &[]).replace("%YAML 1.1\n%TAG !u! tag:unity3d.com,2011:\n", "").replace("&100100000001", "&100100000002");
        let (pkg, plan) = open(package(prefab));
        let r = Report::new("x".into(), pkg.container, None, &pkg.index, &plan);
        assert!(r.degrades.contains(&Degrade::MissingDependency { guid: Guid::parse(BASE).unwrap(), usage: Usage::Prefab }), "{:?}", r.degrades);
        assert_eq!(r.status, Status::Degraded);
    }

    #[test]
    fn empty_input_has_nothing_to_show_and_serializes() {
        let (pkg, plan) = open(PackageBuilder::new().asset(MAT, "Assets/A/Body.mat", material_yaml("Body", LIL, 4800000, None, -1)));
        let r = Report::new("x".into(), pkg.container, None, &pkg.index, &plan);
        assert_eq!(r.degrades, vec![Degrade::NothingToShow]);
        let json = serde_json::to_string(&r).unwrap();
        assert!(json.contains(r#""status":"degraded""#), "{json}");
        assert!(json.contains(r#""kind":"nothing_to_show""#), "{json}");
        assert!(json.contains(r#""container":"unity_package""#), "{json}");
        assert_eq!(Status::Degraded.exit_code(), 1);
    }
}
