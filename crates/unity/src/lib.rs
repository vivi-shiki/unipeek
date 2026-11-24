//! GPU-free half of unipeek: reads Unity asset packages and works out what should be shown.
pub mod error;
pub mod guid;
pub mod source;

pub use error::{Error, Result};
pub use guid::Guid;
pub use source::{Input, Source};
