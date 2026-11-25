//! Everything needed to draw a package, prepared on the CPU: geometry, bound materials,
//! decoded and mip-mapped textures, and the report. Loading never touches the GPU.
use crate::{
    Result,
    model::{Model, Vertex},
    texture,
};
use glam::Vec3;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
    time::Instant,
};
use unipeek_unity::{
    Degrade, fileid, Guid, IndexOptions, Input, Package, PackageIndex, Plan, Report, Timings,
    material::{Look, Material},
    meta::Meta,
};

#[derive(Clone, Debug)]
pub struct LoadOptions {
    pub index: IndexOptions,
    /// Longest side of a decoded texture; larger textures are scaled down before upload.
    pub max_texture: u32,
    /// Decode textures. Off for reports only: sizes come from the file headers and the scene's
    /// materials have no textures.
    pub decode_textures: bool,
}

impl Default for LoadOptions {
    fn default() -> Self {
        LoadOptions { index: IndexOptions::default(), max_texture: 2048, decode_textures: true }
    }
}

#[derive(Clone, Debug)]
pub struct SceneMaterial {
    pub name: String,
    pub look: Look,
    /// RGBA8 mip chain, largest first.
    pub texture: Option<Arc<[image::RgbaImage]>>,
}

#[derive(Clone, Copy, Debug)]
pub struct Part {
    pub material: usize,
    pub first: u32,
    pub count: u32,
}

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
        let mut desc = input.source.describe();
        for inner in &input.inner {
            desc.push('#');
            desc.push_str(inner);
        }
        let mut timings = Timings::default();
        let t = Instant::now();
        let (mut pkg, inner) = Package::open(input, &opts.index)?;
        timings.index_ms = Timings::since(t);
        let t = Instant::now();
        let plan = Plan::new(&pkg.index, inner.as_deref())?;
        let mut report = Report::new(desc, pkg.container, pkg.member.clone(), &pkg.index, &plan);
        timings.plan_ms = Timings::since(t);

        // Fetch models together with the textures their materials are likely to need, so a
        // unitypackage is usually read once more, not twice.
        let mut wanted: BTreeSet<Guid> = plan.models.iter().map(|m| m.guid).collect();
        wanted.extend(likely_textures(&pkg.index, &plan));
        let t = Instant::now();
        let mut bytes = pkg.read_assets(&wanted)?;
        timings.read_ms = Timings::since(t);

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
                        materials.push(SceneMaterial { name: binding.material_name.clone().unwrap_or_else(|| label.clone()), look: binding.look, texture: None });
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
        let missing: BTreeSet<Guid> = material_textures.iter().flatten().filter(|g| !bytes.contains_key(g)).copied().collect();
        if !missing.is_empty() {
            let t = Instant::now();
            bytes.extend(pkg.read_assets(&missing)?);
            timings.read_ms += Timings::since(t);
        }
        let unique: BTreeSet<Guid> = material_textures.iter().flatten().copied().collect();
        let t = Instant::now();
        let decoded = decode_all(&unique, &bytes, opts.max_texture, opts.decode_textures);
        timings.textures_ms = Timings::since(t);
        for (guid, result) in &decoded {
            let path = || pkg.index.get(guid).and_then(|e| e.pathname.clone()).unwrap_or_else(|| guid.to_string());
            match result {
                Err(reason) => report.degrade(Degrade::TextureUnreadable { path: path(), reason: reason.clone() }),
                Ok(t) if t.note.is_some() => report.degrade(Degrade::TextureApproximated { path: path(), reason: t.note.clone().unwrap_or_default() }),
                Ok(_) => {}
            }
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

        let (mut min, mut max) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
        for (_, m) in &models {
            min = min.min(m.min);
            max = max.max(m.max);
        }
        if models.is_empty() {
            (min, max) = (Vec3::ZERO, Vec3::ZERO);
        }
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

/// Main textures of the materials the models' `.meta` files and the prefab point at.
fn likely_textures(index: &PackageIndex, plan: &Plan) -> Vec<Guid> {
    let metas = plan.models.iter().filter_map(|m| index.get(&m.guid)?.meta.as_deref()).map(Meta::parse);
    let mats = metas.flat_map(|m| m.external_materials.into_iter().filter_map(|(_, r)| r.guid)).chain(plan.prefab_materials.iter().copied());
    mats.filter_map(|g| index.get(&g)?.text.as_deref().and_then(Material::parse))
        .filter_map(|m| m.main_texture()?.texture.guid)
        .filter(|g| index.get(g).is_some_and(|e| e.size.is_some()))
        .collect()
}
