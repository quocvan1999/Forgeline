use std::path::PathBuf;

/// Recoverable indexing failure. Parse errors inside a file are skipped, not fatal.
#[derive(Debug, thiserror::Error)]
pub enum GraphError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("failed to load the {0} grammar")]
    Grammar(&'static str),
    #[error("path does not exist: {0}")]
    Missing(PathBuf),
    #[error("path is not a directory: {0}")]
    NotADirectory(PathBuf),
    #[error("refusing to index filesystem root {0}")]
    FilesystemRoot(PathBuf),
    #[error("source has more than {limit} supported files")]
    TooManyFiles { limit: usize },
    #[error("unsupported source file {0}")]
    Unsupported(String),
}
