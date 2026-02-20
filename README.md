# parex

Blazing-fast parallel search engine — generic, embeddable, zero opinions.

parex is a Rust library that owns the parallel walk engine, the trait contracts, and the error type. It does **not** own filesystem logic, output formatting, or built-in matchers — those belong to the caller.

Built to power [ldx](https://github.com/dylanisaiahp/localdex).

---

## Features

- Parallel traversal via a clean `Source` trait — search files, databases, memory, anything
- Custom matching via a `Matcher` trait — substring, regex, fuzzy, metadata, ML scoring
- Typed error handling with `is_recoverable()` — callers decide what to skip vs halt
- Opt-in path and error collection — zero allocation overhead when unused
- `#![forbid(unsafe_code)]`

## Quick Start

```toml
[dependencies]
parex = "0.1"
```

Implement `Source` for whatever you want to search:

```rust
use parex::{Source, Entry, EntryKind, ParexError};
use parex::engine::WalkConfig;

struct DirSource(std::path::PathBuf);

impl Source for DirSource {
    fn walk(&self, _config: &WalkConfig) -> Box<dyn Iterator<Item = Result<Entry, ParexError>>> {
        let entries = walkdir::WalkDir::new(&self.0)
            .into_iter()
            .filter_map(|e| e.ok())
            .map(|e| Ok(Entry {
                name:     e.file_name().to_string_lossy().into_owned(),
                path:     e.path().to_path_buf(),
                kind:     EntryKind::File,
                depth:    e.depth(),
                metadata: None,
            }))
            .collect::<Vec<_>>();
        Box::new(entries.into_iter())
    }
}
```

Run a search:

```rust
let results = parex::search()
    .source(DirSource("/home/user/projects".into()))
    .matching("invoice")
    .limit(50)
    .threads(8)
    .collect_paths(true)
    .collect_errors(true)
    .run()?;

println!("Found {} matches in {}ms",
    results.matches,
    results.stats.duration.as_millis()
);

for path in &results.paths {
    println!("  {}", path.display());
}

for err in &results.errors {
    if err.is_recoverable() {
        eprintln!("⚠ skipped: {:?}", err.path());
    }
}
```

## Custom Matchers

```rust
use parex::{Matcher, Entry};

struct ExtensionMatcher(String);

impl Matcher for ExtensionMatcher {
    fn is_match(&self, entry: &Entry) -> bool {
        entry.path
            .extension()
            .map(|e| e.eq_ignore_ascii_case(&self.0))
            .unwrap_or(false)
    }
}

let results = parex::search()
    .source(my_source)
    .with_matcher(ExtensionMatcher("rs".into()))
    .collect_paths(true)
    .run()?;
```

## Builder API

| Method | Description |
|--------|-------------|
| `.source(s)` | Set the source to search |
| `.matching(pattern)` | Substring match — case-insensitive shorthand |
| `.with_matcher(m)` | Custom `Matcher` implementation |
| `.limit(n)` | Stop after `n` matches |
| `.threads(n)` | Thread count (default: logical CPUs) |
| `.max_depth(d)` | Maximum traversal depth |
| `.collect_paths(bool)` | Collect matched paths into `Results::paths` |
| `.collect_errors(bool)` | Collect recoverable errors into `Results::errors` |

## Error Handling

```rust
for err in &results.errors {
    if let Some(path) = err.path() {
        eprintln!("Error at: {}", path.display());
    }
    if err.is_recoverable() {
        // permission denied, symlink loop — safe to skip
    }
}
```

## Design

parex owns the walk engine, trait contracts, error type, and builder API. It does not own filesystem logic, output formatting, or concrete matchers — those live in the tool built on top.

See [`PAREX_DESIGN.md`](PAREX_DESIGN.md) for the full architecture document.

## License

MIT — see [LICENSE](LICENSE)
