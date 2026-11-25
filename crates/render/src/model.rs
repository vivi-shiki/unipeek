//! FBX -> flat triangle soup, grouped by material slot.
use bytemuck::{Pod, Zeroable};
use glam::Vec3;

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct Vertex {
    pub pos: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
}

pub struct Part {
    pub material: String,
    /// Index of the material in its mesh, which is the submesh slot of a Unity renderer.
    pub slot: usize,
    pub mesh: String,
    /// The node's identifier path in Unity's model prefab (`unipeek_unity::fileid::node_path`).
    pub path: String,
    /// Material slots of the node's mesh, so a renderer's extra materials can be detected.
    pub submeshes: usize,
    pub first: u32,
    pub count: u32,
}

pub struct Model {
    pub vertices: Vec<Vertex>,
    pub parts: Vec<Part>,
    pub min: Vec3,
    pub max: Vec3,
    pub bones: usize,
    pub blend_shapes: usize,
}

fn load_opts() -> ufbx::LoadOpts<'static> {
    ufbx::LoadOpts {
        target_axes: ufbx::CoordinateAxes::right_handed_y_up(),
        target_unit_meters: 1.0,
        space_conversion: ufbx::SpaceConversion::ModifyGeometry,
        generate_missing_normals: true,
        ..Default::default()
    }
}

impl Model {
    pub fn triangles(&self) -> usize {
        self.vertices.len() / 3
    }

    pub fn load_fbx(path: &str) -> crate::Result<Model> {
        let scene = ufbx::load_file(path, load_opts()).map_err(|e| crate::Error::Fbx(format!("{path}: {}", e.description)))?;
        let model = Model::from_scene(&scene);
        if model.vertices.is_empty() {
            return Err(crate::Error::Fbx(format!("{path}: no mesh data")));
        }
        Ok(model)
    }

    /// Parse FBX bytes (binary or ASCII). The error is ufbx's description.
    pub fn from_fbx_bytes(bytes: &[u8]) -> std::result::Result<Model, String> {
        let scene = ufbx::load_memory(bytes, load_opts()).map_err(|e| e.description.to_string())?;
        Ok(Model::from_scene(&scene))
    }

    fn from_scene(scene: &ufbx::Scene) -> Model {
        let mut vertices = Vec::new();
        let mut parts = Vec::new();
        let (mut min, mut max) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
        let mut tri = Vec::new();
        let top_level = scene.root_node.children.len();

        for node in &scene.nodes {
            let Some(mesh) = node.mesh.as_ref() else { continue };
            let mut names = Vec::new();
            let mut cur = Some(node);
            while let Some(n) = cur.filter(|n| !n.is_root) {
                names.push(n.element.name.to_string());
                cur = n.parent.as_deref();
            }
            names.reverse();
            let path = unipeek_unity::fileid::node_path(&names, top_level);
            let submeshes = mesh.materials.len().max(1);
            let to_world = node.geometry_to_world;
            let normal_mat = ufbx::matrix_for_normals(&to_world);
            let local = mesh.skinned_is_local;
            tri.resize(mesh.max_face_triangles * 3, 0u32);

            for part in &mesh.material_parts {
                if part.num_triangles == 0 {
                    continue;
                }
                let first = vertices.len() as u32;
                for &fi in part.face_indices.iter() {
                    let face = mesh.faces[fi as usize];
                    let n = ufbx::triangulate_face(&mut tri, mesh, face) as usize;
                    for &ix in &tri[..n * 3] {
                        let ix = ix as usize;
                        let mut p = mesh.skinned_position[ix];
                        let mut nrm = mesh.skinned_normal[ix];
                        if local {
                            p = ufbx::transform_position(&to_world, p);
                            nrm = ufbx::transform_direction(&normal_mat, nrm);
                        }
                        let uv = if mesh.vertex_uv.exists { mesh.vertex_uv[ix] } else { ufbx::Vec2 { x: 0.0, y: 0.0 } };
                        let pos = Vec3::new(p.x as f32, p.y as f32, p.z as f32);
                        min = min.min(pos);
                        max = max.max(pos);
                        let nv = Vec3::new(nrm.x as f32, nrm.y as f32, nrm.z as f32).normalize_or_zero();
                        vertices.push(Vertex {
                            pos: pos.to_array(),
                            normal: nv.to_array(),
                            // FBX UV origin is bottom-left, textures are top-left.
                            uv: [uv.x as f32, 1.0 - uv.y as f32],
                        });
                    }
                }
                let material = mesh.materials.get(part.index as usize).map(|m| m.element.name.to_string()).unwrap_or_default();
                parts.push(Part { material, slot: part.index as usize, mesh: node.element.name.to_string(), path: path.clone(), submeshes, first, count: vertices.len() as u32 - first });
            }
        }
        if vertices.is_empty() {
            (min, max) = (Vec3::ZERO, Vec3::ZERO);
        }
        Model { vertices, parts, min, max, bones: scene.bones.len(), blend_shapes: scene.blend_channels.len() }
    }
}
