use std::path::PathBuf;
use std::time::Duration;

use crate::error::ParexError;

/// The output of a completed search.
///
/// `paths` and `errors` are both opt-in — disabled by default to avoid
/// allocation overhead in the common case. Enable them on the builder:
/// `.collect_paths(true)` and `.collect_errors(true)`.
pub struct Results {
    /// Total number of entries that matched the search criteria.
    pub matches: usize,

    /// Paths of matched entries, in the order they were found.
    /// Only populated if `.collect_paths(true)` was set on the builder.
    pub paths: Vec<PathBuf>,

    /// Scan performance statistics.
    pub stats: ScanStats,

    /// Non-fatal errors encountered during the search (permission denied, etc.).
    /// Only populated if `.collect_errors(true)` was set on the builder.
    /// Use [`ParexError::is_recoverable`] to distinguish warnings from failures.
    pub errors: Vec<ParexError>,
}

/// Performance statistics for a completed scan.
pub struct ScanStats {
    /// Total number of files encountered (matched or not).
    pub files: usize,

    /// Total number of directories encountered.
    pub dirs: usize,

    /// Wall-clock time from search start to completion.
    pub duration: Duration,

    /// Total entries scanned per second. Convenience field — equals
    /// `(files + dirs) / duration.as_secs_f64()`, clamped to 0 on
    /// zero-duration runs.
    pub entries_per_sec: usize,
}

impl ScanStats {
    /// Compute `entries_per_sec` from raw counts and duration.
    pub(crate) fn compute(files: usize, dirs: usize, duration: Duration) -> Self {
        let total = files + dirs;
        let eps = if duration.as_secs_f64() > 0.0 {
            (total as f64 / duration.as_secs_f64()) as usize
        } else {
            0
        };
        Self {
            files,
            dirs,
            duration,
            entries_per_sec: eps,
        }
    }
}
