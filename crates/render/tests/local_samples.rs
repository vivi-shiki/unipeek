//! Local sample set: real (paid) packages that never leave the developer's machine.
//!
//! Set `UNIPEEK_SAMPLES` to the folder that holds them and `unipeek-samples.json`, the manifest of
//! expected results. Without it every test here prints `skipped:` and passes. Sample names only
//! appear in the manifest and in local output, never in the repository.
//! `UNIPEEK_BIG=1` additionally streams a synthetic package of more than 3 GB.
mod common;

use serde_json::Value;
use std::{
    io::Write,
    path::{Path, PathBuf},
    time::Instant,
};
use unipeek::{
    LoadOptions, Scene,
    thumb::{self, Background, ThumbOptions, View},
    unity::{IndexOptions, Input, PackageIndex, Status},
};

fn samples(test: &str) -> Option<(PathBuf, Value)> {
    let Some(root) = std::env::var_os("UNIPEEK_SAMPLES").map(PathBuf::from) else {
        println!("skipped: {test}: UNIPEEK_SAMPLES is not set");
        return None;
    };
    let manifest = root.join("unipeek-samples.json");
    let text = std::fs::read_to_string(&manifest).unwrap_or_else(|e| panic!("{}: {e}", manifest.display()));
    Some((root, serde_json::from_str(&text).expect("valid manifest")))
}

fn location(root: &Path, rel: &str) -> String {
    root.join(rel).display().to_string()
}

fn status_name(s: Status) -> &'static str {
    match s {
        Status::Ok => "ok",
        Status::Degraded => "degraded",
        Status::Unsupported => "unsupported",
        Status::Error => "error",
    }
}

/// M1: every listed sample plans the same prefab, geometry, bindings and degrade reasons as recorded.
#[test]
fn reports_match_the_manifest() {
    let Some((root, manifest)) = samples("reports_match_the_manifest") else { return };
    let mut failures = Vec::new();
    for s in manifest["samples"].as_array().unwrap() {
        let rel = s["path"].as_str().unwrap();
        let e = &s["expect"];
        let t = Instant::now();
        let scene = match Scene::load(Input::from_location(&location(&root, rel)), &LoadOptions::default()) {
            Ok(scene) => scene,
            Err(err) => {
                failures.push(format!("{rel}: {err}"));
                continue;
            }
        };
        let r = &scene.report;
        let kinds: std::collections::BTreeSet<String> = r.degrades.iter().map(|d| serde_json::to_value(d).unwrap()["kind"].as_str().unwrap().to_string()).collect();
        let expected_kinds: std::collections::BTreeSet<String> = e["degrades"].as_array().unwrap().iter().map(|k| k.as_str().unwrap().to_string()).collect();
        let bound = r.materials.iter().filter(|m| serde_json::to_value(m.source).unwrap() != "unbound").count();
        let mut problems = Vec::new();
        if status_name(r.status) != e["status"] {
            problems.push(format!("status {:?} != {}", r.status, e["status"]));
        }
        if r.prefab.as_deref() != e["prefab"].as_str() {
            problems.push(format!("prefab {:?} != {}", r.prefab, e["prefab"]));
        }
        if scene.triangles() as u64 != e["triangles"].as_u64().unwrap() {
            problems.push(format!("triangles {} != {}", scene.triangles(), e["triangles"]));
        }
        if (bound as u64) < e["bound_materials_at_least"].as_u64().unwrap() {
            problems.push(format!("bound materials {bound} < {}", e["bound_materials_at_least"]));
        }
        if kinds != expected_kinds {
            problems.push(format!("degrades {kinds:?} != {expected_kinds:?}"));
        }
        if r.read.passes > 3 {
            problems.push(format!("read the input {} times", r.read.passes));
        }
        println!("local: {rel}: {} tris, {bound} bound, {:?} in {:?}", scene.triangles(), r.status, t.elapsed());
        if !problems.is_empty() {
            failures.push(format!("{rel}: {}", problems.join("; ")));
        }
    }
    assert!(failures.is_empty(), "{} sample(s) differ:\n{}", failures.len(), failures.join("\n"));
}

/// M1: path, bytes and one-shot reader inputs give the same report.
#[test]
fn sources_agree() {
    let Some((root, manifest)) = samples("sources_agree") else { return };
    for s in manifest["samples"].as_array().unwrap().iter().filter(|s| s["path"].as_str().unwrap().ends_with(".unitypackage")).take(3) {
        let rel = s["path"].as_str().unwrap();
        let path = root.join(rel);
        let bytes = std::fs::read(&path).unwrap();
        let from_path = Scene::load(Input::from_location(&location(&root, rel)), &LoadOptions::default()).unwrap();
        let from_bytes = Scene::load(Input::new(unipeek::unity::Source::Bytes(bytes.clone().into())), &LoadOptions::default()).unwrap();
        let from_reader = Scene::load(Input::new(unipeek::unity::Source::Reader(Box::new(std::io::Cursor::new(bytes)))), &LoadOptions::default()).unwrap();
        for other in [&from_bytes, &from_reader] {
            assert_eq!(other.report.prefab, from_path.report.prefab, "{rel}");
            assert_eq!(other.triangles(), from_path.triangles(), "{rel}");
            assert_eq!(other.report.materials, from_path.report.materials, "{rel}");
        }
        assert_eq!(from_reader.report.read.passes, 1, "{rel}: a one-shot reader is read once");
        println!("local: {rel}: path, bytes and reader agree");
    }
}

/// M1: everything in the folder opens without an error; only listed formats are unsupported.
#[test]
fn every_input_opens() {
    let Some((root, manifest)) = samples("every_input_opens") else { return };
    let unsupported: Vec<&str> = manifest["unsupported"].as_array().unwrap().iter().map(|v| v.as_str().unwrap()).collect();
    let mut seen = 0;
    let mut stack = vec![(root.clone(), 0)];
    while let Some((dir, depth)) = stack.pop() {
        for e in std::fs::read_dir(&dir).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                if depth < 3 {
                    stack.push((p, depth + 1));
                }
                continue;
            }
            let ext = p.extension().and_then(|x| x.to_str()).unwrap_or("").to_ascii_lowercase();
            if !["unitypackage", "zip", "fbx", "7z"].contains(&ext.as_str()) {
                continue;
            }
            let rel = p.strip_prefix(&root).unwrap().to_string_lossy().replace('\\', "/");
            seen += 1;
            match Scene::load(Input::from_location(&p.display().to_string()), &LoadOptions::default()) {
                Ok(_) => assert!(!unsupported.contains(&rel.as_str()), "{rel} is listed as unsupported but opened"),
                Err(e) => assert!(unsupported.contains(&rel.as_str()) && e.status() == Status::Unsupported, "{rel}: {e}"),
            }
        }
    }
    println!("local: {seen} input(s) opened");
}

/// M2: every sample with geometry renders a thumbnail that is neither empty nor all background.
#[test]
fn thumbnails_render() {
    let Some((root, manifest)) = samples("thumbnails_render") else { return };
    let Some(gpu) = common::gpu("thumbnails_render") else { return };
    let out = std::env::temp_dir().join(format!("unipeek-local-thumbs-{}", std::process::id()));
    std::fs::create_dir_all(&out).unwrap();
    for (i, s) in manifest["samples"].as_array().unwrap().iter().enumerate() {
        let rel = s["path"].as_str().unwrap();
        if !s["expect"]["thumbnail"].as_bool().unwrap() {
            continue;
        }
        let t = Instant::now();
        let file = out.join(format!("{i:02}.webp"));
        let opts = ThumbOptions { size: 256, view: View::ThreeQuarter, background: Background::Transparent };
        let report = thumb::write_thumbnail(&gpu, &location(&root, rel), &file, &opts).unwrap_or_else(|e| panic!("{rel}: {e}"));
        let img = image::open(&file).unwrap().to_rgba8();
        let covered = img.pixels().filter(|p| p.0[3] > 0).count() as f64 / (256.0 * 256.0);
        assert!(covered > 0.002, "{rel}: only {:.3}% of the thumbnail is covered", covered * 100.0);
        println!("local: {rel}: thumbnail {:.1}% covered, {:?}, in {:?} -> {}", covered * 100.0, report.status, t.elapsed(), file.display());
    }
}

/// M2: the serve loop handles every sample in one process without leaking a failure into the next.
#[test]
fn serve_runs_the_whole_set() {
    let Some((root, manifest)) = samples("serve_runs_the_whole_set") else { return };
    if common::gpu("serve_runs_the_whole_set").is_none() {
        return;
    }
    let out = std::env::temp_dir().join(format!("unipeek-local-serve-{}", std::process::id()));
    let mut requests = String::new();
    let list = manifest["samples"].as_array().unwrap();
    for (i, s) in list.iter().enumerate() {
        let line = serde_json::json!({"id": i, "op": "thumb", "source": location(&root, s["path"].as_str().unwrap()), "out": out.join(format!("{i:02}.png")).display().to_string(), "size": 128});
        requests += &format!("{line}\n");
    }
    let mut replies = Vec::new();
    let t = Instant::now();
    assert_eq!(unipeek::serve::serve(requests.as_bytes(), &mut replies), 0);
    let replies: Vec<Value> = String::from_utf8(replies).unwrap().lines().map(|l| serde_json::from_str(l).unwrap()).collect();
    assert_eq!(replies.len(), list.len());
    for (s, r) in list.iter().zip(&replies) {
        let has_geometry = s["expect"]["thumbnail"].as_bool().unwrap();
        let expected = if has_geometry { s["expect"]["status"].as_str().unwrap() } else { "unsupported" };
        assert_eq!(r["status"], expected, "{}: {r}", s["path"]);
    }
    println!("local: serve answered {} request(s) in {:?}", replies.len(), t.elapsed());
    let _ = std::fs::remove_dir_all(&out);
}

/// M1: a package of more than 3 GB is indexed in one streaming pass. Opt in with `UNIPEEK_BIG=1`.
#[test]
fn big_package_streams() {
    if std::env::var("UNIPEEK_BIG").is_err() {
        println!("skipped: big_package_streams: UNIPEEK_BIG is not set");
        return;
    }
    const PART: u64 = 1100 << 20;
    let (reader, writer) = std::io::pipe().unwrap();
    let producer = std::thread::spawn(move || {
        let gz = flate2::write::GzEncoder::new(writer, flate2::Compression::fast());
        let mut tar = tar::Builder::new(gz);
        for i in 0..3u32 {
            let guid = format!("{i:032x}");
            let mut header = tar::Header::new_gnu();
            header.set_size(PART);
            header.set_mode(0o644);
            header.set_cksum();
            let body = std::io::Read::chain(&b"Kaydara FBX Binary  \x00"[..], std::io::repeat(0)).take_exact(PART);
            tar.append_data(&mut header, format!("{guid}/asset"), body).unwrap();
            let path = format!("Assets/Big/Part{i}.fbx");
            let mut h = tar::Header::new_gnu();
            h.set_size(path.len() as u64);
            h.set_mode(0o644);
            h.set_cksum();
            tar.append_data(&mut h, format!("{guid}/pathname"), path.as_bytes()).unwrap();
        }
        tar.into_inner().unwrap().finish().unwrap().flush().unwrap();
    });
    let t = Instant::now();
    let index = PackageIndex::read(std::io::BufReader::new(reader), &IndexOptions::default()).unwrap();
    producer.join().unwrap();
    assert_eq!(index.len(), 3);
    assert!(index.entries().all(|e| e.size == Some(PART) && e.bytes.is_none()), "big assets are measured, never kept");
    println!("local: indexed {} GB in {:?}", 3 * PART >> 30, t.elapsed());
}

trait TakeExact: std::io::Read + Sized {
    fn take_exact(self, n: u64) -> std::io::Take<Self> {
        self.take(n)
    }
}

impl<R: std::io::Read> TakeExact for R {}
