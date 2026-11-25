//! Zero-copy presentation into a window (Windows only), with the readback fallback.
#![cfg(windows)]
mod common;

use common::*;
use unipeek::{
    LoadOptions, Scene,
    surface::{SurfaceViewport, Target, ViewportKind},
};
use windows_sys::Win32::{
    System::LibraryLoader::GetModuleHandleW,
    UI::WindowsAndMessaging::{CreateWindowExW, DestroyWindow, WS_OVERLAPPEDWINDOW},
};

/// A hidden window of the predefined STATIC class, destroyed on drop.
struct Window(isize);

impl Window {
    fn new() -> Option<Window> {
        let class: Vec<u16> = "STATIC\0".encode_utf16().collect();
        // SAFETY: a plain top-level window with a predefined class; every pointer is valid or null.
        let hwnd = unsafe { CreateWindowExW(0, class.as_ptr(), std::ptr::null(), WS_OVERLAPPEDWINDOW, 0, 0, 320, 240, std::ptr::null_mut(), std::ptr::null_mut(), GetModuleHandleW(std::ptr::null()), std::ptr::null()) };
        (!hwnd.is_null()).then_some(Window(hwnd as isize))
    }
}

impl Drop for Window {
    fn drop(&mut self) {
        // SAFETY: the window was created by this test and is destroyed once.
        unsafe { DestroyWindow(self.0 as _) };
    }
}

#[test]
fn presents_into_a_window_and_redraws_only_on_change() {
    if gpu("surface").is_none() {
        return;
    }
    let Some(window) = Window::new() else {
        println!("skipped: surface: cannot create a window");
        return;
    };
    // SAFETY: the window outlives the viewport (dropped at the end of the test, after `view`).
    let mut view = match unsafe { SurfaceViewport::new(Target::Hwnd(window.0), 320, 240) } {
        Ok(v) => v,
        Err(e) => {
            println!("skipped: surface: no surface on this machine ({e})");
            return;
        }
    };
    view.set_scene(&Scene::load(input(two_quads()), &LoadOptions::default()).unwrap());
    assert!(view.render().unwrap(), "the first frame is presented");
    assert!(!view.render().unwrap(), "nothing changed");
    view.orbit().rotate(20.0, 0.0);
    assert!(view.render().unwrap());
    view.resize(200, 100);
    assert!(view.render().unwrap());
    drop(view);
}

#[test]
fn bad_targets_fall_back_to_readback() {
    let Some(gpu) = gpu("surface_fallback") else { return };
    // SAFETY: null handles are rejected before any use.
    assert!(unsafe { SurfaceViewport::new(Target::Hwnd(0), 64, 64) }.is_err());
    assert!(unsafe { SurfaceViewport::new(Target::CompositionVisual(std::ptr::null_mut()), 64, 64) }.is_err());
    match unsafe { ViewportKind::open(Target::Hwnd(0), &gpu, 64, 64) } {
        ViewportKind::Readback(mut view, why) => {
            assert!(why.contains("null"), "{why}");
            view.set_scene(Scene::load(input(two_quads()), &LoadOptions::default()).unwrap());
            assert_eq!(view.render().unwrap().unwrap().len(), 64 * 64 * 4);
        }
        ViewportKind::ZeroCopy(_) => panic!("a null window cannot present"),
    }
}
