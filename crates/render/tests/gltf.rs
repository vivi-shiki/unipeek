//! glTF, GLB and VRM inputs, built in the test.
mod common;

use common::*;
use serde_json::json;
use unipeek::{
    LoadOptions, Scene,
    unity::{Input, Source, Status, load::Container, shader::RenderMode, shader::ShaderFamily},
};

/// A GLB with one quad in the XY plane at z = `z`, with `extensions` on the root and `material`.
fn glb(z: f32, material: serde_json::Value, extensions: serde_json::Value) -> Vec<u8> {
    let positions: [[f32; 3]; 4] = [[0.0, 0.0, z], [1.0, 0.0, z], [1.0, 1.0, z], [0.0, 1.0, z]];
    let indices: [u16; 6] = [0, 1, 2, 0, 2, 3];
    let mut bin = Vec::new();
    for p in positions {
        for c in p {
            bin.extend_from_slice(&c.to_le_bytes());
        }
    }
    for i in indices {
        bin.extend_from_slice(&i.to_le_bytes());
    }
    let png = png(4, 4, [255, 255, 255, 255]);
    let image_offset = bin.len();
    bin.extend_from_slice(&png);
    while bin.len() % 4 != 0 {
        bin.push(0);
    }
    let doc = json!({
        "asset": {"version": "2.0"},
        "extensionsUsed": extensions.as_object().map(|o| o.keys().cloned().collect::<Vec<_>>()).unwrap_or_default(),
        "extensions": extensions,
        "scene": 0,
        "scenes": [{"nodes": [0]}],
        "nodes": [{"mesh": 0, "name": "Quad"}],
        "meshes": [{"primitives": [{"attributes": {"POSITION": 0}, "indices": 1, "material": 0}]}],
        "materials": [material],
        "textures": [{"source": 0}],
        "images": [{"bufferView": 2, "mimeType": "image/png"}],
        "buffers": [{"byteLength": bin.len()}],
        "bufferViews": [
            {"buffer": 0, "byteOffset": 0, "byteLength": 48},
            {"buffer": 0, "byteOffset": 48, "byteLength": 12},
            {"buffer": 0, "byteOffset": image_offset, "byteLength": png.len()}
        ],
        "accessors": [
            {"bufferView": 0, "componentType": 5126, "count": 4, "type": "VEC3", "min": [0.0, 0.0, z], "max": [1.0, 1.0, z]},
            {"bufferView": 1, "componentType": 5123, "count": 6, "type": "SCALAR"}
        ]
    });
    let mut json = serde_json::to_vec(&doc).unwrap();
    while json.len() % 4 != 0 {
        json.push(b' ');
    }
    let mut out = b"glTF".to_vec();
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&((12 + 8 + json.len() + 8 + bin.len()) as u32).to_le_bytes());
    out.extend_from_slice(&(json.len() as u32).to_le_bytes());
    out.extend_from_slice(b"JSON");
    out.extend_from_slice(&json);
    out.extend_from_slice(&(bin.len() as u32).to_le_bytes());
    out.extend_from_slice(b"BIN\0");
    out.extend_from_slice(&bin);
    out
}

#[test]
fn glb_with_a_pbr_material_and_an_embedded_texture() {
    let material = json!({"name": "Red", "pbrMetallicRoughness": {"baseColorFactor": [1.0, 0.0, 0.0, 1.0], "baseColorTexture": {"index": 0}}, "alphaMode": "MASK", "doubleSided": true});
    let scene = Scene::load(Input::new(Source::Bytes(glb(0.0, material, json!({})).into())), &LoadOptions::default()).unwrap();
    assert_eq!(scene.triangles(), 2);
    assert_eq!(scene.report.container, Container::Gltf);
    assert_eq!(scene.report.status, Status::Ok, "{:?}", scene.report.degrades);
    let m = &scene.materials[scene.parts[0].material];
    assert_eq!((m.name.as_str(), m.look.color, m.look.mode), ("Red", [1.0, 0.0, 0.0, 1.0], RenderMode::Cutout));
    assert!(m.texture.is_some(), "the texture in the binary chunk is decoded");
    assert_eq!(scene.report.materials[0].family, Some(ShaderFamily::Standard));
    assert_eq!(scene.report.materials[0].texture_size, Some([4, 4]));
    assert!(scene.vertices.iter().all(|v| v.normal == [0.0, 0.0, 1.0]), "missing normals are made from the faces");
}

#[test]
fn vrm_mtoon_and_facing() {
    let mtoon = json!({"name": "Skin", "extensions": {"VRMC_materials_mtoon": {"specVersion": "1.0", "shadeColorFactor": [0.5, 0.4, 0.6], "outlineWidthMode": "worldCoordinates", "outlineWidthFactor": 0.002, "outlineColorFactor": [0.1, 0.0, 0.0]}}});
    let vrm1 = Scene::load(Input::new(Source::Bytes(glb(0.5, mtoon, json!({"VRMC_vrm": {"specVersion": "1.0"}})).into())), &LoadOptions::default()).unwrap();
    let look = vrm1.materials[0].look;
    assert_eq!(look.shadow_color, [0.5, 0.4, 0.6]);
    assert_eq!(look.outline.unwrap().width, 0.002);
    assert_eq!(vrm1.report.materials[0].family, Some(ShaderFamily::MToon));
    assert!((vrm1.max.z - 0.5).abs() < 1e-5, "VRM 1.0 already faces +Z");

    let plain = json!({"name": "Body"});
    let props = json!({"VRM": {"materialProperties": [{"name": "Body", "shader": "VRM/MToon", "floatProperties": {"_BlendMode": 1, "_Cutoff": 0.3}, "vectorProperties": {"_ShadeColor": [0.7, 0.6, 0.5, 1]}}]}});
    let vrm0 = Scene::load(Input::new(Source::Bytes(glb(0.5, plain, props).into())), &LoadOptions::default()).unwrap();
    let look = vrm0.materials[0].look;
    assert_eq!((look.mode, look.cutoff, look.shadow_color), (RenderMode::Cutout, 0.3, [0.7, 0.6, 0.5]));
    assert!((vrm0.min.z + 0.5).abs() < 1e-5 && vrm0.max.x <= 1e-5, "VRM 0.x is turned around to face +Z: {:?} {:?}", vrm0.min, vrm0.max);
}

#[test]
fn broken_gltf_is_unsupported_and_files_are_detected_by_name() {
    let err = Scene::load(Input::new(Source::Bytes(b"glTF\x02\x00\x00\x00 nope".to_vec().into())), &LoadOptions::default()).err().unwrap();
    assert_eq!(err.status(), Status::Unsupported, "{err}");
    let dir = std::env::temp_dir().join(format!("unipeek-gltf-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("Avatar.vrm");
    std::fs::write(&path, glb(0.0, json!({"name": "M"}), json!({}))).unwrap();
    let scene = Scene::load(Input::from_location(&path.display().to_string()), &LoadOptions::default()).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(scene.triangles(), 2);
    assert_eq!(scene.report.models[0].path, "Avatar.vrm");
}
