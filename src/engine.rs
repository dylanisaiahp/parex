use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
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
pub(crate) fn run(opts: EngineOptions) -> Results {
    let start = Instant::now();

    let entries = opts.source.walk(&opts.config);

    let matches = Arc::new(AtomicUsize::new(0));
    let files = Arc::new(AtomicUsize::new(0));
    let dirs = Arc::new(AtomicUsize::new(0));
    let paths = Arc::new(Mutex::new(Vec::<PathBuf>::new()));
    let errors = Arc::new(Mutex::new(Vec::<ParexError>::new()));

    let limit = opts.config.limit;
    let collect_paths = opts.collect_paths;
    let collect_errors = opts.collect_errors;
    let matcher = &opts.matcher;

    for item in entries {
        // Enforce limit before processing next item
        if let Some(lim) = limit {
            if matches.load(Ordering::Relaxed) >= lim {
                break;
            }
        }

        let entry = match item {
            Ok(e) => e,
            Err(err) => {
                // Recoverable error — collect if requested, keep walking
                if collect_errors && err.is_recoverable() {
                    if let Ok(mut errs) = errors.lock() {
                        errs.push(err);
                    }
                }
                continue;
            }
        };

        // Count by kind
        match entry.kind {
            crate::entry::EntryKind::Dir => {
                dirs.fetch_add(1, Ordering::Relaxed);
            }
            crate::entry::EntryKind::File => {
                files.fetch_add(1, Ordering::Relaxed);
            }
            _ => {}
        }

        if !matcher.is_match(&entry) {
            continue;
        }

        let mc = matches.fetch_add(1, Ordering::Relaxed) + 1;

        if collect_paths {
            if let Ok(mut p) = paths.lock() {
                p.push(entry.path.clone());
            }
        }

        if let Some(lim) = limit {
            if mc >= lim {
                break;
            }
        }
    }

    let duration = start.elapsed();
    let matches = matches.load(Ordering::Relaxed);
    let files = files.load(Ordering::Relaxed);
    let dirs = dirs.load(Ordering::Relaxed);
    let paths = Arc::try_unwrap(paths)
        .unwrap_or_default()
        .into_inner()
        .unwrap_or_default();
    let errors = Arc::try_unwrap(errors)
        .unwrap_or_default()
        .into_inner()
        .unwrap_or_default();

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
