//! Errors of the rendering layer. Problems with single assets are degrade reasons in the report.

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Unity(#[from] unipeek_unity::Error),
    #[error("no GPU adapter: {0}")]
    NoGpu(String),
    #[error("gpu: {0}")]
    Gpu(String),
    #[error("nothing to show in {0}")]
    NothingToShow(String),
    #[error("image: {0}")]
    Image(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
