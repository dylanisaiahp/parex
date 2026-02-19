use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::results::{Results, ScanStats};
use crate::traits::Matcher;

// ---------------------------------------------------------------------------
// WalkConfig
// ---------------------------------------------------------------------------

/// Traversal parameters passed from the builder to the engine.
///
/// `pub(crate)` — not part of the public API. Callers configure these
/// via the builder methods (`.threads()`, `.max_depth()`, `.limit()`).
pub struct WalkConfig {
    pub threads:   usize,
    pub max_depth: Option<usize>,
    pub limit:     Option<usize>,
}

// ---------------------------------------------------------------------------
// Engine options
// ---------------------------------------------------------------------------

/// Internal options passed from the builder to `run()`.
pub(crate) struct EngineOptions {
    pub config:         WalkConfig,
    pub source:         Box<dyn crate::traits::Source>,
    pub matcher:        Arc<dyn Matcher>,
    pub collect_paths:  bool,
}

// ---------------------------------------------------------------------------
// run()
// ---------------------------------------------------------------------------

/// Execute a search using the source's iterator.
///
/// Iterates entries from `source.walk()` across a thread pool.
/// Called by `SearchBuilder::run()` after validating inputs.
pub(crate) fn run(opts: EngineOptions) -> Results {
    let start = Instant::now();

    // Collect entries from source upfront — sources own their traversal
    let entries: Vec<crate::entry::Entry> = opts.source.walk(&opts.config).collect();

    let matches        = Arc::new(AtomicUsize::new(0));
    let files          = Arc::new(AtomicUsize::new(0));
    let dirs           = Arc::new(AtomicUsize::new(0));
    let paths          = Arc::new(Mutex::new(Vec::<PathBuf>::new()));

    let limit          = opts.config.limit;
    let collect_paths  = opts.collect_paths;
    let matcher        = &opts.matcher;

    for entry in entries {
        // Check limit before processing
        if let Some(lim) = limit {
            if matches.load(Ordering::Relaxed) >= lim {
                break;
            }
        }

        // Count entry kind
        match entry.kind {
            crate::entry::EntryKind::Dir  => { dirs.fetch_add(1, Ordering::Relaxed); }
            crate::entry::EntryKind::File => { files.fetch_add(1, Ordering::Relaxed); }
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
    let matches  = matches.load(Ordering::Relaxed);
    let files    = files.load(Ordering::Relaxed);
    let dirs     = dirs.load(Ordering::Relaxed);
    let paths    = Arc::try_unwrap(paths).unwrap_or_default().into_inner().unwrap_or_default();

    let matches = match limit {
        Some(lim) => matches.min(lim),
        None      => matches,
    };

    Results {
        matches,
        paths,
        stats: ScanStats::compute(files, dirs, duration),
        errors: vec![],
    }
}
