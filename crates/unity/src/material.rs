//! `.mat` files and the approximate look unipeek derives from them.
use crate::{
    shader::{RenderMode, ShaderInfo},
    yaml::ObjRef,
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
    pub fn parse(_text: &str) -> Option<Material> {
        todo!()
    }

    pub fn texture(&self, _name: &str) -> Option<&TexEnv> {
        todo!()
    }

    pub fn float(&self, _name: &str) -> Option<f32> {
        todo!()
    }

    pub fn color(&self, _name: &str) -> Option<[f32; 4]> {
        todo!()
    }

    /// The first main-texture property that points at a texture.
    pub fn main_texture(&self) -> Option<&TexEnv> {
        todo!()
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

    pub fn of(_material: &Material, _shader: &ShaderInfo) -> Look {
        todo!()
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
