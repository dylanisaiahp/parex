use crate::entry::Entry;
use crate::error::ParexError;

/// A source of entries to search through.
///
/// Implement this to make parex search anything — directories, databases,
/// in-memory collections, API results, or any other traversable data source.
///
/// # Object Safety
///
/// `Source` is object-safe. The builder stores sources as `Box<dyn Source>`,
/// so `walk()` returns `Box<dyn Iterator<Item = Result<Entry, ParexError>>>` rather than
/// `impl Iterator` (which would not be object-safe).
///
/// # Thread Safety
///
/// `Send + Sync` are required — sources are shared across threads during
/// parallel traversal.
///
/// # Error Handling
///
/// Recoverable errors (permission denied, unreadable directories) should be
/// yielded as `Err(ParexError)` rather than panicking or silently skipping.
/// The engine collects these into [`Results::errors`] when
/// `.collect_errors(true)` is set on the builder.
///
/// # Example
///
/// ```rust,ignore
/// use parex::{Source, Entry, EntryKind, ParexError};
/// use parex::engine::WalkConfig;
///
/// struct VecSource(Vec<String>);
///
/// impl Source for VecSource {
///     fn walk(&self, _config: &WalkConfig) -> Box<dyn Iterator<Item = Result<Entry, ParexError>>> {
///         let entries = self.0.iter().map(|name| Ok(Entry {
///             path: name.into(),
///             name: name.clone(),
///             kind: EntryKind::File,
///             depth: 0,
///             metadata: None,
///         })).collect::<Vec<_>>();
///         Box::new(entries.into_iter())
///     }
/// }
/// ```
pub trait Source: Send + Sync {
    /// Traverse the source and yield entries.
    ///
    /// `config` carries traversal parameters (thread count, depth limit, limit)
    /// so sources can honour them during their own traversal logic.
    ///
    /// Yield `Err` for recoverable errors — the engine collects them into
    /// [`Results::errors`] rather than halting the search.
    fn walk(&self, config: &crate::engine::WalkConfig) -> Box<dyn Iterator<Item = Result<Entry, ParexError>>>;
}

/// Determines whether an entry is a match.
///
/// Implement this to define custom matching logic — substring search, extension
/// filtering, regex, fuzzy matching, ML scoring, metadata filters, or anything else.
///
/// # Thread Safety
///
/// `Send + Sync` are required — matchers are shared across threads and called
/// concurrently on different entries.
///
/// # Example
///
/// ```rust
/// use parex::{Matcher, Entry};
///
/// struct ExtensionMatcher(String);
///
/// impl Matcher for ExtensionMatcher {
///     fn is_match(&self, entry: &Entry) -> bool {
///         entry.path
///             .extension()
///             .map(|e| e.eq_ignore_ascii_case(&self.0))
///             .unwrap_or(false)
///     }
/// }
/// ```
pub trait Matcher: Send + Sync {
    /// Returns `true` if this entry should be included in results.
    fn is_match(&self, entry: &Entry) -> bool;
}
