//! Everything needed to draw a package, prepared on the CPU: geometry, bound materials,
//! decoded and mip-mapped textures, and the report. Loading never touches the GPU.
use crate::{model::Vertex, Result};
use glam::Vec3;
use std::sync::Arc;
use unipeek_unity::{IndexOptions, Input, Report, material::Look};

#[derive(Clone, Debug)]
pub struct LoadOptions {
    pub index: IndexOptions,
    /// Longest side of a decoded texture; larger textures are scaled down before upload.
    pub max_texture: u32,
}

impl Default for LoadOptions {
    fn default() -> Self {
        LoadOptions { index: IndexOptions::default(), max_texture: 2048 }
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
    pub fn load(_input: Input, _opts: &LoadOptions) -> Result<Scene> {
        todo!()
    }

    pub fn triangles(&self) -> usize {
        self.vertices.len() / 3
    }

    pub fn is_empty(&self) -> bool {
        self.vertices.is_empty()
    }
}
