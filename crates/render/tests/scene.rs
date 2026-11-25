//! Loading a scene is CPU-only: these tests never need a GPU.
mod common;

use common::*;
use unipeek::{
    LoadOptions, Scene,
    unity::{
        Degrade, Status,
        shader::RenderMode,
        testing::{PackageBuilder, PsdBuilder, PsdCompression, fbx_ascii, material_yaml, model_meta, prefab_instancing},
    },
};

#[test]
fn loads_models_materials_and_textures_from_a_package() {
    let scene = Scene::load(input(two_quads()), &LoadOptions::default()).unwrap();
    assert_eq!(scene.triangles(), 4);
    assert_eq!(scene.parts.len(), 2);
    assert_eq!(scene.materials.len(), 2);
    let names: Vec<_> = scene.materials.iter().map(|m| m.name.as_str()).collect();
    assert_eq!(names, vec!["Red", "Blue"]);
    let red = &scene.materials[0];
    let mips = red.texture.as_ref().expect("red texture bound");
    assert_eq!((mips[0].width(), mips[0].height()), (64, 64));
    assert_eq!(mips[0].get_pixel(10, 10).0, [255, 0, 0, 255]);
    assert!(mips.len() > 1, "mips are built off the GPU");
    assert_eq!(red.look.mode, RenderMode::Opaque);
    let r = &scene.report;
    assert_eq!(r.prefab.as_deref(), Some("Assets/Shop/Quads.prefab"));
    assert_eq!(r.models[0].triangles, Some(4));
    assert_eq!(r.models[0].blend_shapes, Some(0));
    assert_eq!(r.materials.len(), 2);
    assert_eq!(r.materials[0].texture_size, Some([64, 64]));
    assert_eq!(r.status, Status::Ok, "{:?}", r.degrades);
    assert!((scene.max.x - 2.5).abs() < 1e-3 && (scene.max.y - 1.0).abs() < 1e-3, "{:?}", scene.max);
}

#[test]
fn big_textures_are_scaled_down() {
    let scene = Scene::load(input(two_quads()), &LoadOptions { max_texture: 1024, ..Default::default() }).unwrap();
    let blue = scene.materials[1].texture.as_ref().unwrap();
    assert_eq!((blue[0].width(), blue[0].height()), (1024, 4), "aspect kept, longest side clamped");
    assert_eq!(scene.report.materials[1].texture_size, Some([4096, 16]), "the report gives the original size");
}

#[test]
fn missing_and_unreadable_textures_degrade() {
    let gone = material_yaml("Red", LILTOON, 4800000, Some(GONE_TEX), -1);
    let psd = material_yaml("Blue", LILTOON, 4800000, Some(BLUE_TEX), -1);
    let pkg = PackageBuilder::new()
        .asset_with_meta(MODEL, "Assets/Shop/Quads.fbx", fbx_ascii(&["Red", "Blue"]), model_meta(MODEL, &[("Red", RED_MAT), ("Blue", BLUE_MAT)]))
        .asset(RED_MAT, "Assets/Shop/Red.mat", gone)
        .asset(BLUE_MAT, "Assets/Shop/Blue.mat", psd)
        .asset(BLUE_TEX, "Assets/Shop/Blue.psd", b"8BPS\x00\x01 not really a psd".to_vec())
        .asset(PREFAB, "Assets/Shop/Quads.prefab", prefab_instancing(MODEL, &[]))
        .build();
    let scene = Scene::load(input(pkg), &LoadOptions::default()).unwrap();
    assert_eq!(scene.triangles(), 4, "degrading never drops geometry");
    assert!(scene.materials.iter().all(|m| m.texture.is_none()));
    let d = &scene.report.degrades;
    assert!(d.iter().any(|x| matches!(x, Degrade::MissingTexture { .. })), "{d:?}");
    assert!(d.iter().any(|x| matches!(x, Degrade::TextureUnreadable { path, .. } if path == "Assets/Shop/Blue.psd")), "{d:?}");
    assert_eq!(scene.report.status, Status::Degraded);
}

#[test]
fn broken_fbx_degrades_with_the_model_path() {
    let pkg = PackageBuilder::new()
        .asset(MODEL, "Assets/Shop/Broken.fbx", b"; FBX 7.4.0 project file\nthis is not an fbx {{{".to_vec())
        .asset(PREFAB, "Assets/Shop/Broken.prefab", prefab_instancing(MODEL, &[]))
        .build();
    let scene = Scene::load(input(pkg), &LoadOptions::default()).unwrap();
    assert!(scene.is_empty());
    assert!(scene.report.degrades.iter().any(|d| matches!(d, Degrade::ModelUnreadable { path, .. } if path == "Assets/Shop/Broken.fbx")), "{:?}", scene.report.degrades);
}

#[test]
fn single_fbx_bytes_without_materials() {
    let scene = Scene::load(input(fbx_ascii(&["Skin"]).into_bytes()), &LoadOptions::default()).unwrap();
    assert_eq!(scene.triangles(), 2);
    assert_eq!(scene.materials.len(), 1);
    assert!(scene.materials[0].texture.is_none());
    assert!(scene.report.degrades.iter().any(|d| matches!(d, Degrade::MissingMaterial { slot, .. } if slot == "Skin")));
}

#[test]
fn non_assets_are_errors() {
    let err = Scene::load(input(b"hello".to_vec()), &LoadOptions::default()).err().unwrap();
    assert!(err.to_string().contains("not a unitypackage"), "{err}");
}

fn package_with_psds(blue: Vec<u8>) -> Vec<u8> {
    let red = PsdBuilder::rgba8(2, 2, &[[255, 0, 0, 255]; 4], false).compression(PsdCompression::Rle).build();
    PackageBuilder::new()
        .asset_with_meta(MODEL, "Assets/Shop/Quads.fbx", fbx_ascii(&["Red", "Blue"]), model_meta(MODEL, &[("Red", RED_MAT), ("Blue", BLUE_MAT)]))
        .asset(RED_MAT, "Assets/Shop/Red.mat", material_yaml("Red", LILTOON, 4800000, Some(RED_TEX), -1))
        .asset(BLUE_MAT, "Assets/Shop/Blue.mat", material_yaml("Blue", LILTOON, 4800000, Some(BLUE_TEX), -1))
        .asset(RED_TEX, "Assets/Shop/Red.psd", red)
        .asset(BLUE_TEX, "Assets/Shop/Blue.psd", blue)
        .asset(PREFAB, "Assets/Shop/Quads.prefab", prefab_instancing(MODEL, &[]))
        .build()
}

#[test]
fn psd_textures_use_the_composite() {
    let mut cmyk = PsdBuilder::rgba8(2, 2, &[[0, 0, 0, 0]; 4], false);
    (cmyk.mode, cmyk.planes) = (4, vec![vec![255; 4], vec![255; 4], vec![0; 4], vec![255; 4]]);
    let scene = Scene::load(input(package_with_psds(cmyk.build())), &LoadOptions::default()).unwrap();
    let red = scene.materials[0].texture.as_ref().expect("PSD composite decoded");
    assert_eq!(red[0].get_pixel(1, 1).0, [255, 0, 0, 255]);
    let blue = scene.materials[1].texture.as_ref().expect("CMYK composite decoded");
    assert_eq!(blue[0].get_pixel(0, 0).0, [255, 255, 0, 255], "full yellow ink");
    assert_eq!(scene.report.materials[0].texture_size, Some([2, 2]));
    let d = &scene.report.degrades;
    assert!(d.iter().any(|x| matches!(x, Degrade::TextureApproximated { path, reason } if path == "Assets/Shop/Blue.psd" && reason.contains("CMYK"))), "{d:?}");
    assert!(!d.iter().any(|x| matches!(x, Degrade::TextureApproximated { path, .. } if path == "Assets/Shop/Red.psd")), "{d:?}");
}

#[test]
fn psd_without_composite_is_unreadable_and_reports_skip_decoding() {
    let mut flat = PsdBuilder::rgba8(2, 2, &[[0, 0, 255, 255]; 4], false);
    flat.real_merged = Some(false);
    let scene = Scene::load(input(package_with_psds(flat.build())), &LoadOptions::default()).unwrap();
    assert!(scene.materials[1].texture.is_none());
    let d = &scene.report.degrades;
    assert!(d.iter().any(|x| matches!(x, Degrade::TextureUnreadable { reason, .. } if reason.contains("Maximize Compatibility"))), "{d:?}");

    let report_only = Scene::load(input(two_quads()), &LoadOptions { decode_textures: false, ..Default::default() }).unwrap();
    assert!(report_only.materials.iter().all(|m| m.texture.is_none()), "nothing decoded for a report");
    assert_eq!(report_only.report.materials[1].texture_size, Some([4096, 16]), "sizes come from the headers");
    assert_eq!(report_only.report.status, Status::Ok);
}

#[test]
fn prefab_overrides_reach_the_renderer_on_the_fbx_node() {
    use unipeek::unity::fileid;
    // The quads FBX has one top-level node, merged into the model prefab's root.
    let renderer = fileid::component(&fileid::node_path(&["Quads"], 1), "MeshRenderer");
    let size = format!("    - target: {{fileID: {renderer}, guid: {MODEL},\n        type: 3}}\n      propertyPath: m_Materials.Array.size\n      value: 3\n      objectReference: {{fileID: 0}}\n");
    let prefab = prefab_instancing(MODEL, &[(renderer, 1, RED_MAT), (renderer, 2, BLUE_MAT), (99999, 0, BLUE_MAT)]).replace("    m_RemovedComponents", &format!("{size}    m_RemovedComponents"));
    let pkg = PackageBuilder::new()
        .asset_with_meta(MODEL, "Assets/Shop/Quads.fbx", fbx_ascii(&["Red", "Blue"]), model_meta(MODEL, &[("Red", RED_MAT), ("Blue", BLUE_MAT)]))
        .asset(RED_MAT, "Assets/Shop/Red.mat", material_yaml("Red", LILTOON, 4800000, Some(RED_TEX), -1))
        .asset(BLUE_MAT, "Assets/Shop/Blue.mat", material_yaml("Blue", LILTOON, 4800000, Some(BLUE_TEX), -1))
        .asset(RED_TEX, "Assets/Shop/Red.png", png(4, 4, [255, 0, 0, 255]))
        .asset(BLUE_TEX, "Assets/Shop/Blue.png", png(4, 4, [0, 0, 255, 255]))
        .asset(PREFAB, "Assets/Shop/Quads.prefab", prefab)
        .build();
    let scene = Scene::load(input(pkg), &LoadOptions::default()).unwrap();
    let names: Vec<&str> = scene.parts.iter().map(|p| scene.materials[p.material].name.as_str()).collect();
    assert_eq!(names, vec!["Red", "Red", "Blue"], "slot 1 overridden to Red, slot 2 added after the last submesh");
    assert_eq!(scene.parts[1].first, scene.parts[2].first, "the added material draws the last submesh again");
    assert!(!scene.report.degrades.iter().any(|d| matches!(d, Degrade::MaterialOverridesIgnored { .. })), "{:?}", scene.report.degrades);
    assert_eq!(scene.report.stale_overrides, 1, "an override of an object the FBX no longer has is a leftover");

    // A target that matches no node is reported.
    let stray = prefab_instancing(MODEL, &[(12345, 0, BLUE_MAT)]);
    let pkg = PackageBuilder::new()
        .asset_with_meta(MODEL, "Assets/Shop/Quads.fbx", fbx_ascii(&["Red"]), model_meta(MODEL, &[]))
        .asset(PREFAB, "Assets/Shop/Quads.prefab", stray)
        .build();
    let scene = Scene::load(input(pkg), &LoadOptions::default()).unwrap();
    assert!(scene.report.degrades.contains(&Degrade::MaterialOverridesIgnored { count: 1 }), "{:?}", scene.report.degrades);
}
