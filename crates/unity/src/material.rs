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
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Cull {
    Off,
    Front,
    Back,
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
        }
    }

    pub fn of(material: &Material, shader: &ShaderInfo) -> Look {
        let mut look = Look::white();
        look.mode = shader.mode.unwrap_or(match material.render_queue {
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
        look.cull = match material.float("_Cull").map(|c| c.round() as i32) {
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
        look.draw_surface = !matches!(shader.variant, Variant::OutlineOnly | Variant::FakeShadow | Variant::Internal);
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
}
