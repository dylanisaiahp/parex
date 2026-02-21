# parex — Documentation

This document covers the full architecture, API reference, and embedding guide for parex.

For a quick start, see [README.md](README.md).

---

## Table of Contents

- [Architecture](#architecture)
- [Core Traits](#core-traits)
- [Entry](#entry)
- [Builder API](#builder-api)
- [Error Handling](#error-handling)
- [Results](#results)
- [Ordering Guarantees](#ordering-guarantees)
- [Building on parex](#building-on-parex)
- [Real-World Example — ldx](#real-world-example--ldx)

---

## Architecture

parex is built around a strict separation of concerns:

```
parex (engine)
 ├── Source trait     — produce entries from anything traversable
 ├── Matcher trait    — decide what counts as a match
 ├── SearchBuilder    — fluent API to wire everything together
 ├── engine::run()    — parallel execution, result collection, early exit
 └── ParexError       — typed errors with recoverable/fatal distinction
```

The engine owns:
- Thread management
- Result collection
- Early exit on limit
- Error collection

The caller owns:
- Where entries come from (`Source`)
- What counts as a match (`Matcher`)
- Output formatting
- Any filesystem or domain logic

This means parex has zero opinions about what you're searching or why.

---

## Core Traits

### Source

```rust
pub trait Source: Send + Sync {
    fn walk(
        &self,
        config: &WalkConfig,
    ) -> Box<dyn Iterator<Item = Result<Entry, ParexError>>>;
}
```

`Source` produces entries for the engine to process. Implement this to search anything — a filesystem, a database, an in-memory collection, an API response, or a pre-built index.

**Key points:**
- `Send + Sync` required — sources are shared across threads
- Yield `Err(ParexError)` for recoverable errors rather than panicking or silently skipping
- `config` carries traversal parameters (`threads`, `max_depth`) — honour them if your source supports it
- Results are unordered — parallel traversal does not guarantee output order

### Matcher

```rust
pub trait Matcher: Send + Sync {
    fn is_match(&self, entry: &Entry) -> bool;
}
```

`Matcher` decides whether an entry should be included in results.

**Key points:**
- `Send + Sync` required — matchers are called concurrently across threads
- Keep matchers pure and cheap — they are called for every entry
- Avoid interior mutability — shared state requires synchronization overhead

### WalkConfig

```rust
pub struct WalkConfig {
    pub threads:   usize,
    pub max_depth: Option<usize>,
    // limit is pub(crate) — internal to the engine
}
```

Passed to `Source::walk()` so sources can honour traversal parameters. Sources are not required to use these — a simple in-memory source can ignore them entirely.

---

## Entry

```rust
pub struct Entry {
    pub path:     PathBuf,
    pub name:     String,
    pub kind:     EntryKind,
    pub depth:    usize,
    pub metadata: Option<std::fs::Metadata>,
}

pub enum EntryKind {
    File,
    Dir,
    Symlink,
    Other,
}
```

`Entry` is the unit passed from `Source` to `Matcher` to `Results`. Populate only what your source knows — `metadata` is optional and incurs no overhead when `None`.

---

## Builder API

```rust
parex::search()
    .source(my_source)          // required
    .matching("pattern")        // case-insensitive substring shorthand
    .with_matcher(my_matcher)   // custom Matcher — overrides .matching()
    .threads(8)                 // default: logical CPUs
    .limit(100)                 // stop after N matches
    .max_depth(5)               // limit traversal depth
    .collect_paths(true)        // populate Results::paths
    .collect_errors(true)       // populate Results::errors
    .run()?
```

**Notes:**
- `.matching()` and `.with_matcher()` are mutually exclusive — `.with_matcher()` takes precedence
- `.collect_paths(false)` and `.collect_errors(false)` are zero-cost — no allocation occurs
- `.run()` returns `Result<Results, ParexError>` — fatal errors surface here

---

## Error Handling

```rust
#[non_exhaustive]
pub enum ParexError {
    PermissionDenied(PathBuf),   // recoverable
    NotFound(PathBuf),           // recoverable — often a race condition
    SymlinkLoop(PathBuf),        // recoverable
    Io { path, source },         // recoverable
    InvalidSource(PathBuf),      // fatal
    ThreadPool(String),          // fatal
    InvalidPattern(String),      // fatal
    InvalidThreadCount(usize),   // fatal
    Source(Box<dyn Error>),      // third-party source errors
    Matcher(Box<dyn Error>),     // third-party matcher errors
}
```

`#[non_exhaustive]` — new variants will not be breaking changes.

**Convenience constructors for third-party errors:**

```rust
// Instead of ParexError::Source(Box::new(e))
ParexError::source_err(e)
ParexError::matcher_err(e)
```

**Recoverable vs fatal:**

```rust
if err.is_recoverable() {
    // permission denied, not found, symlink loop, IO
    // safe to collect and continue walking
}

if err.is_fatal() {
    // thread pool failure, invalid source
    // halt immediately
}
```

**Path access:**

```rust
if let Some(path) = err.path() {
    eprintln!("Skipped: {}", path.display());
}
```

---

## Results

```rust
pub struct Results {
    pub matches: usize,
    pub paths:   Vec<PathBuf>,   // empty unless collect_paths(true)
    pub errors:  Vec<ParexError>, // empty unless collect_errors(true)
    pub stats:   ScanStats,
}

pub struct ScanStats {
    pub files:    usize,
    pub dirs:     usize,
    pub duration: Duration,
}
```

`ScanStats` counts every entry seen — not just matches. Use this to show scan speed and totals independently of match count.

---

## Ordering Guarantees

**Results are explicitly unordered.**

Parallel traversal distributes work across threads — the order entries are yielded depends on thread scheduling, filesystem layout, and OS behaviour. Two runs over the same data may return matches in different orders.

If your caller requires ordered output, sort `results.paths` after the search completes.

---

## Building on parex

parex is designed to be embedded. The `Source` and `Matcher` traits are the extension points — you bring the data, parex brings the parallelism.

### Filesystem Source

The most common use case — walk a directory tree:

```rust
use parex::{Source, Entry, EntryKind, ParexError};
use parex::engine::WalkConfig;

struct DirSource(std::path::PathBuf);

impl Source for DirSource {
    fn walk(&self, config: &WalkConfig) -> Box<dyn Iterator<Item = Result<Entry, ParexError>>> {
        let entries = walkdir::WalkDir::new(&self.0)
            .max_depth(config.max_depth.unwrap_or(usize::MAX))
            .into_iter()
            .map(|res| match res {
                Ok(e) => Ok(Entry {
                    name:     e.file_name().to_string_lossy().into_owned(),
                    path:     e.path().to_path_buf(),
                    kind:     EntryKind::File,
                    depth:    e.depth(),
                    metadata: e.metadata().ok(),
                }),
                Err(e) => Err(ParexError::Io {
                    path: e.path().map(|p| p.to_path_buf()).unwrap_or_default(),
                    source: e.into_io_error().unwrap_or_else(|| {
                        std::io::Error::new(std::io::ErrorKind::Other, "walkdir error")
                    }),
                }),
            })
            .collect::<Vec<_>>();
        Box::new(entries.into_iter())
    }
}
```

### Database Source

Search a database by implementing `Source` over query results:

```rust
struct DbSource {
    records: Vec<String>,
}

impl Source for DbSource {
    fn walk(&self, _config: &WalkConfig) -> Box<dyn Iterator<Item = Result<Entry, ParexError>>> {
        let entries = self.records.iter().map(|r| Ok(Entry {
            path:     r.into(),
            name:     r.clone(),
            kind:     EntryKind::Other,
            depth:    0,
            metadata: None,
        })).collect::<Vec<_>>();
        Box::new(entries.into_iter())
    }
}
```

### Pre-built Index Source

If you have a pre-built index (e.g. a search index, MFT cache, or database of file paths), implement `Source` to read from it — parex handles parallel matching over your index entries exactly as it would a live filesystem walk:

```rust
struct IndexSource {
    index: Vec<IndexEntry>, // your index type
}

impl Source for IndexSource {
    fn walk(&self, _config: &WalkConfig) -> Box<dyn Iterator<Item = Result<Entry, ParexError>>> {
        let entries = self.index.iter().map(|e| Ok(Entry {
            path:     e.path.clone(),
            name:     e.name.clone(),
            kind:     EntryKind::File,
            depth:    0,
            metadata: None,
        })).collect::<Vec<_>>();
        Box::new(entries.into_iter())
    }
}
```

### Custom Matchers

Implement `Matcher` for any matching logic:

```rust
// Regex matcher
struct RegexMatcher(regex::Regex);

impl Matcher for RegexMatcher {
    fn is_match(&self, entry: &Entry) -> bool {
        self.0.is_match(&entry.name)
    }
}

// Metadata filter — files modified in the last N days
struct StaleMatcher(u64);

impl Matcher for StaleMatcher {
    fn is_match(&self, entry: &Entry) -> bool {
        entry.metadata.as_ref()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.elapsed().ok())
            .map(|age| age.as_secs() > self.0 * 86400)
            .unwrap_or(false)
    }
}
```

---

## Real-World Example — ldx

[ldx](https://github.com/dylanisaiahp/localdex) is a parallel file search CLI built entirely on parex.

- `DirectorySource` implements `Source` using the `ignore` crate's parallel walker
- Custom matchers: `NameMatcher`, `ExtMatcher`, `AllMatcher`, `DirMatcher`
- `search.rs` is a thin wrapper around `parex::search()` — ~150 lines
- Peak throughput: **1,491,712 entries/s** on an i5-13400F at 16 threads

ldx demonstrates that parex's abstraction adds zero meaningful overhead — the engine gets out of the way and lets the source and hardware do the work.
