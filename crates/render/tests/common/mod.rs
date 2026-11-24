//! Synthetic inputs shared by the integration tests. No real asset ever goes in here.
#![allow(dead_code)]
use std::{io::Cursor, sync::Arc};
use unipeek::unity::{
    Input, Source,
    testing::{PackageBuilder, fbx_ascii, material_yaml, model_meta, prefab_instancing},
};

pub const MODEL: &str = "7078d86f15dd32d4bb4540ac2ff9e0ac";
pub const PREFAB: &str = "00715af155a8b554aa5d5ea437e05c8f";
pub const RED_MAT: &str = "c5898e34be6905c45a38f3f063d0172d";
pub const BLUE_MAT: &str = "00f0f13e2177deb409cf3f9658d0fb00";
pub const RED_TEX: &str = "4bbb1657292f0d74dab36e002aa5696d";
pub const BLUE_TEX: &str = "13f95bf558f0bbc45b85354fcd2de4eb";
pub const GONE_TEX: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
pub const LILTOON: &str = "df12117ecd77c31469c224178886498e";

pub fn png(w: u32, h: u32, rgba: [u8; 4]) -> Vec<u8> {
    let img = image::RgbaImage::from_pixel(w, h, image::Rgba(rgba));
    let mut out = Cursor::new(Vec::new());
    img.write_to(&mut out, image::ImageFormat::Png).unwrap();
    out.into_inner()
}

/// Two quads: `Red` (left, red texture) and `Blue` (right, blue texture), white material colors.
pub fn two_quads() -> Vec<u8> {
    let red = material_yaml("Red", LILTOON, 4800000, Some(RED_TEX), -1).replace("_Color: {r: 1, g: 0.5, b: 0.25, a: 1}", "_Color: {r: 1, g: 1, b: 1, a: 1}");
    let blue = material_yaml("Blue", LILTOON, 4800000, Some(BLUE_TEX), -1).replace("_Color: {r: 1, g: 0.5, b: 0.25, a: 1}", "_Color: {r: 1, g: 1, b: 1, a: 1}");
    PackageBuilder::new()
        .asset_with_meta(MODEL, "Assets/Shop/Quads.fbx", fbx_ascii(&["Red", "Blue"]), model_meta(MODEL, &[("Red", RED_MAT), ("Blue", BLUE_MAT)]))
        .asset(RED_MAT, "Assets/Shop/Red.mat", red)
        .asset(BLUE_MAT, "Assets/Shop/Blue.mat", blue)
        .asset(RED_TEX, "Assets/Shop/Red.png", png(64, 64, [255, 0, 0, 255]))
        .asset(BLUE_TEX, "Assets/Shop/Blue.png", png(4096, 16, [0, 0, 255, 255]))
        .asset(PREFAB, "Assets/Shop/Quads.prefab", prefab_instancing(MODEL, &[]))
        .build()
}

pub fn input(bytes: Vec<u8>) -> Input {
    Input::new(Source::Bytes(Arc::from(bytes)))
}

/// A GPU, or `None` with a `skipped:` line so machines without an adapter still pass.
pub fn gpu(test: &str) -> Option<unipeek::gpu::Gpu> {
    match unipeek::gpu::Gpu::headless() {
        Ok(g) => Some(g),
        Err(e) => {
            println!("skipped: {test}: no GPU adapter ({e})");
            None
        }
    }
}
