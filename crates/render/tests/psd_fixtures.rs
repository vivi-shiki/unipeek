//! PSD composites of files written by Photoshop and GIMP (from psd-tools, MIT, see `fixtures/psd-tools`).
use unipeek::psd::decode;

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/psd-tools").join(name)).unwrap()
}

fn first_row(name: &str) -> (Vec<[u8; 4]>, Option<String>) {
    let c = decode(&fixture(name)).unwrap_or_else(|e| panic!("{name}: {e}"));
    assert_eq!(c.image.dimensions(), (4, 4), "{name}");
    ((0..4).map(|x| c.image.get_pixel(x, 0).0).collect(), c.note)
}

const RGB_ROW: [[u8; 4]; 4] = [[10, 0, 178, 255], [132, 0, 89, 255], [255, 0, 0, 255], [255, 126, 0, 255]];

fn close(a: &[[u8; 4]], b: &[[u8; 4]], tolerance: u8) -> bool {
    a.iter().zip(b).all(|(p, q)| (0..4).all(|c| p[c].abs_diff(q[c]) <= tolerance))
}

#[test]
fn the_same_picture_in_every_color_mode() {
    assert_eq!(first_row("4x4_8bit_rgb.psd"), (RGB_ROW.to_vec(), None));
    assert_eq!(first_row("4x4_8bit_index_color.psd").0, RGB_ROW.to_vec());
    let (row, note) = first_row("4x4_16bit_rgb.psd");
    assert!(close(&row, &RGB_ROW, 1), "{row:?}");
    assert!(note.unwrap().contains("16-bit"));
    let (gray, _) = first_row("4x4_8bit_grayscale.psd");
    assert!(gray.iter().all(|p| p[0] == p[1] && p[1] == p[2] && p[3] == 255), "{gray:?}");
    let (bits, _) = first_row("4x4_1bit_bitmap.psd");
    assert!(bits.iter().all(|p| p[..3] == [0, 0, 0] || p[..3] == [255, 255, 255]), "{bits:?}");
}

#[test]
fn thirty_two_bit_is_linear_light() {
    let (row, note) = first_row("4x4_32bit_rgb.psd");
    assert_eq!(row[0], RGB_ROW[0], "a flat color survives the linear round trip");
    assert_eq!(row[2], RGB_ROW[2]);
    assert!(note.unwrap().contains("32-bit"));
}

#[test]
fn gimp_transparency_in_psd_and_psb() {
    for name in ["transparentbg-gimp.psd", "transparentbg-gimp.psb"] {
        let c = decode(&fixture(name)).unwrap();
        assert_eq!(c.image.get_pixel(20, 20).0[3], 0, "{name}: the middle is transparent");
        assert!(c.image.pixels().any(|p| p.0[3] == 255), "{name}: something is opaque");
    }
}

#[test]
fn lab_is_a_reason_not_a_panic() {
    assert!(decode(&fixture("4x4_8bit_lab.psd")).unwrap_err().contains("Lab"));
}
