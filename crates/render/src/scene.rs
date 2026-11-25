//! Everything needed to draw a package, prepared on the CPU: geometry, bound materials,
//! decoded and mip-mapped textures, and the report. Loading never touches the GPU.
use crate::{
    Result,
    model::{self, Model, Vertex},
    texture,
};
use glam::Vec3;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
    time::Instant,
};
use unipeek_unity::{
    Degrade, Resolver, fileid, Guid, IndexOptions, Input, Package, PackageIndex, Plan, Report, Timings,
    material::{Look, Material},
    meta::Meta,
};

#[derive(Clone)]
pub struct LoadOptions {
    pub index: IndexOptions,
    /// Longest side of a decoded texture; larger textures are scaled down before upload.
    pub max_texture: u32,
    /// Decode textures. Off for reports only: sizes come from the file headers and the scene's
    /// materials have no textures.
    pub decode_textures: bool,
    /// Finds GUIDs the package needs but does not contain (base avatar, materials, textures in
    /// other packages). Without one, they are reported as missing.
    pub resolver: Option<Arc<dyn Resolver>>,
}

impl Default for LoadOptions {
    fn default() -> Self {
        LoadOptions { index: IndexOptions::default(), max_texture: 2048, decode_textures: true, resolver: None }
    }
}

impl std::fmt::Debug for LoadOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LoadOptions")
            .field("index", &self.index)
            .field("max_texture", &self.max_texture)
            .field("decode_textures", &self.decode_textures)
            .field("resolver", &self.resolver.as_ref().map(|_| "..."))
            .finish()
    }
}

#[derive(Clone, Debug)]
pub struct SceneMaterial {
    pub name: String,
    pub look: Look,
    /// RGBA8 mip chain, largest first.
    pub texture: Option<Arc<[image::RgbaImage]>>,
    /// Tangent-space normal map (linear, not sRGB), when the look uses one.
    pub normal: Option<Arc<[image::RgbaImage]>>,
    pub emission: Option<Arc<[image::RgbaImage]>>,
    pub matcap: Option<Arc<[image::RgbaImage]>>,
}

#[derive(Clone, Copy, Debug)]
pub struct Part {
    pub material: usize,
    pub first: u32,
    pub count: u32,
}

/// What `Scene::load_with` tells its caller while it works.
pub enum LoadEvent<'a> {
    /// Rough fraction of the work done, from 0 to 1.
    Progress(f32),
    /// Geometry and materials are ready; textures are not decoded yet. A viewer can show this
    /// white model while the textures load.
    Untextured(&'a Scene),
}

#[derive(Clone)]
pub struct Scene {
    pub vertices: Vec<Vertex>,
    pub parts: Vec<Part>,
    pub materials: Vec<SceneMaterial>,
    pub min: Vec3,
    pub max: Vec3,
    pub report: Report,
}

impl Scene {
    pub fn load(input: Input, opts: &LoadOptions) -> Result<Scene> {
        Scene::load_with(input, opts, &mut |_| {})
    }

    /// Like `load`, reporting progress and handing out the untextured scene before textures are decoded.
    pub fn load_with(input: Input, opts: &LoadOptions, events: &mut dyn FnMut(LoadEvent<'_>)) -> Result<Scene> {
        let mut desc = input.source.describe();
        for inner in &input.inner {
            desc.push('#');
            desc.push_str(inner);
        }
        if input.inner.is_empty() {
            match &input.source {
                unipeek_unity::Source::Path(p) if p.is_file() && crate::gltf_scene::is_gltf(&[], p.file_name().and_then(|n| n.to_str())) => {
                    let bytes = std::fs::read(p)?;
                    return crate::gltf_scene::load(&bytes, p.parent(), desc, opts, events);
                }
                unipeek_unity::Source::Bytes(b) if crate::gltf_scene::is_gltf(&b[..b.len().min(4)], None) => {
                    return crate::gltf_scene::load(b, None, desc, opts, events);
                }
                _ => {}
            }
        }
        let mut timings = Timings::default();
        let t = Instant::now();
        let (mut pkg, inner) = Package::open(input, &opts.index)?;
        timings.index_ms = Timings::since(t);
        events(LoadEvent::Progress(0.3));
        let t = Instant::now();
        let mut plan = Plan::new(&pkg.index, inner.as_deref())?;
        if let Some(resolver) = &opts.resolver {
            // Each round can bring in prefabs that need more (a base avatar's own materials are in
            // its package, so one or two rounds are usual).
            // The choice of prefab stays with the package itself, not with what dependencies add.
            let chosen = inner.clone().or_else(|| plan.prefab.as_ref().map(|c| c.path.clone()));
            let candidates = plan.candidates.clone();
            let mut asked = BTreeSet::new();
            for _ in 0..4 {
                let need: Vec<Guid> = needed_elsewhere(&pkg.index, &plan).into_iter().filter(|g| asked.insert(*g)).collect();
                if need.is_empty() || pkg.resolve(resolver.as_ref(), &need, &opts.index)?.is_empty() {
                    break;
                }
                if chosen.is_none() {
                    // Nothing to show on its own (an outfit without a model): keep the first plan.
                    break;
                }
                plan = Plan::new(&pkg.index, chosen.as_deref())?;
                plan.candidates = candidates.clone();
            }
        }
        let mut report = Report::new(desc, pkg.container, pkg.member.clone(), &pkg.index, &plan);
        report.dependencies = pkg.dependencies().map(str::to_string).collect();
        timings.plan_ms = Timings::since(t);

        // Fetch models together with the textures their materials are likely to need, so a
        // unitypackage is usually read once more, not twice.
        let mut wanted: BTreeSet<Guid> = plan.models.iter().map(|m| m.guid).collect();
        wanted.extend(likely_textures(&pkg.index, &plan));
        let t = Instant::now();
        let mut bytes = pkg.read_assets(&wanted)?;
        timings.read_ms = Timings::since(t);
        events(LoadEvent::Progress(0.5));

        let t = Instant::now();
        let mut models = Vec::new();
        for (i, planned) in plan.models.iter().enumerate() {
            let parsed = match bytes.get(&planned.guid) {
                Some(b) => Model::from_fbx_bytes(b),
                None => Err("the model's bytes are not in the input".to_string()),
            };
            match parsed {
                Ok(m) if !m.vertices.is_empty() => {
                    let r = &mut report.models[i];
                    r.triangles = Some(m.triangles() as u64);
                    r.bones = Some(m.bones as u64);
                    r.blend_shapes = Some(m.blend_shapes as u64);
                    models.push((planned, m));
                }
                Ok(_) => report.degrade(Degrade::ModelUnreadable { path: planned.path.clone(), reason: "no mesh data".into() }),
                Err(reason) => report.degrade(Degrade::ModelUnreadable { path: planned.path.clone(), reason }),
            }
        }

        timings.models_ms = Timings::since(t);

        let t = Instant::now();
        let mut materials: Vec<SceneMaterial> = Vec::new();
        let mut material_textures: Vec<Option<Guid>> = Vec::new();
        let mut material_maps: Vec<unipeek_unity::material::Maps> = Vec::new();
        let mut material_slots: Vec<(String, String)> = Vec::new();
        let mut slots: BTreeMap<(Guid, String, usize, String), usize> = BTreeMap::new();
        let mut vertices = Vec::new();
        let mut parts = Vec::new();
        let mut applied: BTreeSet<(Guid, i64)> = BTreeSet::new();
        for (planned, model) in &models {
            let offset = vertices.len() as u32;
            vertices.extend_from_slice(&model.vertices);
            for part in &model.parts {
                let renderers = [fileid::component(&part.path, "SkinnedMeshRenderer"), fileid::component(&part.path, "MeshRenderer")];
                if let Some(o) = plan.renderer_override(planned.guid, &renderers) {
                    applied.insert((o.model, o.target));
                }
                // The last submesh is drawn once more for each material a prefab adds after it.
                let mut extra = Vec::new();
                if part.slot + 1 == part.submeshes {
                    extra = plan.extra_materials(planned.guid, &renderers, part.submeshes);
                }
                let mut bind = |slot: usize, label: String, binding: &dyn Fn() -> unipeek_unity::Binding| {
                    let key = (planned.guid, part.path.clone(), slot, label.clone());
                    *slots.entry(key).or_insert_with(|| {
                        let binding = binding();
                        report.add_binding(&planned.path, &label, &binding);
                        materials.push(SceneMaterial {
                            name: binding.material_name.clone().unwrap_or_else(|| label.clone()),
                            look: binding.look,
                            texture: None,
                            normal: None,
                            emission: None,
                            matcap: None,
                        });
                        material_maps.push(binding.maps);
                        material_textures.push(binding.main_texture.filter(|t| t.in_package).map(|t| t.guid));
                        material_slots.push((planned.path.clone(), label.clone()));
                        materials.len() - 1
                    })
                };
                let material = bind(part.slot, part.material.clone(), &|| plan.bind_renderer(&pkg.index, planned.guid, &part.mesh, &renderers, part.slot, &part.material));
                parts.push(Part { material, first: part.first + offset, count: part.count });
                for (slot, mat) in extra {
                    let label = format!("{}[{slot}]", part.mesh);
                    let material = bind(slot, label, &|| plan.bind_extra(&pkg.index, mat));
                    parts.push(Part { material, first: part.first + offset, count: part.count });
                }
            }
        }
        // Prefabs keep modifications of objects that were later removed from the FBX, and Unity
        // ignores them. When other overrides of the same model matched, an unmatched one is such a
        // leftover; when none matched, the node names did not line up and that is a degrade.
        let mut unmatched = plan.material_overrides;
        for o in plan.renderer_overrides.iter().filter(|o| models.iter().any(|(m, _)| m.guid == o.model) && !applied.contains(&(o.model, o.target))) {
            if applied.iter().any(|(m, _)| *m == o.model) {
                report.stale_overrides += 1;
            } else {
                unmatched += 1;
            }
        }
        if unmatched > 0 {
            report.degrade(Degrade::MaterialOverridesIgnored { count: unmatched });
        }

        timings.bind_ms = Timings::since(t);
        let (mut min, mut max) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
        for (_, m) in &models {
            min = min.min(m.min);
            max = max.max(m.max);
        }
        if models.is_empty() {
            (min, max) = (Vec3::ZERO, Vec3::ZERO);
        }
        if material_maps.iter().any(|m| m.normal.is_some()) {
            model::generate_tangents(&mut vertices);
        }
        let scene = Scene { vertices, parts, materials, min, max, report };
        events(LoadEvent::Progress(0.7));
        events(LoadEvent::Untextured(&scene));
        let Scene { parts, vertices, mut materials, mut report, .. } = scene;
        let map_guids = material_maps.iter().flat_map(|m| [m.normal, m.emission, m.matcap]).flatten();
        let all: BTreeSet<Guid> = material_textures.iter().flatten().copied().chain(map_guids).collect();
        let missing: BTreeSet<Guid> = all.iter().filter(|g| !bytes.contains_key(g)).copied().collect();
        if !missing.is_empty() {
            let t = Instant::now();
            bytes.extend(pkg.read_assets(&missing)?);
            timings.read_ms += Timings::since(t);
        }
        let t = Instant::now();
        let decoded = decode_all(&all, &bytes, opts.max_texture, opts.decode_textures);
        timings.textures_ms = Timings::since(t);
        for (guid, result) in &decoded {
            let path = || pkg.index.get(guid).and_then(|e| e.pathname.clone()).unwrap_or_else(|| guid.to_string());
            match result {
                Err(reason) => report.degrade(Degrade::TextureUnreadable { path: path(), reason: reason.clone() }),
                Ok(t) if t.note.is_some() => report.degrade(Degrade::TextureApproximated { path: path(), reason: t.note.clone().unwrap_or_default() }),
                Ok(_) => {}
            }
        }
        let mips = |g: Option<Guid>| g.and_then(|g| decoded.get(&g)).and_then(|r| r.as_ref().ok()).and_then(|t| t.mips.clone());
        for (m, maps) in materials.iter_mut().zip(&material_maps) {
            (m.normal, m.emission, m.matcap) = (mips(maps.normal), mips(maps.emission), mips(maps.matcap));
        }
        for (i, tex) in material_textures.iter().enumerate() {
            let Some(Ok(t)) = tex.and_then(|g| decoded.get(&g)) else { continue };
            let size = &t.size;
            materials[i].texture = t.mips.clone();
            let (model, slot) = &material_slots[i];
            if let Some(r) = report.materials.iter_mut().find(|r| &r.model == model && &r.slot == slot) {
                r.texture_size = Some(*size);
            }
        }
        report.read = pkg.index.stats;
        report.timings = timings;
        events(LoadEvent::Progress(1.0));
        Ok(Scene { vertices, parts, materials, min, max, report })
    }

    pub fn triangles(&self) -> usize {
        self.vertices.len() / 3
    }

    pub fn is_empty(&self) -> bool {
        self.vertices.is_empty()
    }
}

struct Texture {
    /// `None` when only the size was read.
    mips: Option<Arc<[image::RgbaImage]>>,
    size: [u32; 2],
    note: Option<String>,
}

type Decoded = std::result::Result<Texture, String>;

/// Decode every texture on its own thread; 4K PNGs take most of the load time.
fn decode_all(guids: &BTreeSet<Guid>, bytes: &BTreeMap<Guid, Arc<[u8]>>, max: u32, decode: bool) -> BTreeMap<Guid, Decoded> {
    std::thread::scope(|s| {
        let handles: Vec<_> = guids
            .iter()
            .map(|g| {
                let data = bytes.get(g).cloned();
                (*g, s.spawn(move || -> Decoded {
                    let data = data.ok_or("the texture's bytes are not in the input")?;
                    if !decode {
                        return Ok(Texture { mips: None, size: texture::dimensions(&data)?, note: None });
                    }
                    let d = texture::decode(&data, max)?;
                    Ok(Texture { mips: Some(Arc::from(texture::mips(d.image))), size: d.size, note: d.note })
                }))
            })
            .collect();
        handles.into_iter().map(|(g, h)| (g, h.join().unwrap_or_else(|_| Err("decoder panicked".into())))).collect()
    })
}

/// GUIDs the plan uses that the package does not have: prefabs, meshes and materials the prefab
/// points at, and main textures of the materials it uses.
fn needed_elsewhere(index: &PackageIndex, plan: &Plan) -> Vec<Guid> {
    use unipeek_unity::prefab::Usage;
    let absent = |g: &Guid| index.get(g).is_none_or(|e| e.size.is_none());
    let mut out: BTreeSet<Guid> = plan.missing.iter().filter(|m| matches!(m.usage, Usage::Prefab | Usage::Mesh | Usage::Material | Usage::Texture)).map(|m| m.guid).collect();
    let metas = plan.models.iter().filter_map(|m| index.get(&m.guid)?.meta.as_deref()).map(Meta::parse);
    let overrides = plan.renderer_overrides.iter().flat_map(|o| o.materials.values().flatten().copied());
    let mats = metas.flat_map(|m| m.external_materials.into_iter().filter_map(|(_, r)| r.guid)).chain(plan.prefab_materials.iter().copied()).chain(overrides);
    for m in mats {
        match index.get(&m).and_then(|e| e.text.as_deref()).and_then(Material::parse) {
            Some(mat) => out.extend(mat.main_texture().and_then(|t| t.texture.guid).filter(|g| absent(g))),
            None if absent(&m) => {
                out.insert(m);
            }
            None => {}
        }
    }
    out.into_iter().collect()
}

/// Main textures of the materials the models' `.meta` files and the prefab point at.
fn likely_textures(index: &PackageIndex, plan: &Plan) -> Vec<Guid> {
    let metas = plan.models.iter().filter_map(|m| index.get(&m.guid)?.meta.as_deref()).map(Meta::parse);
    let mats = metas.flat_map(|m| m.external_materials.into_iter().filter_map(|(_, r)| r.guid)).chain(plan.prefab_materials.iter().copied());
    mats.filter_map(|g| index.get(&g)?.text.as_deref().and_then(Material::parse))
        .filter_map(|m| m.main_texture()?.texture.guid)
        .filter(|g| index.get(g).is_some_and(|e| e.size.is_some()))
        .collect()
}
