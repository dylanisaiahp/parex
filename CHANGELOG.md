# Changelog

All notable changes to parex are listed here. Newest first.

---

## v0.2.1 — 2026-02-21 (Docs)
- Added `DOCS.md` — full architecture guide, API reference, custom source/matcher examples, embedding guide
- Added `CONTRIBUTING.md`
- Added `CHANGELOG.md`
- Updated `README.md` — `cargo add parex` replaces manual dependency block, ordering guarantees documented, `is_fatal()` in error handling section, points to `DOCS.md`
- Removed `PAREX_DESIGN.md` — superseded by `DOCS.md`

## v0.2.0 — 2026-02-21
- `#[non_exhaustive]` on `ParexError` — new variants won't break downstream match arms
- `Source` and `Matcher` error variants now use `Box<dyn Error + Send + Sync>` — preserves original error type, enables proper chaining
- `NotFound` added to `is_recoverable()` — race condition during traversal, safe to continue
- `is_fatal()` helper added — explicit inverse of `is_recoverable()`
- `source_err()` and `matcher_err()` convenience constructors added
- Error messages now include the offending value (e.g. `"permission denied: /some/path"`)
- `WalkConfig::limit` changed to `pub(crate)` — internal detail, sources don't need to read it
- Dropped `ignore` dependency — was never used in parex core

## v0.1.0 — 2026-02-19
- Initial release
- `Source` and `Matcher` traits
- `SearchBuilder` fluent API
- `ParexError` with `is_recoverable()`, `path()`
- `Results` and `ScanStats`
- `#![forbid(unsafe_code)]`
- 7 integration tests, 7 doc tests
