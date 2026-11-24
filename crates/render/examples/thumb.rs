//! Headless thumbnail with timings: `thumb <input> <out.png|out.webp> [size] [front|side|back|3q]`.
use std::time::Instant;
use unipeek::{
    LoadOptions, Scene,
    gpu::Gpu,
    thumb::{self, Background, ImageFormat, ThumbOptions},
    unity::Input,
};

fn main() -> anyhow::Result<()> {
    let a: Vec<String> = std::env::args().collect();
    anyhow::ensure!(a.len() >= 3, "usage: thumb <input> <out.png|out.webp> [size] [front|side|back|3q]");
    let size = a.get(3).map(|s| s.parse()).transpose()?.unwrap_or(512);
    let view = a.get(4).map(|s| s.parse()).transpose().map_err(anyhow::Error::msg)?.unwrap_or(thumb::View::ThreeQuarter);
    let t = Instant::now();
    let scene = Scene::load(Input::from_location(&a[1]), &LoadOptions { max_texture: 1024, ..Default::default() })?;
    println!("scene: {} tris, {} materials in {:?}", scene.triangles(), scene.materials.len(), t.elapsed());
    let t = Instant::now();
    let gpu = Gpu::headless()?;
    println!("adapter: {} ({:?}) in {:?}", gpu.adapter.get_info().name, gpu.adapter.get_info().backend, t.elapsed());
    let t = Instant::now();
    let img = thumb::render_thumbnail(&gpu, &scene, &ThumbOptions { size, view, background: Background::Transparent })?;
    println!("render: {:?}", t.elapsed());
    let format = ImageFormat::from_path(&a[2]).unwrap_or(ImageFormat::Png);
    std::fs::write(&a[2], thumb::encode(&img, format)?)?;
    println!("wrote {} ({:?}), status {:?}, {} degrade reason(s)", a[2], format, scene.report.status, scene.report.degrades.len());
    if std::env::var("THUMB_REPORT").is_ok() {
        println!("{}", serde_json::to_string_pretty(&scene.report)?);
    }
    Ok(())
}
