use super::*;
use unipeek_unity::testing::{PsdBuilder, PsdCompression};

const RED: [u8; 4] = [255, 0, 0, 255];
const HALF_BLUE: [u8; 4] = [0, 0, 255, 128];

/// 3x2: a red row and a row of half-transparent blue, matted against white as Photoshop stores it.
fn pixels() -> Vec<[u8; 4]> {
    let matted = [127, 127, 255, 128];
    vec![RED, RED, RED, matted, matted, matted]
}

#[test]
fn rgb_8bit_in_every_compression() {
    for c in [PsdCompression::Raw, PsdCompression::Rle, PsdCompression::Zip, PsdCompression::ZipPredicted] {
        let bytes = PsdBuilder::rgba8(3, 2, &pixels(), false).compression(c).build();
        let out = decode(&bytes).unwrap_or_else(|e| panic!("{c:?}: {e}"));
        assert_eq!(out.image.dimensions(), (3, 2));
        assert_eq!(out.image.get_pixel(0, 0).0, RED, "{c:?}");
        assert_eq!(out.image.get_pixel(2, 1).0, [127, 127, 255, 255], "{c:?}: without transparency alpha is opaque");
        assert_eq!(out.note, None);
    }
}

#[test]
fn merged_transparency_is_unmatted() {
    let bytes = PsdBuilder::rgba8(3, 2, &pixels(), true).compression(PsdCompression::Rle).build();
    let out = decode(&bytes).unwrap();
    assert_eq!(out.image.get_pixel(0, 0).0, RED);
    let p = out.image.get_pixel(1, 1).0;
    assert_eq!(p[3], 128);
    assert!(p[0] <= 1 && p[1] <= 1 && p[2] == 255, "white matte removed: {p:?}");
    let _ = HALF_BLUE;
}

#[test]
fn extra_channel_without_negative_layer_count_is_not_alpha() {
    let mut b = PsdBuilder::rgba8(3, 2, &pixels(), true);
    b.merged_alpha = false;
    let out = decode(&b.build()).unwrap();
    assert_eq!(out.image.get_pixel(1, 1).0[3], 255, "a spot or alpha channel is not the composite's transparency");
}

#[test]
fn psb_with_long_lengths() {
    let mut b = PsdBuilder::rgba8(3, 2, &pixels(), true).compression(PsdCompression::Rle);
    b.psb = true;
    let out = decode(&b.build()).unwrap();
    assert_eq!(out.image.get_pixel(0, 0).0, RED);
    assert_eq!(out.image.get_pixel(0, 1).0[3], 128);
}

#[test]
fn sixteen_and_thirty_two_bit_are_reduced_with_a_note() {
    let mut b = PsdBuilder::rgba8(2, 1, &[RED, RED], false);
    b.depth = 16;
    b.planes = vec![vec![0xff, 0xee, 0xff, 0xee], vec![0x80, 0, 0x80, 0], vec![0, 0, 0, 0]];
    let out = decode(&b.build()).unwrap();
    assert_eq!(out.image.get_pixel(1, 0).0, [255, 128, 0, 255]);
    assert!(out.note.as_deref().unwrap().contains("16-bit"), "{:?}", out.note);

    b.depth = 32;
    let f = |v: f32| v.to_be_bytes().repeat(2);
    b.planes = vec![f(1.0), f(0.2140), f(0.0)];
    let out = decode(&b.build()).unwrap();
    let p = out.image.get_pixel(0, 0).0;
    assert_eq!((p[0], p[2], p[3]), (255, 0, 255));
    assert!((p[1] as i32 - 128).abs() <= 1, "linear float gets the sRGB curve: {p:?}");
    assert!(out.note.as_deref().unwrap().contains("32-bit"));
}

#[test]
fn grayscale_indexed_bitmap_and_cmyk() {
    let mut gray = PsdBuilder::rgba8(2, 1, &[RED, RED], false);
    (gray.mode, gray.planes) = (1, vec![vec![10, 200]]);
    let out = decode(&gray.build()).unwrap();
    assert_eq!(out.image.get_pixel(1, 0).0, [200, 200, 200, 255]);
    assert_eq!(out.note, None);

    let mut indexed = gray.clone();
    indexed.mode = 2;
    indexed.palette = vec![0; 768];
    indexed.palette[200] = 9;
    indexed.palette[256 + 200] = 8;
    indexed.palette[512 + 200] = 7;
    assert_eq!(decode(&indexed.build()).unwrap().image.get_pixel(1, 0).0, [9, 8, 7, 255]);
    indexed.palette.truncate(10);
    assert!(decode(&indexed.build()).unwrap_err().contains("palette"));

    let mut bitmap = gray.clone();
    (bitmap.mode, bitmap.depth, bitmap.width, bitmap.planes) = (0, 1, 9, vec![vec![0b1000_0000, 0b1000_0000]]);
    let out = decode(&bitmap.build()).unwrap();
    assert_eq!(out.image.get_pixel(0, 0).0, [0, 0, 0, 255], "a set bit is black");
    assert_eq!(out.image.get_pixel(1, 0).0, [255, 255, 255, 255]);
    assert_eq!(out.image.get_pixel(8, 0).0, [0, 0, 0, 255]);

    let mut cmyk = gray.clone();
    // Inverted: 255 is no ink. Cyan full, no key -> (0, 255, 255).
    (cmyk.mode, cmyk.planes) = (4, vec![vec![0, 255], vec![255, 255], vec![255, 255], vec![255, 0]]);
    let out = decode(&cmyk.build()).unwrap();
    assert_eq!(out.image.get_pixel(0, 0).0, [0, 255, 255, 255]);
    assert_eq!(out.image.get_pixel(1, 0).0, [0, 0, 0, 255], "full key is black");
    assert!(out.note.as_deref().unwrap().contains("CMYK"));
}

#[test]
fn files_without_a_composite_and_broken_files_are_reasons() {
    let mut b = PsdBuilder::rgba8(3, 2, &pixels(), false);
    b.real_merged = Some(false);
    assert!(decode(&b.build()).unwrap_err().contains("Maximize Compatibility"));
    b.real_merged = Some(true);
    assert!(decode(&b.build()).is_ok());

    let good = b.build();
    for cut in [0, 3, 10, 30, good.len() - 3] {
        assert!(decode(&good[..cut]).is_err(), "cut at {cut}");
    }
    let mut lab = b.clone();
    lab.mode = 9;
    assert!(decode(&lab.build()).unwrap_err().contains("Lab"));
    let mut huge = good.clone();
    huge[14..18].copy_from_slice(&100_000u32.to_be_bytes());
    huge[18..22].copy_from_slice(&100_000u32.to_be_bytes());
    assert!(decode(&huge).unwrap_err().contains("too large"));
}

#[test]
fn damaged_rle_rows_never_panic() {
    let bytes = PsdBuilder::rgba8(3, 2, &pixels(), true).compression(PsdCompression::Rle).build();
    let data_start = bytes.len().saturating_sub(48);
    for i in data_start..bytes.len() {
        for v in [0u8, 0x7f, 0x80, 0x81, 0xff] {
            let mut b = bytes.clone();
            b[i] = v;
            let _ = decode(&b);
        }
    }
}
