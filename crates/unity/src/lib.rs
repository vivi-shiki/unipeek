//! GPU-free half of unipeek: reads Unity asset packages and works out what should be shown.
pub mod error;
pub mod guid;
pub mod load;
pub mod material;
pub mod meta;
pub mod package;
pub mod plan;
pub mod prefab;
pub mod report;
pub mod shader;
pub mod source;
#[cfg(any(test, feature = "testing"))]
pub mod testing;
pub mod yaml;

pub use error::{Error, Result};
pub use guid::Guid;
pub use load::{Container, Package};
pub use plan::{Binding, Plan};
pub use report::{Degrade, Report, Status, Timings};
pub use package::{AssetKind, Entry, IndexOptions, PackageIndex, PackageIssue, ReadStats};
pub use source::{Input, Source};
