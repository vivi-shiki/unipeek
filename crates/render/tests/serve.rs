//! The line protocol a host runs in a child process (`snowcase.exe --thumb-worker`).
mod common;

use common::*;
use serde_json::Value;
use std::path::PathBuf;

struct Dir(PathBuf);

impl Dir {
    fn new(name: &str) -> Dir {
        let p = std::env::temp_dir().join(format!("unipeek-serve-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        Dir(p)
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn run(lines: &[String]) -> (i32, Vec<Value>) {
    let input = lines.join("\n") + "\n";
    let mut out = Vec::new();
    let code = unipeek::serve::serve(input.as_bytes(), &mut out);
    let text = String::from_utf8(out).unwrap();
    (code, text.lines().map(|l| serde_json::from_str(l).unwrap_or_else(|e| panic!("{e}: {l}"))).collect())
}

fn json(v: Value) -> String {
    serde_json::to_string(&v).unwrap()
}

#[test]
fn info_version_and_bad_lines() {
    let dir = Dir::new("info");
    let pkg = dir.0.join("Quads.unitypackage");
    std::fs::write(&pkg, two_quads()).unwrap();
    let (code, replies) = run(&[
        json(serde_json::json!({"id": 1, "op": "info", "source": pkg.display().to_string()})),
        String::new(),
        "this is not json".into(),
        json(serde_json::json!({"id": "v", "op": "version"})),
        json(serde_json::json!({"id": 3, "op": "info", "source": dir.0.join("missing.unitypackage").display().to_string()})),
    ]);
    assert_eq!(code, 0);
    assert_eq!(replies.len(), 4, "empty lines get no reply: {replies:?}");
    assert_eq!(replies[0]["id"], 1);
    assert_eq!(replies[0]["status"], "ok");
    assert_eq!(replies[0]["prefab"], "Assets/Shop/Quads.prefab");
    assert_eq!(replies[0]["report"]["models"][0]["triangles"], 4);
    assert_eq!(replies[1]["status"], "error");
    assert!(replies[1]["error"].as_str().unwrap().contains("json"), "{:?}", replies[1]);
    assert_eq!(replies[2]["id"], "v");
    assert_eq!(replies[2]["renderer_version"], unipeek::thumb::RENDERER_VERSION);
    assert!(replies[2]["version"].as_str().is_some());
    assert_eq!(replies[3]["id"], 3);
    assert_eq!(replies[3]["status"], "error");
}

#[test]
fn thumb_writes_the_file_and_reports_degrades() {
    let dir = Dir::new("thumb");
    let pkg = dir.0.join("Quads.unitypackage");
    std::fs::write(&pkg, two_quads()).unwrap();
    let out = dir.0.join("thumbs/ab/quads.webp");
    let bad = dir.0.join("quads.bmp");
    let (_, replies) = run(&[
        json(serde_json::json!({"op": "thumb", "source": pkg.display().to_string(), "out": out.display().to_string(), "size": 64, "view": "front"})),
        json(serde_json::json!({"op": "thumb", "source": pkg.display().to_string(), "out": bad.display().to_string()})),
        json(serde_json::json!({"op": "thumb", "source": pkg.display().to_string(), "out": out.display().to_string(), "view": "top"})),
    ]);
    assert_eq!(replies[1]["status"], "error", "only .png and .webp: {:?}", replies[1]);
    assert_eq!(replies[2]["status"], "error", "unknown view: {:?}", replies[2]);
    if gpu("serve thumb").is_none() {
        assert_eq!(replies[0]["status"], "error");
        return;
    }
    assert_eq!(replies[0]["status"], "ok", "{:?}", replies[0]);
    assert_eq!(replies[0]["out"], out.display().to_string());
    assert_eq!(replies[0]["prefab"], "Assets/Shop/Quads.prefab");
    assert!(replies[0]["reasons"].as_array().unwrap().is_empty());
    assert!(replies[0]["backend"].as_str().is_some_and(|b| !b.is_empty()), "which GPU rendered it: {:?}", replies[0]);
    assert!(replies[0]["fallback"].is_array());
    let img = image::open(&out).unwrap();
    assert_eq!((img.width(), img.height()), (64, 64));
    assert!(!dir.0.join("thumbs/ab/quads.webp.tmp").exists(), "written through a temporary file");
}
