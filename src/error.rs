use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ParexError {
    // Traversal
    #[error("permission denied")]
    PermissionDenied(PathBuf),

    #[error("path not found")]
    NotFound(PathBuf),

    #[error("invalid source")]
    InvalidSource(PathBuf),

    #[error("symlink loop")]
    SymlinkLoop(PathBuf),

    // Config
    #[error("invalid pattern")]
    InvalidPattern(String),

    #[error("invalid thread count")]
    InvalidThreadCount(usize),

    // Runtime
    #[error("thread pool failure")]
    ThreadPool(String),

    #[error("IO error")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    // Third-party extensibility
    #[error("source error")]
    Source(String),

    #[error("matcher error")]
    Matcher(String),
}

impl ParexError {
    /// The path this error occurred at, if applicable.
    /// Callers use this to present "Skipped: <path>" without pattern matching on variants.
    pub fn path(&self) -> Option<&PathBuf> {
        match self {
            Self::PermissionDenied(p)
            | Self::NotFound(p)
            | Self::InvalidSource(p)
            | Self::SymlinkLoop(p)
            | Self::Io { path: p, .. } => Some(p),
            _ => None,
        }
    }

    /// Whether the search can continue after this error.
    ///
    /// Recoverable errors (permission denied, symlink loops, IO) can be collected
    /// and surfaced after the search completes — the walk keeps going.
    ///
    /// Fatal errors (invalid source, thread pool failure) should halt immediately.
    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            Self::PermissionDenied(_) | Self::SymlinkLoop(_) | Self::Io { .. }
        )
    }
}
