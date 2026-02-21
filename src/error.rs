use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug)]
#[non_exhaustive]
pub enum ParexError {
    // Traversal
    #[error("permission denied: {0}")]
    PermissionDenied(PathBuf),

    #[error("path not found: {0}")]
    NotFound(PathBuf),

    #[error("invalid source: {0}")]
    InvalidSource(PathBuf),

    #[error("symlink loop: {0}")]
    SymlinkLoop(PathBuf),

    // Config
    #[error("invalid pattern: {0}")]
    InvalidPattern(String),

    #[error("invalid thread count: {0}")]
    InvalidThreadCount(usize),

    // Runtime
    #[error("thread pool failure: {0}")]
    ThreadPool(String),

    #[error("IO error at {path}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    // Third-party extensibility — Box<dyn Error> preserves original error type
    // and enables proper chaining via thiserror's #[source]
    #[error("source error: {0}")]
    Source(#[source] Box<dyn std::error::Error + Send + Sync>),

    #[error("matcher error: {0}")]
    Matcher(#[source] Box<dyn std::error::Error + Send + Sync>),
}

impl ParexError {
    /// The path this error occurred at, if applicable.
    /// Callers can present "Skipped: <path>" without pattern matching on variants.
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
    /// Recoverable errors (permission denied, not found, symlink loops, IO)
    /// are collected and surfaced after the search completes — the walk keeps going.
    ///
    /// Fatal errors (invalid source, thread pool failure) should halt immediately.
    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            Self::PermissionDenied(_)
                | Self::NotFound(_)
                | Self::SymlinkLoop(_)
                | Self::Io { .. }
        )
    }

    /// Whether this error should halt the search immediately.
    ///
    /// Inverse of [`is_recoverable`](Self::is_recoverable).
    pub fn is_fatal(&self) -> bool {
        !self.is_recoverable()
    }

    /// Convenience constructor for source errors from third-party types.
    ///
    /// Prefer this over `ParexError::Source(Box::new(e))` for cleaner call sites.
    pub fn source_err(e: impl std::error::Error + Send + Sync + 'static) -> Self {
        Self::Source(Box::new(e))
    }

    /// Convenience constructor for matcher errors from third-party types.
    ///
    /// Prefer this over `ParexError::Matcher(Box::new(e))` for cleaner call sites.
    pub fn matcher_err(e: impl std::error::Error + Send + Sync + 'static) -> Self {
        Self::Matcher(Box::new(e))
    }
}
