//! The `unipeek` command line. Exit codes: 0 ok, 1 degraded, 2 unsupported, 3 error, 64 usage.
use std::process::ExitCode;
use unipeek::{
    LoadOptions, Scene,
    gpu::Gpu,
    thumb::{self, Background, ThumbOptions, View},
    unity::{IndexOptions, Input, Package, Report, Status},
};

const USAGE: &str = "usage:
  unipeek info  <input> [--json]
  unipeek ls    <input>
  unipeek thumb <input> -o <out.png|out.webp> [--size 512] [--view front|side|back|3q] [--bg transparent|#rrggbb[aa]]
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
            let json = flags.iter().any(|f| f == "--json");
            let scene = Scene::load(Input::from_location(input), &LoadOptions::default())?;
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
            let mut opts = ThumbOptions::default();
            let mut out = None;
            let mut it = flags.iter();
            while let Some(flag) = it.next() {
                let mut value = || it.next().ok_or_else(|| Failure::Usage(format!("{flag} needs a value")));
                match flag.as_str() {
                    "-o" | "--out" => out = Some(value()?.clone()),
                    "--size" => opts.size = value()?.parse().map_err(|_| Failure::Usage("--size must be a number".into()))?,
                    "--view" => opts.view = value()?.parse::<View>().map_err(Failure::Usage)?,
                    "--bg" => opts.background = Background::parse(value()?).map_err(Failure::Usage)?,
                    other => return Err(Failure::Usage(format!("unknown option {other}"))),
                }
            }
            let out = out.ok_or_else(|| Failure::Usage("thumb needs -o <out.png|out.webp>".into()))?;
            if thumb::ImageFormat::from_path(&out).is_none() {
                return Err(Failure::Usage(format!("{out} must end with .png or .webp")));
            }
            let gpu = Gpu::headless()?;
            let report = thumb::write_thumbnail(&gpu, input, std::path::Path::new(&out), &opts)?;
            for d in &report.degrades {
                eprintln!("degraded: {}", serde_json::to_string(d).unwrap_or_default());
            }
            Ok(status_code(&report))
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
    println!("status:    {:?}", r.status);
}
