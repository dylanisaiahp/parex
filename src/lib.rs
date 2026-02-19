//! # parex
//!
//! Blazing-fast parallel search engine — generic, embeddable, zero opinions.
//!
//! parex is a parallel execution framework. It owns the walk engine, the
//! contracts ([`Source`], [`Matcher`]), the error type, and the builder API.
//! It does **not** own filesystem-specific logic, built-in matchers, or output
//! formatting — those belong to the caller.
//!
//! # Quick Start
//!
//! ```rust,no_run
//! use parex::search;
//!
//! let results = search()
//!     .source(my_source)
//!     .matching("invoice")
//!     .limit(10)
//!     .threads(8)
//!     .collect_paths(true)
//!     .collect_errors(true)
//!     .run()
//!     .unwrap();
//!
//! println!("Found {} matches in {:.3}s",
//!     results.matches,
//!     results.stats.duration.as_secs_f64()
//! );
//!
//! for err in &results.errors {
//!     if err.is_recoverable() {
//!         eprintln!("⚠ Skipped: {:?} ({})",
//!             err.path().unwrap_or(&std::path::PathBuf::new()),
//!             err
//!         );
//!     }
//! }
//! ```
//!
//! # Custom Sources and Matchers
//!
//! Implement [`Source`] to search anything traversable:
//!
//! ```rust
//! use parex::{Source, Entry, EntryKind};
//! use parex::engine::WalkConfig;
//!
//! struct VecSource(Vec<String>);
//!
//! impl Source for VecSource {
//!     fn walk(&self, _config: &WalkConfig) -> Box<dyn Iterator<Item = Entry>> {
//!         let entries: Vec<Entry> = self.0.iter().map(|name| Entry {
//!             path: name.into(),
//!             name: name.clone(),
//!             kind: EntryKind::File,
//!             depth: 0,
//!             metadata: None,
//!         }).collect();
//!         Box::new(entries.into_iter())
//!     }
//! }
//! ```
//!
//! Implement [`Matcher`] for custom matching logic:
//!
//! ```rust
//! use parex::{Matcher, Entry};
//!
//! struct ExtensionMatcher(String);
//!
//! impl Matcher for ExtensionMatcher {
//!     fn is_match(&self, entry: &Entry) -> bool {
//!         entry.path
//!             .extension()
//!             .map(|e| e.eq_ignore_ascii_case(&self.0))
//!             .unwrap_or(false)
//!     }
//! }
//! ```

#![forbid(unsafe_code)]

pub mod engine;

mod builder;
mod entry;
mod error;
mod results;
mod traits;

// ── Public re-exports ─────────────────────────────────────────────────────────

pub use builder::SearchBuilder;
pub use entry::{Entry, EntryKind};
pub use error::ParexError;
pub use results::{Results, ScanStats};
pub use traits::{Matcher, Source};

// ── Entry point ───────────────────────────────────────────────────────────────

/// Create a new [`SearchBuilder`] to configure and run a search.
///
/// ```rust,no_run
/// let results = parex::search()
///     .source(my_source)
///     .matching("invoice")
///     .run()
///     .unwrap();
/// ```
pub fn search() -> SearchBuilder {
    SearchBuilder::default()
}
