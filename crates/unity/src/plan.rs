//! Decide what to show from a package: the main prefab, its models, and the material of every
//! FBX material slot. Everything here is GPU-free; the renderer only follows the plan.
use crate::{
    AssetKind, Entry, Error, Guid, PackageIndex, Result,
    material::{Look, Material},
    meta::Meta,
    prefab::{Prefab, Usage},
    shader::{self, ShaderInfo},
    yaml::ObjRef,
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct Candidate {
    pub guid: Guid,
    pub path: String,
    /// Models in this package the prefab uses, directly or through nested prefabs.
    pub models: usize,
    pub variant: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct PlannedModel {
    pub guid: Guid,
    pub path: String,
    pub size: Option<u64>,
}

/// Materials a prefab renderer assigns to the mesh on one FBX node, by submesh slot.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct RendererMaterials {
    pub model: Guid,
    /// Name of the renderer's game object, which Unity names after the FBX node.
    pub object: String,
    /// Local file ID of the mesh in the model (`fileid::mesh` of the FBX node's name).
    pub mesh: i64,
    pub materials: Vec<Option<Guid>>,
}

/// `m_Materials` modifications a prefab makes to one renderer inside an instance of a model.
/// `target` is the renderer's local file ID in the model (see `fileid`).
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct RendererOverride {
    pub model: Guid,
    pub target: i64,
    /// Slot index → material (`None` clears the slot). The outermost prefab wins.
    pub materials: BTreeMap<usize, Option<Guid>>,
    /// `m_Materials.Array.size`, when the prefab changes how many materials the renderer has.
    pub size: Option<usize>,
}

/// A GUID the plan needs that is not in the package.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize)]
pub struct MissingRef {
    pub guid: Guid,
    pub usage: Usage,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize)]
pub struct Plan {
    /// The prefab shown; `None` when the package has no usable prefab and a model is shown alone.
    pub prefab: Option<Candidate>,
    /// Prefabs that use a model, best first.
    pub candidates: Vec<Candidate>,
    pub models: Vec<PlannedModel>,
    /// Materials the prefab assigns (renderers, overrides), used to bind FBX slots by name.
    pub prefab_materials: Vec<Guid>,
    /// Renderers in the prefab that draw meshes of the models.
    pub renderer_materials: Vec<RendererMaterials>,
    pub missing: Vec<MissingRef>,
    /// `m_Materials` overrides on instances of models, by target renderer.
    pub renderer_overrides: Vec<RendererOverride>,
    /// `m_Materials` overrides whose target is not a renderer of a model in the package.
    pub material_overrides: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BindSource {
    /// The FBX `.meta` remaps the slot (`externalObjects`).
    ExternalObject,
    /// The prefab's renderer for this mesh assigns this slot.
    PrefabRenderer,
    /// A prefab overrides the slot on its instance of the model (`m_Materials` modification).
    PrefabOverride,
    /// A material the prefab assigns has the slot's name.
    PrefabMaterial,
    /// A material in the package has the slot's name.
    NameMatch,
    /// No material; a texture whose name ends with the slot's name.
    TextureName,
    Unbound,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct TextureRef {
    pub guid: Guid,
    pub path: Option<String>,
    pub in_package: bool,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct Binding {
    pub source: BindSource,
    pub material: Option<Guid>,
    pub material_name: Option<String>,
    /// `pathname` of the bound `.mat`.
    pub material_path: Option<String>,
    pub shader: Option<ShaderInfo>,
    pub look: Look,
    pub main_texture: Option<TextureRef>,
    /// Why the bound `.mat` could not be read (binary serialization, too large), if so.
    pub unreadable: Option<String>,
}

impl Plan {
    /// Plan the package. `inner` selects a prefab or model by its path inside the package.
    pub fn new(index: &PackageIndex, inner: Option<&str>) -> Result<Plan> {
        let mut walker = Walker { index, prefabs: BTreeMap::new() };
        let largest = index.entries().filter(|e| is_model(e)).max_by_key(|e| e.size.unwrap_or(0)).map(|e| e.guid);
        let mut candidates: Vec<(Candidate, (bool, bool, usize, bool, usize, String))> = Vec::new();
        for e in index.entries().filter(|e| is_prefab(e)) {
            let walk = walker.walk(e.guid);
            if walk.models.is_empty() {
                continue;
            }
            let path = e.pathname.clone().unwrap_or_default();
            let stem = e.stem().unwrap_or_default().to_lowercase();
            let matches_model = walk.models.iter().any(|m| index.get(m).and_then(Entry::stem).is_some_and(|s| s.to_lowercase() == stem));
            let variant = walker.variant_of_prefab(e.guid);
            // The package's biggest model is usually the product; a name matching a small extra
            // model (a pet, a prop) must not beat it.
            let has_largest = largest.is_some_and(|g| walk.models.contains(&g));
            let key = (variant, !has_largest, path.matches('/').count(), !matches_model, stem.chars().count(), path.to_lowercase());
            candidates.push((Candidate { guid: e.guid, path, models: walk.models.len(), variant }, key));
        }
        candidates.sort_by(|a, b| a.1.cmp(&b.1));
        let candidates: Vec<Candidate> = candidates.into_iter().map(|(c, _)| c).collect();

        let mut plan = Plan { candidates, ..Plan::default() };
        let chosen = match inner {
            Some(path) => {
                let e = index.by_path(path).ok_or_else(|| Error::NotFound(format!("{path} is not in the package")))?;
                if is_prefab(e) {
                    Some(e.guid)
                } else if is_model(e) {
                    plan.models.push(planned(e));
                    None
                } else {
                    return Err(Error::Unsupported(format!("{path} is neither a prefab nor a model")));
                }
            }
            None => plan.candidates.first().map(|c| c.guid),
        };
        if let Some(guid) = chosen {
            let walk = walker.walk(guid);
            plan.prefab = Some(match plan.candidates.iter().find(|c| c.guid == guid) {
                Some(c) => c.clone(),
                None => Candidate {
                    guid,
                    path: index.get(&guid).and_then(|e| e.pathname.clone()).unwrap_or_default(),
                    models: walk.models.len(),
                    variant: walker.variant_of_prefab(guid),
                },
            });
            plan.models = walk.models.iter().filter_map(|g| index.get(g)).map(planned).collect();
            plan.prefab_materials = walk.materials;
            plan.renderer_materials = walk.renderers;
            plan.missing = walk.missing.into_iter().collect();
            plan.material_overrides = walk.overrides;
            plan.renderer_overrides = walk.renderer_overrides;
        }
        if plan.models.is_empty() && inner.is_none() {
            let largest = index.entries().filter(|e| is_model(e)).max_by_key(|e| (e.size.unwrap_or(0), std::cmp::Reverse(e.pathname.clone())));
            plan.models.extend(largest.map(planned));
        }
        Ok(plan)
    }

    /// The override of the renderer with one of the file IDs `renderers` in `model`, if the prefab has one.
    pub fn renderer_override(&self, model: Guid, renderers: &[i64]) -> Option<&RendererOverride> {
        self.renderer_overrides.iter().find(|o| o.model == model && renderers.contains(&o.target))
    }

    /// Like `bind_slot`, but first applies the prefab's `m_Materials` override of the renderer on
    /// this node; `renderers` are the node's possible renderer file IDs (`fileid::component` for
    /// `SkinnedMeshRenderer` and `MeshRenderer`).
    pub fn bind_renderer(&self, index: &PackageIndex, model: Guid, object: &str, renderers: &[i64], slot_index: usize, slot: &str) -> Binding {
        if let Some(o) = self.renderer_override(model, renderers)
            && let Some(mat) = o.materials.get(&slot_index)
        {
            return match mat {
                Some(mat) => bind_material(index, *mat, BindSource::PrefabOverride),
                None => Binding { source: BindSource::Unbound, material: None, material_name: None, material_path: None, shader: None, look: Look::white(), main_texture: None, unreadable: None },
            };
        }
        self.bind_slot(index, model, object, slot_index, slot)
    }

    /// Materials the prefab adds after the last submesh (`m_Materials.Array.size` larger than the
    /// mesh's submesh count). Unity draws the last submesh again with each of them.
    pub fn extra_materials(&self, model: Guid, renderers: &[i64], submeshes: usize) -> Vec<(usize, Guid)> {
        let Some(o) = self.renderer_override(model, renderers) else { return Vec::new() };
        let size = o.size.unwrap_or(0);
        o.materials.iter().filter(|(slot, _)| **slot >= submeshes && **slot < size).filter_map(|(slot, m)| Some((*slot, (*m)?))).collect()
    }

    /// Bind an added material (see `extra_materials`).
    pub fn bind_extra(&self, index: &PackageIndex, material: Guid) -> Binding {
        bind_material(index, material, BindSource::PrefabOverride)
    }

    /// Like `bind`, but first uses the material a prefab renderer assigns to slot `slot_index` of
    /// the mesh on the FBX node `object`.
    pub fn bind_slot(&self, index: &PackageIndex, model: Guid, object: &str, slot_index: usize, slot: &str) -> Binding {
        let mine: Vec<&RendererMaterials> = self.renderer_materials.iter().filter(|r| r.model == model).collect();
        // The mesh's file ID names the FBX node exactly; the game object may have been renamed.
        let mesh = crate::fileid::mesh(object);
        let renderer = mine
            .iter()
            .find(|r| r.mesh == mesh)
            .or_else(|| mine.iter().find(|r| r.object.eq_ignore_ascii_case(object)))
            .or(if mine.len() == 1 { mine.first() } else { None });
        if let Some(Some(mat)) = renderer.and_then(|r| r.materials.get(slot_index)) {
            return bind_material(index, *mat, BindSource::PrefabRenderer);
        }
        self.bind(index, model, slot)
    }

    /// Material and texture for the FBX material slot `slot` of `model`.
    pub fn bind(&self, index: &PackageIndex, model: Guid, slot: &str) -> Binding {
        let model_entry = index.get(&model);
        if let Some(mat) = model_entry.and_then(|e| e.meta.as_deref()).map(Meta::parse).and_then(|m| m.material_for(slot)) {
            return bind_material(index, mat, BindSource::ExternalObject);
        }
        if slot.trim().is_empty() {
            return Binding { source: BindSource::Unbound, material: None, material_name: None, material_path: None, shader: None, look: Look::white(), main_texture: None, unreadable: None };
        }
        let named = |e: &Entry| -> bool {
            let name = e.text.as_deref().and_then(Material::parse).map(|m| m.name);
            name.is_some_and(|n| n.eq_ignore_ascii_case(slot)) || e.stem().is_some_and(|s| s.eq_ignore_ascii_case(slot))
        };
        if let Some(e) = self.prefab_materials.iter().filter_map(|g| index.get(g)).find(|e| named(e)) {
            return bind_material(index, e.guid, BindSource::PrefabMaterial);
        }
        let model_path = model_entry.and_then(|e| e.pathname.as_deref()).unwrap_or_default();
        let closest = |e: &&Entry| (common_prefix(model_path, e.pathname.as_deref().unwrap_or_default()), std::cmp::Reverse(e.pathname.clone()));
        if let Some(e) = index.entries().filter(|e| e.extension().as_deref() == Some("mat") && named(e)).max_by_key(closest) {
            return bind_material(index, e.guid, BindSource::NameMatch);
        }
        let slot_lower = slot.to_lowercase();
        let texture = index
            .entries()
            .filter(|e| matches!(e.kind, AssetKind::Png | AssetKind::Jpeg | AssetKind::Psd))
            .filter(|e| e.stem().is_some_and(|s| s.to_lowercase().ends_with(&slot_lower)))
            .filter(|e| !is_normal_map(e))
            .max_by_key(closest);
        let mut binding = Binding { source: BindSource::Unbound, material: None, material_name: None, material_path: None, shader: None, look: Look::white(), main_texture: None, unreadable: None };
        if let Some(t) = texture {
            binding.source = BindSource::TextureName;
            binding.main_texture = Some(TextureRef { guid: t.guid, path: t.pathname.clone(), in_package: true });
        }
        binding
    }
}

fn planned(e: &Entry) -> PlannedModel {
    PlannedModel { guid: e.guid, path: e.pathname.clone().unwrap_or_default(), size: e.size }
}

fn is_prefab(e: &Entry) -> bool {
    e.extension().as_deref() == Some("prefab") && e.text.is_some()
}

fn is_model(e: &Entry) -> bool {
    e.kind == AssetKind::Fbx || e.extension().as_deref() == Some("fbx")
}

/// Normal maps are never main textures: `textureType: 1` in the `.meta`, or "normal" in the name.
fn is_normal_map(e: &Entry) -> bool {
    e.meta.as_deref().is_some_and(|m| Meta::parse(m).normal_map) || e.stem().is_some_and(|s| s.to_lowercase().contains("normal"))
}

/// Unity's built-in resources (`unity default resources`, `unity_builtin_extra`) are always there.
fn is_builtin(g: &Guid) -> bool {
    let s = g.to_string();
    s == "0000000000000000e000000000000000" || s == crate::shader::BUILTIN_GUID
}

fn common_prefix(a: &str, b: &str) -> usize {
    a.split('/').zip(b.split('/')).take_while(|(x, y)| x.eq_ignore_ascii_case(y)).count()
}

fn bind_material(index: &PackageIndex, mat: Guid, source: BindSource) -> Binding {
    let entry = index.get(&mat);
    let mut b = Binding {
        source,
        material: Some(mat),
        material_name: None,
        material_path: entry.and_then(|e| e.pathname.clone()),
        shader: None,
        look: Look::white(),
        main_texture: None,
        unreadable: None,
    };
    let Some(entry) = entry.filter(|e| e.size.is_some()) else { return b };
    let Some(m) = entry.text.as_deref().and_then(Material::parse) else {
        b.unreadable = Some(match entry.kind {
            AssetKind::Yaml if entry.text.is_none() => "too large to read".into(),
            AssetKind::Yaml => "no Material document".into(),
            _ => "binary serialization; only text YAML is read".into(),
        });
        return b;
    };
    let shader_ref = m.shader.unwrap_or(ObjRef { file_id: 0, guid: None, kind: None });
    let name_in_package = shader_ref.guid.and_then(|g| index.get(&g)).and_then(|e| e.shader_name.as_deref());
    let shader = shader::identify(shader_ref, name_in_package);
    b.look = Look::of(&m, &shader);
    b.shader = Some(shader);
    b.main_texture = m.main_texture().and_then(|t| t.texture.guid).map(|guid| {
        let e = index.get(&guid).filter(|e| e.size.is_some());
        TextureRef { guid, path: e.and_then(|e| e.pathname.clone()), in_package: e.is_some() }
    });
    b.material_name = Some(m.name);
    b
}

#[derive(Clone, Default)]
struct Walk {
    models: Vec<Guid>,
    materials: Vec<Guid>,
    renderers: Vec<RendererMaterials>,
    missing: BTreeSet<MissingRef>,
    overrides: usize,
    renderer_overrides: Vec<RendererOverride>,
}

struct Walker<'a> {
    index: &'a PackageIndex,
    prefabs: BTreeMap<Guid, Prefab>,
}

impl Walker<'_> {
    fn prefab(&mut self, guid: Guid) -> Option<&Prefab> {
        if !self.prefabs.contains_key(&guid) {
            let text = self.index.get(&guid).filter(|e| is_prefab(e))?.text.clone()?;
            self.prefabs.insert(guid, Prefab::parse(&text));
        }
        self.prefabs.get(&guid)
    }

    /// The root instance of this prefab is another prefab of the package (not a model).
    fn variant_of_prefab(&mut self, guid: Guid) -> bool {
        let index = self.index;
        self.prefab(guid).is_some_and(|p| {
            p.instances.iter().any(|i| i.parent == 0 && i.source.guid.and_then(|g| index.get(&g)).is_some_and(is_prefab))
        })
    }

    fn walk(&mut self, root: Guid) -> Walk {
        let mut walk = Walk::default();
        let mut visited = BTreeSet::new();
        self.visit(root, 0, &mut visited, &mut walk);
        walk
    }

    fn visit(&mut self, guid: Guid, depth: usize, visited: &mut BTreeSet<Guid>, walk: &mut Walk) {
        // Unity itself stops prefab variant chains at 32 levels; popone does the same.
        if depth > 32 || !visited.insert(guid) {
            return;
        }
        let index = self.index;
        let Some(prefab) = self.prefab(guid).cloned() else { return };
        for inst in &prefab.instances {
            for m in inst.modifications.iter().filter(|m| m.property.starts_with("m_Materials.Array.")) {
                let model = m.target.guid.filter(|g| index.get(g).is_some_and(is_model));
                let Some(model) = model else {
                    if m.property.starts_with("m_Materials.Array.data[") {
                        walk.overrides += 1;
                    }
                    continue;
                };
                let pos = walk.renderer_overrides.iter().position(|o| o.model == model && o.target == m.target.file_id).unwrap_or_else(|| {
                    walk.renderer_overrides.push(RendererOverride { model, target: m.target.file_id, materials: BTreeMap::new(), size: None });
                    walk.renderer_overrides.len() - 1
                });
                let o = &mut walk.renderer_overrides[pos];
                if m.property == "m_Materials.Array.size" {
                    // Outer prefabs are visited first and win.
                    if o.size.is_none() {
                        o.size = m.value.as_deref().and_then(|v| v.trim().parse().ok());
                    }
                } else if let Some(slot) = m.property.strip_prefix("m_Materials.Array.data[").and_then(|r| r.strip_suffix(']')).and_then(|s| s.parse().ok()) {
                    o.materials.entry(slot).or_insert(m.reference.and_then(|r| r.guid).filter(|g| !g.is_zero()));
                }
            }
        }
        for m in prefab.material_refs() {
            if !walk.materials.contains(&m) {
                walk.materials.push(m);
            }
        }
        for (g, usage) in &prefab.references {
            if index.get(g).is_none_or(|e| e.size.is_none()) && *usage != Usage::Other && !is_builtin(g) {
                walk.missing.insert(MissingRef { guid: *g, usage: *usage });
            }
        }
        for r in &prefab.renderers {
            let Some(model) = r.mesh.and_then(|m| m.guid).filter(|g| index.get(g).is_some_and(is_model)) else { continue };
            let object = prefab.objects.iter().find(|o| o.file_id == r.game_object).map(|o| o.name.clone()).unwrap_or_default();
            let mesh = r.mesh.map_or(0, |m| m.file_id);
            walk.renderers.push(RendererMaterials { model, object, mesh, materials: r.materials.iter().map(|m| m.guid.filter(|g| !g.is_zero())).collect() });
        }
        let sources = prefab.instances.iter().filter_map(|i| i.source.guid);
        let meshes = prefab.renderers.iter().filter_map(|r| r.mesh).chain(prefab.mesh_filters.iter().map(|(_, m)| *m)).filter_map(|m| m.guid);
        let mut children = Vec::new();
        for g in sources.chain(meshes) {
            match index.get(&g) {
                Some(e) if is_model(e) => {
                    if !walk.models.contains(&g) {
                        walk.models.push(g);
                    }
                }
                Some(e) if is_prefab(e) => children.push(g),
                _ => {}
            }
        }
        for child in children {
            self.visit(child, depth + 1, visited, walk);
        }
    }
}

#[cfg(test)]
mod tests;
