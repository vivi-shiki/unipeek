use super::*;
use crate::testing::{FBX_BINARY_MAGIC, PNG_MAGIC, PackageBuilder};
use std::io::Cursor;

const G1: &str = "7078d86f15dd32d4bb4540ac2ff9e0ac";
const G2: &str = "c5898e34be6905c45a38f3f063d0172d";
const G3: &str = "00715af155a8b554aa5d5ea437e05c8f";
const G4: &str = "4bbb1657292f0d74dab36e002aa5696d";

fn g(s: &str) -> Guid {
    Guid::parse(s).unwrap()
}

fn fbx_bytes(len: usize) -> Vec<u8> {
    let mut v = FBX_BINARY_MAGIC.to_vec();
    v.resize(len, 7);
    v
}

fn read(bytes: &[u8]) -> PackageIndex {
    PackageIndex::read(Cursor::new(bytes), &IndexOptions::default()).unwrap()
}

#[test]
fn indexes_assets_by_guid_with_pathname_and_kind() {
    let pkg = PackageBuilder::new()
        .asset(G1, "Assets/Shop/FBX/Avatar.fbx", fbx_bytes(5000))
        .asset(G2, "Assets/Shop/Material/Body.mat", "%YAML 1.1\n--- !u!21 &2100000\nMaterial:\n  m_Name: Body\n")
        .folder(G3, "Assets/Shop")
        .build();
    let idx = read(&pkg);
    assert_eq!(idx.len(), 3);
    let fbx = idx.get(&g(G1)).unwrap();
    assert_eq!(fbx.pathname.as_deref(), Some("Assets/Shop/FBX/Avatar.fbx"));
    assert_eq!(fbx.kind, AssetKind::Fbx);
    assert_eq!(fbx.size, Some(5000));
    assert!(fbx.text.is_none() && fbx.bytes.is_none());
    assert!(fbx.has_meta);
    assert_eq!(fbx.extension().as_deref(), Some("fbx"));
    assert_eq!(fbx.stem(), Some("Avatar"));
    let mat = idx.get(&g(G2)).unwrap();
    assert_eq!(mat.kind, AssetKind::Yaml);
    assert!(mat.text.as_deref().unwrap().contains("m_Name: Body"));
    assert!(mat.meta.as_deref().unwrap().contains(G2));
    let folder = idx.get(&g(G3)).unwrap();
    assert_eq!(folder.kind, AssetKind::Folder);
    assert_eq!(folder.size, None);
    assert!(idx.issues.is_empty(), "{:?}", idx.issues);
    assert_eq!(idx.stats.passes, 1);
    assert_eq!(idx.stats.bytes, pkg.len() as u64);
}

#[test]
fn pathname_uses_first_line_only_and_trims_cr() {
    let pkg = PackageBuilder::new()
        .raw(&format!("{G1}/pathname"), "Assets/A.prefab\r\n00\n")
        .raw(&format!("{G1}/asset"), "%YAML 1.1\n")
        .build();
    let idx = read(&pkg);
    assert_eq!(idx.get(&g(G1)).unwrap().pathname.as_deref(), Some("Assets/A.prefab"));
}

#[test]
fn dot_slash_prefix_is_ignored() {
    let idx = read(&PackageBuilder::new().dot_slash().asset(G1, "Assets/A.fbx", fbx_bytes(100)).build());
    assert_eq!(idx.get(&g(G1)).unwrap().pathname.as_deref(), Some("Assets/A.fbx"));
}

#[test]
fn pathname_after_asset_still_resolves() {
    let pkg = PackageBuilder::new()
        .raw(&format!("{G1}/asset"), fbx_bytes(64))
        .raw(&format!("{G2}/pathname"), "Assets/B.mat")
        .raw(&format!("{G1}/pathname"), "Assets/A.fbx")
        .raw(&format!("{G2}/asset"), "%YAML 1.1\n--- !u!21 &2100000\nMaterial:\n  m_Name: B\n")
        .build();
    let idx = read(&pkg);
    assert_eq!(idx.get(&g(G1)).unwrap().kind, AssetKind::Fbx);
    assert_eq!(idx.by_path("assets\\a.FBX").unwrap().guid, g(G1));
    assert!(idx.get(&g(G2)).unwrap().text.is_some());
}

#[test]
fn non_guid_folders_are_skipped_and_reported_once() {
    let pkg = PackageBuilder::new()
        .raw("Assets/readme.txt", "x")
        .raw("Assets/other.txt", "y")
        .raw("not-a-guid/pathname", "Assets/x")
        .asset(G1, "Assets/A.fbx", fbx_bytes(10))
        .build();
    let idx = read(&pkg);
    assert_eq!(idx.len(), 1);
    let names: Vec<_> = idx
        .issues
        .iter()
        .filter_map(|i| match i {
            PackageIssue::NonGuidFolder { name } => Some(name.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(names, vec!["Assets", "not-a-guid"]);
}

#[test]
fn hostile_pathnames_are_flagged() {
    let pkg = PackageBuilder::new()
        .asset(G1, "../../evil.fbx", fbx_bytes(10))
        .asset(G2, "C:/Windows/evil.mat", "%YAML 1.1\n")
        .asset(G3, "/etc/evil.prefab", "%YAML 1.1\n")
        .asset(G4, "Assets/fine..name.png", PNG_MAGIC.to_vec())
        .build();
    let idx = read(&pkg);
    assert!(idx.get(&g(G1)).unwrap().unsafe_path);
    assert!(idx.get(&g(G2)).unwrap().unsafe_path);
    assert!(idx.get(&g(G3)).unwrap().unsafe_path);
    assert!(!idx.get(&g(G4)).unwrap().unsafe_path);
    assert_eq!(idx.issues.iter().filter(|i| matches!(i, PackageIssue::UnsafePath { .. })).count(), 3);
}

#[test]
fn missing_pathname_and_duplicates_are_reported() {
    let pkg = PackageBuilder::new()
        .raw(&format!("{G1}/asset"), fbx_bytes(10))
        .asset(G2, "Assets/A.mat", "%YAML 1.1\n")
        .raw(&format!("{G2}/pathname"), "Assets/Other.mat")
        .build();
    let idx = read(&pkg);
    assert!(idx.issues.contains(&PackageIssue::MissingPathname { guid: g(G1) }));
    assert!(idx.issues.contains(&PackageIssue::DuplicateEntry { guid: g(G2), file: "pathname".into() }));
    assert_eq!(idx.get(&g(G2)).unwrap().pathname.as_deref(), Some("Assets/A.mat"));
}

#[test]
fn not_gzip_is_an_error() {
    let err = PackageIndex::read(Cursor::new(b"PK\x03\x04 this is a zip".to_vec()), &IndexOptions::default()).unwrap_err();
    assert!(matches!(err, crate::Error::NotUnityPackage(_)), "{err}");
}

#[test]
fn truncated_package_keeps_what_was_read() {
    let big: Vec<u8> = (0..200_000u32).map(|i| (i.wrapping_mul(2654435761) >> 24) as u8).collect();
    let pkg = PackageBuilder::new()
        .asset(G1, "Assets/A.mat", "%YAML 1.1\n")
        .asset(G2, "Assets/B.png", [PNG_MAGIC, &big].concat())
        .build();
    let cut = &pkg[..pkg.len() * 3 / 4];
    let idx = read(cut);
    assert!(idx.get(&g(G1)).is_some_and(|e| e.pathname.is_some()));
    assert!(idx.issues.iter().any(|i| matches!(i, PackageIssue::Truncated { .. })), "{:?}", idx.issues);
}

#[test]
fn animation_yaml_is_measured_but_not_kept() {
    let pkg = PackageBuilder::new()
        .asset(G1, "Assets/A.anim", "%YAML 1.1\n%TAG !u! tag:unity3d.com,2011:\n--- !u!74 &7400000\nAnimationClip:\n  m_Name: A\n")
        .asset(G2, "Assets/P.prefab", "%YAML 1.1\n%TAG !u! tag:unity3d.com,2011:\n--- !u!1 &1\nGameObject:\n  m_Name: P\n")
        .build();
    let idx = read(&pkg);
    let anim = idx.get(&g(G1)).unwrap();
    assert_eq!(anim.kind, AssetKind::Yaml);
    assert!(anim.text.is_none());
    assert!(idx.get(&g(G2)).unwrap().text.is_some());
}

#[test]
fn large_text_is_not_kept() {
    let mut yaml = "%YAML 1.1\n--- !u!21 &2100000\nMaterial:\n".to_string();
    yaml.push_str(&"  x: 1\n".repeat(200));
    let pkg = PackageBuilder::new().asset(G1, "Assets/A.mat", yaml).build();
    let idx = PackageIndex::read(Cursor::new(pkg), &IndexOptions { max_text_bytes: 100, ..Default::default() }).unwrap();
    assert!(idx.get(&g(G1)).unwrap().text.is_none());
}

#[test]
fn shader_name_is_extracted() {
    let shader = "// Made with Amplify\nShader \".poiyomi/Poiyomi 8.0/Poiyomi Toon\"\n{\n  Properties {}\n}\n";
    let idx = read(&PackageBuilder::new().asset(G1, "Assets/S/Toon.shader", shader).build());
    let e = idx.get(&g(G1)).unwrap();
    assert_eq!(e.kind, AssetKind::Shader);
    assert_eq!(e.shader_name.as_deref(), Some(".poiyomi/Poiyomi 8.0/Poiyomi Toon"));
    assert!(e.text.is_none());
}

#[test]
fn preview_png_is_noted() {
    let pkg = PackageBuilder::new()
        .asset(G1, "Assets/A.prefab", "%YAML 1.1\n")
        .raw(&format!("{G1}/preview.png"), PNG_MAGIC.to_vec())
        .build();
    assert!(read(&pkg).get(&g(G1)).unwrap().has_preview);
}

#[test]
fn second_pass_returns_wanted_assets_only() {
    let png = [PNG_MAGIC, b"pixels"].concat();
    let pkg = PackageBuilder::new()
        .asset(G1, "Assets/A.fbx", fbx_bytes(300))
        .asset(G2, "Assets/B.png", png.clone())
        .asset(G3, "Assets/C.fbx", fbx_bytes(20))
        .build();
    let mut idx = read(&pkg);
    let wanted: BTreeSet<Guid> = [g(G1), g(G2)].into_iter().collect();
    let got = idx.read_assets(Cursor::new(&pkg), &wanted).unwrap();
    assert_eq!(got.len(), 2);
    assert_eq!(got[&g(G1)].len(), 300);
    assert_eq!(&got[&g(G2)][..], &png[..]);
    assert_eq!(idx.stats.passes, 2);
    assert_eq!(idx.stats.bytes, 2 * pkg.len() as u64);
}

#[test]
fn kept_binaries_need_no_second_pass() {
    let pkg = PackageBuilder::new()
        .asset(G1, "Assets/A.fbx", fbx_bytes(300))
        .asset(G2, "Assets/B.anim", "%YAML 1.1\n--- !u!74 &1\n")
        .build();
    let mut idx = PackageIndex::read(Cursor::new(&pkg), &IndexOptions { keep_binaries: true, ..Default::default() }).unwrap();
    assert_eq!(idx.get(&g(G1)).unwrap().bytes.as_ref().unwrap().len(), 300);
    let wanted: BTreeSet<Guid> = [g(G1)].into_iter().collect();
    let got = idx.read_assets(Cursor::new(Vec::<u8>::new()), &wanted).unwrap();
    assert_eq!(got[&g(G1)].len(), 300);
    assert_eq!(idx.stats.passes, 1);
}

#[test]
fn binary_budget_is_enforced() {
    let pkg = PackageBuilder::new()
        .asset(G1, "Assets/A.fbx", fbx_bytes(300))
        .asset(G2, "Assets/B.fbx", fbx_bytes(300))
        .build();
    let opts = IndexOptions { keep_binaries: true, max_binary_bytes: 400, ..Default::default() };
    let idx = PackageIndex::read(Cursor::new(&pkg), &opts).unwrap();
    assert_eq!(idx.entries().filter(|e| e.bytes.is_some()).count(), 1);
    assert!(idx.issues.contains(&PackageIssue::BinaryBudgetExceeded { limit: 400 }));
}
