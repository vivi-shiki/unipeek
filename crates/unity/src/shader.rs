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

/// lilToon 1.8.3 (MIT): GUID, file name and shader name of `jp.lilxyzw.liltoon/Shader/*.shader`.
/// The GUIDs come from the `.meta` files and have stayed the same across lilToon releases.
pub const LILTOON_SHADERS: &[(&str, &str, &str)] = &[
    ("df12117ecd77c31469c224178886498e", "lts", "lilToon"),
    ("85d6126cae43b6847aff4b13f4adb8ec", "lts_cutout", "Hidden/lilToonCutout"),
    ("3b4aa19949601f046a20ca8bdaee929f", "lts_cutout_o", "Hidden/lilToonCutoutOutline"),
    ("3b3957e6c393b114bab6f835b4ed8f5d", "lts_cutout_oo", "_lil/[Optional] lilToonOutlineOnlyCutout"),
    ("00795bf598b44dc4e9bd363348e77085", "lts_fakeshadow", "_lil/[Optional] lilToonFakeShadow"),
    ("55706696b2bdb5d4d8541b89e17085c8", "lts_fur", "Hidden/lilToonFur"),
    ("544c75f56e9af8048b29a6ace5f52091", "lts_fur_cutout", "Hidden/lilToonFurCutout"),
    ("54bc8b41278802d4a81b27fe402994e2", "lts_fur_two", "Hidden/lilToonFurTwoPass"),
    ("33aad051c4a3a844a8f9330addb86a97", "lts_furonly", "_lil/[Optional] lilToonFurOnlyTransparent"),
    ("7ec9f85eb7ee04943adfe19c2ba5901f", "lts_furonly_cutout", "_lil/[Optional] lilToonFurOnlyCutout"),
    ("f8d9dfac6dbfaaf4c9c3aaf4bd8c955f", "lts_furonly_two", "_lil/[Optional] lilToonFurOnlyTwoPass"),
    ("a8d94439709469942bc7dcc9156ba110", "lts_gem", "Hidden/lilToonGem"),
    ("efa77a80ca0344749b4f19fdd5891cbe", "lts_o", "Hidden/lilToonOutline"),
    ("b269573b9937b8340b3e9e191a3ba5a8", "lts_onetrans", "Hidden/lilToonOnePassTransparent"),
    ("7171688840c632447b22ec14e2bdef7e", "lts_onetrans_o", "Hidden/lilToonOnePassTransparentOutline"),
    ("fba17785d6b2c594ab6c0303c834da65", "lts_oo", "_lil/[Optional] lilToonOutlineOnly"),
    ("94274b8ef5d3af842b9427384cba3a8f", "lts_overlay", "_lil/[Optional] lilToonOverlay"),
    ("33e950d038b8dfd4f824f3985c2abfb7", "lts_overlay_one", "_lil/[Optional] lilToonOverlayOnePass"),
    ("dce3f3e248acc7b4daeda00daf616b4d", "lts_ref", "Hidden/lilToonRefraction"),
    ("3fb94a39b2685ee4d9817dcaf6542d99", "lts_ref_blur", "Hidden/lilToonRefractionBlur"),
    ("3eef4aee6ba0de047b0d40409ea2891c", "lts_tess", "Hidden/lilToonTessellation"),
    ("bbfffd5515b843c41a85067191cbf687", "lts_tess_cutout", "Hidden/lilToonTessellationCutout"),
    ("5ba517885727277409feada18effa4a6", "lts_tess_cutout_o", "Hidden/lilToonTessellationCutoutOutline"),
    ("c6d605ee23b18fc46903f38c67db701f", "lts_tess_o", "Hidden/lilToonTessellationOutline"),
    ("90f83c35b0769a748abba5d0880f36d5", "lts_tess_onetrans", "Hidden/lilToonTessellationOnePassTransparent"),
    ("67ed0252d63362a4ab23707a720508b7", "lts_tess_onetrans_o", "Hidden/lilToonTessellationOnePassTransparentOutline"),
    ("afa1a194f5a2fd243bda3a17bca1b36e", "lts_tess_trans", "Hidden/lilToonTessellationTransparent"),
    ("9b0c2630b12933248922527d4507cfa9", "lts_tess_trans_o", "Hidden/lilToonTessellationTransparentOutline"),
    ("7e398ea50f9b70045b1774e05b46a39f", "lts_tess_twotrans", "Hidden/lilToonTessellationTwoPassTransparent"),
    ("7e61dbad981ad4f43a03722155db1c6a", "lts_tess_twotrans_o", "Hidden/lilToonTessellationTwoPassTransparentOutline"),
    ("165365ab7100a044ca85fc8c33548a62", "lts_trans", "Hidden/lilToonTransparent"),
    ("3c79b10c7e0b2784aaa4c2f8dd17d55e", "lts_trans_o", "Hidden/lilToonTransparentOutline"),
    ("0c762f24b85918a49812fc5690619178", "lts_trans_oo", "_lil/[Optional] lilToonOutlineOnlyTransparent"),
    ("6a77405f7dfdc1447af58854c7f43f39", "lts_twotrans", "Hidden/lilToonTwoPassTransparent"),
    ("9cf054060007d784394b8b0bb703e441", "lts_twotrans_o", "Hidden/lilToonTwoPassTransparentOutline"),
    ("381af8ba8e1740a41b9768ccfb0416c2", "ltsl", "Hidden/lilToonLite"),
    ("b957dce3d03ff5445ac989f8de643c7f", "ltsl_cutout", "Hidden/lilToonLiteCutout"),
    ("8cf5267d397b04846856f6d3d9561da0", "ltsl_cutout_o", "Hidden/lilToonLiteCutoutOutline"),
    ("583a88005abb81a4ebbce757b4851a0d", "ltsl_o", "Hidden/lilToonLiteOutline"),
    ("34c2907eba944ed45a43970e0c11bcfd", "ltsl_onetrans", "Hidden/lilToonLiteOnePassTransparent"),
    ("701268c07d37f5441b25b2cb99fae4b3", "ltsl_onetrans_o", "Hidden/lilToonLiteOnePassTransparentOutline"),
    ("d28e4b78ba8368e49a44f86c0291df58", "ltsl_overlay", "_lil/[Optional] lilToonLiteOverlay"),
    ("dc9ded9f9d6f16c4e92cbb8f4269ae31", "ltsl_overlay_one", "_lil/[Optional] lilToonLiteOverlayOnePass"),
    ("0e3ece1bd59542743bccadb21f68318e", "ltsl_trans", "Hidden/lilToonLiteTransparent"),
    ("1c12a37046f07ac4486881deaf0187ea", "ltsl_trans_o", "Hidden/lilToonLiteTransparentOutline"),
    ("82226adb1a0b8c4418f574cfdcf523da", "ltsl_twotrans", "Hidden/lilToonLiteTwoPassTransparent"),
    ("62df797f407281640a224388953448cc", "ltsl_twotrans_o", "Hidden/lilToonLiteTwoPassTransparentOutline"),
    ("9294844b15dca184d914a632279b24e1", "ltsmulti", "_lil/lilToonMulti"),
    ("1e50f1bc4d1b0e34cbf16b82589f6407", "ltsmulti_fur", "Hidden/lilToonMultiFur"),
    ("69f861c14129e724096c0955f8079012", "ltsmulti_gem", "Hidden/lilToonMultiGem"),
    ("51b2dee0ab07bd84d8147601ff89e511", "ltsmulti_o", "Hidden/lilToonMultiOutline"),
    ("d7af54cdd86902d41b8c240e06b93009", "ltsmulti_ref", "Hidden/lilToonMultiRefraction"),
    ("f96a89829ccb1e54b85214550519a8d6", "ltspass_baker", "Hidden/ltsother_baker"),
    ("ad219df2a46e841488aee6a013e84e36", "ltspass_cutout", "Hidden/ltspass_cutout"),
    ("2bde4bd29a2a70a4d9cf98772a6717ac", "ltspass_dummy", "Hidden/ltspass_dummy"),
    ("8a6ef0489c3ffbf46812460af3d52bb0", "ltspass_lite_cutout", "Hidden/ltspass_lite_cutout"),
    ("59b5e58e88aae8a4ca42d1a7253e2fb2", "ltspass_lite_opaque", "Hidden/ltspass_lite_opaque"),
    ("8773c83ab40fff24b800f74360819a6c", "ltspass_lite_transparent", "Hidden/ltspass_lite_transparent"),
    ("61b4f98a5d78b4a4a9d89180fac793fc", "ltspass_opaque", "Hidden/ltspass_opaque"),
    ("fd68f52288a6b0243bf6c217bf0930ea", "ltspass_proponly", "Hidden/ltspass_proponly"),
    ("14006db8206fb304aa86110d57626d40", "ltspass_tess_cutout", "Hidden/ltspass_tess_cutout"),
    ("7a7ac427f85673a45a3e4190fc10bc28", "ltspass_tess_opaque", "Hidden/ltspass_tess_opaque"),
    ("bdf24b2e925ce8a4fb0e903889a52e0e", "ltspass_tess_transparent", "Hidden/ltspass_tess_transparent"),
    ("2683fad669f20ec49b8e9656954a33a8", "ltspass_transparent", "Hidden/ltspass_transparent"),
];

/// Identify a shader by its reference and, when the shader file is in the package, its name.
pub fn identify(shader: ObjRef, name_in_package: Option<&str>) -> ShaderInfo {
    let mut info = ShaderInfo {
        family: ShaderFamily::Unknown,
        mode: None,
        outline: false,
        variant: Variant::Normal,
        name: name_in_package.map(str::to_string),
        guid: shader.guid,
    };
    let guid = shader.guid.map(|g| g.to_string());
    if guid.as_deref() == Some(BUILTIN_GUID) {
        // fileID 46 is Standard, 45 Standard (Specular setup).
        info.family = if matches!(shader.file_id, 45 | 46) { ShaderFamily::Standard } else { ShaderFamily::UnityBuiltin };
        return info;
    }
    if let Some((_, file, name)) = LILTOON_SHADERS.iter().find(|(g, _, _)| Some(*g) == guid.as_deref()) {
        liltoon_variant(file, &mut info);
        info.name = Some(name.to_string());
        return info;
    }
    if let Some(name) = name_in_package {
        let lower = name.to_ascii_lowercase();
        info.family = if lower.contains("poiyomi") {
            ShaderFamily::Poiyomi
        } else if lower.contains("mtoon") {
            ShaderFamily::MToon
        } else if lower.contains("liltoon") {
            ShaderFamily::LilToon
        } else if lower == "standard" || lower == "standard (specular setup)" {
            ShaderFamily::Standard
        } else {
            ShaderFamily::Unknown
        };
    }
    info
}

/// Decode a lilToon shader file name such as `lts_twotrans_o` or `ltsl_cutout`.
fn liltoon_variant(file: &str, info: &mut ShaderInfo) {
    let (family, rest) = if let Some(rest) = file.strip_prefix("ltsmulti") {
        (ShaderFamily::LilToonMulti, rest)
    } else if let Some(rest) = file.strip_prefix("ltspass") {
        (ShaderFamily::LilToon, rest)
    } else if let Some(rest) = file.strip_prefix("ltsl") {
        (ShaderFamily::LilToonLite, rest)
    } else {
        (ShaderFamily::LilToon, file.strip_prefix("lts").unwrap_or(file))
    };
    info.family = family;
    let parts: Vec<&str> = rest.split('_').filter(|p| !p.is_empty()).collect();
    let has = |p: &str| parts.contains(&p);
    info.outline = has("o") || has("oo");
    info.variant = if file.starts_with("ltspass") {
        Variant::Internal
    } else if has("oo") {
        Variant::OutlineOnly
    } else if has("fakeshadow") {
        Variant::FakeShadow
    } else if has("fur") || has("furonly") {
        Variant::Fur
    } else if has("gem") {
        Variant::Gem
    } else if has("ref") {
        Variant::Refraction
    } else if has("overlay") {
        Variant::Overlay
    } else {
        Variant::Normal
    };
    info.mode = if family == ShaderFamily::LilToonMulti {
        None
    } else if has("cutout") {
        Some(RenderMode::Cutout)
    } else if ["trans", "onetrans", "twotrans", "two", "furonly", "fur", "gem", "ref", "overlay"].iter().any(|p| has(p)) {
        Some(RenderMode::Transparent)
    } else {
        Some(RenderMode::Opaque)
    };
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
        for (g, file, _) in LILTOON_SHADERS {
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
