//! `.mat` files and the approximate look unipeek derives from them.
use crate::{
    shader::{RenderMode, ShaderFamily, ShaderInfo, Variant},
    yaml::{self, Node, ObjRef},
};

#[derive(Clone, Debug, PartialEq)]
pub struct TexEnv {
    pub name: String,
    pub texture: ObjRef,
    pub scale: [f32; 2],
    pub offset: [f32; 2],
}

#[derive(Clone, Debug, PartialEq)]
pub struct Material {
    pub name: String,
    pub shader: Option<ObjRef>,
    /// `m_CustomRenderQueue`; -1 means the shader's default.
    pub render_queue: i32,
    pub textures: Vec<TexEnv>,
    pub floats: Vec<(String, f32)>,
    pub colors: Vec<(String, [f32; 4])>,
    /// Shader keywords (`m_ShaderKeywords` or `m_ValidKeywords`), e.g. `_EMISSION`.
    pub keywords: Vec<String>,
}

/// Property names of the main texture, in the order shaders commonly use them.
pub const MAIN_TEXTURE_NAMES: &[&str] = &["_MainTex", "_BaseMap", "_BaseColorMap", "_MainTexture"];

impl Material {
    /// `None` when the text has no `Material` document (binary serialization, other asset).
    pub fn parse(text: &str) -> Option<Material> {
        let doc = yaml::parse_documents(text).into_iter().find(|d| d.class_id == 21)?;
        let body = &doc.body;
        let props = body.get("m_SavedProperties");
        let pairs = |key: &str| props.and_then(|p| p.get(key)).map(Node::items).unwrap_or_default().iter().filter_map(|n| n.entries().first());
        let vec2 = |n: Option<&Node>, d: f32| [n.and_then(|v| v.get("x")).and_then(Node::as_f32).unwrap_or(d), n.and_then(|v| v.get("y")).and_then(Node::as_f32).unwrap_or(d)];
        let textures = pairs("m_TexEnvs")
            .filter_map(|(name, v)| {
                Some(TexEnv {
                    name: name.clone(),
                    texture: v.get("m_Texture")?.reference()?,
                    scale: vec2(v.get("m_Scale"), 1.0),
                    offset: vec2(v.get("m_Offset"), 0.0),
                })
            })
            .collect();
        let floats = pairs("m_Floats").chain(pairs("m_Ints")).filter_map(|(k, v)| Some((k.clone(), v.as_f32()?))).collect();
        let colors = pairs("m_Colors")
            .filter_map(|(k, v)| {
                let c = |key| v.get(key).and_then(Node::as_f32);
                Some((k.clone(), [c("r")?, c("g")?, c("b")?, c("a").unwrap_or(1.0)]))
            })
            .collect();
        Some(Material {
            name: body.get("m_Name").and_then(Node::as_str).unwrap_or_default().to_string(),
            shader: body.get("m_Shader").and_then(Node::reference),
            render_queue: body.get("m_CustomRenderQueue").and_then(Node::as_i64).unwrap_or(-1) as i32,
            textures,
            floats,
            colors,
            keywords: {
                let mut k: Vec<String> = body.get("m_ShaderKeywords").and_then(Node::as_str).unwrap_or_default().split_whitespace().map(str::to_string).collect();
                k.extend(body.get("m_ValidKeywords").map(Node::items).unwrap_or_default().iter().filter_map(Node::as_str).map(str::to_string));
                k
            },
        })
    }

    pub fn texture(&self, name: &str) -> Option<&TexEnv> {
        self.textures.iter().find(|t| t.name == name)
    }

    pub fn float(&self, name: &str) -> Option<f32> {
        self.floats.iter().find(|(k, _)| k == name).map(|(_, v)| *v)
    }

    pub fn color(&self, name: &str) -> Option<[f32; 4]> {
        self.colors.iter().find(|(k, _)| k == name).map(|(_, v)| *v)
    }

    /// The first main-texture property that points at a texture.
    pub fn main_texture(&self) -> Option<&TexEnv> {
        MAIN_TEXTURE_NAMES.iter().filter_map(|n| self.texture(n)).find(|t| !t.texture.is_null())
    }

    fn first_color(&self, names: &[&str]) -> Option<[f32; 4]> {
        names.iter().find_map(|n| self.color(n))
    }

    /// The texture of the first of `names` that points at one.
    fn map(&self, names: &[&str]) -> Option<crate::Guid> {
        names.iter().find_map(|n| self.textures.iter().find(|t| t.name == *n && !t.texture.is_null())).and_then(|t| t.texture.guid)
    }

    fn on(&self, name: &str) -> bool {
        self.float(name).is_some_and(|v| v >= 0.5)
    }

    /// The extra textures the approximate look uses besides the main texture.
    pub fn maps(&self, shader: &ShaderInfo) -> Maps {
        use ShaderFamily::*;
        let lil = matches!(shader.family, LilToon | LilToonLite | LilToonMulti);
        let normal = match shader.family {
            LilToon | LilToonMulti if !self.on("_UseBumpMap") => None,
            LilToonLite | VrchatMobile | UnityBuiltin => None,
            _ => self.map(&["_BumpMap", "_NormalMap"]),
        };
        let emission = match shader.family {
            _ if lil && !self.on("_UseEmission") => None,
            Poiyomi if !self.on("_EnableEmission") => None,
            Standard if !self.keywords.iter().any(|k| k == "_EMISSION") => None,
            Unknown | UnityBuiltin | VrchatMobile => None,
            _ => self.map(&["_EmissionMap"]),
        };
        let matcap = match shader.family {
            LilToon | LilToonMulti if self.on("_UseMatCap") => self.map(&["_MatCapTex"]),
            Poiyomi if self.on("_MatcapEnable") => self.map(&["_Matcap"]),
            MToon => self.map(&["_SphereAdd"]),
            VrchatMobile if shader.name.as_deref().is_some_and(|n| n.contains("MatCap")) => self.map(&["_MatCap"]),
            _ => None,
        };
        Maps { normal, emission, matcap }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Cull {
    Off,
    Front,
    Back,
}

/// Textures besides the main texture, by GUID.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize)]
pub struct Maps {
    pub normal: Option<crate::Guid>,
    pub emission: Option<crate::Guid>,
    pub matcap: Option<crate::Guid>,
}

/// How a MatCap texture is combined with the lit color.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MatcapBlend {
    /// Replace the color by the MatCap, weighted.
    Mix,
    Add,
    Multiply,
}

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize)]
pub struct Matcap {
    /// Tint; alpha is the weight.
    pub color: [f32; 4],
    pub blend: MatcapBlend,
}

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize)]
pub struct Outline {
    /// In meters.
    pub width: f32,
    pub color: [f32; 4],
}

/// What the renderer needs to approximate a material. Colors are linear 0..1 as stored by Unity.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize)]
pub struct Look {
    pub mode: RenderMode,
    pub color: [f32; 4],
    pub shadow_color: [f32; 3],
    pub cull: Cull,
    pub cutoff: f32,
    pub outline: Option<Outline>,
    /// Draw the surface. False for outline-only and fake-shadow passes.
    pub draw_surface: bool,
    /// Strength of the normal map, when the material has one.
    pub normal_scale: f32,
    /// Emitted color, added after lighting (multiplied by the emission map, if any). Black is none.
    pub emission: [f32; 3],
    /// MatCap, when the material has one (the texture is in `Maps::matcap`).
    pub matcap: Option<Matcap>,
}

impl Look {
    pub fn white() -> Look {
        Look {
            mode: RenderMode::Opaque,
            color: [1.0; 4],
            shadow_color: [0.82, 0.76, 0.85],
            cull: Cull::Back,
            cutoff: 0.5,
            outline: None,
            draw_surface: true,
            normal_scale: 1.0,
            emission: [0.0; 3],
            matcap: None,
        }
    }

    pub fn of(material: &Material, shader: &ShaderInfo) -> Look {
        let mut look = Look::white();
        use ShaderFamily::*;
        let by_property = match shader.family {
            // Standard and Poiyomi: 0 opaque, 1 cutout, more is a kind of transparency.
            Standard | Poiyomi => material.float("_Mode"),
            MToon => material.float("_BlendMode"),
            _ => None,
        }
        .map(|m| match m.round() as i32 {
            0 => RenderMode::Opaque,
            1 => RenderMode::Cutout,
            _ => RenderMode::Transparent,
        });
        look.mode = shader.mode.or(by_property).unwrap_or(match material.render_queue {
            q if q >= 3000 => RenderMode::Transparent,
            q if q >= 2450 => RenderMode::Cutout,
            _ => RenderMode::Opaque,
        });
        if let Some(c) = material.first_color(&["_Color", "_BaseColor", "_MainColor"]) {
            look.color = c;
        }
        if let Some([r, g, b, _]) = material.color("_ShadowColor") {
            look.shadow_color = [r, g, b];
        }
        if shader.family == MToon
            && let Some([r, g, b, _]) = material.color("_ShadeColor")
        {
            look.shadow_color = [r, g, b];
        }
        look.cull = match material.float("_Cull").or_else(|| material.float("_CullMode")).map(|c| c.round() as i32) {
            Some(0) => Cull::Off,
            Some(1) => Cull::Front,
            _ => Cull::Back,
        };
        look.cutoff = material.float("_Cutoff").unwrap_or(0.5);
        let lil = matches!(shader.family, ShaderFamily::LilToon | ShaderFamily::LilToonLite | ShaderFamily::LilToonMulti);
        let wants_outline = shader.outline || (shader.family == ShaderFamily::LilToonMulti && material.float("_UseOutline") == Some(1.0));
        if lil && wants_outline {
            // lilToon stores the outline width in centimeters.
            let width = material.float("_OutlineWidth").unwrap_or(0.08) * 0.01;
            let color = material.color("_OutlineColor").unwrap_or([0.6, 0.56, 0.73, 1.0]);
            if width > 0.0 {
                look.outline = Some(Outline { width, color });
            }
        }
        if shader.family == MToon && material.on("_OutlineWidthMode") {
            // The world-space outline width of MToon is in centimeters too.
            let width = material.float("_OutlineWidth").unwrap_or(0.5) * 0.01;
            if width > 0.0 {
                look.outline = Some(Outline { width, color: material.color("_OutlineColor").unwrap_or([0.0, 0.0, 0.0, 1.0]) });
            }
        }
        look.draw_surface = !matches!(shader.variant, Variant::OutlineOnly | Variant::FakeShadow | Variant::Internal);

        let maps = material.maps(shader);
        if maps.normal.is_some() {
            look.normal_scale = material.float("_BumpScale").unwrap_or(1.0);
        }
        let emission_on = match shader.family {
            LilToon | LilToonLite | LilToonMulti => material.on("_UseEmission"),
            Poiyomi => material.on("_EnableEmission"),
            Standard => material.keywords.iter().any(|k| k == "_EMISSION"),
            MToon => true,
            _ => false,
        };
        if emission_on && let Some([r, g, b, _]) = material.color("_EmissionColor") {
            let strength = if shader.family == Poiyomi { material.float("_EmissionStrength").unwrap_or(1.0) } else { 1.0 };
            look.emission = [r * strength, g * strength, b * strength];
        }
        if maps.matcap.is_some() {
            look.matcap = Some(match shader.family {
                Poiyomi => {
                    let [r, g, b, _] = material.color("_MatcapColor").unwrap_or([1.0; 4]);
                    let weight = material.float("_MatcapIntensity").unwrap_or(1.0);
                    let blend = if material.float("_MatcapAdd").unwrap_or(0.0) > 0.0 {
                        MatcapBlend::Add
                    } else if material.float("_MatcapMultiply").unwrap_or(0.0) > 0.0 {
                        MatcapBlend::Multiply
                    } else {
                        MatcapBlend::Mix
                    };
                    Matcap { color: [r, g, b, weight.clamp(0.0, 1.0)], blend }
                }
                MToon | VrchatMobile => Matcap { color: [1.0; 4], blend: MatcapBlend::Add },
                _ => {
                    let [r, g, b, a] = material.color("_MatCapColor").unwrap_or([1.0; 4]);
                    let blend = match material.float("_MatCapBlendMode").map(|m| m.round() as i32) {
                        Some(1 | 2) => MatcapBlend::Add,
                        Some(3) => MatcapBlend::Multiply,
                        _ => MatcapBlend::Mix,
                    };
                    Matcap { color: [r, g, b, a * material.float("_MatCapBlend").unwrap_or(1.0)], blend }
                }
            });
        }
        look
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Guid,
        shader::{self, ShaderFamily},
        testing::material_yaml,
        yaml::ObjRef,
    };

    const LIL_OUTLINE: &str = "efa77a80ca0344749b4f19fdd5891cbe";
    const LIL_TWOTRANS_O: &str = "9cf054060007d784394b8b0bb703e441";
    const LIL_FAKESHADOW: &str = "00795bf598b44dc4e9bd363348e77085";
    const TEX: &str = "4bbb1657292f0d74dab36e002aa5696d";

    fn info(guid: &str, file_id: i64) -> ShaderInfo {
        shader::identify(ObjRef { file_id, guid: Guid::parse(guid), kind: Some(3) }, None)
    }

    #[test]
    fn parses_shader_textures_floats_colors() {
        let m = Material::parse(&material_yaml("Body", LIL_OUTLINE, 4800000, Some(TEX), 2000)).unwrap();
        assert_eq!(m.name, "Body");
        assert_eq!(m.shader.unwrap().guid, Guid::parse(LIL_OUTLINE));
        assert_eq!(m.render_queue, 2000);
        assert_eq!(m.textures.len(), 2);
        let main = m.main_texture().unwrap();
        assert_eq!(main.name, "_MainTex");
        assert_eq!(main.texture.guid, Guid::parse(TEX));
        assert_eq!(main.scale, [1.0, 1.0]);
        assert!(m.texture("_BumpMap").unwrap().texture.is_null());
        assert_eq!(m.float("_Cutoff"), Some(0.5));
        assert_eq!(m.color("_Color"), Some([1.0, 0.5, 0.25, 1.0]));
        assert_eq!(m.float("_Nope"), None);
    }

    #[test]
    fn main_texture_skips_empty_slots() {
        let m = Material::parse(&material_yaml("Body", LIL_OUTLINE, 4800000, None, -1)).unwrap();
        assert!(m.main_texture().is_none());
    }

    #[test]
    fn not_a_material() {
        assert!(Material::parse("%YAML 1.1\n--- !u!1 &1\nGameObject:\n  m_Name: x\n").is_none());
        assert!(Material::parse("").is_none());
    }

    #[test]
    fn liltoon_outline_variant_gives_outline_and_colors() {
        let m = Material::parse(&material_yaml("Body", LIL_OUTLINE, 4800000, Some(TEX), 2000)).unwrap();
        let s = info(LIL_OUTLINE, 4800000);
        assert_eq!(s.family, ShaderFamily::LilToon);
        let look = Look::of(&m, &s);
        assert_eq!(look.mode, RenderMode::Opaque);
        assert_eq!(look.color, [1.0, 0.5, 0.25, 1.0]);
        assert_eq!(look.shadow_color, [0.8, 0.7, 0.9]);
        assert_eq!(look.cull, Cull::Back);
        let outline = look.outline.unwrap();
        assert!((outline.width - 0.0008).abs() < 1e-6, "lilToon widths are centimeters: {}", outline.width);
        assert_eq!(outline.color, [0.2, 0.1, 0.1, 1.0]);
        assert!(look.draw_surface);
    }

    #[test]
    fn transparent_variant_wins_over_queue() {
        let m = Material::parse(&material_yaml("Alpha", LIL_TWOTRANS_O, 4800000, None, 2000)).unwrap();
        let look = Look::of(&m, &info(LIL_TWOTRANS_O, 4800000));
        assert_eq!(look.mode, RenderMode::Transparent);
        assert!(look.outline.is_some());
    }

    #[test]
    fn fake_shadow_is_not_drawn() {
        let m = Material::parse(&material_yaml("Hair_fakeshadow", LIL_FAKESHADOW, 4800000, None, 2450)).unwrap();
        assert!(!Look::of(&m, &info(LIL_FAKESHADOW, 4800000)).draw_surface);
    }

    #[test]
    fn unknown_shader_uses_queue_and_cull() {
        let unknown = "0123456789abcdef0123456789abcdef";
        let text = material_yaml("Glass", unknown, 4800000, None, 3000).replace("_Cull: 2", "_Cull: 0");
        let m = Material::parse(&text).unwrap();
        let s = info(unknown, 4800000);
        assert_eq!(s.family, ShaderFamily::Unknown);
        let look = Look::of(&m, &s);
        assert_eq!(look.mode, RenderMode::Transparent);
        assert_eq!(look.cull, Cull::Off);
        assert_eq!(look.outline, None);
        let cut = Material::parse(&material_yaml("Leaf", unknown, 4800000, None, 2450)).unwrap();
        assert_eq!(Look::of(&cut, &s).mode, RenderMode::Cutout);
        let opaque = Material::parse(&material_yaml("Skin", unknown, 4800000, None, -1)).unwrap();
        assert_eq!(Look::of(&opaque, &s).mode, RenderMode::Opaque);
    }

    const NORMAL: &str = "1111111111111111aaaaaaaaaaaaaaaa";
    const GLOW: &str = "2222222222222222aaaaaaaaaaaaaaaa";
    const CAP: &str = "3333333333333333aaaaaaaaaaaaaaaa";
    const SHADER: &str = "4444444444444444aaaaaaaaaaaaaaaa";

    /// A material with extra float, color and texture properties and keywords.
    fn with(floats: &[(&str, f32)], colors: &[(&str, [f32; 4])], textures: &[(&str, &str)], keywords: &str) -> Material {
        let mut yaml = material_yaml("M", SHADER, 4800000, Some(TEX), -1);
        let tex: String = textures.iter().map(|(n, g)| format!("    - {n}:\n        m_Texture: {{fileID: 2800000, guid: {g}, type: 3}}\n        m_Scale: {{x: 1, y: 1}}\n        m_Offset: {{x: 0, y: 0}}\n")).collect();
        yaml = yaml.replace("    m_Ints: []\n", &format!("{tex}    m_Ints: []\n"));
        let fl: String = floats.iter().map(|(n, v)| format!("    - {n}: {v}\n")).collect();
        yaml = yaml.replace("    - _Cull: 2\n", &format!("    - _Cull: 2\n{fl}"));
        let co: String = colors.iter().map(|(n, [r, g, b, a])| format!("    - {n}: {{r: {r}, g: {g}, b: {b}, a: {a}}}\n")).collect();
        yaml = yaml.replace("    m_Colors:\n", &format!("    m_Colors:\n{co}"));
        yaml = yaml.replace("  m_CustomRenderQueue:", &format!("  m_ShaderKeywords: {keywords}\n  m_CustomRenderQueue:"));
        Material::parse(&yaml).unwrap()
    }

    fn named(name: &str) -> ShaderInfo {
        shader::identify(ObjRef { file_id: 4800000, guid: Guid::parse(SHADER), kind: Some(3) }, Some(name))
    }

    #[test]
    fn liltoon_maps_only_when_their_switch_is_on() {
        let textures = [("_BumpMap", NORMAL), ("_EmissionMap", GLOW), ("_MatCapTex", CAP)];
        let off = with(&[], &[("_EmissionColor", [1.0, 0.5, 0.0, 1.0])], &textures, "");
        let lil = info("df12117ecd77c31469c224178886498e", 4800000);
        assert_eq!(off.maps(&lil), Maps::default());
        assert_eq!(Look::of(&off, &lil).emission, [0.0; 3]);
        let on = with(&[("_UseBumpMap", 1.0), ("_BumpScale", 0.5), ("_UseEmission", 1.0), ("_UseMatCap", 1.0), ("_MatCapBlendMode", 1.0), ("_MatCapBlend", 0.5)], &[("_EmissionColor", [1.0, 0.5, 0.0, 1.0]), ("_MatCapColor", [1.0, 1.0, 1.0, 1.0])], &textures, "");
        let maps = on.maps(&lil);
        assert_eq!((maps.normal, maps.emission, maps.matcap), (Guid::parse(NORMAL), Guid::parse(GLOW), Guid::parse(CAP)));
        let look = Look::of(&on, &lil);
        assert_eq!((look.normal_scale, look.emission), (0.5, [1.0, 0.5, 0.0]));
        assert_eq!(look.matcap, Some(Matcap { color: [1.0, 1.0, 1.0, 0.5], blend: MatcapBlend::Add }));
    }

    #[test]
    fn poiyomi_mode_emission_and_matcap() {
        let poi = named(".poiyomi/Poiyomi 8.1/Poiyomi Toon");
        let m = with(&[("_Mode", 1.0), ("_EnableEmission", 1.0), ("_EmissionStrength", 2.0), ("_MatcapEnable", 1.0), ("_MatcapIntensity", 0.7), ("_MatcapMultiply", 1.0)], &[("_EmissionColor", [0.5, 0.25, 0.0, 1.0])], &[("_Matcap", CAP), ("_BumpMap", NORMAL)], "");
        let look = Look::of(&m, &poi);
        assert_eq!(look.mode, RenderMode::Cutout);
        assert_eq!(look.emission, [1.0, 0.5, 0.0], "strength multiplies the color");
        assert_eq!(look.matcap.unwrap().blend, MatcapBlend::Multiply);
        assert_eq!(m.maps(&poi).normal, Guid::parse(NORMAL), "Poiyomi uses its normal map without a switch");
    }

    #[test]
    fn mtoon_shade_color_outline_and_sphere() {
        let mtoon = named("VRM/MToon");
        let m = with(&[("_BlendMode", 2.0), ("_CullMode", 0.0), ("_OutlineWidthMode", 1.0), ("_OutlineWidth", 0.2)], &[("_ShadeColor", [0.6, 0.5, 0.7, 1.0]), ("_OutlineColor", [0.1, 0.0, 0.0, 1.0])], &[("_SphereAdd", CAP)], "");
        let look = Look::of(&m, &mtoon);
        assert_eq!(look.mode, RenderMode::Transparent);
        assert_eq!(look.shadow_color, [0.6, 0.5, 0.7]);
        assert_eq!(look.cull, Cull::Back, "_Cull wins over _CullMode when both are there");
        let outline = look.outline.unwrap();
        assert!((outline.width - 0.002).abs() < 1e-6);
        assert_eq!(look.matcap.unwrap().blend, MatcapBlend::Add);
    }

    #[test]
    fn standard_emission_needs_the_keyword() {
        let standard = shader::identify(ObjRef { file_id: 46, guid: Guid::parse(shader::BUILTIN_GUID), kind: Some(0) }, None);
        let colors = [("_EmissionColor", [0.0, 1.0, 0.0, 1.0])];
        let plain = with(&[("_Mode", 3.0)], &colors, &[("_EmissionMap", GLOW)], "");
        assert_eq!(Look::of(&plain, &standard).emission, [0.0; 3]);
        assert_eq!(Look::of(&plain, &standard).mode, RenderMode::Transparent);
        let glowing = with(&[], &colors, &[("_EmissionMap", GLOW)], "_EMISSION _NORMALMAP");
        assert_eq!(glowing.keywords, vec!["_EMISSION", "_NORMALMAP"]);
        assert_eq!(Look::of(&glowing, &standard).emission, [0.0, 1.0, 0.0]);
        assert_eq!(glowing.maps(&standard).emission, Guid::parse(GLOW));
    }
}
