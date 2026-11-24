//! unipeek: preview Unity asset packages. Loads a `.unitypackage`, zip, folder or FBX on the CPU
//! (`Scene::load`), renders toon-style thumbnails on a headless GPU (`render_thumbnail`), and runs
//! the `serve` line protocol for hosts that want rendering in a child process.
//! The GPU-free parsing layer is re-exported as `unity`.
pub mod camera;
pub mod error;
pub mod gpu;
pub mod model;
pub mod renderer;
pub mod scene;
pub mod serve;
pub mod texture;
pub mod thumb;

pub use error::{Error, Result};
pub use glam;
pub use gpu::Gpu;
pub use scene::{LoadOptions, Scene};
pub use serve::serve;
pub use thumb::{Background, ImageFormat, RENDERER_VERSION, ThumbOptions, View, encode, render_thumbnail, write_thumbnail};
pub use unipeek_unity as unity;
pub use unipeek_unity::{Input, Report, Source, Status};
pub use wgpu;
