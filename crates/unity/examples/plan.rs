//! Print what unipeek would show for a package: `plan <package.unitypackage> [inner path]`.
use std::{fs::File, io::BufReader, time::Instant};
use unipeek_unity::{IndexOptions, PackageIndex, Plan};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let Some(path) = args.get(1) else {
        eprintln!("usage: plan <package.unitypackage> [inner path]");
        std::process::exit(2);
    };
    let t = Instant::now();
    let file = File::open(path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let index = PackageIndex::read(BufReader::new(file), &IndexOptions::default()).unwrap_or_else(|e| panic!("{e}"));
    println!("index: {} entries, {} issues, {} bytes in {:?}", index.len(), index.issues.len(), index.stats.bytes, t.elapsed());
    let plan = Plan::new(&index, args.get(2).map(String::as_str)).unwrap_or_else(|e| panic!("{e}"));
    println!("prefab: {:?}", plan.prefab.as_ref().map(|c| &c.path));
    for c in &plan.candidates {
        println!("  candidate {} models={} variant={}", c.path, c.models, c.variant);
    }
    for m in &plan.models {
        println!("model: {} ({:?} bytes)", m.path, m.size);
        let meta = index.get(&m.guid).and_then(|e| e.meta.as_deref()).map(unipeek_unity::meta::Meta::parse).unwrap_or_default();
        for (slot, _) in &meta.external_materials {
            let b = plan.bind(&index, m.guid, slot);
            println!(
                "  slot {slot:<20} {:?} mat={:?} shader={:?} mode={:?} outline={} tex={:?}",
                b.source,
                b.material_name,
                b.shader.as_ref().map(|s| s.family),
                b.look.mode,
                b.look.outline.is_some(),
                b.main_texture.as_ref().map(|t| (t.path.as_deref().unwrap_or("-"), t.in_package))
            );
        }
    }
    println!("prefab materials: {}, overrides: {}", plan.prefab_materials.len(), plan.material_overrides);
    for m in &plan.missing {
        println!("missing {} {:?}", m.guid, m.usage);
    }
}
