//! Headless thumbnails. Tests that need a GPU print `skipped:` and pass when there is none.
mod common;

use common::*;
use unipeek::{
    LoadOptions, Scene,
    thumb::{self, Background, ImageFormat, RENDERER_VERSION, ThumbOptions, View},
    unity::testing::{PackageBuilder, fbx_ascii, material_yaml, model_meta, prefab_instancing},
};

const LIL_CUTOUT: &str = "85d6126cae43b6847aff4b13f4adb8ec";

fn opts(view: View) -> ThumbOptions {
    ThumbOptions { size: 128, view, background: Background::Transparent }
}

/// Share of opaque pixels in the left and right halves that are mostly red / mostly blue.
fn colors(img: &image::RgbaImage) -> (usize, usize, usize, usize) {
    let (mut red_left, mut red_right, mut blue_left, mut blue_right) = (0, 0, 0, 0);
    for (x, _, p) in img.enumerate_pixels() {
        let [r, g, b, a] = p.0;
        if a < 200 {
            continue;
        }
        let left = x < img.width() / 2;
        if r > 120 && b < 80 && g < 80 {
            if left { red_left += 1 } else { red_right += 1 }
        }
        if b > 120 && r < 80 && g < 80 {
            if left { blue_left += 1 } else { blue_right += 1 }
        }
    }
    (red_left, red_right, blue_left, blue_right)
}

fn quads_with(cull: i32) -> Scene {
    let bytes = PackageBuilder::new()
        .asset_with_meta(MODEL, "Assets/S/Quads.fbx", fbx_ascii(&["Red", "Blue"]), model_meta(MODEL, &[("Red", RED_MAT), ("Blue", BLUE_MAT)]))
        .asset(RED_MAT, "Assets/S/Red.mat", material_yaml("Red", LILTOON, 4800000, Some(RED_TEX), -1).replace("_Cull: 2", &format!("_Cull: {cull}")).replace("g: 0.5, b: 0.25", "g: 1, b: 1"))
        .asset(BLUE_MAT, "Assets/S/Blue.mat", material_yaml("Blue", LILTOON, 4800000, Some(BLUE_TEX), -1).replace("_Cull: 2", &format!("_Cull: {cull}")).replace("g: 0.5, b: 0.25", "g: 1, b: 1"))
        .asset(RED_TEX, "Assets/S/Red.png", png(16, 16, [255, 0, 0, 255]))
        .asset(BLUE_TEX, "Assets/S/Blue.png", png(16, 16, [0, 0, 255, 255]))
        .asset(PREFAB, "Assets/S/Quads.prefab", prefab_instancing(MODEL, &[]))
        .build();
    Scene::load(input(bytes), &LoadOptions::default()).unwrap()
}

#[test]
fn renderer_version_is_set() {
    const { assert!(RENDERER_VERSION >= 1) };
}

#[test]
fn front_view_shows_textures_on_a_transparent_background() {
    let Some(gpu) = gpu("front_view") else { return };
    let scene = Scene::load(input(two_quads()), &LoadOptions::default()).unwrap();
    let img = thumb::render_thumbnail(&gpu, &scene, &opts(View::Front)).unwrap();
    assert_eq!((img.width(), img.height()), (128, 128));
    assert_eq!(img.get_pixel(0, 0).0[3], 0, "background is transparent");
    assert_eq!(img.get_pixel(127, 127).0[3], 0);
    let (rl, rr, bl, br) = colors(&img);
    assert!(rl > 200 && bl < rl / 10, "red quad on the left: {rl} {bl}");
    assert!(br > 200 && rr < br / 10, "blue quad on the right: {br} {rr}");
}

#[test]
fn back_faces_follow_the_material_cull_mode() {
    let Some(gpu) = gpu("back_faces") else { return };
    let culled = thumb::render_thumbnail(&gpu, &quads_with(2), &opts(View::Back)).unwrap();
    assert!(culled.pixels().all(|p| p.0[3] == 0), "back faces are culled with _Cull 2");
    let both = thumb::render_thumbnail(&gpu, &quads_with(0), &opts(View::Back)).unwrap();
    let (rl, rr, _, br) = colors(&both);
    assert!(rr > 200 && rl < rr / 10, "seen from the back, red is on the right: {rl} {rr}");
    assert!(br == 0 || br < rr, "blue moved left");
}

#[test]
fn side_view_differs_and_rendering_is_deterministic() {
    let Some(gpu) = gpu("deterministic") else { return };
    let scene = Scene::load(input(two_quads()), &LoadOptions::default()).unwrap();
    let a = thumb::render_thumbnail(&gpu, &scene, &opts(View::ThreeQuarter)).unwrap();
    let b = thumb::render_thumbnail(&gpu, &scene, &opts(View::ThreeQuarter)).unwrap();
    assert_eq!(a, b);
    let front = thumb::render_thumbnail(&gpu, &scene, &opts(View::Front)).unwrap();
    assert_ne!(a, front);
}

#[test]
fn cutout_discards_pixels_below_the_cutoff() {
    let Some(gpu) = gpu("cutout") else { return };
    let bytes = PackageBuilder::new()
        .asset_with_meta(MODEL, "Assets/S/Leaf.fbx", fbx_ascii(&["Leaf"]), model_meta(MODEL, &[("Leaf", RED_MAT)]))
        .asset(RED_MAT, "Assets/S/Leaf.mat", material_yaml("Leaf", LIL_CUTOUT, 4800000, Some(RED_TEX), 2450))
        .asset(RED_TEX, "Assets/S/Leaf.png", png(16, 16, [0, 255, 0, 60]))
        .asset(PREFAB, "Assets/S/Leaf.prefab", prefab_instancing(MODEL, &[]))
        .build();
    let scene = Scene::load(input(bytes), &LoadOptions::default()).unwrap();
    let img = thumb::render_thumbnail(&gpu, &scene, &opts(View::Front)).unwrap();
    assert!(img.pixels().all(|p| p.0[3] == 0), "alpha 60/255 is below _Cutoff 0.5");
}

#[test]
fn solid_background_and_empty_scene() {
    let Some(gpu) = gpu("background") else { return };
    let scene = Scene::load(input(two_quads()), &LoadOptions::default()).unwrap();
    let o = ThumbOptions { size: 64, view: View::Front, background: Background::Color([10, 20, 30, 255]) };
    let img = thumb::render_thumbnail(&gpu, &scene, &o).unwrap();
    assert_eq!(img.get_pixel(0, 0).0, [10, 20, 30, 255]);
    let empty = Scene::load(input(PackageBuilder::new().build()), &LoadOptions::default()).unwrap();
    assert!(matches!(thumb::render_thumbnail(&gpu, &empty, &o), Err(unipeek::Error::NothingToShow(_))));
}

#[test]
fn png_and_webp_round_trip() {
    let mut img = image::RgbaImage::new(8, 4);
    img.put_pixel(1, 2, image::Rgba([200, 100, 50, 128]));
    for format in [ImageFormat::Png, ImageFormat::Webp] {
        let bytes = thumb::encode(&img, format).unwrap();
        let back = image::load_from_memory(&bytes).unwrap().to_rgba8();
        assert_eq!(back, img, "{format:?} is lossless");
    }
    assert_eq!(ImageFormat::from_path("a/b.WEBP"), Some(ImageFormat::Webp));
    assert_eq!(ImageFormat::from_path("a/b.png"), Some(ImageFormat::Png));
    assert_eq!(ImageFormat::from_path("a/b.jpg"), None);
}

#[test]
fn view_names() {
    for (name, view) in [("front", View::Front), ("side", View::Side), ("back", View::Back), ("3q", View::ThreeQuarter)] {
        assert_eq!(name.parse::<View>().unwrap(), view);
        assert_eq!(view.to_string(), name);
    }
    assert!("top".parse::<View>().is_err());
}

const LIL_OUTLINE: &str = "efa77a80ca0344749b4f19fdd5891cbe";
const LIL_TRANS: &str = "165365ab7100a044ca85fc8c33548a62";

fn cube(shader: &str) -> Scene {
    let mat = material_yaml("Box", shader, 4800000, Some(RED_TEX), -1)
        .replace("_OutlineWidth: 0.08", "_OutlineWidth: 5")
        .replace("_OutlineColor: {r: 0.2, g: 0.1, b: 0.1, a: 1}", "_OutlineColor: {r: 0, g: 1, b: 0, a: 1}")
        .replace("g: 0.5, b: 0.25", "g: 1, b: 1");
    let bytes = PackageBuilder::new()
        .asset_with_meta(MODEL, "Assets/S/Box.fbx", unipeek::unity::testing::fbx_ascii_cube("Box"), model_meta(MODEL, &[("Box", RED_MAT)]))
        .asset(RED_MAT, "Assets/S/Box.mat", mat)
        .asset(RED_TEX, "Assets/S/Red.png", png(16, 16, [255, 0, 0, 255]))
        .asset(PREFAB, "Assets/S/Box.prefab", prefab_instancing(MODEL, &[]))
        .build();
    Scene::load(input(bytes), &LoadOptions::default()).unwrap()
}

#[test]
fn liltoon_outline_is_drawn_around_the_silhouette() {
    let Some(gpu) = gpu("outline") else { return };
    let green = |img: &image::RgbaImage| img.pixels().filter(|p| p.0[1] > 150 && p.0[0] < 80 && p.0[3] > 200).count();
    let with = thumb::render_thumbnail(&gpu, &cube(LIL_OUTLINE), &opts(View::ThreeQuarter)).unwrap();
    let without = thumb::render_thumbnail(&gpu, &cube(LILTOON), &opts(View::ThreeQuarter)).unwrap();
    assert!(green(&with) > 50, "outline pixels: {}", green(&with));
    assert_eq!(green(&without), 0);
    let covered = |img: &image::RgbaImage| img.pixels().filter(|p| p.0[3] > 0).count();
    assert!(covered(&with) > covered(&without), "the outline grows the silhouette");
}

#[test]
fn transparent_variants_blend_and_opaque_ones_do_not() {
    let Some(gpu) = gpu("transparency") else { return };
    let glass = |shader: &str| {
        let bytes = PackageBuilder::new()
            .asset_with_meta(MODEL, "Assets/S/Glass.fbx", fbx_ascii(&["Glass"]), model_meta(MODEL, &[("Glass", RED_MAT)]))
            .asset(RED_MAT, "Assets/S/Glass.mat", material_yaml("Glass", shader, 4800000, Some(RED_TEX), -1).replace("g: 0.5, b: 0.25", "g: 1, b: 1"))
            .asset(RED_TEX, "Assets/S/Glass.png", png(16, 16, [255, 0, 0, 128]))
            .asset(PREFAB, "Assets/S/Glass.prefab", prefab_instancing(MODEL, &[]))
            .build();
        let scene = Scene::load(input(bytes), &LoadOptions::default()).unwrap();
        thumb::render_thumbnail(&gpu, &scene, &opts(View::Front)).unwrap()
    };
    let center = |img: &image::RgbaImage| img.get_pixel(64, 64).0;
    let [r, g, b, a] = center(&glass(LIL_TRANS));
    assert!((100..=160).contains(&a), "half transparent: alpha {a}");
    assert!(r > 150 && g < 60 && b < 60, "straight alpha keeps the color: {r} {g} {b}");
    assert_eq!(center(&glass(LILTOON))[3], 255, "opaque variants ignore texture alpha");
}

#[test]
fn gpu_fallback_is_recorded() {
    if gpu("fallback").is_none() {
        return;
    }
    let gpu = unipeek::Gpu::headless_from(&[unipeek::wgpu::Backends::empty(), unipeek::wgpu::Backends::all()]).unwrap();
    assert_eq!(gpu.fallback.len(), 1, "{:?}", gpu.fallback);
    assert!(!gpu.describe().is_empty());
    assert!(unipeek::Gpu::headless().unwrap().describe().contains(':'));
}

#[test]
fn one_prefab_inside_a_package_by_its_path() {
    let Some(gpu) = gpu("inner_prefab") else { return };
    // A second prefab paints both quads blue through an override of the model's renderer.
    let renderer = unipeek::unity::fileid::component(&unipeek::unity::fileid::node_path(&["Quads"], 1), "MeshRenderer");
    let mut bytes = Vec::new();
    let red = material_yaml("Red", LILTOON, 4800000, Some(RED_TEX), -1).replace("_Color: {r: 1, g: 0.5, b: 0.25, a: 1}", "_Color: {r: 1, g: 1, b: 1, a: 1}");
    let blue_mat = material_yaml("Blue", LILTOON, 4800000, Some(BLUE_TEX), -1).replace("_Color: {r: 1, g: 0.5, b: 0.25, a: 1}", "_Color: {r: 1, g: 1, b: 1, a: 1}");
    bytes.extend(
        PackageBuilder::new()
            .asset_with_meta(MODEL, "Assets/Shop/Quads.fbx", fbx_ascii(&["Red", "Blue"]), model_meta(MODEL, &[("Red", RED_MAT), ("Blue", BLUE_MAT)]))
            .asset(RED_MAT, "Assets/Shop/Red.mat", red)
            .asset(BLUE_MAT, "Assets/Shop/Blue.mat", blue_mat)
            .asset(RED_TEX, "Assets/Shop/Red.png", png(8, 8, [255, 0, 0, 255]))
            .asset(BLUE_TEX, "Assets/Shop/Blue.png", png(8, 8, [0, 0, 255, 255]))
            .asset(PREFAB, "Assets/Shop/Quads.prefab", prefab_instancing(MODEL, &[]))
            .asset("dddddddddddddddddddddddddddddddd", "Assets/Shop/Quads_Blue.prefab", prefab_instancing(MODEL, &[(renderer, 0, BLUE_MAT)]))
            .build(),
    );
    let dir = std::env::temp_dir().join(format!("unipeek-inner-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("Quads.unitypackage");
    std::fs::write(&file, &bytes).unwrap();
    let main = thumb::write_thumbnail(&gpu, &file.display().to_string(), &dir.join("main.png"), &opts(View::Front)).unwrap();
    let inner = thumb::write_thumbnail(&gpu, &format!("{}#Assets/Shop/Quads_Blue.prefab", file.display()), &dir.join("inner.png"), &opts(View::Front)).unwrap();
    assert_eq!(main.prefab.as_deref(), Some("Assets/Shop/Quads.prefab"));
    assert_eq!(inner.prefab.as_deref(), Some("Assets/Shop/Quads_Blue.prefab"));
    let (red_left, _, _, _) = colors(&image::open(dir.join("main.png")).unwrap().to_rgba8());
    let (red_left_inner, _, blue_left_inner, _) = colors(&image::open(dir.join("inner.png")).unwrap().to_rgba8());
    let _ = std::fs::remove_dir_all(&dir);
    assert!(red_left > 100 && red_left_inner == 0 && blue_left_inner > 100, "{red_left} {red_left_inner} {blue_left_inner}");
}

#[test]
fn turntable_and_view_sets_share_one_setup() {
    let Some(gpu) = gpu("turntable") else { return };
    let scene = Scene::load(input(two_quads()), &LoadOptions::default()).unwrap();
    let frames = thumb::render_turntable(&gpu, &scene, 64, Background::Transparent, 4).unwrap();
    assert_eq!(frames.len(), 4);
    assert_ne!(frames[0], frames[1], "the model turns");
    let views = thumb::render_views(&gpu, &scene, 224, Background::Transparent, &[View::Front, View::Side, View::Back, View::ThreeQuarter]).unwrap();
    assert_eq!(views.len(), 4);
    assert!(views.iter().all(|v| v.dimensions() == (224, 224)));
    let single = thumb::render_thumbnail(&gpu, &scene, &ThumbOptions { size: 224, view: View::Side, background: Background::Transparent }).unwrap();
    assert_eq!(views[1], single, "a view set renders the same pixels as single thumbnails");
}
