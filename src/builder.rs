use std::path::PathBuf;
use std::sync::Arc;

use crate::engine::{EngineOptions, WalkConfig, run};
use crate::error::ParexError;
use crate::results::Results;
use crate::traits::{Matcher, Source};

// ---------------------------------------------------------------------------
// SearchBuilder
// ---------------------------------------------------------------------------

/// Entry point for configuring and executing a parex search.
///
/// Created via [`parex::search()`](crate::search). Configure with chained
/// builder methods, then call [`run()`](SearchBuilder::run) to execute.
///
/// # Example
///
/// ```rust
/// let results = parex::search()
///     .source(my_source)
///     .matching(my_matcher)
///     .limit(10)
///     .threads(8)
///     .collect_paths(true)
///     .run()?;
/// ```
pub struct SearchBuilder {
    source:         Option<Box<dyn Source>>,
    matcher:        Option<Box<dyn Matcher>>,
    limit:          Option<usize>,
    threads:        usize,
    max_depth:      Option<usize>,
    collect_paths:  bool,
    collect_errors: bool,
}

impl Default for SearchBuilder {
    fn default() -> Self {
        Self {
            source:         None,
            matcher:        None,
            limit:          None,
            threads:        num_cpus(),
            max_depth:      None,
            collect_paths:  false,
            collect_errors: false,
        }
    }
}

impl SearchBuilder {
    // ── Source ────────────────────────────────────────────────────────────

    /// Set the source to search through.
    ///
    /// Any type implementing [`Source`] is accepted — filesystem directories,
    /// in-memory collections, databases, etc.
    pub fn source(mut self, s: impl Source + 'static) -> Self {
        self.source = Some(Box::new(s));
        self
    }

    // ── Matcher ───────────────────────────────────────────────────────────

    /// Set a custom matcher.
    ///
    /// Any type implementing [`Matcher`] is accepted. Use this for custom
    /// matching logic — regex, fuzzy search, metadata filters, ML scoring, etc.
    ///
    /// For the common case of substring matching, prefer `.matching()`.
    pub fn with_matcher(mut self, m: impl Matcher + 'static) -> Self {
        self.matcher = Some(Box::new(m));
        self
    }

    /// Shorthand for substring matching.
    ///
    /// Equivalent to `.with_matcher(SubstringMatcher::new(pattern))`.
    /// Pattern matching is case-insensitive by default.
    ///
    /// For custom matching logic, use `.with_matcher()` instead.
    pub fn matching(mut self, pattern: impl Into<String>) -> Self {
        self.matcher = Some(Box::new(SubstringMatcher {
            pattern: pattern.into().to_lowercase(),
        }));
        self
    }

    // ── Options ───────────────────────────────────────────────────────────

    /// Stop after `n` matches.
    ///
    /// The actual match count may be slightly higher under concurrency —
    /// parex clamps the reported count to this limit in [`Results`].
    pub fn limit(mut self, n: usize) -> Self {
        self.limit = Some(n);
        self
    }

    /// Number of threads to use for parallel traversal.
    ///
    /// Defaults to the number of logical CPU cores. Values exceeding the
    /// available core count are accepted but won't improve performance.
    pub fn threads(mut self, n: usize) -> Self {
        self.threads = n;
        self
    }

    /// Maximum traversal depth. `0` means the root only, `1` means one
    /// level of children, and so on. Unlimited by default.
    pub fn max_depth(mut self, d: usize) -> Self {
        self.max_depth = Some(d);
        self
    }

    /// Collect matched paths into [`Results::paths`].
    ///
    /// Disabled by default to avoid allocation overhead when paths aren't needed.
    pub fn collect_paths(mut self, yes: bool) -> Self {
        self.collect_paths = yes;
        self
    }

    /// Collect non-fatal errors into [`Results::errors`].
    ///
    /// Disabled by default. When enabled, recoverable errors (permission denied,
    /// symlink loops) are stored in [`Results::errors`] rather than silently skipped.
    pub fn collect_errors(mut self, yes: bool) -> Self {
        self.collect_errors = yes;
        self
    }

    // ── Execute ───────────────────────────────────────────────────────────

    /// Execute the search and return results.
    ///
    /// Blocks until the search completes. For streaming results or cancellation
    /// support, see the async API (coming in a future release).
    ///
    /// # Errors
    ///
    /// Returns `Err` for fatal configuration errors (no source provided,
    /// invalid source path, thread pool failure). Non-fatal errors during
    /// traversal are collected into [`Results::errors`] when
    /// `.collect_errors(true)` is set.
    pub fn run(self) -> Result<Results, ParexError> {
        let source = self.source.ok_or_else(|| {
            ParexError::InvalidSource("no source provided".into())
        })?;

        // Default matcher: match everything
        let matcher: Arc<dyn Matcher> = match self.matcher {
            Some(m) => Arc::from(m),
            None    => Arc::new(AllMatcher),
        };

        // Resolve the root from the source
        // DirectorySource (in ldx) provides the root — we ask it via walk()
        // For now, the engine expects a PathBuf root directly.
        // We extract it by downcasting if the source is a DirectorySource,
        // or use a sentinel path for custom sources.
        //
        // NOTE: This is a known limitation of the v0.1.0 sync API.
        // A future iteration will have Source::root() -> Option<&Path>
        // so the engine can always know where to start.
        let root = source_root(&*source);

        let opts = EngineOptions {
            config: WalkConfig {
                threads:   self.threads,
                max_depth: self.max_depth,
                limit:     self.limit,
            },
            matcher,
            collect_paths:  self.collect_paths,
            collect_errors: self.collect_errors,
        };

        Ok(run(&root, opts))
    }
}

// ---------------------------------------------------------------------------
// Built-in matchers (parex ships these as conveniences)
// ---------------------------------------------------------------------------

/// Matches entries whose name contains `pattern` (case-insensitive).
struct SubstringMatcher {
    pattern: String,
}

impl Matcher for SubstringMatcher {
    fn is_match(&self, entry: &crate::entry::Entry) -> bool {
        entry.name.to_lowercase().contains(&self.pattern)
    }
}

/// Matches every entry. Used when no matcher is specified.
struct AllMatcher;

impl Matcher for AllMatcher {
    fn is_match(&self, _entry: &crate::entry::Entry) -> bool {
        true
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Get the logical CPU count, with a safe fallback.
fn num_cpus() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
}

/// Extract a root path from a source.
///
/// This is a temporary shim for v0.1.0. The engine needs a `PathBuf` to hand
/// to the `ignore` walker. Custom sources that don't map to a filesystem path
/// should implement their own traversal and not use this engine directly.
///
/// A future `Source::root() -> Option<&Path>` method will make this clean.
fn source_root(source: &dyn Source) -> PathBuf {
    // Walk with a zero-depth config just to get the root
    // Sources that override walk() can return their root as the first entry
    let config = WalkConfig { threads: 1, max_depth: Some(0), limit: Some(1) };
    source
        .walk(&config)
        .next()
        .map(|e| e.path)
        .unwrap_or_else(|| PathBuf::from("."))
}
