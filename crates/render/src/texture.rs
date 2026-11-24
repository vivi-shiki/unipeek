//! Texture decoding and mip generation on the CPU, so a UI thread never waits on it.
use std::path::{Path, PathBuf};

pub const MAX_SIZE: u32 = 2048;

pub fn guess_texture(dir: &Path, material: &str) -> Option<PathBuf> {
    let key = material.to_lowercase();
    let mut best: Option<PathBuf> = None;
    for e in std::fs::read_dir(dir).ok()?.flatten() {
        let p = e.path();
        if p.extension().and_then(|s| s.to_str()) != Some("png") {
            continue;
        }
        let stem = p.file_stem()?.to_str()?.to_lowercase();
        if stem.ends_with(&key) && best.as_ref().is_none_or(|b| p.as_os_str().len() < b.as_os_str().len()) {
            best = Some(p);
        }
    }
    best
}

/// RGBA8 mip chain, largest first, base clamped to MAX_SIZE.
pub fn load_mips(path: &Path) -> anyhow::Result<Vec<image::RgbaImage>> {
    let img = image::open(path)?.to_rgba8();
    Ok(mips(clamp(img, MAX_SIZE)))
}

/// Decode PNG or JPEG bytes. Returns the image scaled to `max` on its longest side and the original size.
pub fn decode(bytes: &[u8], max: u32) -> Result<(image::RgbaImage, [u32; 2]), String> {
    let img = image::load_from_memory(bytes).map_err(|e| e.to_string())?.to_rgba8();
    let size = [img.width(), img.height()];
    Ok((clamp(img, max), size))
}

fn clamp(img: image::RgbaImage, max: u32) -> image::RgbaImage {
    let (w, h) = (img.width(), img.height());
    let longest = w.max(h);
    if longest <= max {
        return img;
    }
    let scale = max as f64 / longest as f64;
    let (nw, nh) = (((w as f64 * scale).round() as u32).max(1), ((h as f64 * scale).round() as u32).max(1));
    image::imageops::resize(&img, nw, nh, image::imageops::FilterType::Triangle)
}

/// Mip chain down to 4 pixels on the shorter side, largest first.
pub fn mips(img: image::RgbaImage) -> Vec<image::RgbaImage> {
    let mut mips = vec![img];
    loop {
        let last = mips.last().unwrap();
        if last.width() <= 4 || last.height() <= 4 {
            break;
        }
        let next = image::imageops::resize(last, last.width() / 2, last.height() / 2, image::imageops::FilterType::Triangle);
        mips.push(next);
    }
    mips
}
