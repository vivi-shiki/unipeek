//! Texture decoding and mip generation on the CPU, so a UI thread never waits on it.

/// A texture decoded for upload.
pub struct Decoded {
    /// Scaled to the requested maximum on its longest side.
    pub image: image::RgbaImage,
    /// The size of the file's image before scaling.
    pub size: [u32; 2],
    /// How the image was approximated (PSD in 16-bit or CMYK, ...), for the report.
    pub note: Option<String>,
}

/// Decode PNG, JPEG, WebP or the composite of a PSD.
pub fn decode(bytes: &[u8], max: u32) -> Result<Decoded, String> {
    let (img, note) = if bytes.starts_with(b"8BPS") {
        let c = crate::psd::decode(bytes)?;
        (c.image, c.note)
    } else {
        (image::load_from_memory(bytes).map_err(|e| e.to_string())?.to_rgba8(), None)
    };
    let size = [img.width(), img.height()];
    Ok(Decoded { image: clamp(img, max), size, note })
}

/// Width and height from the file header without decoding the pixels.
pub fn dimensions(bytes: &[u8]) -> Result<[u32; 2], String> {
    if bytes.starts_with(b"8BPS") {
        let b = bytes.get(14..22).ok_or("file is truncated")?;
        let (h, w) = (u32::from_be_bytes([b[0], b[1], b[2], b[3]]), u32::from_be_bytes([b[4], b[5], b[6], b[7]]));
        return Ok([w, h]);
    }
    let reader = image::ImageReader::new(std::io::Cursor::new(bytes)).with_guessed_format().map_err(|e| e.to_string())?;
    let (w, h) = reader.into_dimensions().map_err(|e| e.to_string())?;
    Ok([w, h])
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
