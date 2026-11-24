//! GPU-free half of unipeek: reads Unity asset packages and works out what should be shown.
pub mod error;
pub mod guid;
pub mod package;
pub mod source;
#[cfg(any(test, feature = "testing"))]
pub mod testing;

pub use error::{Error, Result};
pub use guid::Guid;
pub use package::{AssetKind, Entry, IndexOptions, PackageIndex, PackageIssue, ReadStats};
pub use source::{Input, Source};
