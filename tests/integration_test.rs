use std::fs;
use std::path::PathBuf;

use parex::engine::WalkConfig;
use parex::{search, Entry, EntryKind, Matcher, ParexError, Source};

// ---------------------------------------------------------------------------
// Test helpers
// ---------------------------------------------------------------------------

/// Create a temporary directory tree for testing.
///
/// Structure:
/// ```
/// tmp/
///   invoice_jan.txt
///   invoice_feb.txt
///   report.txt
///   notes.md
///   subdir/
///     invoice_mar.txt
///     other.rs
/// ```
fn setup_test_dir() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

    fs::write(root.join("invoice_jan.txt"), "january invoice").unwrap();
    fs::write(root.join("invoice_feb.txt"), "february invoice").unwrap();
    fs::write(root.join("report.txt"), "quarterly report").unwrap();
    fs::write(root.join("notes.md"), "some notes").unwrap();

    let sub = root.join("subdir");
    fs::create_dir(&sub).unwrap();
    fs::write(sub.join("invoice_mar.txt"), "march invoice").unwrap();
    fs::write(sub.join("other.rs"), "fn main() {}").unwrap();

    dir
}

/// A simple DirectorySource for tests — mirrors what ldx will provide.
struct TestDirSource(PathBuf);

impl Source for TestDirSource {
    fn walk(&self, _config: &WalkConfig) -> Box<dyn Iterator<Item = Result<Entry, ParexError>>> {
        let root = self.0.clone();
        let entries = walkdir::WalkDir::new(&root)
            .into_iter()
            .filter(move |e| e.as_ref().map(|e| e.path() != root).unwrap_or(true))
            .map(|e| match e {
                Ok(e) => {
                    let kind = if e.file_type().is_dir() {
                        EntryKind::Dir
                    } else if e.file_type().is_symlink() {
                        EntryKind::Symlink
                    } else {
                        EntryKind::File
                    };
                    Ok(Entry {
                        name: e.file_name().to_string_lossy().into_owned(),
                        path: e.path().to_path_buf(),
                        kind,
                        depth: e.depth(),
                        metadata: None,
                    })
                }
                Err(e) => {
                    let path = e.path().map(|p| p.to_path_buf()).unwrap_or_default();
                    Err(ParexError::Io {
                        path,
                        source: e.into_io_error().unwrap_or_else(|| {
                            std::io::Error::new(std::io::ErrorKind::Other, "walk error")
                        }),
                    })
                }
            })
            .collect::<Vec<_>>();
        Box::new(entries.into_iter())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn finds_matching_files() {
    let dir = setup_test_dir();
    let results = search()
        .source(TestDirSource(dir.path().to_path_buf()))
        .matching("invoice")
        .collect_paths(true)
        .run()
        .unwrap();

    assert_eq!(results.matches, 3, "should find 3 invoice files");
    assert_eq!(results.paths.len(), 3);
    assert!(results.paths.iter().all(|p| p
        .file_name()
        .unwrap()
        .to_string_lossy()
        .contains("invoice")));
}

#[test]
fn respects_limit() {
    let dir = setup_test_dir();
    let results = search()
        .source(TestDirSource(dir.path().to_path_buf()))
        .matching("invoice")
        .limit(2)
        .collect_paths(true)
        .run()
        .unwrap();

    assert!(results.matches <= 2, "matches should be clamped to limit");
    assert!(results.paths.len() <= 2);
}

#[test]
fn all_files_when_no_matcher() {
    let dir = setup_test_dir();
    let results = search()
        .source(TestDirSource(dir.path().to_path_buf()))
        .run()
        .unwrap();

    // 6 files + 1 subdir = 7 entries total
    assert_eq!(
        results.stats.files + results.stats.dirs,
        7,
        "should scan all 7 entries"
    );
}

#[test]
fn stats_are_populated() {
    let dir = setup_test_dir();
    let results = search()
        .source(TestDirSource(dir.path().to_path_buf()))
        .run()
        .unwrap();

    assert!(results.stats.duration.as_nanos() > 0);
    assert!(results.stats.files > 0);
    assert!(results.stats.dirs > 0);
}

#[test]
fn custom_matcher_works() {
    struct RustMatcher;
    impl Matcher for RustMatcher {
        fn is_match(&self, entry: &Entry) -> bool {
            entry.path.extension().map(|e| e == "rs").unwrap_or(false)
        }
    }

    let dir = setup_test_dir();
    let results = search()
        .source(TestDirSource(dir.path().to_path_buf()))
        .with_matcher(RustMatcher)
        .collect_paths(true)
        .run()
        .unwrap();

    assert_eq!(results.matches, 1, "should find exactly 1 .rs file");
    assert!(results.paths[0].to_string_lossy().ends_with("other.rs"));
}

#[test]
fn paths_empty_when_not_collecting() {
    let dir = setup_test_dir();
    let results = search()
        .source(TestDirSource(dir.path().to_path_buf()))
        .matching("invoice")
        .run()
        .unwrap();

    assert!(
        results.paths.is_empty(),
        "paths should be empty when collect_paths is false"
    );
    assert_eq!(results.matches, 3, "matches should still be counted");
}

#[test]
fn errors_empty_when_not_collecting() {
    let dir = setup_test_dir();
    let results = search()
        .source(TestDirSource(dir.path().to_path_buf()))
        .run()
        .unwrap();

    assert!(
        results.errors.is_empty(),
        "errors should be empty when collect_errors is false"
    );
}
