//! Headless thumbnails: render a scene offscreen and read the pixels back.
use crate::{Result, Scene, gpu::Gpu};

/// Bump whenever rendered pixels change, so callers' thumbnail caches are rebuilt.
pub const RENDERER_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum View {
    Front,
    Side,
    Back,
    ThreeQuarter,
}

impl std::fmt::Display for View {
    fn fmt(&self, _f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        todo!()
    }
}

impl std::str::FromStr for View {
    type Err = String;
    fn from_str(_s: &str) -> std::result::Result<View, String> {
        todo!()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Background {
    Transparent,
    Color([u8; 4]),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ThumbOptions {
    pub size: u32,
    pub view: View,
    pub background: Background,
}

impl Default for ThumbOptions {
    fn default() -> Self {
        ThumbOptions { size: 512, view: View::ThreeQuarter, background: Background::Transparent }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageFormat {
    Png,
    Webp,
}

impl ImageFormat {
    pub fn from_path(_path: impl AsRef<std::path::Path>) -> Option<ImageFormat> {
        todo!()
    }
}

/// Render `scene` into a square RGBA image (straight alpha, sRGB).
pub fn render_thumbnail(_gpu: &Gpu, _scene: &Scene, _opts: &ThumbOptions) -> Result<image::RgbaImage> {
    todo!()
}

/// Lossless PNG or WebP.
pub fn encode(_img: &image::RgbaImage, _format: ImageFormat) -> Result<Vec<u8>> {
    todo!()
}
