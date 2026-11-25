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

/// The 3 GB test measures this process's memory, so it runs alone: it takes the write lock,
/// every other test here a read lock.
static EXCLUSIVE: std::sync::RwLock<()> = std::sync::RwLock::new(());

fn shared() -> std::sync::RwLockReadGuard<'static, ()> {
    EXCLUSIVE.read().unwrap_or_else(|e| e.into_inner())
}

fn samples(test: &str) -> Option<(PathBuf, Value)> {
    let Some(root) = std::env::var_os("UNIPEEK_SAMPLES").filter(|v| !v.is_empty()).map(PathBuf::from) else {
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
    let _shared = shared();
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
    let _shared = shared();
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
    let _shared = shared();
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
    let _shared = shared();
    let Some((root, manifest)) = samples("thumbnails_render") else { return };
    let Some(gpu) = common::gpu("thumbnails_render") else { return };
    let second = unipeek::gpu::Gpu::headless().unwrap();
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
        // Same input, same RENDERER_VERSION, another device: the same pixels.
        let again = thumb::write_thumbnail(&second, &location(&root, rel), &out.join(format!("{i:02}-again.webp")), &opts).unwrap();
        assert_eq!(again.status, report.status);
        // Alpha-blended parts differ slightly between renders, even on one device: on two samples
        // a few dozen pixels (under 0.1%) change by up to about 10. With every material forced
        // opaque the renders are identical. More than 0.2% of the pixels is a real difference.
        let other = image::open(out.join(format!("{i:02}-again.webp"))).unwrap().to_rgba8();
        let diffs: Vec<u8> = img.pixels().zip(other.pixels()).filter(|(a, b)| a != b).map(|(a, b)| (0..4).map(|c| a.0[c].abs_diff(b.0[c])).max().unwrap()).collect();
        if !diffs.is_empty() {
            println!("local: {rel}: second render: {} pixel(s) differ by at most {}", diffs.len(), diffs.iter().max().unwrap());
        }
        assert!(diffs.len() <= 128 && diffs.iter().all(|&d| d <= 16), "{rel}: a second render differs in {} pixel(s)", diffs.len());
        let covered = img.pixels().filter(|p| p.0[3] > 0).count() as f64 / (256.0 * 256.0);
        assert!(covered > 0.002, "{rel}: only {:.3}% of the thumbnail is covered", covered * 100.0);
        println!("local: {rel}: thumbnail {:.1}% covered, {:?}, in {:?} -> {}", covered * 100.0, report.status, t.elapsed(), file.display());
    }
}

/// M2: the serve loop handles every sample in one process without leaking a failure into the next.
#[test]
fn serve_runs_the_whole_set() {
    let _shared = shared();
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
    let _alone = EXCLUSIVE.write().unwrap_or_else(|e| e.into_inner());
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
    let baseline = working_set();
    let done = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let watcher = {
        let done = done.clone();
        std::thread::spawn(move || {
            let mut peak = 0u64;
            while !done.load(std::sync::atomic::Ordering::Relaxed) {
                peak = peak.max(working_set().unwrap_or(0));
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
            peak
        })
    };
    let index = PackageIndex::read(std::io::BufReader::new(reader), &IndexOptions::default()).unwrap();
    producer.join().unwrap();
    done.store(true, std::sync::atomic::Ordering::Relaxed);
    let peak = watcher.join().unwrap();
    assert_eq!(index.len(), 3);
    assert!(index.entries().all(|e| e.size == Some(PART) && e.bytes.is_none()), "big assets are measured, never kept");
    match baseline {
        Some(base) => {
            let grew = peak.saturating_sub(base) >> 20;
            println!("local: indexed {} GB in {:?}, working set grew {grew} MB", 3 * PART >> 30, t.elapsed());
            assert!(grew < 256, "memory must not follow package size: grew {grew} MB while streaming 3 GB");
        }
        None => println!("local: indexed {} GB in {:?} (memory not measured on this platform)", 3 * PART >> 30, t.elapsed()),
    }
}

/// M2: `serve` over the whole set, three times in one process: committed memory after the first
/// round (caches and the device warmed up) must not keep growing.
#[test]
fn serve_memory_stays_flat() {
    let Some((root, manifest)) = samples("serve_memory_stays_flat") else { return };
    let _alone = EXCLUSIVE.write().unwrap_or_else(|e| e.into_inner());
    if common::gpu("serve_memory_stays_flat").is_none() {
        return;
    }
    let out = std::env::temp_dir().join(format!("unipeek-local-leak-{}", std::process::id()));
    let mut requests = String::new();
    for (i, s) in manifest["samples"].as_array().unwrap().iter().enumerate() {
        let line = serde_json::json!({"id": i, "op": "thumb", "source": location(&root, s["path"].as_str().unwrap()), "out": out.join(format!("{i:02}.webp")).display().to_string(), "size": 256});
        requests += &format!("{line}\n");
        requests += &format!("{}\n", serde_json::json!({"id": i, "op": "info", "source": location(&root, s["path"].as_str().unwrap())}));
    }
    // One serve loop for all rounds, as in a host's long-lived child process.
    let rounds = 3;
    let all = requests.repeat(rounds);
    let (reader, mut writer) = std::io::pipe().unwrap();
    let feeder = std::thread::spawn(move || writer.write_all(all.as_bytes()));
    let per_round = requests.lines().count();
    let mut sink = RoundCounter { per_round, seen: 0, after: Vec::new() };
    let t = Instant::now();
    assert_eq!(unipeek::serve::serve(std::io::BufReader::new(reader), &mut sink), 0);
    feeder.join().unwrap().unwrap();
    let _ = std::fs::remove_dir_all(&out);
    let mb: Vec<u64> = sink.after.iter().map(|b| b >> 20).collect();
    println!("local: serve answered {} requests in {:?}; committed MB after each round: {mb:?}", per_round * rounds, t.elapsed());
    if let [first, .., last] = sink.after[..] {
        let grew = last.saturating_sub(first) >> 20;
        assert!(grew < 64, "serve keeps {grew} MB more after {rounds} rounds than after the first: {mb:?}");
    }
}

/// Counts reply lines and records committed memory at the end of each round.
struct RoundCounter {
    per_round: usize,
    seen: usize,
    after: Vec<u64>,
}

impl Write for RoundCounter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.seen += buf.iter().filter(|&&b| b == b'\n').count();
        if self.seen > 0 && self.seen % self.per_round == 0 && self.after.len() < self.seen / self.per_round {
            self.after.extend(committed());
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Committed (private) bytes of this process (Windows only).
fn committed() -> Option<u64> {
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::{ProcessStatus::{K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS}, Threading::GetCurrentProcess};
        let mut c: PROCESS_MEMORY_COUNTERS = unsafe { std::mem::zeroed() };
        c.cb = size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
        // SAFETY: the pseudo handle of the current process is always valid and `c` is sized for the call.
        let ok = unsafe { K32GetProcessMemoryInfo(GetCurrentProcess(), &mut c, c.cb) };
        (ok != 0).then_some(c.PagefileUsage as u64)
    }
    #[cfg(not(windows))]
    {
        None
    }
}

/// Current working set of this process in bytes (Windows only).
fn working_set() -> Option<u64> {
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::{ProcessStatus::{K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS}, Threading::GetCurrentProcess};
        let mut c: PROCESS_MEMORY_COUNTERS = unsafe { std::mem::zeroed() };
        c.cb = size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
        // SAFETY: the pseudo handle of the current process is always valid and `c` is sized for the call.
        let ok = unsafe { K32GetProcessMemoryInfo(GetCurrentProcess(), &mut c, c.cb) };
        (ok != 0).then_some(c.WorkingSetSize as u64)
    }
    #[cfg(not(windows))]
    {
        None
    }
}

/// M1 done criterion: material binding through prefabs, `.meta` and `.mat` finds at least as many
/// textures as the lab's old rule (a PNG whose name ends with the FBX material name), listed per sample.
#[test]
fn bindings_beat_name_matching() {
    let _shared = shared();
    let Some((root, manifest)) = samples("bindings_beat_name_matching") else { return };
    let (mut new_total, mut old_total, mut slots_total) = (0, 0, 0);
    let mut table = vec!["| sample | slots | textured now | textured by name only |".to_string(), "|---|---|---|---|".to_string()];
    for s in manifest["samples"].as_array().unwrap() {
        let rel = s["path"].as_str().unwrap();
        if !rel.ends_with(".unitypackage") {
            continue;
        }
        let scene = Scene::load(Input::from_location(&location(&root, rel)), &LoadOptions::default()).unwrap();
        let (pkg, _) = unipeek::unity::Package::open(Input::from_location(&location(&root, rel)), &IndexOptions::default()).unwrap();
        let pngs: Vec<String> = pkg.index.entries().filter(|e| e.extension().as_deref() == Some("png")).filter_map(|e| e.stem().map(str::to_lowercase)).collect();
        let slots = scene.report.materials.len();
        let now = scene.report.materials.iter().filter(|m| m.texture.is_some()).count();
        let old = scene.report.materials.iter().filter(|m| !m.slot.is_empty() && pngs.iter().any(|p| p.ends_with(&m.slot.to_lowercase()))).count();
        table.push(format!("| {rel} | {slots} | {now} | {old} |"));
        (new_total, old_total, slots_total) = (new_total + now, old_total + old, slots_total + slots);
    }
    table.push(format!("| total | {slots_total} | {new_total} | {old_total} |"));
    for line in &table {
        println!("local: {line}");
    }
    assert!(new_total >= old_total, "binding found {new_total} textures, name matching {old_total}");
}

trait TakeExact: std::io::Read + Sized {
    fn take_exact(self, n: u64) -> std::io::Take<Self> {
        self.take(n)
    }
}

impl<R: std::io::Read> TakeExact for R {}

/// PSD composites: every `.psd` / `.psb` next to the samples (Unity's `Library/` excluded) decodes,
/// unless the manifest lists it under `psd_unreadable` with the expected reason.
#[test]
fn psd_composites_decode() {
    let _shared = shared();
    let Some((root, manifest)) = samples("psd_composites_decode") else { return };
    let unreadable: Vec<(&str, &str)> = manifest["psd_unreadable"]
        .as_array()
        .map(|a| a.iter().map(|v| (v["path"].as_str().unwrap(), v["reason"].as_str().unwrap())).collect())
        .unwrap_or_default();
    let mut found = Vec::new();
    let mut stack = vec![(root.clone(), 0)];
    while let Some((dir, depth)) = stack.pop() {
        for e in std::fs::read_dir(&dir).unwrap().flatten() {
            let p = e.path();
            let name = e.file_name().to_string_lossy().to_lowercase();
            if p.is_dir() {
                if depth < 6 && name != "library" {
                    stack.push((p, depth + 1));
                }
            } else if name.ends_with(".psd") || name.ends_with(".psb") {
                found.push(p);
            }
        }
    }
    found.sort();
    let mut failures = Vec::new();
    for p in &found {
        let rel = p.strip_prefix(&root).unwrap().to_string_lossy().replace('\\', "/");
        let bytes = std::fs::read(p).unwrap();
        let t = Instant::now();
        let expected = unreadable.iter().find(|(path, _)| *path == rel).map(|(_, r)| *r);
        match (unipeek::psd::decode(&bytes), expected) {
            (Ok(c), None) => {
                println!("local: {rel}: {}x{} {:?} in {:?}", c.image.width(), c.image.height(), c.note, t.elapsed());
                // Shops often ship the same texture exported as PNG in a sibling `PNG/` folder.
                let stem = p.file_stem().unwrap().to_string_lossy().into_owned();
                let png = p.parent().and_then(Path::parent).map(|d| d.join("PNG").join(format!("{stem}.png")));
                if let Some(png) = png.filter(|p| p.is_file()) {
                    let png = image::open(&png).unwrap().to_rgba8();
                    if png.dimensions() == c.image.dimensions() {
                        let (mut sum, mut n) = (0u64, 0u64);
                        for (a, b) in png.pixels().zip(c.image.pixels()).filter(|(a, _)| a.0[3] == 255) {
                            sum += (0..3).map(|i| (a.0[i] as i64 - b.0[i] as i64).unsigned_abs()).sum::<u64>();
                            n += 3;
                        }
                        let mean = sum as f64 / n.max(1) as f64;
                        println!("local: {rel}: mean difference to the PNG export {mean:.2}");
                        if mean > 8.0 {
                            failures.push(format!("{rel}: composite differs from the PNG export by {mean:.2} on average"));
                        }
                    }
                }
            }
            (Err(e), Some(reason)) if e.contains(reason) => println!("local: {rel}: unreadable as expected: {e}"),
            (Ok(_), Some(reason)) => failures.push(format!("{rel}: decoded, but the manifest expects {reason:?}")),
            (Err(e), _) => failures.push(format!("{rel}: {e}")),
        }
    }
    println!("local: {} PSD file(s)", found.len());
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// M4: in a 690² viewport (the 460 px drawer at 150%), the untextured model of the biggest sample
/// appears within a second and each dragged frame takes at most 5 ms to render and read back.
/// The limits are only checked in release builds.
#[test]
fn viewport_speed() {
    let _shared = shared();
    let Some((root, manifest)) = samples("viewport_speed") else { return };
    let Some(gpu) = common::gpu("viewport_speed") else { return };
    let biggest = manifest["samples"].as_array().unwrap().iter().filter(|s| s["path"].as_str().unwrap().ends_with(".unitypackage")).max_by_key(|s| s["expect"]["triangles"].as_u64().unwrap()).unwrap();
    let rel = biggest["path"].as_str().unwrap();
    let mut view = unipeek::Viewport::offscreen(&gpu, 690, 690);
    let t = Instant::now();
    view.load(Input::from_location(&location(&root, rel)), LoadOptions { max_texture: 1024, ..Default::default() });
    let mut first = None;
    while view.progress().is_some() {
        if view.update() && first.is_none() {
            view.render().unwrap();
            first = Some(t.elapsed());
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    view.update();
    let textured = t.elapsed();
    let first = first.expect("the untextured model arrives before the textures");
    view.render().unwrap();
    let frames = 120;
    let t = Instant::now();
    for _ in 0..frames {
        view.orbit().rotate(4.0, 0.5);
        assert!(view.render().unwrap().is_some());
    }
    let per_frame = t.elapsed().as_secs_f64() * 1e3 / frames as f64;
    println!("local: viewport {rel}: untextured on screen in {first:?}, textured in {textured:?}, {per_frame:.2} ms per 690x690 frame");
    if cfg!(not(debug_assertions)) {
        assert!(first.as_secs_f64() <= 1.0, "first frame after {first:?}");
        assert!(per_frame <= 5.0, "{per_frame:.2} ms per frame");
    }
}
