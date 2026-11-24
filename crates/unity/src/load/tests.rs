use super::*;
use crate::{
    AssetKind, Plan, Source,
    testing::{FBX_BINARY_MAGIC, PNG_MAGIC, PackageBuilder, model_meta, prefab_instancing, zip, zip_with_flags},
};
use std::{path::PathBuf, sync::Arc};

const MODEL: &str = "7078d86f15dd32d4bb4540ac2ff9e0ac";
const PREFAB: &str = "00715af155a8b554aa5d5ea437e05c8f";
const OTHER: &str = "f12727134b9968946aca776d25845173";

struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> TempDir {
        let p = std::env::temp_dir().join(format!("unipeek-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        TempDir(p)
    }

    fn write(&self, rel: &str, data: &[u8]) -> PathBuf {
        let p = self.0.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, data).unwrap();
        p
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn fbx(len: usize) -> Vec<u8> {
    let mut v = FBX_BINARY_MAGIC.to_vec();
    v.resize(len, 3);
    v
}

fn small_package() -> Vec<u8> {
    PackageBuilder::new()
        .asset_with_meta(MODEL, "Assets/A/Avatar.fbx", fbx(600), model_meta(MODEL, &[]))
        .asset(PREFAB, "Assets/A/Avatar.prefab", prefab_instancing(MODEL, &[]))
        .build()
}

fn all_bytes(pkg: &mut Package) -> BTreeMap<Guid, Arc<[u8]>> {
    let wanted: BTreeSet<Guid> = pkg.index.entries().filter(|e| e.kind == AssetKind::Fbx || e.kind == AssetKind::Png).map(|e| e.guid).collect();
    pkg.read_assets(&wanted).unwrap()
}

#[test]
fn unitypackage_from_path_bytes_and_reader_agree() {
    let bytes = small_package();
    let dir = TempDir::new("paths");
    let path = dir.write("Avatar.unitypackage", &bytes);
    let inputs = [
        Input::new(Source::Path(path)),
        Input::new(Source::Bytes(Arc::from(bytes.clone()))),
        Input::new(Source::Reader(Box::new(std::io::Cursor::new(bytes.clone())))),
    ];
    for input in inputs {
        let (mut pkg, inner) = Package::open(input, &IndexOptions::default()).unwrap();
        assert_eq!(pkg.container, Container::UnityPackage);
        assert_eq!(inner, None);
        assert_eq!(pkg.index.len(), 2);
        let got = all_bytes(&mut pkg);
        assert_eq!(got[&Guid::parse(MODEL).unwrap()].len(), 600);
    }
}

#[test]
fn inner_path_is_passed_through() {
    let mut input = Input::from_location("x/Avatar.unitypackage#Assets/A/Avatar.prefab");
    input.source = Source::Bytes(Arc::from(small_package()));
    let (pkg, inner) = Package::open(input, &IndexOptions::default()).unwrap();
    assert_eq!(inner.as_deref(), Some("Assets/A/Avatar.prefab"));
    assert!(Plan::new(&pkg.index, inner.as_deref()).unwrap().prefab.is_some());
}

#[test]
fn zip_picks_the_largest_unitypackage_and_reads_it_twice() {
    let big = small_package();
    let tiny = PackageBuilder::new().asset(OTHER, "Assets/B/Tiny.fbx", fbx(10)).build();
    let z = zip(&[(b"Item/readme.txt", b"hi", true), (b"Item/Tiny.unitypackage", &tiny, true), (b"Item/Avatar.unitypackage", &big, true)]);
    let dir = TempDir::new("zip");
    let path = dir.write("Item.zip", &z);
    for input in [Input::new(Source::Path(path.clone())), Input::new(Source::Bytes(Arc::from(z.clone())))] {
        let (mut pkg, inner) = Package::open(input, &IndexOptions::default()).unwrap();
        assert_eq!(pkg.container, Container::Zip);
        assert_eq!(pkg.member.as_deref(), Some("Item/Avatar.unitypackage"));
        assert_eq!(inner, None);
        assert_eq!(all_bytes(&mut pkg)[&Guid::parse(MODEL).unwrap()].len(), 600);
    }
    let (pkg, inner) = Package::open(Input::from_location(&format!("{}#Item/Tiny.unitypackage#Assets/B/Tiny.fbx", path.display())), &IndexOptions::default()).unwrap();
    assert_eq!(pkg.member.as_deref(), Some("Item/Tiny.unitypackage"));
    assert_eq!(inner.as_deref(), Some("Assets/B/Tiny.fbx"));
}

#[test]
fn zip_without_unitypackage_is_loose_and_decodes_shift_jis() {
    // "衣装/衣装.fbx" in Shift-JIS, without the UTF-8 flag.
    let sjis_dir: &[u8] = &[0x88, 0xdf, 0x91, 0x95];
    let fbx_name = [sjis_dir, b"/", sjis_dir, b".fbx"].concat();
    let png_name = [sjis_dir, b"/Tex/", sjis_dir, b"_Body.png"].concat();
    let model = fbx(300);
    let z = zip(&[(&fbx_name, &model, false), (&png_name, PNG_MAGIC, false), (b"Project/Library/cache.fbx", &fbx(9000), true)]);
    let (mut pkg, inner) = Package::open(Input::new(Source::Bytes(Arc::from(z))), &IndexOptions::default()).unwrap();
    assert_eq!(pkg.container, Container::Zip);
    assert_eq!(pkg.member, None);
    assert_eq!(inner, None);
    let paths: Vec<String> = pkg.index.entries().filter_map(|e| e.pathname.clone()).collect();
    assert!(paths.contains(&"衣装/衣装.fbx".to_string()), "{paths:?}");
    assert!(paths.contains(&"衣装/Tex/衣装_Body.png".to_string()), "{paths:?}");
    assert!(!paths.iter().any(|p| p.contains("Library")), "Library/ is skipped: {paths:?}");
    let plan = Plan::new(&pkg.index, None).unwrap();
    assert_eq!(plan.models[0].path, "衣装/衣装.fbx");
    let binding = plan.bind(&pkg.index, plan.models[0].guid, "Body");
    assert_eq!(binding.main_texture.unwrap().path.as_deref(), Some("衣装/Tex/衣装_Body.png"));
    assert_eq!(all_bytes(&mut pkg)[&plan.models[0].guid].len(), 300);
}

#[test]
fn folder_prefers_a_unitypackage() {
    let dir = TempDir::new("folder-pkg");
    dir.write("Item/Avatar.unitypackage", &small_package());
    dir.write("Item/extra/Other.fbx", &fbx(50));
    let (pkg, _) = Package::open(Input::new(Source::Path(dir.0.clone())), &IndexOptions::default()).unwrap();
    assert_eq!(pkg.container, Container::Folder);
    assert_eq!(pkg.member.as_deref(), Some("Item/Avatar.unitypackage"));
    assert_eq!(pkg.index.len(), 2);
}

#[test]
fn folder_and_single_fbx_are_loose() {
    let dir = TempDir::new("loose");
    let model = dir.write("Model/Avatar.fbx", &fbx(400));
    dir.write("Model/Textures/Avatar_Hair.png", PNG_MAGIC);
    dir.write("Library/junk.fbx", &fbx(9999));
    let (mut pkg, _) = Package::open(Input::new(Source::Path(dir.0.clone())), &IndexOptions::default()).unwrap();
    assert_eq!(pkg.container, Container::Folder);
    let plan = Plan::new(&pkg.index, None).unwrap();
    assert_eq!(plan.models[0].path, "Model/Avatar.fbx");
    assert_eq!(all_bytes(&mut pkg)[&plan.models[0].guid].len(), 400);

    let (mut single, _) = Package::open(Input::new(Source::Path(model)), &IndexOptions::default()).unwrap();
    assert_eq!(single.container, Container::Model);
    let plan = Plan::new(&single.index, None).unwrap();
    assert_eq!(plan.models[0].path, "Avatar.fbx");
    let b = plan.bind(&single.index, plan.models[0].guid, "Hair");
    assert_eq!(b.main_texture.unwrap().path.as_deref(), Some("Textures/Avatar_Hair.png"));
    assert_eq!(all_bytes(&mut single).len(), 2);
}

#[test]
fn unsupported_and_missing_inputs() {
    let err = Package::open(Input::new(Source::Bytes(Arc::from(&b"just text"[..]))), &IndexOptions::default()).err().unwrap();
    assert!(matches!(err, crate::Error::Unsupported(_)), "{err}");
    let err = Package::open(Input::new(Source::Path("Z:/definitely/not/here.unitypackage".into())), &IndexOptions::default()).err().unwrap();
    assert!(matches!(err, crate::Error::Io(_) | crate::Error::NotFound(_)), "{err}");
}

#[test]
fn nested_and_encrypted_zips_are_unsupported() {
    let inner = zip(&[(b"a.fbx", &fbx(10), true)]);
    let outer = zip(&[(b"Item/inner.zip", &inner, true)]);
    let mut input = Input::from_location("x/Outer.zip#Item/inner.zip");
    input.source = Source::Bytes(Arc::from(outer));
    let err = Package::open(input, &IndexOptions::default()).err().unwrap();
    assert!(matches!(err, crate::Error::Unsupported(_)), "{err}");

    let locked = zip_with_flags(&[(b"Item/Avatar.unitypackage", &small_package(), 0x0001)]);
    let err = Package::open(Input::new(Source::Bytes(Arc::from(locked))), &IndexOptions::default()).err().unwrap();
    assert!(matches!(err, crate::Error::Unsupported(_)), "{err}");
    assert!(err.to_string().to_lowercase().contains("password") || err.to_string().to_lowercase().contains("encrypt"), "{err}");
}
