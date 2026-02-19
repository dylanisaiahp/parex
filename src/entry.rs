use std::path::PathBuf;

/// A single item produced by a [`Source`](crate::traits::Source) during traversal.
///
/// Intentionally generic — not filesystem-specific. `name` and `kind` are neutral
/// enough to represent directory entries, database records, API results, or anything
/// else a custom `Source` might produce.
///
/// `metadata` is populated lazily — only when a matcher explicitly requests it
/// (e.g. [`StaleMatcher`]). This avoids unnecessary `stat()` syscalls on every
/// entry when no metadata-aware matcher is in use.
pub struct Entry {
    /// Full path to the entry.
    pub path: PathBuf,

    /// The entry's name — filename, record ID, or any identifying string.
    pub name: String,

    /// What kind of entry this is.
    pub kind: EntryKind,

    /// How deep in the traversal this entry was found. Root = 0.
    pub depth: usize,

    /// Filesystem metadata, populated on demand.
    /// Matchers that need it (e.g. for modification time) call
    /// `std::fs::metadata(&entry.path)` themselves and cache the result here.
    pub metadata: Option<std::fs::Metadata>,
}

/// The kind of a traversed entry.
///
/// Kept generic so parex can represent non-filesystem sources cleanly.
/// Filesystem sources map `DirEntry` file types to these variants.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntryKind {
    /// A regular file.
    File,

    /// A directory.
    Dir,

    /// A symbolic link.
    Symlink,

    /// Anything else (device files, pipes, sockets, etc.).
    Other,
}
