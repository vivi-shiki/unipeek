//! The interactive viewport: offscreen BGRA frames, redraw only on change, background loading.
mod common;

use common::*;
use unipeek::{
    LoadOptions, Loaded, Loading, Scene, Viewport,
    camera::Orbit,
    glam::Vec3,
    thumb::{Background, View},
};

fn center_bgra(frame: &[u8], w: u32, x: u32, y: u32) -> [u8; 4] {
    let i = ((y * w + x) * 4) as usize;
    frame[i..i + 4].try_into().unwrap()
}

#[test]
fn frames_are_bgra_and_only_redrawn_on_change() {
    let Some(gpu) = gpu("viewport") else { return };
    let mut view = Viewport::offscreen(&gpu, 64, 48);
    assert!(view.render().unwrap().is_none(), "nothing to show yet");
    view.set_scene(Scene::load(input(two_quads()), &LoadOptions::default()).unwrap());
    view.set_background(Background::Color([0, 255, 0, 255]));
    view.orbit().preset(View::Front);
    let frame = view.render().unwrap().expect("first frame").to_vec();
    assert_eq!(frame.len(), 64 * 48 * 4);
    assert_eq!(center_bgra(&frame, 64, 0, 0), [0, 255, 0, 255], "green background, BGRA");
    let left = center_bgra(&frame, 64, 22, 24);
    assert!(left[2] > 150 && left[0] < 60, "the red quad is red in BGRA order: {left:?}");
    assert!(view.render().unwrap().is_none(), "unchanged camera: no redraw");

    view.orbit().rotate(30.0, 0.0);
    assert!(view.render().unwrap().is_some(), "camera moved");
    view.resize(32, 32);
    assert_eq!(view.render().unwrap().unwrap().len(), 32 * 32 * 4);
    view.invalidate();
    assert!(view.render().unwrap().is_some());
}

#[test]
fn loading_shows_the_untextured_model_first() {
    let mut loading = Loading::start(input(two_quads()), LoadOptions::default());
    let first = loading.wait().unwrap();
    let Loaded::Untextured(white) = first else { panic!("untextured first") };
    assert!(white.materials.iter().all(|m| m.texture.is_none()));
    assert_eq!(white.triangles(), 4);
    let Loaded::Textured(done) = loading.wait().unwrap() else { panic!("then textured") };
    assert!(done.materials.iter().all(|m| m.texture.is_some()));
    assert!(loading.is_done());
    assert_eq!(loading.progress(), 1.0);

    let Some(gpu) = gpu("viewport_load") else { return };
    let mut view = Viewport::offscreen(&gpu, 32, 32);
    view.load(input(two_quads()), LoadOptions::default());
    let t = std::time::Instant::now();
    while view.progress().is_some() && t.elapsed().as_secs() < 30 {
        view.update();
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    view.update();
    assert!(view.scene().unwrap().materials.iter().all(|m| m.texture.is_some()));
    assert_eq!(view.report().unwrap().prefab.as_deref(), Some("Assets/Shop/Quads.prefab"));
    assert!(view.render().unwrap().is_some());

    view.load(input(b"not a package".to_vec()), LoadOptions::default());
    while view.progress().is_some() {
        view.update();
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert!(view.error().is_some(), "a failed load is reported, the old scene stays");
    assert!(view.scene().is_some());
}

#[test]
fn orbit_reset_and_presets_frame_the_model() {
    let mut o = Orbit::fit(Vec3::new(-1.0, 0.0, -1.0), Vec3::new(1.0, 2.0, 1.0));
    let home = o;
    o.rotate(100.0, 40.0);
    o.pan(30.0, 10.0);
    o.zoom(3.0);
    assert_ne!(o, home);
    o.reset();
    assert_eq!(o, home);
    o.preset(View::Back);
    assert!((o.yaw - std::f32::consts::PI).abs() < 1e-6);
    assert_eq!((o.target, o.distance), home.home);
    o.zoom(1000.0);
    assert!(o.distance >= o.radius * 0.2, "zoom is clamped");
}
