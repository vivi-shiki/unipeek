use super::*;
use crate::{
    IndexOptions,
    material::Cull,
    shader::{RenderMode, ShaderFamily},
    testing::{FBX_BINARY_MAGIC, PNG_MAGIC, PackageBuilder, material_yaml, model_meta, prefab_instancing, prefab_with_renderer},
};
use std::io::Cursor;

const MODEL: &str = "7078d86f15dd32d4bb4540ac2ff9e0ac";
const TOY: &str = "f12727134b9968946aca776d25845173";
const PREFAB: &str = "00715af155a8b554aa5d5ea437e05c8f";
const VARIANT: &str = "875fb1d2161594e42a00c2890e28d006";
const DYNAMICS: &str = "4f920a12728c5f44db851043221361cb";
const BODY_MAT: &str = "c5898e34be6905c45a38f3f063d0172d";
const HAIR_MAT: &str = "00f0f13e2177deb409cf3f9658d0fb00";
const ALT_MAT: &str = "fd7c89bf2761e894eaf746251d625bab";
const BODY_TEX: &str = "4bbb1657292f0d74dab36e002aa5696d";
const HAIR_TEX: &str = "13f95bf558f0bbc45b85354fcd2de4eb";
const MISSING_TEX: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const BASE: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const SCRIPT: &str = "5f3a8e3ff2bbb2a46b9cbd0cec5d2c6d";
const LIL_O: &str = "efa77a80ca0344749b4f19fdd5891cbe";
const POI_SHADER: &str = "cccccccccccccccccccccccccccccccc";

fn g(s: &str) -> Guid {
    Guid::parse(s).unwrap()
}

fn fbx(len: usize) -> Vec<u8> {
    let mut v = FBX_BINARY_MAGIC.to_vec();
    v.resize(len, 1);
    v
}

fn index(b: PackageBuilder) -> PackageIndex {
    PackageIndex::read(Cursor::new(b.build()), &IndexOptions::default()).unwrap()
}

fn script_only_prefab() -> String {
    format!("%YAML 1.1\n--- !u!1 &1\nGameObject:\n  m_Name: Dynamics\n  m_IsActive: 1\n--- !u!114 &2\nMonoBehaviour:\n  m_GameObject: {{fileID: 1}}\n  m_Script: {{fileID: 11500000, guid: {SCRIPT}, type: 3}}\n")
}

fn avatar() -> PackageBuilder {
    PackageBuilder::new()
        .asset_with_meta(MODEL, "Assets/Shop/Avatar/FBX/Avatar.fbx", fbx(4000), model_meta(MODEL, &[("Body", BODY_MAT)]))
        .asset_with_meta(TOY, "Assets/Shop/Avatar/FBX/Toy.fbx", fbx(100), model_meta(TOY, &[]))
        .asset(BODY_MAT, "Assets/Shop/Avatar/Material/Body.mat", material_yaml("Body", LIL_O, 4800000, Some(BODY_TEX), 2000))
        .asset(HAIR_MAT, "Assets/Shop/Avatar/Material/Hair.mat", material_yaml("Hair", POI_SHADER, 4800000, Some(HAIR_TEX), -1))
        .asset(ALT_MAT, "Assets/Shop/Avatar/Material/Body_alt.mat", material_yaml("Body_alt", LIL_O, 4800000, Some(MISSING_TEX), 2000))
        .asset(BODY_TEX, "Assets/Shop/Avatar/Texture/Body.png", PNG_MAGIC.to_vec())
        .asset(HAIR_TEX, "Assets/Shop/Avatar/Texture/Hair.png", PNG_MAGIC.to_vec())
        .asset(POI_SHADER, "Assets/Poiyomi/Toon.shader", "Shader \".poiyomi/Poiyomi 8.0/Poiyomi Toon\" {\n}\n")
        .asset(PREFAB, "Assets/Shop/Avatar/Prefab/Avatar.prefab", prefab_instancing(MODEL, &[(-751155818122113113, 1, ALT_MAT)]))
        .asset(VARIANT, "Assets/Shop/Avatar/Prefab/Avatar_kisekae.prefab", prefab_instancing(PREFAB, &[]))
        .asset(DYNAMICS, "Assets/Shop/Avatar/Prefab/AvatarDynamics.prefab", script_only_prefab())
}

#[test]
fn main_prefab_is_the_plain_one_matching_the_model() {
    let idx = index(avatar());
    let plan = Plan::new(&idx, None).unwrap();
    let chosen = plan.prefab.as_ref().unwrap();
    assert_eq!(chosen.path, "Assets/Shop/Avatar/Prefab/Avatar.prefab");
    let paths: Vec<_> = plan.candidates.iter().map(|c| c.path.as_str()).collect();
    assert_eq!(paths, vec!["Assets/Shop/Avatar/Prefab/Avatar.prefab", "Assets/Shop/Avatar/Prefab/Avatar_kisekae.prefab"]);
    assert!(plan.candidates[1].variant);
    assert_eq!(plan.models, vec![PlannedModel { guid: g(MODEL), path: "Assets/Shop/Avatar/FBX/Avatar.fbx".into(), size: Some(4000) }]);
    assert_eq!(plan.material_overrides, 1);
    assert_eq!(plan.prefab_materials, vec![g(ALT_MAT)]);
    assert!(plan.missing.is_empty(), "{:?}", plan.missing);
}

#[test]
fn inner_path_selects_a_prefab_or_a_model() {
    let idx = index(avatar());
    let variant = Plan::new(&idx, Some("Assets/Shop/Avatar/Prefab/Avatar_kisekae.prefab")).unwrap();
    assert_eq!(variant.prefab.unwrap().guid, g(VARIANT));
    assert_eq!(variant.models.len(), 1, "models of nested prefabs count");
    let toy = Plan::new(&idx, Some("assets/shop/avatar/fbx/toy.fbx")).unwrap();
    assert!(toy.prefab.is_none());
    assert_eq!(toy.models[0].guid, g(TOY));
    assert!(matches!(Plan::new(&idx, Some("Assets/Nope.prefab")), Err(Error::NotFound(_))));
    assert!(matches!(Plan::new(&idx, Some("Assets/Shop/Avatar/Material/Body.mat")), Err(Error::Unsupported(_))));
}

#[test]
fn without_prefabs_the_largest_model_is_shown() {
    let idx = index(
        PackageBuilder::new()
            .asset(TOY, "Assets/A/Small.fbx", fbx(100))
            .asset(MODEL, "Assets/A/Big.fbx", fbx(9000))
            .asset(DYNAMICS, "Assets/A/Dynamics.prefab", script_only_prefab()),
    );
    let plan = Plan::new(&idx, None).unwrap();
    assert!(plan.prefab.is_none() && plan.candidates.is_empty());
    assert_eq!(plan.models[0].path, "Assets/A/Big.fbx");
}

#[test]
fn nothing_to_show() {
    let idx = index(PackageBuilder::new().asset(BODY_MAT, "Assets/A/Body.mat", material_yaml("Body", LIL_O, 4800000, None, -1)));
    let plan = Plan::new(&idx, None).unwrap();
    assert!(plan.prefab.is_none() && plan.models.is_empty());
}

#[test]
fn outfit_reports_missing_base_and_ignores_scripts_in_degrades() {
    let outfit = format!(
        "{}--- !u!114 &9\nMonoBehaviour:\n  m_GameObject: {{fileID: 1}}\n  m_Script: {{fileID: 11500000, guid: {SCRIPT}, type: 3}}\n",
        prefab_instancing(BASE, &[]).replace("%YAML 1.1\n%TAG !u! tag:unity3d.com,2011:\n", "")
    );
    let text = format!("%YAML 1.1\n{}{}", prefab_with_renderer("Outfit", MODEL, &[BODY_MAT]).replace("%YAML 1.1\n%TAG !u! tag:unity3d.com,2011:\n", ""), outfit);
    let idx = index(
        PackageBuilder::new()
            .asset_with_meta(MODEL, "Assets/Shop/Outfit/Outfit.fbx", fbx(500), model_meta(MODEL, &[]))
            .asset(BODY_MAT, "Assets/Shop/Outfit/Outfit.mat", material_yaml("Outfit", LIL_O, 4800000, None, -1))
            .asset(PREFAB, "Assets/Shop/Outfit/Outfit_for_Base.prefab", text),
    );
    let plan = Plan::new(&idx, None).unwrap();
    assert_eq!(plan.prefab.as_ref().unwrap().guid, g(PREFAB));
    assert_eq!(plan.models[0].guid, g(MODEL));
    assert!(plan.missing.contains(&MissingRef { guid: g(BASE), usage: Usage::Prefab }));
    assert!(plan.missing.contains(&MissingRef { guid: g(SCRIPT), usage: Usage::Script }));
    assert!(!plan.missing.iter().any(|m| m.guid == g(MODEL) || m.guid == g(BODY_MAT)));
}

#[test]
fn nested_prefab_cycles_terminate() {
    let idx = index(
        PackageBuilder::new()
            .asset(MODEL, "Assets/A/M.fbx", fbx(10))
            .asset(PREFAB, "Assets/A/A.prefab", prefab_instancing(VARIANT, &[]))
            .asset(VARIANT, "Assets/A/B.prefab", prefab_instancing(PREFAB, &[])),
    );
    let plan = Plan::new(&idx, None).unwrap();
    assert!(plan.candidates.is_empty());
    assert_eq!(plan.models[0].path, "Assets/A/M.fbx");
}

#[test]
fn bind_through_external_objects() {
    let idx = index(avatar());
    let plan = Plan::new(&idx, None).unwrap();
    let b = plan.bind(&idx, g(MODEL), "Body");
    assert_eq!(b.source, BindSource::ExternalObject);
    assert_eq!(b.material, Some(g(BODY_MAT)));
    assert_eq!(b.material_name.as_deref(), Some("Body"));
    assert_eq!(b.shader.as_ref().unwrap().family, ShaderFamily::LilToon);
    assert!(b.look.outline.is_some());
    assert_eq!(b.look.cull, Cull::Back);
    let tex = b.main_texture.unwrap();
    assert_eq!(tex, TextureRef { guid: g(BODY_TEX), path: Some("Assets/Shop/Avatar/Texture/Body.png".into()), in_package: true });
}

#[test]
fn bind_by_name_and_shader_in_package() {
    let idx = index(avatar());
    let plan = Plan::new(&idx, None).unwrap();
    let b = plan.bind(&idx, g(MODEL), "hair");
    assert_eq!(b.source, BindSource::NameMatch);
    assert_eq!(b.material, Some(g(HAIR_MAT)));
    assert_eq!(b.shader.as_ref().unwrap().family, ShaderFamily::Poiyomi);
    assert_eq!(b.look.mode, RenderMode::Opaque);
}

#[test]
fn bind_prefab_material_and_missing_texture() {
    let idx = index(avatar());
    let plan = Plan::new(&idx, None).unwrap();
    let b = plan.bind(&idx, g(MODEL), "Body_alt");
    assert_eq!(b.source, BindSource::PrefabMaterial);
    let tex = b.main_texture.unwrap();
    assert_eq!((tex.guid, tex.in_package, tex.path), (g(MISSING_TEX), false, None));
}

#[test]
fn bind_falls_back_to_texture_names_then_white() {
    let idx = index(
        PackageBuilder::new()
            .asset(MODEL, "Assets/A/M.fbx", fbx(10))
            .asset(BODY_TEX, "Assets/A/Tex/M_Skin.png", PNG_MAGIC.to_vec()),
    );
    let plan = Plan::new(&idx, None).unwrap();
    let b = plan.bind(&idx, g(MODEL), "Skin");
    assert_eq!(b.source, BindSource::TextureName);
    assert_eq!(b.main_texture.unwrap().guid, g(BODY_TEX));
    let none = plan.bind(&idx, g(MODEL), "Eyes");
    assert_eq!(none.source, BindSource::Unbound);
    assert!(none.main_texture.is_none() && none.material.is_none());
    assert_eq!(none.look, Look::white());
}

#[test]
fn renderer_slots_bind_by_object_and_index() {
    let pc01 = "51a2d3220584fc74dbf60acfa0885301";
    let idx = index(
        PackageBuilder::new()
            .asset_with_meta(MODEL, "Assets/S/FBX/Body.fbx", fbx(100), model_meta(MODEL, &[]))
            .asset(pc01, "Assets/S/MAT/PC01.mat", material_yaml("PC01", LIL_O, 4800000, Some(BODY_TEX), -1))
            .asset(BODY_TEX, "Assets/S/TEX/Body.png", PNG_MAGIC.to_vec())
            .asset(PREFAB, "Assets/S/Body.prefab", prefab_with_renderer("Body", MODEL, &[pc01])),
    );
    let plan = Plan::new(&idx, None).unwrap();
    let b = plan.bind_slot(&idx, g(MODEL), "Body", 0, "Body_PC_Mat");
    assert_eq!(b.source, BindSource::PrefabRenderer);
    assert_eq!(b.material, Some(g(pc01)));
    assert_eq!(b.main_texture.unwrap().guid, g(BODY_TEX));
    let only = plan.bind_slot(&idx, g(MODEL), "Armature_Body", 0, "Body_PC_Mat");
    assert_eq!(only.source, BindSource::PrefabRenderer, "the only renderer of a model is used even if names differ");
    let past_end = plan.bind_slot(&idx, g(MODEL), "Body", 3, "PC01");
    assert_eq!(past_end.source, BindSource::PrefabMaterial, "slots past the renderer's list fall back to names");
}

#[test]
fn texture_name_fallback_skips_empty_slots_and_normal_maps() {
    let normal_meta = format!("fileFormatVersion: 2\nguid: {HAIR_TEX}\nTextureImporter:\n  textureType: 1\n");
    let idx = index(
        PackageBuilder::new()
            .asset(MODEL, "Assets/A/M.fbx", fbx(10))
            .asset_with_meta(HAIR_TEX, "Assets/A/Tex/Skin.png", PNG_MAGIC.to_vec(), normal_meta)
            .asset(BODY_TEX, "Assets/A/Tex/Cloth_Normal_OpenGL.png", PNG_MAGIC.to_vec()),
    );
    let plan = Plan::new(&idx, None).unwrap();
    assert_eq!(plan.bind(&idx, g(MODEL), "").source, BindSource::Unbound, "an empty slot name matches nothing");
    assert_eq!(plan.bind(&idx, g(MODEL), "Skin").source, BindSource::Unbound, "normal maps (textureType 1) are not main textures");
    assert_eq!(plan.bind(&idx, g(MODEL), "OpenGL").source, BindSource::Unbound, "nor are files named like normal maps");
}

#[test]
fn unity_builtin_guids_are_never_missing() {
    let builtin_mesh = format!(
        "%YAML 1.1\n--- !u!1 &1\nGameObject:\n  m_Name: Cube\n  m_IsActive: 1\n--- !u!33 &2\nMeshFilter:\n  m_GameObject: {{fileID: 1}}\n  m_Mesh: {{fileID: 10202, guid: 0000000000000000e000000000000000, type: 0}}\n--- !u!23 &3\nMeshRenderer:\n  m_GameObject: {{fileID: 1}}\n  m_Materials:\n  - {{fileID: 10303, guid: 0000000000000000f000000000000000, type: 0}}\n"
    );
    let text = format!("{}{}", prefab_instancing(MODEL, &[]), builtin_mesh.replace("%YAML 1.1\n", ""));
    let idx = index(PackageBuilder::new().asset(MODEL, "Assets/A/M.fbx", fbx(10)).asset(PREFAB, "Assets/A/M.prefab", text));
    let plan = Plan::new(&idx, None).unwrap();
    assert!(plan.missing.is_empty(), "{:?}", plan.missing);
}

#[test]
fn main_prefab_uses_the_largest_model_before_name_matches() {
    let idx = index(
        PackageBuilder::new()
            .asset(MODEL, "Assets/Shop/FBX/Outfit.fbx", fbx(9000))
            .asset(TOY, "Assets/Shop/FBX/Pet.fbx", fbx(500))
            .asset(PREFAB, "Assets/Shop/Outfit_Black.prefab", prefab_instancing(MODEL, &[]))
            .asset(VARIANT, "Assets/Shop/Outfit_withPet.prefab", prefab_instancing(MODEL, &[]) + &prefab_instancing(TOY, &[]).replace("%YAML 1.1\n%TAG !u! tag:unity3d.com,2011:\n", "").replace("&100100000001", "&100100000002"))
            .asset(DYNAMICS, "Assets/Shop/Extra/Pet.prefab", prefab_instancing(TOY, &[])),
    );
    let plan = Plan::new(&idx, None).unwrap();
    assert_eq!(plan.prefab.unwrap().path, "Assets/Shop/Outfit_Black.prefab");
}

#[test]
fn binary_materials_are_reported_unreadable() {
    let binary_mat: Vec<u8> = [&[0u8, 0, 0, 0, 0, 0, 0x10, 0, 0, 0, 0, 0x16][..], &[0x42; 64]].concat();
    let idx = index(
        PackageBuilder::new()
            .asset_with_meta(MODEL, "Assets/A/M.fbx", fbx(10), model_meta(MODEL, &[("Body", BODY_MAT)]))
            .asset(BODY_MAT, "Assets/A/Body.mat", binary_mat),
    );
    let plan = Plan::new(&idx, None).unwrap();
    let b = plan.bind(&idx, g(MODEL), "Body");
    assert_eq!(b.material, Some(g(BODY_MAT)));
    assert!(b.unreadable.as_deref().is_some_and(|r| r.contains("binary")), "{:?}", b.unreadable);
    assert_eq!(b.look, Look::white());
}

#[test]
fn nested_prefabs_stop_at_32_levels() {
    let guids: Vec<String> = (0..40).map(|i| format!("{:032x}", 0xabc0_0000u64 + i)).collect();
    let chain = |len: usize| {
        let mut b = PackageBuilder::new().asset(MODEL, "Assets/A/M.fbx", fbx(10));
        for i in 0..len {
            let next = if i + 1 == len { MODEL.to_string() } else { guids[i + 1].clone() };
            b = b.asset(&guids[i], &format!("Assets/A/P{i:02}.prefab"), prefab_instancing(&next, &[]));
        }
        index(b)
    };
    let short = chain(10);
    let plan = Plan::new(&short, Some("Assets/A/P00.prefab")).unwrap();
    assert_eq!(plan.models.len(), 1, "a 10-level chain reaches the model");
    let long = chain(40);
    let plan = Plan::new(&long, Some("Assets/A/P00.prefab")).unwrap();
    assert!(plan.models.is_empty(), "the walk stops after 32 nested prefabs");
}
