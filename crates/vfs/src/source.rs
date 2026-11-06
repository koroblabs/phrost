use std::{
    fs, io,
    num::NonZeroU64,
    path::Path,
    sync::{Arc, OnceLock},
    time::UNIX_EPOCH,
};

use phrost_model::LineIndex;

/// Source text together with the line map and content stamp for that exact revision.
///
/// The three travel as one value on purpose. A line index built from text that has since changed
/// is the classic source of positions that are silently off by a few lines, and a stamp taken at
/// a different moment than the bytes it describes makes cache invalidation lie.
///
/// The line index is built the first time it is asked for, and from this value's own text, so it
/// cannot describe another revision. Lazily because indexing never asks: a closed file is read,
/// hashed, lowered and dropped, and building its line map first was 3 % of a cold start and one
/// allocation the size of the file's line count for each of thirty thousand files.
#[derive(Clone, Debug)]
pub struct SourceText {
    text: Arc<str>,
    line_index: OnceLock<LineIndex>,
    stamp: FileStamp,
}

impl SourceText {
    pub fn new(text: impl Into<Arc<str>>) -> Self {
        let text: Arc<str> = text.into();
        Self {
            line_index: OnceLock::new(),
            stamp: FileStamp::of(text.as_bytes()),
            text,
        }
    }

    /// Decodes bytes as UTF-8, replacing anything invalid rather than failing.
    ///
    /// This is deliberate and hard-won. A PHP project can contain files with stray Latin-1 bytes
    /// — a `©` in a comment is enough — and composer's own generated files are not exempt. A
    /// strict decode turns those into an `Err` that a guard clause then swallows, and the visible
    /// symptom is not "bad encoding" but "this entire dependency does not resolve". Replacing the
    /// bad bytes keeps every identifier in the file readable, and identifiers are ASCII.
    pub fn from_bytes(bytes: &[u8]) -> Self {
        match std::str::from_utf8(bytes) {
            Ok(text) => Self::new(text),
            Err(_) => Self::new(String::from_utf8_lossy(bytes).into_owned()),
        }
    }

    /// The same, taking the buffer rather than borrowing it.
    ///
    /// The strict decode first, because it validates in place: nearly every file is valid UTF-8, and
    /// the lossy path both walks the bytes more slowly and copies them into a second buffer. That
    /// copy, on every file of a cold start, was 3 % of it. The lossy decode is still what happens
    /// to a file that needs it -- see [`Self::from_bytes`].
    pub fn from_vec(bytes: Vec<u8>) -> Self {
        match String::from_utf8(bytes) {
            Ok(text) => Self::new(text),
            Err(invalid) => Self::new(String::from_utf8_lossy(invalid.as_bytes()).into_owned()),
        }
    }

    pub fn read(path: &Path) -> io::Result<Self> {
        Ok(Self::from_vec(fs::read(path)?))
    }

    /// Reads a file and says what its metadata was at that moment.
    ///
    /// The metadata is taken before the bytes, not after. Taken after, a write landing between the
    /// two would leave a fingerprint newer than the text it is stored beside, and the next start
    /// would keep a row for content it never saw. Taken before, the same race leaves it older, and
    /// an older fingerprint only costs that file a read next time.
    pub fn read_with_fingerprint(path: &Path) -> io::Result<(Self, Option<FileFingerprint>)> {
        let mut file = fs::File::open(path)?;
        let metadata = file.metadata()?;
        let mut bytes = Vec::with_capacity(usize::try_from(metadata.len()).unwrap_or(0));
        io::Read::read_to_end(&mut file, &mut bytes)?;
        Ok((Self::from_vec(bytes), FileFingerprint::of(&metadata)))
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn shared_text(&self) -> Arc<str> {
        Arc::clone(&self.text)
    }

    pub fn line_index(&self) -> &LineIndex {
        self.line_index.get_or_init(|| LineIndex::new(&self.text))
    }

    pub fn stamp(&self) -> FileStamp {
        self.stamp
    }

    pub fn len(&self) -> usize {
        self.text.len()
    }

    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }
}

/// Content identity of one file revision.
///
/// A content hash rather than mtime plus size. Mtime is not stable across a fresh checkout, a
/// `git switch`, or a restored build cache, so an mtime stamp invalidates work that was still
/// valid and — worse, because it is silent — occasionally keeps work that was not.
///
/// This is the identity a stored row is *trusted* on. [`FileFingerprint`] is the cheaper question
/// asked before it: metadata that says a file cannot have changed, so that a start which finds
/// nothing changed does not have to read thirty thousand files to know it.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct FileStamp {
    hash: [u8; 32],
    len: u64,
}

impl FileStamp {
    /// The hash and length, for writing a stamp to a cache.
    pub fn parts(self) -> ([u8; 32], u64) {
        (self.hash, self.len)
    }

    /// Rebuilds one from [`Self::parts`].
    pub fn from_parts(hash: [u8; 32], len: u64) -> Self {
        Self { hash, len }
    }

    pub fn of(bytes: &[u8]) -> Self {
        Self {
            hash: *blake3::hash(bytes).as_bytes(),
            len: bytes.len() as u64,
        }
    }

    /// Byte length of the hashed content.
    ///
    /// Named `byte_len` rather than `len` because a stamp is not a collection, and the usual
    /// `len`/`is_empty` pairing would read as if it were.
    pub fn byte_len(self) -> u64 {
        self.len
    }
}

impl std::fmt::Debug for FileStamp {
    /// Prints a short hash prefix: full 32 bytes make snapshots unreadable, and the prefix is
    /// enough to see at a glance whether two stamps differ.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let prefix: String = self.hash[..4]
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        write!(f, "FileStamp({prefix}..; {} bytes)", self.len)
    }
}

/// What a file's metadata said when it was read, for telling without reading it whether it has
/// changed since.
///
/// A content hash is the identity a stored row is trusted on -- see [`FileStamp`] -- and taking one
/// means reading the file. On the reference checkout that is `open()` on thirty thousand files,
/// 0.9 s of a warm start spent confirming that nothing had changed, and `open()` on this platform
/// costs eight times what `stat()` does. So the metadata is asked first: one `stat` against three
/// numbers. A match keeps the row unread. A mismatch falls back to the hash, which is why a
/// `git switch` there and back costs a read and keeps the row, and why the stored fingerprint is
/// refreshed when the hash matches -- so the next start takes the short path again.
///
/// Modification time alone would be a guess: a tool restoring a file can restore its mtime. The
/// inode change time is beside it, which no user tool sets, and the length. A restore that
/// preserved all three and changed the bytes is not a thing a machine does to itself.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct FileFingerprint {
    /// Nanoseconds since the epoch. Non-zero so that `Option<Self>` costs no extra word in the
    /// thirty thousand rows holding one; a file dated to the epoch itself has no fingerprint and
    /// is hashed instead, which is the harmless direction.
    modified: NonZeroU64,
    /// Inode change time, the same way. Zero where the platform has none.
    changed: u64,
    len: u64,
}

impl FileFingerprint {
    pub fn of(metadata: &fs::Metadata) -> Option<Self> {
        let modified = metadata.modified().ok()?.duration_since(UNIX_EPOCH).ok()?;
        Some(Self {
            modified: NonZeroU64::new(u64::try_from(modified.as_nanos()).ok()?)?,
            changed: changed_nanos(metadata),
            len: metadata.len(),
        })
    }

    /// One `stat`, and nothing else.
    pub fn of_path(path: &Path) -> Option<Self> {
        Self::of(&fs::metadata(path).ok()?)
    }

    /// The three numbers, for writing to a cache.
    pub fn parts(self) -> (u64, u64, u64) {
        (self.modified.get(), self.changed, self.len)
    }

    /// Rebuilds one from [`Self::parts`]. `None` for a zero modification time, which is never
    /// written.
    pub fn from_parts(modified: u64, changed: u64, len: u64) -> Option<Self> {
        Some(Self {
            modified: NonZeroU64::new(modified)?,
            changed,
            len,
        })
    }
}

#[cfg(unix)]
fn changed_nanos(metadata: &fs::Metadata) -> u64 {
    use std::os::unix::fs::MetadataExt as _;
    u64::try_from(metadata.ctime())
        .unwrap_or(0)
        .saturating_mul(1_000_000_000)
        .saturating_add(u64::try_from(metadata.ctime_nsec()).unwrap_or(0))
}

#[cfg(not(unix))]
fn changed_nanos(_: &fs::Metadata) -> u64 {
    0
}
