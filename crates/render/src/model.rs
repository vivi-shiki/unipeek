//! FBX -> flat triangle soup, grouped by material.
use anyhow::{Context, Result};
use bytemuck::{Pod, Zeroable};
use glam::Vec3;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct Vertex {
    pub pos: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
}

pub struct Part {
    pub material: String,
    pub mesh: String,
    pub first: u32,
    pub count: u32,
}

pub struct Model {
    pub vertices: Vec<Vertex>,
    pub parts: Vec<Part>,
    pub min: Vec3,
    pub max: Vec3,
}

impl Model {
    pub fn triangles(&self) -> usize {
        self.vertices.len() / 3
    }

    pub fn load_fbx(path: &str) -> Result<Model> {
        let opts = ufbx::LoadOpts {
            target_axes: ufbx::CoordinateAxes::right_handed_y_up(),
            target_unit_meters: 1.0,
            space_conversion: ufbx::SpaceConversion::ModifyGeometry,
            generate_missing_normals: true,
            ..Default::default()
        };
        let scene = ufbx::load_file(path, opts)
            .map_err(|e| anyhow::anyhow!("ufbx: {:?}", e.description))
            .with_context(|| format!("loading {path}"))?;

        let mut vertices = Vec::new();
        let mut parts = Vec::new();
        let (mut min, mut max) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
        let mut tri = Vec::new();

        for node in &scene.nodes {
            let Some(mesh) = node.mesh.as_ref() else { continue };
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
                let material = mesh
                    .materials
                    .get(part.index as usize)
                    .map(|m| m.element.name.to_string())
                    .unwrap_or_default();
                parts.push(Part {
                    material,
                    mesh: node.element.name.to_string(),
                    first,
                    count: vertices.len() as u32 - first,
                });
            }
        }
        anyhow::ensure!(!vertices.is_empty(), "no mesh data in {path}");
        Ok(Model { vertices, parts, min, max })
    }
}
