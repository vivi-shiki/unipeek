//! glTF 2.0, GLB and VRM (0.x and 1.0) as a `Scene`: meshes in their rest pose, glTF materials as
//! looks, MToon parameters from the VRM extensions. Textures go through the same decoder as Unity
//! packages, on worker threads.
use crate::{
    Error, Result,
    model::{self, Vertex},
    scene::{LoadEvent, LoadOptions, Part, Scene, SceneMaterial},
    texture,
};
use glam::{Mat4, Vec3, Vec4};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::Arc,
    time::Instant,
};
use unipeek_unity::{
    Degrade, Report, Status, Timings,
    load::Container,
    material::{Cull, Look, Matcap, MatcapBlend, Outline},
    report::{MaterialReport, ModelReport},
    shader::{RenderMode, ShaderFamily},
    plan::BindSource,
};

/// Whether `bytes` (the start of a file) or the file name says glTF.
pub fn is_gltf(head: &[u8], name: Option<&str>) -> bool {
    head.starts_with(b"glTF") || name.is_some_and(|n| {
        let n = n.to_ascii_lowercase();
        n.ends_with(".gltf") || n.ends_with(".glb") || n.ends_with(".vrm")
    })
}

/// Load a glTF, GLB or VRM file. `base` is the folder external buffers and images are read from.
pub fn load(bytes: &[u8], base: Option<&Path>, input: String, opts: &LoadOptions, events: &mut dyn FnMut(LoadEvent<'_>)) -> Result<Scene> {
    let t = Instant::now();
    let mut timings = Timings::default();
    let gltf = gltf::Gltf::from_slice(bytes).map_err(|e| Error::Unity(unipeek_unity::Error::Unsupported(format!("unreadable glTF: {e}"))))?;
    let json = raw_json(bytes).unwrap_or(Value::Null);
    let blob = gltf.blob.clone();
    let doc = gltf.document;
    let buffers = gltf::import_buffers(&doc, base, blob).map_err(|e| Error::Unity(unipeek_unity::Error::Unsupported(format!("glTF buffers: {e}"))))?;
    timings.index_ms = Timings::since(t);
    events(LoadEvent::Progress(0.3));

    let vrm0 = json.pointer("/extensions/VRM").is_some();
    let vrm1 = json.pointer("/extensions/VRMC_vrm").is_some();
    let mut report = Report {
        input,
        container: Container::Gltf,
        member: None,
        prefab: None,
        candidates: Vec::new(),
        models: Vec::new(),
        materials: Vec::new(),
        missing: Vec::new(),
        degrades: Vec::new(),
        issues: Vec::new(),
        read: unipeek_unity::ReadStats { passes: 1, bytes: bytes.len() as u64 },
        timings: Timings::default(),
        stale_overrides: 0,
        dependencies: Vec::new(),
        status: Status::Ok,
    };

    // Materials first, by glTF index; the last one is the default material.
    let t = Instant::now();
    let mut materials: Vec<SceneMaterial> = Vec::new();
    let mut textures: Vec<[Option<usize>; 4]> = Vec::new();
    for (i, m) in doc.materials().enumerate() {
        let (look, maps, family, shader) = material_look(&m, json.pointer(&format!("/materials/{i}")), vrm0.then(|| vrm0_material(&json, m.name())).flatten());
        let name = m.name().map(str::to_string).unwrap_or_else(|| format!("material {i}"));
        report.materials.push(MaterialReport {
            model: report_name(&report.input),
            slot: name.clone(),
            source: BindSource::ExternalObject,
            material: Some(name.clone()),
            shader: Some(shader.to_string()),
            family: Some(family),
            mode: look.mode,
            texture: maps[0].map(|t| format!("texture {t}")),
            texture_size: None,
        });
        materials.push(SceneMaterial { name, look, texture: None, normal: None, emission: None, matcap: None });
        textures.push(maps);
    }
    let default_material = materials.len();
    materials.push(SceneMaterial { name: "default".into(), look: Look::white(), texture: None, normal: None, emission: None, matcap: None });
    textures.push([None; 4]);

    // Geometry in the rest pose: node transforms, and skinned meshes through their joints.
    let scene = doc.default_scene().or_else(|| doc.scenes().next());
    let mut world: BTreeMap<usize, Mat4> = BTreeMap::new();
    if let Some(scene) = &scene {
        for root in scene.nodes() {
            walk(&root, Mat4::IDENTITY, &mut world);
        }
    } else {
        for n in doc.nodes() {
            world.insert(n.index(), Mat4::from_cols_array_2d(&n.transform().matrix()));
        }
    }
    // VRM 0.x models face -Z; turn them to face +Z like everything else.
    let facing = if vrm0 && !vrm1 { Mat4::from_rotation_y(std::f32::consts::PI) } else { Mat4::IDENTITY };
    let mut vertices: Vec<Vertex> = Vec::new();
    let mut parts: Vec<Part> = Vec::new();
    let (mut bones, mut morphs) = (0u64, 0u64);
    let get = |b: gltf::Buffer<'_>| buffers.get(b.index()).map(|d| &d.0[..]);
    for node in doc.nodes() {
        let Some(mesh) = node.mesh() else { continue };
        let Some(&node_world) = world.get(&node.index()) else { continue };
        let joints: Option<Vec<Mat4>> = node.skin().map(|skin| {
            bones = bones.max(skin.joints().count() as u64);
            let inverse: Vec<Mat4> = skin.reader(get).read_inverse_bind_matrices().map(|m| m.map(|c| Mat4::from_cols_array_2d(&c)).collect()).unwrap_or_default();
            skin.joints().enumerate().map(|(j, joint)| world.get(&joint.index()).copied().unwrap_or(Mat4::IDENTITY) * inverse.get(j).copied().unwrap_or(Mat4::IDENTITY)).collect()
        });
        for prim in mesh.primitives() {
            if prim.mode() != gltf::mesh::Mode::Triangles {
                continue;
            }
            let reader = prim.reader(get);
            let Some(positions) = reader.read_positions() else { continue };
            let positions: Vec<[f32; 3]> = positions.collect();
            let normals: Option<Vec<[f32; 3]>> = reader.read_normals().map(Iterator::collect);
            let uvs: Option<Vec<[f32; 2]>> = reader.read_tex_coords(0).map(|t| t.into_f32().collect());
            let skin_data = joints.as_ref().and_then(|_| Some((reader.read_joints(0)?.into_u16().collect::<Vec<_>>(), reader.read_weights(0)?.into_f32().collect::<Vec<_>>())));
            morphs += prim.morph_targets().count() as u64;
            let indices: Vec<u32> = match reader.read_indices() {
                Some(i) => i.into_u32().collect(),
                None => (0..positions.len() as u32).collect(),
            };
            let transform = |i: usize| -> Mat4 {
                match (&joints, &skin_data) {
                    (Some(j), Some((ids, ws))) if i < ids.len() => {
                        let mut m = Mat4::ZERO;
                        for k in 0..4 {
                            let w = ws[i][k];
                            if w > 0.0 {
                                m += *j.get(ids[i][k] as usize).unwrap_or(&Mat4::IDENTITY) * w;
                            }
                        }
                        if m == Mat4::ZERO { node_world } else { m }
                    }
                    _ => node_world,
                }
            };
            let first = vertices.len() as u32;
            for tri in indices.chunks_exact(3) {
                let ids = [tri[0] as usize, tri[1] as usize, tri[2] as usize];
                if ids.iter().any(|&i| i >= positions.len()) {
                    continue;
                }
                let p = ids.map(|i| (facing * transform(i)).transform_point3(Vec3::from(positions[i])));
                let flat = (p[1] - p[0]).cross(p[2] - p[0]).normalize_or_zero();
                for (k, &i) in ids.iter().enumerate() {
                    let m = facing * transform(i);
                    let n = normals.as_ref().and_then(|n| n.get(i)).map(|n| (m.inverse().transpose() * Vec4::from((Vec3::from(*n), 0.0))).truncate().normalize_or_zero()).unwrap_or(flat);
                    vertices.push(Vertex { pos: p[k].to_array(), normal: n.to_array(), uv: uvs.as_ref().and_then(|u| u.get(i)).copied().unwrap_or([0.0; 2]), tangent: [1.0, 0.0, 0.0, 1.0] });
                }
            }
            let count = vertices.len() as u32 - first;
            if count > 0 {
                parts.push(Part { material: prim.material().index().unwrap_or(default_material), first, count });
            }
        }
    }
    timings.models_ms = Timings::since(t);
    let triangles = vertices.len() as u64 / 3;
    report.models.push(ModelReport { path: report_name(&report.input), triangles: Some(triangles), bones: Some(bones), blend_shapes: Some(morphs) });
    if vertices.is_empty() {
        report.degrade(Degrade::NothingToShow);
    }
    if textures.iter().any(|t| t[1].is_some()) {
        model::generate_tangents(&mut vertices);
    }
    let (mut min, mut max) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
    for v in &vertices {
        min = min.min(Vec3::from(v.pos));
        max = max.max(Vec3::from(v.pos));
    }
    if vertices.is_empty() {
        (min, max) = (Vec3::ZERO, Vec3::ZERO);
    }
    let mut scene = Scene { vertices, parts, materials, min, max, report };
    events(LoadEvent::Progress(0.7));
    events(LoadEvent::Untextured(&scene));

    // Textures: the encoded bytes of every image used, decoded on worker threads.
    let t = Instant::now();
    let images: Vec<gltf::Image<'_>> = doc.images().collect();
    let texture_image = |t: usize| doc.textures().nth(t).map(|t| t.source().index());
    let used: Vec<usize> = textures.iter().flatten().flatten().filter_map(|&t| texture_image(t)).collect::<std::collections::BTreeSet<_>>().into_iter().collect();
    let decoded: BTreeMap<usize, std::result::Result<texture::Decoded, String>> = std::thread::scope(|s| {
        let handles: Vec<_> = used
            .iter()
            .map(|&i| {
                let encoded = image_bytes(&images[i], base, &buffers);
                let max = opts.max_texture;
                let decode = opts.decode_textures;
                (i, s.spawn(move || {
                    let bytes = encoded?;
                    if decode {
                        texture::decode(&bytes, max)
                    } else {
                        let size = texture::dimensions(&bytes)?;
                        Ok(texture::Decoded { image: image::RgbaImage::new(0, 0), size, note: None })
                    }
                }))
            })
            .collect();
        handles.into_iter().map(|(i, h)| (i, h.join().unwrap_or_else(|_| Err("decoder panicked".into())))).collect()
    });
    let mips: BTreeMap<usize, Arc<[image::RgbaImage]>> =
        decoded.iter().filter(|_| opts.decode_textures).filter_map(|(i, d)| Some((*i, Arc::from(texture::mips(d.as_ref().ok()?.image.clone()))))).collect();
    for (i, d) in &decoded {
        if let Err(reason) = d {
            scene.report.degrade(Degrade::TextureUnreadable { path: format!("image {i}"), reason: reason.clone() });
        }
    }
    for (m, maps) in scene.materials.iter_mut().zip(&textures) {
        let get = |t: Option<usize>| t.and_then(texture_image).and_then(|i| mips.get(&i).cloned());
        (m.texture, m.normal, m.emission, m.matcap) = (get(maps[0]), get(maps[1]), get(maps[2]), get(maps[3]));
        if m.normal.is_none() {
            m.look.normal_scale = 0.0;
        }
    }
    for (r, maps) in scene.report.materials.iter_mut().zip(&textures) {
        r.texture_size = maps[0].and_then(texture_image).and_then(|i| decoded.get(&i)?.as_ref().ok()).map(|d| d.size);
    }
    timings.textures_ms = Timings::since(t);
    scene.report.timings = timings;
    events(LoadEvent::Progress(1.0));
    Ok(scene)
}

fn report_name(input: &str) -> String {
    input.rsplit(['/', '\\']).next().unwrap_or(input).to_string()
}

fn walk(node: &gltf::Node<'_>, parent: Mat4, out: &mut BTreeMap<usize, Mat4>) {
    let m = parent * Mat4::from_cols_array_2d(&node.transform().matrix());
    out.insert(node.index(), m);
    for child in node.children() {
        walk(&child, m, out);
    }
}

/// The JSON of a `.gltf` file or of a GLB's first chunk, for extensions the `gltf` crate drops.
fn raw_json(bytes: &[u8]) -> Option<Value> {
    if bytes.starts_with(b"glTF") {
        let len = u32::from_le_bytes(bytes.get(12..16)?.try_into().ok()?) as usize;
        serde_json::from_slice(bytes.get(20..20 + len)?).ok()
    } else {
        serde_json::from_slice(bytes).ok()
    }
}

/// The MToon properties UniVRM 0.x stores for the material named `name`.
fn vrm0_material<'a>(json: &'a Value, name: Option<&str>) -> Option<&'a Value> {
    json.pointer("/extensions/VRM/materialProperties")?.as_array()?.iter().find(|p| p["name"].as_str() == name)
}

/// The look of a glTF material, the glTF texture indices of its main, normal, emission and MatCap
/// textures, its shader family and a shader name for the report.
fn material_look(m: &gltf::Material<'_>, raw: Option<&Value>, vrm0: Option<&Value>) -> (Look, [Option<usize>; 4], ShaderFamily, &'static str) {
    let mut look = Look::white();
    let pbr = m.pbr_metallic_roughness();
    look.color = pbr.base_color_factor();
    look.mode = match m.alpha_mode() {
        gltf::material::AlphaMode::Opaque => RenderMode::Opaque,
        gltf::material::AlphaMode::Mask => RenderMode::Cutout,
        gltf::material::AlphaMode::Blend => RenderMode::Transparent,
    };
    look.cutoff = m.alpha_cutoff().unwrap_or(0.5);
    look.cull = if m.double_sided() { Cull::Off } else { Cull::Back };
    look.emission = m.emissive_factor();
    let mut maps = [pbr.base_color_texture().map(|t| t.texture().index()), None, m.emissive_texture().map(|t| t.texture().index()), None];
    if let Some(n) = m.normal_texture() {
        maps[1] = Some(n.texture().index());
        look.normal_scale = n.scale();
    }
    let f = |v: &Value| v.as_f64().map(|x| x as f32);
    let rgb = |v: &Value| Some([f(&v[0])?, f(&v[1])?, f(&v[2])?]);
    if let Some(mtoon) = raw.and_then(|r| r.pointer("/extensions/VRMC_materials_mtoon")) {
        if let Some(c) = rgb(&mtoon["shadeColorFactor"]) {
            look.shadow_color = c;
        }
        if mtoon["outlineWidthMode"].as_str() == Some("worldCoordinates")
            && let Some(w) = f(&mtoon["outlineWidthFactor"]).filter(|w| *w > 0.0)
        {
            let [r, g, b] = rgb(&mtoon["outlineColorFactor"]).unwrap_or([0.0; 3]);
            look.outline = Some(Outline { width: w, color: [r, g, b, 1.0] });
        }
        if let Some(t) = mtoon.pointer("/matcapTexture/index").and_then(Value::as_u64) {
            maps[3] = Some(t as usize);
            let [r, g, b] = rgb(&mtoon["matcapFactor"]).unwrap_or([1.0; 3]);
            look.matcap = Some(Matcap { color: [r, g, b, 1.0], blend: MatcapBlend::Add });
        }
        return (look, maps, ShaderFamily::MToon, "VRMC_materials_mtoon");
    }
    if let Some(p) = vrm0.filter(|p| p["shader"].as_str().is_some_and(|s| s.contains("MToon"))) {
        let float = |k: &str| f(&p["floatProperties"][k]);
        let vector = |k: &str| rgb(&p["vectorProperties"][k]);
        if let Some(c) = vector("_Color") {
            let a = f(&p["vectorProperties"]["_Color"][3]).unwrap_or(1.0);
            look.color = [c[0], c[1], c[2], a];
        }
        if let Some(c) = vector("_ShadeColor") {
            look.shadow_color = c;
        }
        look.mode = match float("_BlendMode").map(|b| b.round() as i32) {
            Some(1) => RenderMode::Cutout,
            Some(2 | 3) => RenderMode::Transparent,
            Some(_) => RenderMode::Opaque,
            None => look.mode,
        };
        if let Some(c) = float("_Cutoff") {
            look.cutoff = c;
        }
        look.cull = match float("_CullMode").map(|c| c.round() as i32) {
            Some(0) => Cull::Off,
            Some(1) => Cull::Front,
            Some(_) => Cull::Back,
            None => look.cull,
        };
        if float("_OutlineWidthMode").is_some_and(|m| m >= 0.5)
            && let Some(w) = float("_OutlineWidth").filter(|w| *w > 0.0)
        {
            // UniVRM 0.x stores the world-space outline width in centimeters.
            let [r, g, b] = vector("_OutlineColor").unwrap_or([0.0; 3]);
            look.outline = Some(Outline { width: w * 0.01, color: [r, g, b, 1.0] });
        }
        if let Some(t) = p["textureProperties"]["_SphereAdd"].as_u64() {
            maps[3] = Some(t as usize);
            look.matcap = Some(Matcap { color: [1.0; 4], blend: MatcapBlend::Add });
        }
        if let Some(t) = p["textureProperties"]["_MainTex"].as_u64() {
            maps[0] = Some(t as usize);
        }
        return (look, maps, ShaderFamily::MToon, "VRM/MToon");
    }
    (look, maps, ShaderFamily::Standard, "glTF metallic-roughness")
}

/// The encoded bytes of an image: inside a buffer view, or a file next to the glTF.
fn image_bytes(image: &gltf::Image<'_>, base: Option<&Path>, buffers: &[gltf::buffer::Data]) -> std::result::Result<Vec<u8>, String> {
    match image.source() {
        gltf::image::Source::View { view, .. } => {
            let data = buffers.get(view.buffer().index()).ok_or("missing buffer")?;
            data.0.get(view.offset()..view.offset() + view.length()).map(<[u8]>::to_vec).ok_or_else(|| "image outside its buffer".to_string())
        }
        gltf::image::Source::Uri { uri, .. } => {
            if uri.starts_with("data:") {
                return Err("images in data URIs are not read".into());
            }
            let base = base.ok_or("an external image needs the glTF's folder")?;
            let rel: PathBuf = uri.split('/').collect();
            if rel.components().any(|c| !matches!(c, std::path::Component::Normal(_))) {
                return Err(format!("image path {uri} leaves the glTF's folder"));
            }
            std::fs::read(base.join(rel)).map_err(|e| format!("{uri}: {e}"))
        }
    }
}
