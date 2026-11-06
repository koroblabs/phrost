//! Reading the skeleton back from disk instead of building it again.
//!
//! Indexing a real project takes a couple of seconds every time a server starts, and nothing about
//! the answer changes between one start and the next unless a file did. So it is written down.
//!
//! # What is stored, and what is rebuilt
//!
//! Only the primary facts: the type table's nodes, and per file its path, stamp, origin, scope,
//! classes and functions. Everything derived is rebuilt on load -- the catalog of who declares what,
//! the type table's lookup maps, the interner. Storing a derived structure would be storing the same
//! fact twice, and two copies of a fact can disagree; rebuilding it reuses the same code the fresh
//! path runs, so it cannot.
//!
//! # Why the format is written by hand
//!
//! A derive on every stored type would make the shape of `MemberRow`, `TypeRef` and a dozen others
//! into a wire format nobody can see, and spread a serialization dependency across five crates. It
//! also would not answer the question that actually matters -- whether a field was forgotten -- which
//! only the `cold == warm` invariant can.
//!
//! # File identity
//!
//! A `FileId` is a session-local index and is never stored. Each file's path is, and the caller
//! assigns ids as the entries come back. Order on disk is deterministic, so ids are too -- which is
//! the same property `incremental == from-scratch` already checks the built index for.

mod bytes;
mod files;
mod members;
mod scope;
mod types;

use std::{
    fs, io,
    path::{Path, PathBuf},
};

use phrost_model::{FileId, Interner};

use crate::ProjectSkeleton;

use self::bytes::{Reader, Writer};

/// Four bytes and a version, so a file from another build is refused rather than misread.
///
/// Bumping the version is what a change to any stored *shape* requires. Nothing negotiates: an
/// index that does not match is rebuilt, which costs the two seconds it would have cost anyway.
const MAGIC: &[u8; 4] = b"phrs";
const VERSION: u32 = 80;

impl ProjectSkeleton {
    /// Writes the skeleton as bytes, stamped with the build that derived them.
    ///
    /// `build` is the load-bearing half and the version is not. A version says the *format* changed
    /// and a human has to remember to bump it; what actually invalidates an index is any change to
    /// the code that derives it -- and those are the changes nobody thinks of as a format change.
    /// One was made this evening: a docblock beside a native hint started being read, so every
    /// member row written before it says something different from the same row written after, in a
    /// format that did not change by a byte. A stored row like that is reused for every file whose
    /// bytes have not moved, and the symptom is a server that is wrong until you type something.
    pub fn encode(&self, build: u64) -> Vec<u8> {
        let mut bytes = Vec::new();
        self.write_to(&mut bytes, build)
            .expect("writing to a vector cannot fail");
        bytes
    }

    /// The same, straight into whatever it is being stored in.
    ///
    /// The form the server uses. Collecting the index into a buffer first is what left 93.8 MB of
    /// freed-but-dirty large regions behind on every cold start -- see [`Writer`] -- and there is
    /// nothing in the format that needs the whole thing in hand, since every list writes its length
    /// before its items.
    pub fn write_to(&self, sink: &mut dyn std::io::Write, build: u64) -> std::io::Result<()> {
        let mut out = Writer::new(sink);
        out.bytes(MAGIC);
        out.u32(VERSION);
        out.u64(build);
        out.option(self.root().as_ref(), |out, root| {
            out.str(&root.to_string_lossy());
        });

        types::write(&mut out, self.types());
        let mut files = self.files_in_path_order();
        files.sort_by_key(|(_, entry)| entry.path().to_path_buf());
        out.list(&files, |out, (file, entry)| {
            files::write(out, self, *file, entry);
        });

        out.finish()
    }

    /// Reads one back, asking `assign` for the id of each path as it goes.
    ///
    /// `None` for anything that is not a skeleton *this* build wrote, or that is truncated. A partly
    /// decoded index is worse than none: it would answer, and wrongly.
    pub fn decode(
        source: &mut dyn io::Read,
        len: u64,
        build: u64,
        mut assign: impl FnMut(&Path) -> FileId,
    ) -> Option<Self> {
        let mut input = Reader::new(source, len);
        if input.array::<4>()? != *MAGIC || input.u32()? != VERSION || input.u64()? != build {
            return None;
        }

        let mut interner = Interner::new();
        let root = input.option(|input| Some(std::path::PathBuf::from(input.str()?)))?;
        let types = types::read(&mut input, &mut interner)?;
        // Each file goes into the skeleton as it is read. Collecting them first was a vector of
        // thirty thousand rows held for the whole decode, and the tables were then grown from
        // empty by doubling -- see `from_cache` for what that cost a warm server. The count is
        // bounded by what the file still holds, as every list's is: a corrupt length must not
        // size a table for a gigabyte up front.
        let file_count = input.usize()?;
        let mut skeleton = Self::from_cache(
            root,
            interner,
            types,
            file_count.min(usize::try_from(input.left()).unwrap_or(usize::MAX)),
        );
        for _ in 0..file_count {
            let (file, entry, said) =
                files::read(&mut input, skeleton.interner_mut(), &mut assign)?;
            skeleton.install_cached(file, entry, said);
        }

        // Trailing bytes mean the writer and the reader disagree about the shape, which is exactly
        // the mistake a hand-written format invites. Refusing here turns it into a rebuild.
        if !input.is_done() {
            return None;
        }

        // Left loading on purpose: what came off disk describes the project as it was, and nothing
        // has checked it against the project as it is. The caller does that, and marks it ready.
        Some(skeleton)
    }
}

/// Writes bytes so that a reader never sees a half-written file.
///
/// Through a temporary beside the target and a rename, which is atomic on every platform this runs
/// on. The alternative -- writing in place -- means a server killed mid-write leaves a file that
/// decodes to something plausible, and the whole point of the magic-and-version check is to never
/// have to hope about that.
pub fn store(
    path: &Path,
    write: impl FnOnce(&mut dyn io::Write) -> io::Result<()>,
) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let temporary = scratch_beside(path);
    {
        // Buffered, because the codec writes a few bytes at a time and an unbuffered `File` would
        // make that one `write` syscall per integer.
        let mut file = io::BufWriter::new(fs::File::create(&temporary)?);
        write(&mut file)?;
        io::Write::flush(&mut file)?;
    }
    fs::rename(&temporary, path)
}

/// What is left after clearing out a cache directory.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Pruned {
    pub removed: usize,
    pub freed: u64,
    pub kept: usize,
}

/// Deletes the cached indexes that can never be read again, and caps what is left.
///
/// A cache keyed by project root never forgets a project, and nothing here ever deleted one: the
/// directory grew by one entry for every root ever opened, at a megabyte or so each. A test harness
/// that indexes a throwaway project makes that pathological -- seven hundred entries and most of a
/// gigabyte, for roots that were deleted the same second.
///
/// Three reasons to drop one, and the first two are certainties rather than heuristics: an entry this
/// build cannot decode is unreadable by definition, and an entry whose project root is gone is about
/// nothing. Only the third is a policy -- past `keep`, the least recently used go.
///
/// The header alone is read, which is why this is affordable: a few hundred bytes per entry decides
/// it, against a file that is a megabyte.
pub fn prune(directory: &Path, build: u64, keep: usize) -> Pruned {
    let Ok(entries) = fs::read_dir(directory) else {
        return Pruned::default();
    };

    let mut survivors = Vec::new();
    let mut pruned = Pruned::default();
    for entry in entries.flatten() {
        let path = entry.path();
        if path
            .extension()
            .is_none_or(|extension| extension != "phrost")
        {
            continue;
        }
        let Ok(metadata) = entry.metadata() else {
            continue;
        };

        match header_of(&path).filter(|header| header.build == build && header.root_exists()) {
            Some(_) => survivors.push((metadata.modified().ok(), metadata.len(), path)),
            None => remove(&path, metadata.len(), &mut pruned),
        }
    }

    // Most recently used first, so what goes is what has been idle longest.
    survivors.sort_by_key(|(modified, ..)| std::cmp::Reverse(*modified));
    for (_, size, path) in survivors.iter().skip(keep) {
        remove(path, *size, &mut pruned);
    }
    pruned.kept = survivors.len().min(keep);

    pruned
}

fn remove(path: &Path, size: u64, pruned: &mut Pruned) {
    if fs::remove_file(path).is_ok() {
        pruned.removed += 1;
        pruned.freed += size;
    }
}

/// What a cached index says about itself, without decoding the rest of it.
struct Header {
    build: u64,
    root: Option<PathBuf>,
}

impl Header {
    /// Whether the project this was written for is still there.
    ///
    /// An entry with no root recorded is kept: it says nothing either way, and deleting on "no
    /// evidence" is how a cache learns to throw away work somebody wanted.
    fn root_exists(&self) -> bool {
        self.root.as_ref().is_none_or(|root| root.exists())
    }
}

/// Reads the prefix every entry starts with: magic, version, build, and the root it was built for.
fn header_of(path: &Path) -> Option<Header> {
    // Enough for the fixed prefix and a path of any length anyone has.
    let mut prefix = vec![0_u8; 8 * 1024];
    let read = {
        use std::io::Read as _;
        let mut file = fs::File::open(path).ok()?;
        file.read(&mut prefix).ok()?
    };
    prefix.truncate(read);

    let len = prefix.len() as u64;
    let mut slice = prefix.as_slice();
    let mut input = Reader::new(&mut slice, len);
    if input.array::<4>()? != *MAGIC || input.u32()? != VERSION {
        return None;
    }
    let build = input.u64()?;
    let root = input.option(|input| Some(PathBuf::from(input.str()?)))?;

    Some(Header { build, root })
}

/// A name next to the target, so the rename stays on one filesystem.
///
/// Per process, because two servers over two projects can share a cache directory and must not
/// overwrite each other's half-written file.
fn scratch_beside(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(format!(".{}.tmp", std::process::id()));
    path.with_file_name(name)
}
