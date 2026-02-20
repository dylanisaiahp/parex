# parex — Design Blueprint

**Status:** Pre-implementation design doc  
**Target:** parex v0.1.0 (sync only), prx v0.1.0  
**Rust version:** 1.93.1 stable  

---

## Core Philosophy

parex is a parallel execution framework — nothing more. It owns:

- The parallel walk engine
- The contracts (traits) that sources and matchers implement
- The error type and severity classification
- The builder API

parex does **not** own:
- Filesystem-specific logic (that's prx's `DirectorySource`)
- Built-in matchers (that's prx's `SubstringMatcher`, `ExtensionMatcher` etc.)
- Output formatting (that's the caller's job)
- Error message strings presented to users (caller decides presentation)

The goal: parex should be embeddable in anything. File search, database record search, API result filtering, in-memory collections. Zero assumptions about what is being searched.

---

## File Structure

```
parex/
├── Cargo.toml
└── src/
    ├── lib.rs        — public API surface, re-exports
    ├── builder.rs    — SearchBuilder
    ├── engine.rs     — parallel walk execution
    ├── entry.rs      — Entry, EntryKind
    ├── error.rs      — ParexError (enum + impls only)
    ├── results.rs    — Results, ScanStats
    └── traits.rs     — Source, Matcher traits
```

One file, one job. `error.rs` contains only the error enum and its impl blocks — nothing else ever goes in there.

---

## Traits

```rust
// traits.rs

/// A source of entries to search through.
/// Implement this to search anything — directories, databases, APIs, vecs.
/// Send + Sync required: sources are shared across threads.
pub trait Source: Send + Sync {
    fn walk(&self, config: &WalkConfig) -> impl Iterator<Item = Entry>;
}

/// Determines whether an entry is a match.
/// Send + Sync required: matchers are cloned/shared across threads.
/// Implement this for custom matching logic — regex, fuzzy, ML scoring, etc.
pub trait Matcher: Send + Sync {
    fn is_match(&self, entry: &Entry) -> bool;
}
```

ldx (localdex repo) provides all concrete implementations. Third parties bring their own.

---

## Entry

```rust
// entry.rs

/// A single item produced by a Source during traversal.
/// Intentionally generic — not filesystem-specific.
/// `metadata` is lazy: only populated when a Matcher requests it.
pub struct Entry {
    pub path: PathBuf,
    pub name: String,           // filename or record identifier
    pub kind: EntryKind,
    pub depth: usize,
    pub metadata: Option<std::fs::Metadata>,
}

pub enum EntryKind {
    File,
    Dir,
    Symlink,
    Other,
}
```

`name` and `kind` are deliberately neutral — no "file_" prefix, works for any data source.

---

## Error Type

```rust
// error.rs

use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ParexError {
    // Traversal
    #[error("permission denied")]
    PermissionDenied(PathBuf),

    #[error("path not found")]
    NotFound(PathBuf),

    #[error("invalid source")]
    InvalidSource(PathBuf),

    #[error("symlink loop")]
    SymlinkLoop(PathBuf),

    // Config
    #[error("invalid pattern")]
    InvalidPattern(String),

    #[error("invalid thread count")]
    InvalidThreadCount(usize),

    // Runtime
    #[error("thread pool failure")]
    ThreadPool(String),

    #[error("IO error")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    // Third-party extensibility
    #[error("source error")]
    Source(String),

    #[error("matcher error")]
    Matcher(String),
}

impl ParexError {
    /// The path this error occurred at, if applicable.
    /// Callers use this to show "Skipped: <path>" without knowing the variant.
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
    /// Fatal errors (InvalidSource, ThreadPool) should halt the search.
    /// Recoverable errors (PermissionDenied, SymlinkLoop, Io) can be collected
    /// and surfaced after the search completes.
    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            Self::PermissionDenied(_) | Self::SymlinkLoop(_) | Self::Io { .. }
        )
    }
}
```

**Design rationale:**
- Error strings are minimal — callers own presentation, icons, colors
- `path()` helper means callers never need to pattern match just to get the path
- `is_recoverable()` means prx's `--warn` and parallax's console plugin share the same logic without duplicating it
- `Source(String)` and `Matcher(String)` give third parties a clean escape hatch

---

## Results

```rust
// results.rs

pub struct Results {
    pub matches: usize,
    pub paths: Vec<PathBuf>,     // only populated if .collect_paths(true)
    pub stats: ScanStats,
    pub errors: Vec<ParexError>, // only populated if .collect_errors(true)
}

pub struct ScanStats {
    pub files: usize,
    pub dirs: usize,
    pub duration: std::time::Duration,
}
```

Both `paths` and `errors` are opt-in. Default search: zero allocation overhead for either.

---

## Builder API

```rust
// builder.rs

pub fn search() -> SearchBuilder { SearchBuilder::default() }

pub struct SearchBuilder {
    source:         Option<Box<dyn Source>>,
    matcher:        Option<Box<dyn Matcher>>,
    limit:          Option<usize>,
    threads:        usize,
    max_depth:      Option<usize>,
    collect_paths:  bool,
    collect_errors: bool,
}

impl SearchBuilder {
    // Source
    pub fn source(mut self, s: impl Source + 'static) -> Self { ... }

    // Matcher
    pub fn matching(mut self, m: impl Matcher + 'static) -> Self { ... }

    // Options
    pub fn limit(mut self, n: usize) -> Self { ... }
    pub fn threads(mut self, n: usize) -> Self { ... }
    pub fn max_depth(mut self, d: usize) -> Self { ... }
    pub fn collect_paths(mut self, yes: bool) -> Self { ... }
    pub fn collect_errors(mut self, yes: bool) -> Self { ... }

    // Execute
    pub fn run(self) -> Result<Results, ParexError> { ... }  // sync, blocking

    // Future (async / parallax):
    // pub fn stream(self) -> (JoinHandle, Receiver<Entry>, CancelToken) { ... }
}
```

Usage from ldx:
```rust
let results = parex::search()
    .source(DirectorySource::new("~/projects").exclude(vec!["target"]))
    .matching(SubstringMatcher::new("invoice"))
    .limit(10)
    .threads(8)
    .collect_paths(true)
    .collect_errors(true)
    .run()?;

for err in &results.errors {
    if err.is_recoverable() {
        eprintln!("⚠ Skipped: {} ({})", err.path().unwrap().display(), err);
    }
}
```

---

## WalkConfig

Internal struct passed from builder to `Source::walk()`. Not public API.

```rust
pub(crate) struct WalkConfig {
    pub threads:   usize,
    pub max_depth: Option<usize>,
    pub limit:     Option<usize>,
}
```

Sources use this to configure their traversal. `DirectorySource` passes it to the `ignore` crate walker.

---

## ldx's Concrete Implementations

These live in `ldx` (localdex repo), not `parex`:

```rust
// DirectorySource — wraps ignore::WalkBuilder
pub struct DirectorySource {
    root:            PathBuf,
    exclude:         Vec<String>,
    follow_links:    bool,
    same_filesystem: bool,
    include_hidden:  bool,
}

// Built-in matchers
pub struct SubstringMatcher { pattern: AhoCorasick, case_sensitive: bool }
pub struct ExtensionMatcher { ext: String, case_sensitive: bool }
pub struct AllMatcher;
pub struct StaleMatcher     { cutoff: std::time::SystemTime }  // --stale N days
```

`StaleMatcher` — computed once at CLI parse time:
```rust
let cutoff = SystemTime::now() - Duration::from_secs(days * 86400);
// In is_match(): entry.metadata?.modified() < cutoff
```

---

## Async / Cancellation — Future Design

Not in v0.1.0. Notes for when parallax needs it:

```rust
// Option A — per-search cancel token (preferred, supports concurrent searches)
let (handle, cancel) = parex::search()
    .source(...)
    .matching(...)
    .stream();

cancel.cancel(); // atomic flag all threads check between entries

// Option B — global stop (simpler, only works if one search at a time)
parex::stop();
```

Decision: implement Option A when parallax starts. Parallax may run two searches concurrently (e.g. different scopes), so per-search tokens are the safer choice.

---

## Naming — Final Decisions

| Component | Name | Repo | crates.io |
|-----------|------|------|-----------|
| Engine | `parex` | new `parex` repo | ✓ publish |
| CLI | `ldx` (binary) | `localdex` repo (existing) | ✗ binary only |
| GUI | `parallax` | new `parallax` repo | ✗ binary only |

`ldx` stays permanent — no rename to `prx` or anything else. Already in users' PATH, already has brand recognition, no conflicts anywhere. The suite doesn't need phonetic consistency (`git`/`GitHub`, `cargo`/`crates.io`).

---

## Crates.io Publishing Checklist

Before `parex v0.1.0` publish:
- [ ] `#![forbid(unsafe_code)]` in lib.rs
- [ ] Full `///` doc comments on every public item
- [ ] `cargo doc --no-deps --open` — verify docs render cleanly
- [ ] `cargo clippy -- -D warnings` zero warnings
- [ ] `cargo test` — at least trait impl tests, builder tests, error tests
- [ ] `README.md` in parex repo with quick-start example
- [ ] `Cargo.toml` — `description`, `license`, `repository`, `keywords`, `categories`
- [ ] Verify `parex` name available on crates.io (checked: available ✓)

---

## Open Questions (decide before coding)

1. **`impl Trait` in `Source::walk()`** — requires boxing or GATs for object safety. May need `Box<dyn Iterator<Item = Entry>>` return type instead.
2. **Metadata laziness** — populate eagerly for all entries, or only when a `StaleMatcher` is detected in the builder? Lazy is faster but adds complexity.
3. **`with_matcher()` vs `matching()`** — `matching()` is ergonomic for the common case. `with_matcher(Box::new(x))` for custom matchers. Have both?

---

*Drop this file in the parex repo root when the repo is created. It's the starting blueprint.*

🦀
