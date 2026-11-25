//! Cross-package dependencies: an outfit whose prefab nests the base avatar from another package.
mod common;

use common::*;
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};
use unipeek::{
    LoadOptions, Scene,
    unity::{
        Degrade, Guid, IndexOptions, Located, Resolver, Sources, Status,
        prefab::Usage,
        testing::{PackageBuilder, fbx_ascii, material_yaml, model_meta, prefab_instancing},
    },
};

const BASE_MODEL: &str = "b0000000000000000000000000000001";
const BASE_PREFAB: &str = "b0000000000000000000000000000002";
const SKIN_MAT: &str = "b0000000000000000000000000000003";
const SKIN_TEX: &str = "b0000000000000000000000000000004";
const OUTFIT_MODEL: &str = "c0000000000000000000000000000001";
const OUTFIT_PREFAB: &str = "c0000000000000000000000000000002";
const CLOTH_MAT: &str = "c0000000000000000000000000000003";

fn base() -> Vec<u8> {
    PackageBuilder::new()
        .asset_with_meta(BASE_MODEL, "Assets/Base/Base.fbx", fbx_ascii(&["Skin"]), model_meta(BASE_MODEL, &[("Skin", SKIN_MAT)]))
        .asset(SKIN_MAT, "Assets/Base/Skin.mat", material_yaml("Skin", LILTOON, 4800000, Some(SKIN_TEX), -1))
        .asset(SKIN_TEX, "Assets/Base/Skin.png", png(4, 4, [250, 200, 180, 255]))
        .asset(BASE_PREFAB, "Assets/Base/Base.prefab", prefab_instancing(BASE_MODEL, &[]))
        .build()
}

/// The outfit's prefab nests the base prefab (not in this package) and its own model, and one of its
/// slots uses a material from the base package.
fn outfit() -> Vec<u8> {
    let nested = prefab_instancing(BASE_PREFAB, &[]).replace("%YAML 1.1\n%TAG !u! tag:unity3d.com,2011:\n", "").replace("&100100000001", "&100100000002");
    PackageBuilder::new()
        .asset_with_meta(OUTFIT_MODEL, "Assets/Outfit/Outfit.fbx", fbx_ascii(&["Cloth", "Belt"]), model_meta(OUTFIT_MODEL, &[("Cloth", CLOTH_MAT), ("Belt", SKIN_MAT)]))
        .asset(CLOTH_MAT, "Assets/Outfit/Cloth.mat", material_yaml("Cloth", LILTOON, 4800000, None, -1))
        .asset(OUTFIT_PREFAB, "Assets/Outfit/Outfit.prefab", prefab_instancing(OUTFIT_MODEL, &[]) + &nested)
        .build()
}

struct Dir(PathBuf);

impl Dir {
    fn new(name: &str) -> Dir {
        let p = std::env::temp_dir().join(format!("unipeek-resolver-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        std::fs::write(p.join("Base.unitypackage"), base()).unwrap();
        std::fs::write(p.join("Outfit.unitypackage"), outfit()).unwrap();
        Dir(p)
    }

    fn path(&self, name: &str) -> String {
        self.0.join(name).display().to_string()
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn g(s: &str) -> Guid {
    Guid::parse(s).unwrap()
}

#[test]
fn without_a_resolver_the_base_is_listed_as_missing() {
    let scene = Scene::load(input(outfit()), &LoadOptions::default()).unwrap();
    assert_eq!(scene.triangles(), 4, "only the outfit");
    let d = &scene.report.degrades;
    assert!(d.contains(&Degrade::MissingDependency { guid: g(BASE_PREFAB), usage: Usage::Prefab }), "{d:?}");
    assert!(d.contains(&Degrade::MissingDependency { guid: g(SKIN_MAT), usage: Usage::Material }), "a slot's material from another package: {d:?}");
    assert_eq!(scene.report.status, Status::Degraded);
}

#[test]
fn sources_bring_in_the_base_avatar() {
    let dir = Dir::new("sources");
    let with = Sources::new(&[dir.0.display().to_string()], IndexOptions::default());
    assert_eq!(with.locations().count(), 2, "a folder counts as each package in it");
    let opts = LoadOptions { resolver: Some(Arc::new(with)), ..Default::default() };
    let scene = Scene::load(unipeek::unity::Input::from_location(&dir.path("Outfit.unitypackage")), &opts).unwrap();
    assert_eq!(scene.triangles(), 6, "outfit and base together");
    let r = &scene.report;
    assert_eq!(r.status, Status::Ok, "{:?}", r.degrades);
    assert_eq!(r.dependencies, vec![dir.path("Base.unitypackage")]);
    assert!(r.models.iter().any(|m| m.path == "Assets/Base/Base.fbx"));
    let skin = scene.materials.iter().find(|m| m.name == "Skin").expect("the base's material");
    assert_eq!(skin.texture.as_ref().unwrap()[0].get_pixel(0, 0).0, [250, 200, 180, 255], "its texture is read from the base package");
    assert!(r.read.passes >= 3, "outfit index, base index, and reads: {:?}", r.read);
}

/// A host's resolver: records each batch and answers from a fixed table.
struct Table {
    calls: Mutex<Vec<Vec<Guid>>>,
    base: String,
}

impl Resolver for Table {
    fn locate(&self, guids: &[Guid]) -> Vec<(Guid, Located)> {
        self.calls.lock().unwrap().push(guids.to_vec());
        guids.iter().filter(|g| g.to_string().starts_with('b')).map(|g| (*g, Located::Package { location: self.base.clone(), guid: *g })).collect()
    }
}

#[test]
fn lookups_are_batched_and_each_package_is_opened_once() {
    let dir = Dir::new("table");
    let table = Arc::new(Table { calls: Mutex::new(Vec::new()), base: dir.path("Base.unitypackage") });
    let opts = LoadOptions { resolver: Some(table.clone()), ..Default::default() };
    let scene = Scene::load(input(outfit()), &opts).unwrap();
    assert_eq!(scene.triangles(), 6);
    let calls = table.calls.lock().unwrap();
    assert!(calls[0].contains(&g(BASE_PREFAB)) && calls[0].contains(&g(SKIN_MAT)), "one batch with every missing GUID: {calls:?}");
    assert_eq!(scene.report.dependencies.len(), 1, "the base package was merged once");
    assert!(calls.iter().skip(1).all(|c| !c.contains(&g(BASE_PREFAB))), "a GUID is asked for once: {calls:?}");
}

#[test]
fn loose_files_and_unknown_guids() {
    let dir = Dir::new("files");
    std::fs::write(dir.0.join("Skin.mat"), material_yaml("Skin", LILTOON, 4800000, None, -1)).unwrap();
    struct Files(PathBuf);
    impl Resolver for Files {
        fn locate(&self, guids: &[Guid]) -> Vec<(Guid, Located)> {
            guids.iter().filter(|g| g.to_string() == SKIN_MAT).map(|g| (*g, Located::File(self.0.join("Skin.mat")))).collect()
        }
    }
    let opts = LoadOptions { resolver: Some(Arc::new(Files(dir.0.clone()))), ..Default::default() };
    let scene = Scene::load(input(outfit()), &opts).unwrap();
    assert!(scene.materials.iter().any(|m| m.name == "Skin"), "a .mat on disk binds the slot");
    let d = &scene.report.degrades;
    assert!(!d.iter().any(|x| matches!(x, Degrade::MissingDependency { usage: Usage::Material, .. })), "{d:?}");
    assert!(d.contains(&Degrade::MissingDependency { guid: g(BASE_PREFAB), usage: Usage::Prefab }), "what the resolver does not know stays missing: {d:?}");
}

#[test]
fn command_line_and_serve_take_with() {
    let dir = Dir::new("cli");
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_unipeek"))
        .args(["info", &dir.path("Outfit.unitypackage"), "--json", "--with", &dir.path("Base.unitypackage")])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0), "{}", String::from_utf8_lossy(&out.stderr));
    let report: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(report["dependencies"][0], dir.path("Base.unitypackage"));
    let missing = std::process::Command::new(env!("CARGO_BIN_EXE_unipeek")).args(["info", &dir.path("Outfit.unitypackage"), "--with"]).output().unwrap();
    assert_eq!(missing.status.code(), Some(64));

    let request = serde_json::json!({"id": 1, "op": "info", "source": dir.path("Outfit.unitypackage"), "with": [dir.0.display().to_string()]});
    let mut reply = Vec::new();
    unipeek::serve::serve(format!("{request}\n").as_bytes(), &mut reply);
    let reply: serde_json::Value = serde_json::from_slice(&reply).unwrap();
    assert_eq!(reply["status"], "ok", "{reply}");
    let without = serde_json::json!({"id": 2, "op": "info", "source": dir.path("Outfit.unitypackage")});
    let mut reply = Vec::new();
    unipeek::serve::serve(format!("{without}\n").as_bytes(), &mut reply);
    let reply: serde_json::Value = serde_json::from_slice(&reply).unwrap();
    assert_eq!(reply["status"], "degraded");
    let guids: Vec<&str> = reply["missing_guids"].as_array().unwrap().iter().map(|v| v.as_str().unwrap()).collect();
    assert!(guids.contains(&BASE_PREFAB) && guids.contains(&SKIN_MAT), "{guids:?}");
}
