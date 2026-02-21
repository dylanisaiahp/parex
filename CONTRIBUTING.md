# Contributing to parex

Thanks for your interest in contributing! parex is a parallel search engine library for Rust — generic, embeddable, and built to stay lean. Contributions of all kinds are welcome — bug fixes, performance improvements, documentation, and new examples.

---

## Getting Started

### Prerequisites

- [Rust](https://rustup.rs) (stable)
- [Git](https://git-scm.com/)

### Clone and Build

```bash
git clone https://github.com/dylanisaiahp/parex
cd parex
cargo build
```

### Run Tests

```bash
cargo test
```

All 7 integration tests and 7 doc tests should pass.

---

## Development Workflow

```bash
# Lint — must be clean before submitting
cargo clippy -- -D warnings

# Format
cargo fmt

# Test
cargo test
```

Please ensure `cargo clippy -- -D warnings` produces zero warnings and `cargo fmt` has been run before opening a PR.

---

## Design Philosophy

Before contributing, please read [DOCS.md](DOCS.md) to understand the architecture. A few principles worth keeping in mind:

- **parex owns the engine, not the domain.** No filesystem logic, no built-in matchers, no output formatting belongs in this crate. Those live in the caller.
- **Keep it lean.** parex is currently 330 SLoC. New additions should have a clear reason to exist in core rather than in a wrapper crate.
- **`#![forbid(unsafe_code)]`** is non-negotiable. All contributions must remain safe Rust.
- **`#[non_exhaustive]`** on `ParexError` — new error variants are welcome but should be discussed in an issue first to avoid unnecessary breaking changes.

---

## Pull Request Guidelines

- Keep PRs focused — one feature or fix per PR
- Run `cargo clippy -- -D warnings` and `cargo fmt` before submitting
- Add or update tests for any changed behaviour
- Update `DOCS.md` if the change affects the public API
- Add an entry to `CHANGELOG.md` under a new version heading

---

## Reporting Issues

- Check existing issues before opening a new one
- Include Rust version (`rustc --version`) and OS
- Minimal reproducible examples are very helpful

---

## What Makes a Good Contribution?

Things that fit parex well:

- Performance improvements to the engine
- New `ParexError` variants for better error coverage
- Doc improvements and examples
- Additional tests — especially edge cases around limits, errors, and thread counts

Things that probably belong in a wrapper crate instead:

- Concrete `Source` implementations (filesystem, database, etc.)
- Concrete `Matcher` implementations (regex, glob, fuzzy)
- Output formatting or CLI logic

If you're unsure, open an issue first and we'll figure it out together.

---

## License

By contributing, you agree that your contributions will be licensed under the [MIT License](LICENSE).
