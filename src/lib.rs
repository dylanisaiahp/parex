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
//! ```rust
//! use parex::{Source, Entry, EntryKind, ParexError, Matcher};
//! use parex::engine::WalkConfig;
//!
//! // A minimal in-memory source for demonstration
//! struct NameSource(Vec<&'static str>);
//!
//! impl Source for NameSource {
//!     fn walk(&self, _config: &WalkConfig) -> Box<dyn Iterator<Item = Result<Entry, ParexError>>> {
//!         let entries = self.0.iter().map(|name| Ok(Entry {
//!             path:     name.into(),
//!             name:     name.to_string(),
//!             kind:     EntryKind::File,
//!             depth:    0,
//!             metadata: None,
//!         })).collect::<Vec<_>>();
//!         Box::new(entries.into_iter())
//!     }
//! }
//!
//! let results = parex::search()
//!     .source(NameSource(vec!["invoice_jan.txt", "invoice_feb.txt", "report.txt"]))
//!     .matching("invoice")
//!     .collect_paths(true)
//!     .run()
//!     .unwrap();
//!
//! assert_eq!(results.matches, 2);
//! println!("Found {} matches in {:.3}s",
//!     results.matches,
//!     results.stats.duration.as_secs_f64()
//! );
//! ```
//!
//! # Custom Sources and Matchers
//!
//! Implement [`Source`] to search anything traversable:
//!
//! ```rust
//! use parex::{Source, Entry, EntryKind, ParexError};
//! use parex::engine::WalkConfig;
//!
//! struct VecSource(Vec<String>);
//!
//! impl Source for VecSource {
//!     fn walk(&self, _config: &WalkConfig) -> Box<dyn Iterator<Item = Result<Entry, ParexError>>> {
//!         let entries = self.0.iter().map(|name| Ok(Entry {
//!             path:     name.into(),
//!             name:     name.clone(),
//!             kind:     EntryKind::File,
//!             depth:    0,
//!             metadata: None,
//!         })).collect::<Vec<_>>();
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
/// # Example
///
/// ```rust
/// use parex::{Source, Entry, EntryKind, ParexError};
/// use parex::engine::WalkConfig;
///
/// struct NameSource(Vec<&'static str>);
///
/// impl Source for NameSource {
///     fn walk(&self, _config: &WalkConfig) -> Box<dyn Iterator<Item = Result<Entry, ParexError>>> {
///         let entries = self.0.iter().map(|name| Ok(Entry {
///             path: name.into(), name: name.to_string(),
///             kind: EntryKind::File, depth: 0, metadata: None,
///         })).collect::<Vec<_>>();
///         Box::new(entries.into_iter())
///     }
/// }
///
/// let results = parex::search()
///     .source(NameSource(vec!["invoice.txt", "report.txt"]))
///     .matching("invoice")
///     .run()
///     .unwrap();
///
/// assert_eq!(results.matches, 1);
/// ```
pub fn search() -> SearchBuilder {
    SearchBuilder::default()
}
