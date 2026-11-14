//! Texture lookup by material name (minimal stand-in for .meta/.mat binding).
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
    let mut img = image::open(path)?.to_rgba8();
    if img.width() > MAX_SIZE || img.height() > MAX_SIZE {
        img = image::imageops::resize(&img, MAX_SIZE.min(img.width()), MAX_SIZE.min(img.height()), image::imageops::FilterType::Triangle);
    }
    let mut mips = vec![img];
    loop {
        let last = mips.last().unwrap();
        if last.width() <= 4 || last.height() <= 4 {
            break;
        }
        let next = image::imageops::resize(last, last.width() / 2, last.height() / 2, image::imageops::FilterType::Triangle);
        mips.push(next);
    }
    Ok(mips)
}
