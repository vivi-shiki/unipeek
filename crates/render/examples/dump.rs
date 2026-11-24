fn main() -> anyhow::Result<()> {
    let path = std::env::args().nth(1).expect("fbx path");
    let t = std::time::Instant::now();
    let m = unipeek::model::Model::load_fbx(&path)?;
    println!("loaded in {:?}: {} tris, bounds {:?} .. {:?}", t.elapsed(), m.triangles(), m.min, m.max);
    for p in &m.parts {
        println!("  mesh={:<24} material={:<28} tris={}", p.mesh, p.material, p.count / 3);
    }
    Ok(())
}
