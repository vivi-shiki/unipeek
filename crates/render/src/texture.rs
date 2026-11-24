//! Texture decoding and mip generation on the CPU, so a UI thread never waits on it.

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
