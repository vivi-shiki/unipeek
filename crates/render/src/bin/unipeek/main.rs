//! The `unipeek` command line. Exit codes: 0 ok, 1 degraded, 2 unsupported, 3 error, 64 usage.
#[cfg(feature = "view")]
mod view;

use std::{process::ExitCode, sync::Arc};
use unipeek::{
    LoadOptions, Scene,
    gpu::Gpu,
    thumb::{self, Background, ThumbOptions, View},
    unity::{IndexOptions, Input, Package, Report, Resolver, Sources, Status},
};

const USAGE: &str = "usage:
  unipeek info  <input> [--json] [--with <package or folder>]...
  unipeek ls    <input>
  unipeek thumb <input> -o <out.png|out.webp> [--size 512] [--view front|side|back|3q] [--bg transparent|#rrggbb[aa]] [--timings] [--with <package or folder>]...
  unipeek turn  <input> -o <folder> [--frames 24] [--size 512] [--format webp|png] [--bg ...] [--with <package or folder>]...
  unipeek view  <input> [--with <package or folder>]...
  unipeek serve
  unipeek version

<input> is a .unitypackage, .zip, folder or .fbx, optionally followed by #<path inside>,
e.g. Avatar.unitypackage#Assets/Shop/Avatar.prefab";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = match run(&args) {
        Ok(code) => code,
        Err(Failure::Usage(msg)) => {
            eprintln!("{msg}\n\n{USAGE}");
            64
        }
        Err(Failure::Run(e)) => {
            eprintln!("error: {e}");
            e.status().exit_code()
        }
    };
    ExitCode::from(code as u8)
}

enum Failure {
    Usage(String),
    Run(unipeek::Error),
}

impl From<unipeek::Error> for Failure {
    fn from(e: unipeek::Error) -> Self {
        Failure::Run(e)
    }
}

impl From<unipeek::unity::Error> for Failure {
    fn from(e: unipeek::unity::Error) -> Self {
        Failure::Run(e.into())
    }
}

fn run(args: &[String]) -> Result<i32, Failure> {
    let Some((command, rest)) = args.split_first() else { return Err(Failure::Usage("missing command".into())) };
    match command.as_str() {
        "info" => {
            let (input, flags) = input_and_flags(rest)?;
            let (with, flags) = with_sources(flags)?;
            if let Some(f) = flags.iter().find(|f| *f != "--json") {
                return Err(Failure::Usage(format!("unknown option {f}")));
            }
            let json = flags.iter().any(|f| f == "--json");
            let scene = Scene::load(Input::from_location(input), &LoadOptions { decode_textures: false, resolver: with, ..Default::default() })?;
            if json {
                println!("{}", serde_json::to_string_pretty(&scene.report).unwrap_or_default());
            } else {
                print_report(&scene.report);
            }
            Ok(status_code(&scene.report))
        }
        "ls" => {
            let (input, _) = input_and_flags(rest)?;
            let (pkg, _) = Package::open(Input::from_location(input), &IndexOptions::default())?;
            let mut entries: Vec<_> = pkg.index.entries().filter_map(|e| Some((e.pathname.clone()?, e.kind, e.size))).collect();
            entries.sort_by(|a, b| a.0.cmp(&b.0));
            for (path, kind, size) in entries {
                let size = size.map_or("-".to_string(), |s| s.to_string());
                println!("{size:>12}  {:<7} {path}", format!("{kind:?}").to_lowercase());
            }
            Ok(0)
        }
        "thumb" => {
            let (input, flags) = input_and_flags(rest)?;
            let (with, flags) = with_sources(flags)?;
            let mut opts = ThumbOptions::default();
            let mut out = None;
            let mut timings = false;
            let mut it = flags.iter();
            while let Some(flag) = it.next() {
                let mut value = || it.next().ok_or_else(|| Failure::Usage(format!("{flag} needs a value")));
                match flag.as_str() {
                    "-o" | "--out" => out = Some(value()?.clone()),
                    "--size" => opts.size = value()?.parse().map_err(|_| Failure::Usage("--size must be a number".into()))?,
                    "--view" => opts.view = value()?.parse::<View>().map_err(Failure::Usage)?,
                    "--bg" => opts.background = Background::parse(value()?).map_err(Failure::Usage)?,
                    "--timings" => timings = true,
                    other => return Err(Failure::Usage(format!("unknown option {other}"))),
                }
            }
            let out = out.ok_or_else(|| Failure::Usage("thumb needs -o <out.png|out.webp>".into()))?;
            if thumb::ImageFormat::from_path(&out).is_none() {
                return Err(Failure::Usage(format!("{out} must end with .png or .webp")));
            }
            let t = std::time::Instant::now();
            let gpu = Gpu::headless()?;
            let gpu_ms = unipeek::unity::Timings::since(t);
            for failed in &gpu.fallback {
                eprintln!("fallback: {failed}");
            }
            let report = thumb::write_thumbnail_with(&gpu, input, std::path::Path::new(&out), &opts, with)?;
            for d in &report.degrades {
                eprintln!("degraded: {}", serde_json::to_string(d).unwrap_or_default());
            }
            if timings {
                eprintln!("gpu:       {gpu_ms} ms ({})", gpu.describe());
                print_timings(&report, &mut std::io::stderr());
            }
            Ok(status_code(&report))
        }
        "turn" => {
            let (input, flags) = input_and_flags(rest)?;
            let (with, flags) = with_sources(flags)?;
            let (mut frames, mut size, mut background, mut format, mut out) = (24u32, 512u32, Background::Transparent, thumb::ImageFormat::Webp, None);
            let mut it = flags.iter();
            while let Some(flag) = it.next() {
                let mut value = || it.next().ok_or_else(|| Failure::Usage(format!("{flag} needs a value")));
                match flag.as_str() {
                    "-o" | "--out" => out = Some(std::path::PathBuf::from(value()?)),
                    "--frames" => frames = value()?.parse().ok().filter(|n| (1..=360).contains(n)).ok_or_else(|| Failure::Usage("--frames must be 1 to 360".into()))?,
                    "--size" => size = value()?.parse().map_err(|_| Failure::Usage("--size must be a number".into()))?,
                    "--bg" => background = Background::parse(value()?).map_err(Failure::Usage)?,
                    "--format" => {
                        format = match value()?.as_str() {
                            "png" => thumb::ImageFormat::Png,
                            "webp" => thumb::ImageFormat::Webp,
                            other => return Err(Failure::Usage(format!("--format {other}: expected png or webp"))),
                        }
                    }
                    other => return Err(Failure::Usage(format!("unknown option {other}"))),
                }
            }
            let out = out.ok_or_else(|| Failure::Usage("turn needs -o <folder>".into()))?;
            let scene = Scene::load(Input::from_location(input), &LoadOptions { max_texture: size.saturating_mul(2).clamp(256, 2048), resolver: with, ..Default::default() })?;
            let gpu = Gpu::headless()?;
            let images = thumb::render_turntable(&gpu, &scene, size.clamp(16, 4096), background, frames)?;
            std::fs::create_dir_all(&out).map_err(unipeek::Error::Io)?;
            let ext = if format == thumb::ImageFormat::Png { "png" } else { "webp" };
            for (i, img) in images.iter().enumerate() {
                let file = out.join(format!("frame_{i:03}.{ext}"));
                let tmp = out.join(format!("frame_{i:03}.{ext}.tmp"));
                std::fs::write(&tmp, thumb::encode(img, format)?).map_err(unipeek::Error::Io)?;
                std::fs::rename(&tmp, &file).map_err(unipeek::Error::Io)?;
            }
            eprintln!("wrote {} frame(s) to {}", images.len(), out.display());
            for d in &scene.report.degrades {
                eprintln!("degraded: {}", serde_json::to_string(d).unwrap_or_default());
            }
            Ok(status_code(&scene.report))
        }
        "view" => {
            let (input, flags) = input_and_flags(rest)?;
            let (with, flags) = with_sources(flags)?;
            if let Some(f) = flags.first() {
                return Err(Failure::Usage(format!("unknown option {f}")));
            }
            view_window(input, with)
        }
        "serve" => {
            let stdin = std::io::stdin().lock();
            let stdout = std::io::stdout().lock();
            Ok(unipeek::serve::serve(stdin, stdout))
        }
        "version" => {
            println!("unipeek {} (renderer {})", env!("CARGO_PKG_VERSION"), thumb::RENDERER_VERSION);
            Ok(0)
        }
        other => Err(Failure::Usage(format!("unknown command {other:?}"))),
    }
}

/// Take every `--with <package or folder>` out of the flags.
fn with_sources(flags: &[String]) -> Result<(Option<Arc<dyn Resolver>>, Vec<String>), Failure> {
    let mut with = Vec::new();
    let mut rest = Vec::new();
    let mut it = flags.iter();
    while let Some(f) = it.next() {
        if f == "--with" {
            with.push(it.next().ok_or_else(|| Failure::Usage("--with needs a package or folder".into()))?.clone());
        } else {
            rest.push(f.clone());
        }
    }
    let resolver: Option<Arc<dyn Resolver>> = (!with.is_empty()).then(|| Arc::new(Sources::new(&with, IndexOptions::default())) as Arc<dyn Resolver>);
    Ok((resolver, rest))
}

#[cfg(feature = "view")]
fn view_window(input: &str, with: Option<Arc<dyn Resolver>>) -> Result<i32, Failure> {
    view::run(input, with).or_else(|e| {
        eprintln!("error: {e}");
        Ok(3)
    })
}

#[cfg(not(feature = "view"))]
fn view_window(_: &str, _: Option<Arc<dyn Resolver>>) -> Result<i32, Failure> {
    Err(Failure::Usage("this unipeek was built without the `view` feature".into()))
}

fn input_and_flags(rest: &[String]) -> Result<(&str, &[String]), Failure> {
    let (input, flags) = rest.split_first().ok_or_else(|| Failure::Usage("missing <input>".into()))?;
    Ok((input.as_str(), flags))
}

fn status_code(report: &Report) -> i32 {
    match report.status {
        Status::Ok => 0,
        s => s.exit_code(),
    }
}

fn print_timings(r: &Report, out: &mut impl std::io::Write) {
    let t = &r.timings;
    let _ = writeln!(
        out,
        "timings:   index {} ms, plan {} ms, read {} ms, models {} ms, bind {} ms, textures {} ms, render {} ms, encode {} ms",
        t.index_ms, t.plan_ms, t.read_ms, t.models_ms, t.bind_ms, t.textures_ms, t.render_ms, t.encode_ms
    );
}

fn print_report(r: &Report) {
    println!("input:     {}", r.input);
    if let Some(m) = &r.member {
        println!("member:    {m}");
    }
    println!("prefab:    {}", r.prefab.as_deref().unwrap_or("-"));
    for c in r.candidates.iter().skip(1) {
        println!("  also:    {c}");
    }
    for m in &r.models {
        println!(
            "model:     {} ({} triangles, {} bones, {} blend shapes)",
            m.path,
            m.triangles.map_or("?".into(), |v| v.to_string()),
            m.bones.map_or("?".into(), |v| v.to_string()),
            m.blend_shapes.map_or("?".into(), |v| v.to_string())
        );
    }
    for m in &r.materials {
        let size = m.texture_size.map(|[w, h]| format!(" {w}x{h}")).unwrap_or_default();
        println!(
            "material:  {:<24} {:<20} {:?} {:?} {}{size}",
            m.slot,
            m.material.as_deref().unwrap_or("-"),
            m.family,
            m.mode,
            m.texture.as_deref().unwrap_or("-")
        );
    }
    for d in &r.degrades {
        println!("degraded:  {}", serde_json::to_string(d).unwrap_or_default());
    }
    println!("read:      {} pass(es), {} bytes", r.read.passes, r.read.bytes);
    print_timings(r, &mut std::io::stdout());
    println!("status:    {:?}", r.status);
}
