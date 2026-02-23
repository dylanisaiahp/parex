use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use crate::error::ParexError;
use crate::results::{Results, ScanStats};
use crate::traits::Matcher;

// ---------------------------------------------------------------------------
// WalkConfig
// ---------------------------------------------------------------------------

/// Traversal parameters passed from the builder to the engine and source.
///
/// Sources receive this so they can honour depth limits, thread counts,
/// and result limits during their own traversal logic.
pub struct WalkConfig {
    pub threads: usize,
    pub max_depth: Option<usize>,
    pub(crate) limit: Option<usize>,
}

// ---------------------------------------------------------------------------
// Engine options
// ---------------------------------------------------------------------------

/// Internal options passed from the builder to `run()`.
pub(crate) struct EngineOptions {
    pub config: WalkConfig,
    pub source: Box<dyn crate::traits::Source>,
    pub matcher: Arc<dyn Matcher>,
    pub collect_paths: bool,
    pub collect_errors: bool,
}

// ---------------------------------------------------------------------------
// run()
// ---------------------------------------------------------------------------

/// Execute a search using the source's iterator.
///
/// Iterates `Result<Entry, ParexError>` items from `source.walk()`.
/// `Ok` entries are matched and collected. `Err` entries are counted as
/// recoverable errors and stored in `Results::errors` when
/// `collect_errors` is enabled.
///
/// Uses plain locals instead of `Arc<Mutex>` / `Arc<AtomicUsize>` — the
/// engine is single-consumer, so shared-state primitives add overhead with
/// no benefit.
pub(crate) fn run(opts: EngineOptions) -> Results {
    let start = Instant::now();

    let entries = opts.source.walk(&opts.config);

    let limit = opts.config.limit;
    let collect_paths = opts.collect_paths;
    let collect_errors = opts.collect_errors;
    let matcher = opts.matcher;

    let mut matches = 0usize;
    let mut files = 0usize;
    let mut dirs = 0usize;
    let mut paths: Vec<PathBuf> = if collect_paths {
        Vec::with_capacity(1024)
    } else {
        Vec::new()
    };
    let mut errors: Vec<ParexError> = if collect_errors {
        Vec::with_capacity(64)
    } else {
        Vec::new()
    };

    for item in entries {
        // Enforce limit before processing next item
        if let Some(lim) = limit {
            if matches >= lim {
                break;
            }
        }

        let entry = match item {
            Ok(e) => e,
            Err(err) => {
                if collect_errors && err.is_recoverable() {
                    errors.push(err);
                }
                continue;
            }
        };

        // Count by kind
        match entry.kind {
            crate::entry::EntryKind::Dir => dirs += 1,
            crate::entry::EntryKind::File => files += 1,
            _ => {}
        }

        if !matcher.is_match(&entry) {
            continue;
        }

        matches += 1;

        if collect_paths {
            paths.push(entry.path.clone());
        }

        if let Some(lim) = limit {
            if matches >= lim {
                break;
            }
        }
    }

    let duration = start.elapsed();

    let matches = match limit {
        Some(lim) => matches.min(lim),
        None => matches,
    };

    Results {
        matches,
        paths,
        stats: ScanStats::compute(files, dirs, duration),
        errors,
    }
}
