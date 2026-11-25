//! The composite (merged) image of a Photoshop file, the picture Unity imports from a `.psd`.
//!
//! Only the header, the image resources and the image data section are read; layers are skipped.
//! RGB, grayscale, indexed, bitmap, duotone and CMYK documents with 1, 8, 16 or 32 bits per channel,
//! raw, RLE or ZIP compressed, PSD and PSB. Everything that is not 8-bit RGB or grayscale comes back
//! with a note saying how it was approximated.
use std::io::Read;

/// Larger composites are refused instead of allocating gigabytes for a thumbnail.
const MAX_PIXELS: u64 = 1 << 27;

#[derive(Debug)]
pub struct Composite {
    pub image: image::RgbaImage,
    /// How the composite was approximated (16-bit, CMYK, ...); `None` for plain 8-bit RGB or gray.
    pub note: Option<String>,
}

/// Decode the composite image of a PSD or PSB file. Errors are short English reasons for a report.
pub fn decode(bytes: &[u8]) -> Result<Composite, String> {
    let mut r = Cursor { data: bytes, pos: 0 };
    if r.take(4)? != b"8BPS" {
        return Err("not a Photoshop file".into());
    }
    let version = r.u16()?;
    let psb = match version {
        1 => false,
        2 => true,
        v => return Err(format!("unknown Photoshop file version {v}")),
    };
    r.skip(6)?;
    let channels = r.u16()? as usize;
    let height = r.u32()?;
    let width = r.u32()?;
    let depth = r.u16()?;
    let mode = r.u16()?;
    if channels == 0 || channels > 56 || width == 0 || height == 0 {
        return Err(format!("invalid header: {channels} channel(s), {width}x{height}"));
    }
    if u64::from(width) * u64::from(height) > MAX_PIXELS {
        return Err(format!("composite of {width}x{height} is too large"));
    }
    if !matches!(depth, 1 | 8 | 16 | 32) {
        return Err(format!("unsupported depth of {depth} bits"));
    }
    let color_data = r.block(false)?;
    let resources = r.block(false)?;
    let layers = r.block(psb)?;
    let merged_transparency = layer_count(layers, psb).is_some_and(|n| n < 0) || has_merged_transparency_block(layers);
    if !has_real_merged_data(resources) {
        return Err("no composite image (saved without Maximize Compatibility)".into());
    }

    let compression = r.u16()?;
    let (w, h) = (width as usize, height as usize);
    let row_bytes = if depth == 1 { w.div_ceil(8) } else { w * depth as usize / 8 };
    let planes = match compression {
        0 => r.rest().get(..row_bytes * h * channels).ok_or("image data is truncated")?.to_vec(),
        1 => rle(&mut r, psb, channels, h, row_bytes)?,
        2 | 3 => {
            let mut out = Vec::with_capacity(row_bytes * h * channels);
            flate2::read::ZlibDecoder::new(r.rest()).take((row_bytes * h * channels) as u64).read_to_end(&mut out).map_err(|e| format!("zip data: {e}"))?;
            if out.len() < row_bytes * h * channels {
                return Err("image data is truncated".into());
            }
            if compression == 3 {
                unpredict(&mut out, row_bytes, depth, w)?;
            }
            out
        }
        c => return Err(format!("unknown compression {c}")),
    };

    let plane = |c: usize| -> &[u8] { &planes[c * row_bytes * h..(c + 1) * row_bytes * h] };
    // Every channel as 8-bit values; float channels are linear and get the sRGB curve.
    let sample = |c: usize, i: usize| -> u8 {
        let p = plane(c);
        match depth {
            1 => {
                let (y, x) = (i / w, i % w);
                if p[y * row_bytes + x / 8] & (0x80 >> (x % 8)) != 0 { 0 } else { 255 }
            }
            8 => p[i],
            16 => p[i * 2],
            _ => {
                let v = f32::from_be_bytes([p[i * 4], p[i * 4 + 1], p[i * 4 + 2], p[i * 4 + 3]]);
                (linear_to_srgb(v.clamp(0.0, 1.0)) * 255.0).round() as u8
            }
        }
    };
    let (base, mut note) = match mode {
        0 => (1, None),
        1 => (1, None),
        2 => (1, None),
        3 => (3, None),
        4 => (4, Some("CMYK converted to RGB without a color profile".to_string())),
        8 => (1, Some("duotone shown as grayscale".to_string())),
        7 => return Err("multichannel documents are not supported".into()),
        9 => return Err("Lab documents are not supported".into()),
        m => return Err(format!("unknown color mode {m}")),
    };
    if channels < base {
        return Err(format!("{channels} channel(s) for color mode {mode}"));
    }
    if mode == 2 && color_data.len() < 768 {
        return Err("indexed color without a palette".into());
    }
    if matches!(depth, 16 | 32) {
        let reduced = format!("{depth}-bit channels reduced to 8 bits");
        note = Some(note.map_or(reduced.clone(), |n| format!("{n}; {reduced}")));
    }
    let alpha = (merged_transparency && channels > base).then_some(base);

    let mut out = image::RgbaImage::new(width, height);
    for (i, px) in out.pixels_mut().enumerate() {
        let a = alpha.map_or(255, |c| sample(c, i));
        let rgb = match mode {
            3 => [sample(0, i), sample(1, i), sample(2, i)],
            4 => {
                // Photoshop stores CMYK inverted: 255 means no ink.
                let k = sample(3, i) as u32;
                [0, 1, 2].map(|c| (sample(c, i) as u32 * k / 255) as u8)
            }
            2 => {
                let ix = sample(0, i) as usize;
                [color_data[ix], color_data[256 + ix], color_data[512 + ix]]
            }
            _ => [sample(0, i); 3],
        };
        // The merged image of a transparent document is matted against white.
        let rgb = if a > 0 && a < 255 { rgb.map(|c| unmatte(c, a)) } else { rgb };
        px.0 = [rgb[0], rgb[1], rgb[2], a];
    }
    Ok(Composite { image: out, note })
}

fn unmatte(c: u8, a: u8) -> u8 {
    let (c, a) = (c as i32, a as i32);
    ((c - (255 - a)) * 255 / a).clamp(0, 255) as u8
}

fn linear_to_srgb(v: f32) -> f32 {
    if v <= 0.003_130_8 { v * 12.92 } else { 1.055 * v.powf(1.0 / 2.4) - 0.055 }
}

/// The tagged blocks `Mtrn`, `Mt16` and `Mt32` after the layer info mark a composite with
/// transparency even when the layer count is positive.
fn has_merged_transparency_block(section: &[u8]) -> bool {
    [b"Mtrn", b"Mt16", b"Mt32"].iter().any(|key| section.windows(8).any(|w| (&w[..4] == b"8BIM" || &w[..4] == b"8B64") && &w[4..] == *key))
}

/// Layer count from the layer info, if the file has one. Negative means the first extra channel
/// of the composite is its transparency.
fn layer_count(section: &[u8], psb: bool) -> Option<i16> {
    let mut r = Cursor { data: section, pos: 0 };
    let info = r.block(psb).ok()?;
    let mut r = Cursor { data: info, pos: 0 };
    Some(r.u16().ok()? as i16)
}

/// Resource 1057 (version info) says whether the composite is real. Files without it are old and have one.
fn has_real_merged_data(resources: &[u8]) -> bool {
    let mut r = Cursor { data: resources, pos: 0 };
    while r.remaining() >= 12 {
        let Ok(sig) = r.take(4) else { break };
        if sig != b"8BIM" && sig != b"MeSa" {
            break;
        }
        let Ok(id) = r.u16() else { break };
        let Ok(name_len) = r.u8() else { break };
        // The Pascal name, length byte included, is padded to an even size.
        let pad = (1 + name_len as usize) % 2;
        if r.skip(name_len as usize + pad).is_err() {
            break;
        }
        let Ok(size) = r.u32() else { break };
        let Ok(data) = r.take(size as usize) else { break };
        // Padding missing at the very end is harmless.
        let _ = r.skip(size as usize % 2);
        if id == 1057 && data.len() >= 5 {
            return data[4] != 0;
        }
    }
    true
}

fn rle(r: &mut Cursor<'_>, psb: bool, channels: usize, h: usize, row_bytes: usize) -> Result<Vec<u8>, String> {
    let rows = channels * h;
    let mut counts = Vec::with_capacity(rows);
    for _ in 0..rows {
        counts.push(if psb { r.u32()? as usize } else { r.u16()? as usize });
    }
    let mut out = vec![0u8; rows * row_bytes];
    for (row, &count) in counts.iter().enumerate() {
        let src = r.take(count)?;
        unpack_bits(src, &mut out[row * row_bytes..(row + 1) * row_bytes]);
    }
    Ok(out)
}

/// PackBits. A short or damaged row leaves the rest of it zero rather than failing the image.
fn unpack_bits(mut src: &[u8], dst: &mut [u8]) {
    let mut o = 0;
    while let [n, rest @ ..] = src
        && o < dst.len()
    {
        let n = *n as i8;
        if n >= 0 {
            let len = (n as usize + 1).min(rest.len()).min(dst.len() - o);
            dst[o..o + len].copy_from_slice(&rest[..len]);
            o += len;
            src = &rest[(n as usize + 1).min(rest.len())..];
        } else if n != -128 {
            let Some((&v, rest)) = rest.split_first() else { break };
            let len = (1 - n as isize) as usize;
            let len = len.min(dst.len() - o);
            dst[o..o + len].fill(v);
            o += len;
            src = rest;
        } else {
            src = rest;
        }
    }
}

/// Undo ZIP-with-prediction: deltas along each row (per sample for 8 and 16 bits; for 32 bits the
/// bytes of a row are split into planes before the deltas).
fn unpredict(data: &mut [u8], row_bytes: usize, depth: u16, w: usize) -> Result<(), String> {
    for row in data.chunks_exact_mut(row_bytes) {
        match depth {
            8 => {
                for i in 1..row.len() {
                    row[i] = row[i].wrapping_add(row[i - 1]);
                }
            }
            16 => {
                let mut prev = 0u16;
                for s in row.chunks_exact_mut(2) {
                    let v = u16::from_be_bytes([s[0], s[1]]).wrapping_add(prev);
                    s.copy_from_slice(&v.to_be_bytes());
                    prev = v;
                }
            }
            32 => {
                for i in 1..row.len() {
                    row[i] = row[i].wrapping_add(row[i - 1]);
                }
                let planar = row.to_vec();
                for x in 0..w {
                    for b in 0..4 {
                        row[x * 4 + b] = planar[b * w + x];
                    }
                }
            }
            _ => return Err("prediction on 1-bit data".into()),
        }
    }
    Ok(())
}

struct Cursor<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        let end = self.pos.checked_add(n).filter(|&e| e <= self.data.len()).ok_or("file is truncated")?;
        let s = &self.data[self.pos..end];
        self.pos = end;
        Ok(s)
    }

    fn skip(&mut self, n: usize) -> Result<(), String> {
        self.take(n).map(|_| ())
    }

    fn remaining(&self) -> usize {
        self.data.len() - self.pos
    }

    fn rest(&self) -> &'a [u8] {
        &self.data[self.pos..]
    }

    fn u8(&mut self) -> Result<u8, String> {
        Ok(self.take(1)?[0])
    }

    fn u16(&mut self) -> Result<u16, String> {
        let b = self.take(2)?;
        Ok(u16::from_be_bytes([b[0], b[1]]))
    }

    fn u32(&mut self) -> Result<u32, String> {
        let b = self.take(4)?;
        Ok(u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
    }

    fn u64(&mut self) -> Result<u64, String> {
        let b = self.take(8)?;
        Ok(u64::from_be_bytes(b.try_into().map_err(|_| "file is truncated")?))
    }

    /// A length-prefixed section; PSB uses 8-byte lengths for some of them.
    fn block(&mut self, long: bool) -> Result<&'a [u8], String> {
        let len = if long { self.u64()? } else { u64::from(self.u32()?) };
        self.take(usize::try_from(len).map_err(|_| "section is too large")?)
    }
}

#[cfg(test)]
mod tests;
