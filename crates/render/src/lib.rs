//! unipeek: preview Unity asset packages. Loads a `.unitypackage`, zip, folder or FBX on the CPU
//! (`Scene::load`), renders toon-style thumbnails on a headless GPU (`render_thumbnail`), and runs
//! the `serve` line protocol for hosts that want rendering in a child process.
//! The GPU-free parsing layer is re-exported as `unity`.
pub mod camera;
pub mod error;
pub mod gltf_scene;
pub mod gpu;
pub mod model;
pub mod psd;
pub mod renderer;
pub mod scene;
pub mod serve;
#[cfg(windows)]
pub mod surface;
pub mod texture;
pub mod thumb;
pub mod viewport;

pub use error::{Error, Result};
pub use glam;
pub use gpu::Gpu;
pub use scene::{LoadEvent, LoadOptions, Scene};
pub use serve::{Worker, WorkerError, serve};
pub use thumb::{Background, ImageFormat, RENDERER_VERSION, ThumbOptions, View, encode, render_thumbnail, render_turntable, render_views, write_thumbnail, write_thumbnail_with};
pub use viewport::{Loaded, Loading, Viewport};
pub use unipeek_unity as unity;
pub use unipeek_unity::{Input, Report, Source, Status};
pub use wgpu;

/// The integration guide's Rust examples are compiled as doctests so they cannot drift from the API.
#[cfg(doctest)]
#[doc = include_str!("../../../docs/USAGE.md")]
mod usage_guide {}
