//! The `unipeek` command line: output and exit codes (0 ok, 1 degraded, 2 unsupported, 3 error, 64 usage).
mod common;

use common::*;
use std::{path::PathBuf, process::Command};
use unipeek::unity::testing::{PackageBuilder, fbx_ascii, prefab_instancing};

struct Dir(PathBuf);

impl Dir {
    fn new(name: &str) -> Dir {
        let p = std::env::temp_dir().join(format!("unipeek-cli-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        Dir(p)
    }

    fn file(&self, name: &str, bytes: &[u8]) -> String {
        let p = self.0.join(name);
        std::fs::write(&p, bytes).unwrap();
        p.display().to_string()
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn unipeek(args: &[&str]) -> (i32, String, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_unipeek")).args(args).output().unwrap();
    (out.status.code().unwrap_or(-1), String::from_utf8_lossy(&out.stdout).into_owned(), String::from_utf8_lossy(&out.stderr).into_owned())
}

#[test]
fn info_json_and_text() {
    let dir = Dir::new("info");
    let pkg = dir.file("Quads.unitypackage", &two_quads());
    let (code, out, err) = unipeek(&["info", &pkg, "--json"]);
    assert_eq!(code, 0, "{err}");
    let report: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(report["prefab"], "Assets/Shop/Quads.prefab");
    assert_eq!(report["status"], "ok");
    let (code, out, _) = unipeek(&["info", &pkg]);
    assert_eq!(code, 0);
    assert!(out.contains("Assets/Shop/Quads.prefab") && out.contains("triangles"), "{out}");
}

#[test]
fn ls_lists_paths() {
    let dir = Dir::new("ls");
    let pkg = dir.file("Quads.unitypackage", &two_quads());
    let (code, out, _) = unipeek(&["ls", &pkg]);
    assert_eq!(code, 0);
    assert!(out.contains("Assets/Shop/Quads.fbx") && out.contains("Assets/Shop/Red.png"), "{out}");
}

#[test]
fn degraded_unsupported_error_and_usage_codes() {
    let dir = Dir::new("codes");
    let degraded = PackageBuilder::new()
        .asset(MODEL, "Assets/S/Quads.fbx", fbx_ascii(&["Skin"]))
        .asset(PREFAB, "Assets/S/Quads.prefab", prefab_instancing(MODEL, &[]))
        .build();
    let degraded = dir.file("Degraded.unitypackage", &degraded);
    assert_eq!(unipeek(&["info", &degraded]).0, 1);
    let text = dir.file("notes.txt", b"hello");
    assert_eq!(unipeek(&["info", &text]).0, 2);
    assert_eq!(unipeek(&["info", &dir.0.join("missing.unitypackage").display().to_string()]).0, 3);
    assert_eq!(unipeek(&[]).0, 64);
    assert_eq!(unipeek(&["frobnicate", &degraded]).0, 64);
    assert_eq!(unipeek(&["thumb", &degraded]).0, 64, "thumb needs -o");
}

#[test]
fn thumb_writes_an_image() {
    let dir = Dir::new("thumb");
    let pkg = dir.file("Quads.unitypackage", &two_quads());
    let out = dir.0.join("quads.png");
    let (code, _, err) = unipeek(&["thumb", &pkg, "-o", &out.display().to_string(), "--size", "48", "--view", "front", "--bg", "#102030"]);
    if gpu("cli thumb").is_none() {
        assert_eq!(code, 3, "{err}");
        return;
    }
    assert_eq!(code, 0, "{err}");
    let img = image::open(&out).unwrap().to_rgba8();
    assert_eq!(img.dimensions(), (48, 48));
    assert_eq!(img.get_pixel(0, 0).0, [0x10, 0x20, 0x30, 255]);
}

#[test]
fn view_opens_a_window_or_says_why_not() {
    let dir = Dir::new("view");
    let pkg = dir.file("Quads.unitypackage", &two_quads());
    let out = Command::new(env!("CARGO_BIN_EXE_unipeek")).args(["view", &pkg]).env("UNIPEEK_VIEW_SECS", "2").output().unwrap();
    let err = String::from_utf8_lossy(&out.stderr);
    match out.status.code() {
        Some(0) => assert!(err.contains("view: ") && err.contains("4 triangles, textures loaded: true"), "{err}"),
        // Machines without a display or GPU cannot open the window; that is an error, not a crash.
        Some(3) => {
            assert!(err.contains("error: "), "{err}");
            println!("skipped: view_opens_a_window_or_says_why_not: {err}");
        }
        other => panic!("exit {other:?}: {err}"),
    }
    let (code, _, err) = unipeek(&["view"]);
    assert_eq!(code, 64, "{err}");
}
