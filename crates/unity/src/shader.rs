//! Recognize shaders without their source. The shader is rarely in the package (lilToon and
//! Poiyomi are installed through VPM), so known GUIDs come first and names second.
use crate::{Guid, yaml::ObjRef};

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ShaderFamily {
    LilToon,
    LilToonLite,
    LilToonMulti,
    Poiyomi,
    MToon,
    Standard,
    UnityBuiltin,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RenderMode {
    Opaque,
    Cutout,
    Transparent,
}

/// Passes a variant draws, besides the surface.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Variant {
    Normal,
    OutlineOnly,
    FakeShadow,
    Fur,
    Gem,
    Refraction,
    Overlay,
    /// lilToon's internal passes, never assigned by hand.
    Internal,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct ShaderInfo {
    pub family: ShaderFamily,
    /// Fixed by the variant; `None` means "decide by the material's render queue".
    pub mode: Option<RenderMode>,
    pub outline: bool,
    pub variant: Variant,
    /// The shader's name, when known from the table or from the package.
    pub name: Option<String>,
    pub guid: Option<Guid>,
}

/// Unity's built-in shaders all share this GUID and differ by file ID.
pub const BUILTIN_GUID: &str = "0000000000000000f000000000000000";

/// lilToon 1.8.3 (MIT): GUIDs and file names of `jp.lilxyzw.liltoon/Shader/*.shader`.
/// The GUIDs come from the `.meta` files and have stayed the same across lilToon releases.
pub const LILTOON_SHADERS: &[(&str, &str)] = &[
    ("df12117ecd77c31469c224178886498e", "lts"),
    ("85d6126cae43b6847aff4b13f4adb8ec", "lts_cutout"),
    ("3b4aa19949601f046a20ca8bdaee929f", "lts_cutout_o"),
    ("3b3957e6c393b114bab6f835b4ed8f5d", "lts_cutout_oo"),
    ("00795bf598b44dc4e9bd363348e77085", "lts_fakeshadow"),
    ("55706696b2bdb5d4d8541b89e17085c8", "lts_fur"),
    ("544c75f56e9af8048b29a6ace5f52091", "lts_fur_cutout"),
    ("54bc8b41278802d4a81b27fe402994e2", "lts_fur_two"),
    ("33aad051c4a3a844a8f9330addb86a97", "lts_furonly"),
    ("7ec9f85eb7ee04943adfe19c2ba5901f", "lts_furonly_cutout"),
    ("f8d9dfac6dbfaaf4c9c3aaf4bd8c955f", "lts_furonly_two"),
    ("a8d94439709469942bc7dcc9156ba110", "lts_gem"),
    ("efa77a80ca0344749b4f19fdd5891cbe", "lts_o"),
    ("b269573b9937b8340b3e9e191a3ba5a8", "lts_onetrans"),
    ("7171688840c632447b22ec14e2bdef7e", "lts_onetrans_o"),
    ("fba17785d6b2c594ab6c0303c834da65", "lts_oo"),
    ("94274b8ef5d3af842b9427384cba3a8f", "lts_overlay"),
    ("33e950d038b8dfd4f824f3985c2abfb7", "lts_overlay_one"),
    ("dce3f3e248acc7b4daeda00daf616b4d", "lts_ref"),
    ("3fb94a39b2685ee4d9817dcaf6542d99", "lts_ref_blur"),
    ("3eef4aee6ba0de047b0d40409ea2891c", "lts_tess"),
    ("bbfffd5515b843c41a85067191cbf687", "lts_tess_cutout"),
    ("5ba517885727277409feada18effa4a6", "lts_tess_cutout_o"),
    ("c6d605ee23b18fc46903f38c67db701f", "lts_tess_o"),
    ("90f83c35b0769a748abba5d0880f36d5", "lts_tess_onetrans"),
    ("67ed0252d63362a4ab23707a720508b7", "lts_tess_onetrans_o"),
    ("afa1a194f5a2fd243bda3a17bca1b36e", "lts_tess_trans"),
    ("9b0c2630b12933248922527d4507cfa9", "lts_tess_trans_o"),
    ("7e398ea50f9b70045b1774e05b46a39f", "lts_tess_twotrans"),
    ("7e61dbad981ad4f43a03722155db1c6a", "lts_tess_twotrans_o"),
    ("165365ab7100a044ca85fc8c33548a62", "lts_trans"),
    ("3c79b10c7e0b2784aaa4c2f8dd17d55e", "lts_trans_o"),
    ("0c762f24b85918a49812fc5690619178", "lts_trans_oo"),
    ("6a77405f7dfdc1447af58854c7f43f39", "lts_twotrans"),
    ("9cf054060007d784394b8b0bb703e441", "lts_twotrans_o"),
    ("381af8ba8e1740a41b9768ccfb0416c2", "ltsl"),
    ("b957dce3d03ff5445ac989f8de643c7f", "ltsl_cutout"),
    ("8cf5267d397b04846856f6d3d9561da0", "ltsl_cutout_o"),
    ("583a88005abb81a4ebbce757b4851a0d", "ltsl_o"),
    ("34c2907eba944ed45a43970e0c11bcfd", "ltsl_onetrans"),
    ("701268c07d37f5441b25b2cb99fae4b3", "ltsl_onetrans_o"),
    ("d28e4b78ba8368e49a44f86c0291df58", "ltsl_overlay"),
    ("dc9ded9f9d6f16c4e92cbb8f4269ae31", "ltsl_overlay_one"),
    ("0e3ece1bd59542743bccadb21f68318e", "ltsl_trans"),
    ("1c12a37046f07ac4486881deaf0187ea", "ltsl_trans_o"),
    ("82226adb1a0b8c4418f574cfdcf523da", "ltsl_twotrans"),
    ("62df797f407281640a224388953448cc", "ltsl_twotrans_o"),
    ("9294844b15dca184d914a632279b24e1", "ltsmulti"),
    ("1e50f1bc4d1b0e34cbf16b82589f6407", "ltsmulti_fur"),
    ("69f861c14129e724096c0955f8079012", "ltsmulti_gem"),
    ("51b2dee0ab07bd84d8147601ff89e511", "ltsmulti_o"),
    ("d7af54cdd86902d41b8c240e06b93009", "ltsmulti_ref"),
    ("f96a89829ccb1e54b85214550519a8d6", "ltspass_baker"),
    ("ad219df2a46e841488aee6a013e84e36", "ltspass_cutout"),
    ("2bde4bd29a2a70a4d9cf98772a6717ac", "ltspass_dummy"),
    ("8a6ef0489c3ffbf46812460af3d52bb0", "ltspass_lite_cutout"),
    ("59b5e58e88aae8a4ca42d1a7253e2fb2", "ltspass_lite_opaque"),
    ("8773c83ab40fff24b800f74360819a6c", "ltspass_lite_transparent"),
    ("61b4f98a5d78b4a4a9d89180fac793fc", "ltspass_opaque"),
    ("fd68f52288a6b0243bf6c217bf0930ea", "ltspass_proponly"),
    ("14006db8206fb304aa86110d57626d40", "ltspass_tess_cutout"),
    ("7a7ac427f85673a45a3e4190fc10bc28", "ltspass_tess_opaque"),
    ("bdf24b2e925ce8a4fb0e903889a52e0e", "ltspass_tess_transparent"),
    ("2683fad669f20ec49b8e9656954a33a8", "ltspass_transparent"),
];

/// Identify a shader by its reference and, when the shader file is in the package, its name.
pub fn identify(_shader: ObjRef, _name_in_package: Option<&str>) -> ShaderInfo {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn by_guid(guid: &str) -> ShaderInfo {
        identify(ObjRef { file_id: 4800000, guid: Guid::parse(guid), kind: Some(3) }, None)
    }

    #[test]
    fn table_has_unique_valid_guids() {
        let mut seen = std::collections::BTreeSet::new();
        for (g, file) in LILTOON_SHADERS {
            assert!(Guid::parse(g).is_some(), "{file}");
            assert!(seen.insert(*g), "duplicate {g}");
        }
    }

    #[test]
    fn liltoon_variants() {
        let base = by_guid("df12117ecd77c31469c224178886498e");
        assert_eq!((base.family, base.mode, base.outline, base.variant), (ShaderFamily::LilToon, Some(RenderMode::Opaque), false, Variant::Normal));
        assert_eq!(base.name.as_deref(), Some("lilToon"));

        let o = by_guid("efa77a80ca0344749b4f19fdd5891cbe");
        assert_eq!((o.mode, o.outline), (Some(RenderMode::Opaque), true));
        assert_eq!(o.name.as_deref(), Some("Hidden/lilToonOutline"));

        let cut = by_guid("3b4aa19949601f046a20ca8bdaee929f");
        assert_eq!((cut.mode, cut.outline), (Some(RenderMode::Cutout), true));

        let two = by_guid("9cf054060007d784394b8b0bb703e441");
        assert_eq!((two.mode, two.outline), (Some(RenderMode::Transparent), true));

        let lite = by_guid("b957dce3d03ff5445ac989f8de643c7f");
        assert_eq!((lite.family, lite.mode), (ShaderFamily::LilToonLite, Some(RenderMode::Cutout)));

        let oo = by_guid("fba17785d6b2c594ab6c0303c834da65");
        assert_eq!((oo.variant, oo.outline), (Variant::OutlineOnly, true));

        assert_eq!(by_guid("00795bf598b44dc4e9bd363348e77085").variant, Variant::FakeShadow);
        assert_eq!(by_guid("55706696b2bdb5d4d8541b89e17085c8").variant, Variant::Fur);
        assert_eq!(by_guid("a8d94439709469942bc7dcc9156ba110").variant, Variant::Gem);

        let multi = by_guid("51b2dee0ab07bd84d8147601ff89e511");
        assert_eq!((multi.family, multi.mode, multi.outline), (ShaderFamily::LilToonMulti, None, true));
    }

    #[test]
    fn unity_builtin_by_file_id() {
        let standard = identify(ObjRef { file_id: 46, guid: Guid::parse(BUILTIN_GUID), kind: Some(0) }, None);
        assert_eq!((standard.family, standard.mode), (ShaderFamily::Standard, None));
        let other = identify(ObjRef { file_id: 10752, guid: Guid::parse(BUILTIN_GUID), kind: Some(0) }, None);
        assert_eq!(other.family, ShaderFamily::UnityBuiltin);
    }

    #[test]
    fn names_from_the_package() {
        let r = ObjRef { file_id: 4800000, guid: Guid::parse("0123456789abcdef0123456789abcdef"), kind: Some(3) };
        assert_eq!(identify(r, Some(".poiyomi/Poiyomi 8.0/Poiyomi Toon")).family, ShaderFamily::Poiyomi);
        assert_eq!(identify(r, Some("Hidden/Locked/.poiyomi/Poiyomi Pro/abc")).family, ShaderFamily::Poiyomi);
        assert_eq!(identify(r, Some("VRM/MToon")).family, ShaderFamily::MToon);
        assert_eq!(identify(r, Some("lilToonCustom/MyShader")).family, ShaderFamily::LilToon);
        let unknown = identify(r, Some("Custom/Thing"));
        assert_eq!((unknown.family, unknown.name.as_deref()), (ShaderFamily::Unknown, Some("Custom/Thing")));
        assert_eq!(identify(r, None).family, ShaderFamily::Unknown);
        assert_eq!(identify(r, None).guid, r.guid);
    }
}
