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
 ├── engine::run()    — execution, result collection, early exit
 └── ParexError       — typed errors with recoverable/fatal distinction
```

The engine owns:
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
}
```

Passed to `Source::walk()` so sources can honour traversal parameters. Sources are not required to use these — a simple in-memory source can ignore them entirely.

---

## Entry

```rust
pub struct Entry {
    pub path:     PathBuf,
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

`Entry` is the unit passed from `Source` to `Matcher` to `Results`. The entry name can be derived from `path.file_name()` when needed — this avoids a redundant `String` allocation per entry. Populate only what your source knows — `metadata` is optional and incurs no overhead when `None`.

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
ParexError::source_err(e)
ParexError::matcher_err(e)
```

**Recoverable vs fatal:**

```rust
if err.is_recoverable() {
    // permission denied, not found, symlink loop, IO
    // safe to collect and continue
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
    pub paths:   Vec<PathBuf>,    // empty unless collect_paths(true)
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

parex is designed to be embedded. The `Source` and `Matcher` traits are the extension points — you bring the data, parex brings the engine.

### Filesystem Source (recommended: parawalk)

For filesystem traversal, [parawalk](https://github.com/dylanisaiahp/parawalk) is the recommended `Source` implementation:

```rust
use parex::{Source, Entry, EntryKind, ParexError};
use parex::engine::WalkConfig;
use parawalk::{EntryKind as WalkKind, EntryRef, WalkConfig as ParaConfig};
use std::sync::mpsc;

struct DirectorySource(std::path::PathBuf);

impl Source for DirectorySource {
    fn walk(&self, config: &WalkConfig) -> Box<dyn Iterator<Item = Result<Entry, ParexError>>> {
        let (tx, rx) = mpsc::channel::<Vec<Entry>>();
        let root = self.0.clone();

        std::thread::spawn(move || {
            parawalk::walk(
                root,
                ParaConfig { threads: config.threads, max_depth: config.max_depth, follow_links: false },
                None::<fn(&EntryRef<'_>) -> bool>,
                move || {
                    let tx = tx.clone();
                    let mut batch = Vec::with_capacity(128);
                    move |walked: parawalk::Entry| {
                        let kind = match walked.kind {
                            WalkKind::File => EntryKind::File,
                            WalkKind::Dir => EntryKind::Dir,
                            WalkKind::Symlink => EntryKind::Symlink,
                            WalkKind::Other => return,
                        };
                        batch.push(Entry { path: walked.path, kind, depth: walked.depth, metadata: None });
                        if batch.len() >= 128 {
                            let _ = tx.send(std::mem::take(&mut batch));
                            batch = Vec::with_capacity(128);
                        }
                    }
                },
            );
        });

        Box::new(rx.into_iter().flatten().map(Ok))
    }
}
```

### In-Memory Source

```rust
struct VecSource(Vec<String>);

impl Source for VecSource {
    fn walk(&self, _config: &WalkConfig) -> Box<dyn Iterator<Item = Result<Entry, ParexError>>> {
        let entries = self.0.iter().map(|r| Ok(Entry {
            path:     r.into(),
            kind:     EntryKind::File,
            depth:    0,
            metadata: None,
        })).collect::<Vec<_>>();
        Box::new(entries.into_iter())
    }
}
```

### Database Source

```rust
struct DbSource { records: Vec<String> }

impl Source for DbSource {
    fn walk(&self, _config: &WalkConfig) -> Box<dyn Iterator<Item = Result<Entry, ParexError>>> {
        let entries = self.records.iter().map(|r| Ok(Entry {
            path:     r.into(),
            kind:     EntryKind::Other,
            depth:    0,
            metadata: None,
        })).collect::<Vec<_>>();
        Box::new(entries.into_iter())
    }
}
```

### Custom Matchers

```rust
// Regex matcher
struct RegexMatcher(regex::Regex);

impl Matcher for RegexMatcher {
    fn is_match(&self, entry: &Entry) -> bool {
        entry.path.file_name()
            .and_then(|n| n.to_str())
            .map(|n| self.0.is_match(n))
            .unwrap_or(false)
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

- `DirectorySource` implements `Source` using [parawalk](https://github.com/dylanisaiahp/parawalk) for parallel directory traversal
- Custom matchers: `NameMatcher`, `ExtMatcher`, `AllMatcher`, `DirMatcher`
- `search.rs` is a thin wrapper around `parex::search()`

ldx demonstrates that parex's abstraction adds zero meaningful overhead — the engine gets out of the way and lets the source and hardware do the work.
