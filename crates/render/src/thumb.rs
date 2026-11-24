//! Headless thumbnails: render a scene offscreen and read the pixels back.
use crate::{Error, Result, Scene, camera::Orbit, gpu::Gpu, renderer::Renderer};

/// Bump whenever rendered pixels change, so callers' thumbnail caches are rebuilt.
pub const RENDERER_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum View {
    Front,
    Side,
    Back,
    ThreeQuarter,
}

impl View {
    /// Camera yaw and pitch in radians. Yaw 0 looks at the model's front (+Z).
    fn angles(self) -> (f32, f32) {
        match self {
            View::Front => (0.0, 0.05),
            View::Side => (std::f32::consts::FRAC_PI_2, 0.05),
            View::Back => (std::f32::consts::PI, 0.05),
            View::ThreeQuarter => (0.6, 0.12),
        }
    }
}

impl std::fmt::Display for View {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            View::Front => "front",
            View::Side => "side",
            View::Back => "back",
            View::ThreeQuarter => "3q",
        })
    }
}

impl std::str::FromStr for View {
    type Err = String;
    fn from_str(s: &str) -> std::result::Result<View, String> {
        Ok(match s {
            "front" => View::Front,
            "side" => View::Side,
            "back" => View::Back,
            "3q" => View::ThreeQuarter,
            other => return Err(format!("unknown view {other:?}, expected front, side, back or 3q")),
        })
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
    pub fn from_path(path: impl AsRef<std::path::Path>) -> Option<ImageFormat> {
        match path.as_ref().extension()?.to_str()?.to_ascii_lowercase().as_str() {
            "png" => Some(ImageFormat::Png),
            "webp" => Some(ImageFormat::Webp),
            _ => None,
        }
    }
}

/// Render `scene` into a square RGBA image (straight alpha, sRGB).
pub fn render_thumbnail(gpu: &Gpu, scene: &Scene, opts: &ThumbOptions) -> Result<image::RgbaImage> {
    if scene.is_empty() {
        return Err(Error::NothingToShow(scene.report.input.clone()));
    }
    let size = opts.size.clamp(1, 8192);
    let format = wgpu::TextureFormat::Rgba8UnormSrgb;
    let mut renderer = Renderer::new(&gpu.device, &gpu.queue, format, scene);
    renderer.clear = match opts.background {
        Background::Transparent => wgpu::Color::TRANSPARENT,
        Background::Color([r, g, b, a]) => {
            let lin = |c: u8| srgb_to_linear(c as f64 / 255.0);
            wgpu::Color { r: lin(r), g: lin(g), b: lin(b), a: a as f64 / 255.0 }
        }
    };
    let mut camera = Orbit::fit(scene.min, scene.max);
    (camera.yaw, camera.pitch) = opts.view.angles();
    let target = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("thumbnail"),
        size: wgpu::Extent3d { width: size, height: size, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = target.create_view(&Default::default());
    let mut enc = gpu.device.create_command_encoder(&Default::default());
    renderer.render(&gpu.device, &gpu.queue, &mut enc, &view, size, size, &camera);
    gpu.queue.submit([enc.finish()]);
    let mut pixels = gpu.read_texture(&target, size, size)?;
    if opts.background == Background::Transparent {
        unpremultiply(&mut pixels);
    }
    image::RgbaImage::from_raw(size, size, pixels).ok_or_else(|| Error::Gpu("readback size mismatch".into()))
}

/// Edges resolved against a transparent black background come out premultiplied.
fn unpremultiply(pixels: &mut [u8]) {
    for p in pixels.chunks_exact_mut(4) {
        let a = p[3] as u32;
        if a > 0 && a < 255 {
            for c in &mut p[..3] {
                *c = ((*c as u32 * 255 + a / 2) / a).min(255) as u8;
            }
        }
    }
}

fn srgb_to_linear(c: f64) -> f64 {
    if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}

impl Background {
    /// `transparent`, `#rrggbb` or `#rrggbbaa`.
    pub fn parse(s: &str) -> std::result::Result<Background, String> {
        if s == "transparent" {
            return Ok(Background::Transparent);
        }
        let hex = s.strip_prefix('#').filter(|h| (h.len() == 6 || h.len() == 8) && h.bytes().all(|b| b.is_ascii_hexdigit()));
        let Some(hex) = hex else { return Err(format!("background {s:?} is not transparent, #rrggbb or #rrggbbaa")) };
        let byte = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).unwrap_or(0);
        Ok(Background::Color([byte(0), byte(2), byte(4), if hex.len() == 8 { byte(6) } else { 255 }]))
    }
}

/// Load `location` (a path, optionally with `#inner/path`), render it and write `out` (`.png` or
/// `.webp`) through a temporary file. Returns the report of the loaded scene.
pub fn write_thumbnail(gpu: &Gpu, location: &str, out: &std::path::Path, opts: &ThumbOptions) -> Result<unipeek_unity::Report> {
    let format = ImageFormat::from_path(out).ok_or_else(|| Error::Image(format!("{} must end with .png or .webp", out.display())))?;
    let load = crate::LoadOptions { max_texture: (opts.size.saturating_mul(2)).clamp(256, 2048), ..Default::default() };
    let scene = Scene::load(unipeek_unity::Input::from_location(location), &load)?;
    let img = render_thumbnail(gpu, &scene, opts)?;
    let bytes = encode(&img, format)?;
    if let Some(dir) = out.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = out.with_file_name(format!("{}.tmp", out.file_name().map(|n| n.to_string_lossy()).unwrap_or_default()));
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, out)?;
    Ok(scene.report)
}

/// Lossless PNG or WebP.
pub fn encode(img: &image::RgbaImage, format: ImageFormat) -> Result<Vec<u8>> {
    let mut out = std::io::Cursor::new(Vec::new());
    let f = match format {
        ImageFormat::Png => image::ImageFormat::Png,
        ImageFormat::Webp => image::ImageFormat::WebP,
    };
    img.write_to(&mut out, f).map_err(|e| Error::Image(e.to_string()))?;
    Ok(out.into_inner())
}
