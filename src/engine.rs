use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use ignore::{DirEntry, WalkBuilder, WalkState};

use crate::entry::{Entry, EntryKind};
use crate::error::ParexError;
use crate::results::{Results, ScanStats};
use crate::traits::Matcher;

// ---------------------------------------------------------------------------
// WalkConfig
// ---------------------------------------------------------------------------

/// Traversal parameters passed from the builder to the engine.
///
/// `pub(crate)` — not part of the public API. Callers configure these
/// via the builder methods (`.threads()`, `.max_depth()`, `.limit()`).
pub(crate) struct WalkConfig {
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
    pub matcher:        Arc<dyn Matcher>,
    pub collect_paths:  bool,
    pub collect_errors: bool,
}

// ---------------------------------------------------------------------------
// run()
// ---------------------------------------------------------------------------

/// Execute a parallel search over `root` using the given options.
///
/// This is the core engine — all parallelism lives here.
/// Called by `SearchBuilder::run()` after validating inputs.
pub(crate) fn run(root: &PathBuf, opts: EngineOptions) -> Results {
    let mut builder = WalkBuilder::new(root);
    builder
        .standard_filters(false)
        .ignore(false)
        .parents(false)
        .hidden(false)
        .follow_links(false)
        .same_file_system(false)
        .threads(opts.config.threads);

    if let Some(depth) = opts.config.max_depth {
        builder.max_depth(Some(depth));
    }

    let walker = builder.build_parallel();

    // Shared state across threads
    let matches    = Arc::new(AtomicUsize::new(0));
    let files      = Arc::new(AtomicUsize::new(0));
    let dirs       = Arc::new(AtomicUsize::new(0));
    let paths      = Arc::new(Mutex::new(Vec::<PathBuf>::new()));
    let errors     = Arc::new(Mutex::new(Vec::<ParexError>::new()));

    let start = Instant::now();

    walker.run(|| {
        let matcher        = Arc::clone(&opts.matcher);
        let matches        = Arc::clone(&matches);
        let files          = Arc::clone(&files);
        let dirs           = Arc::clone(&dirs);
        let paths          = Arc::clone(&paths);
        let errors         = Arc::clone(&errors);
        let limit          = opts.config.limit;
        let collect_paths  = opts.collect_paths;
        let collect_errors = opts.collect_errors;
        let root           = root.clone();

        Box::new(move |res: Result<DirEntry, ignore::Error>| -> WalkState {
            // Handle traversal errors
            let entry = match res {
                Ok(e) => e,
                Err(e) => {
                    if collect_errors {
                        let err = map_ignore_error(e);
                        if let Ok(mut errs) = errors.lock() {
                            errs.push(err);
                        }
                    }
                    return WalkState::Continue;
                }
            };

            let ft = match entry.file_type() {
                Some(ft) => ft,
                None     => return WalkState::Continue,
            };

            // Count and classify
            if ft.is_dir() {
                dirs.fetch_add(1, Ordering::Relaxed);
            } else if ft.is_file() {
                files.fetch_add(1, Ordering::Relaxed);
            }

            // Skip the root itself
            if entry.depth() == 0 {
                return WalkState::Continue;
            }

            // Build a parex Entry from the ignore DirEntry
            let kind = if ft.is_dir() {
                EntryKind::Dir
            } else if ft.is_file() {
                EntryKind::File
            } else if ft.is_symlink() {
                EntryKind::Symlink
            } else {
                EntryKind::Other
            };

            let name = entry
                .file_name()
                .to_string_lossy()
                .into_owned();

            let parex_entry = Entry {
                path:     entry.path().to_path_buf(),
                name,
                kind,
                depth:    entry.depth(),
                metadata: None, // lazy — matchers populate if needed
            };

            // Run matcher
            if !matcher.is_match(&parex_entry) {
                return WalkState::Continue;
            }

            // Increment and enforce limit — two-guard approach handles
            // the race where multiple threads overshoot before WalkState::Quit
            // propagates across all threads.
            let mc = matches.fetch_add(1, Ordering::Relaxed) + 1;

            // Early guard: already over limit before collecting/printing
            if let Some(lim) = limit {
                if mc > lim {
                    return WalkState::Quit;
                }
            }

            if collect_paths {
                if let Ok(mut p) = paths.lock() {
                    p.push(parex_entry.path.clone());
                }
            }

            // At-limit guard: quit after collecting if we've hit exactly
            if let Some(lim) = limit {
                if mc >= lim {
                    return WalkState::Quit;
                }
            }

            WalkState::Continue
        })
    });

    let duration = start.elapsed();

    let matches    = matches.load(Ordering::Relaxed);
    let files      = files.load(Ordering::Relaxed);
    let dirs       = dirs.load(Ordering::Relaxed);
    let paths      = Arc::try_unwrap(paths).unwrap_or_default().into_inner().unwrap_or_default();
    let errors     = Arc::try_unwrap(errors).unwrap_or_default().into_inner().unwrap_or_default();

    // Clamp matches to limit — atomic counter can overshoot under concurrency
    let matches = match opts.config.limit {
        Some(lim) => matches.min(lim),
        None      => matches,
    };

    Results {
        matches,
        paths,
        stats: ScanStats::compute(files, dirs, duration),
        errors,
    }
}

// ---------------------------------------------------------------------------
// Map ignore::Error to ParexError
// ---------------------------------------------------------------------------

fn map_ignore_error(e: ignore::Error) -> ParexError {
    match e {
        ignore::Error::WithPath { path, err } => match *err {
            ignore::Error::Io(io_err) => {
                if io_err.kind() == std::io::ErrorKind::PermissionDenied {
                    ParexError::PermissionDenied(path)
                } else {
                    ParexError::Io { path, source: io_err }
                }
            }
            _ => ParexError::Source(format!("{}", err)),
        },
        ignore::Error::Loop { child, .. } => ParexError::SymlinkLoop(child),
        ignore::Error::Io(io_err)         => ParexError::Io {
            path: PathBuf::new(),
            source: io_err,
        },
        other => ParexError::Source(other.to_string()),
    }
}
