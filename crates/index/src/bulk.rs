//! Taking a batch of files into the index on every core the machine has.
//!
//! Indexing used to run on one thread. Measured on a 30 965-file checkout, `phrost index` spent
//! 4.12 s of wall clock on 2.42 s of user and 1.68 s of system time -- 99.5 % of *one* core of
//! twelve -- and the phase split says why that is worth changing rather than accepting:
//!
//! | | cold, 2.92 s | warm, 1.08 s |
//! | --- | ---: | ---: |
//! | reading the file | 1.019 s | 0.921 s |
//! | registering it in the vfs | 0.016 s | 0.009 s |
//! | lowering it | 1.627 s | -- |
//! | claiming it in the skeleton | 0.255 s | 0.004 s |
//!
//! Reading and lowering are 90 % of a cold start and 85 % of a warm one, and neither touches
//! anything shared. What is left is the claim, which touches everything: the interner, the catalog,
//! the member tables. So the shape is fixed by the measurement rather than chosen -- read and lower
//! in parallel, claim on one thread.
//!
//! A warm start being *entirely* reading is worth stating, because it is not obvious: a file's
//! stamp is a hash of its **content**, so asking whether a stored row is still good means reading
//! the file that produced it. And on this platform the read is the `open()`: 21 us a file against
//! 2.5 us for a `stat()`, 27 % of a cold start and most of a warm one. So there is a cheaper
//! question, asked first -- whether the file's metadata still matches what the row was read under
//! -- and the hash is what settles it when the metadata does not. See `FileFingerprint` for why
//! the metadata can be trusted at all, and [`Reused`] for the count that says whether it was.
//!
//! # What makes it sound
//!
//! **A worker interns into its own table.** [`ProjectSkeleton::ingest_lowered`] already documents
//! taking a HIR built against a different interner -- a `Name` is an `Arc<str>` comparing by
//! content, and the scope is rebuilt against the index's own -- because opening a buffer has always
//! worked that way. The table is made per batch and dropped with it, so eight workers do not
//! accumulate eight copies of the project's names; the `Arc`s they minted outlive the table.
//!
//! **The claim does not depend on the order results arrive in.** A worker sends each file the
//! moment it is lowered, so the order is whichever thread finished first -- and that is safe
//! because [`ProjectSkeleton::rank`] sorts claimants by origin and path, with a doc-comment saying
//! "never by `FileId`" for exactly this reason. It was checked rather than assumed: reversing the
//! claim order leaves the whole-index comparison in `tests::parallel_ingest` byte-identical.
//!
//! Sending each result rather than collecting the batch is also what keeps the memory flat, and
//! that was measured the hard way. Collecting first held a batch's worth of file text and HIR --
//! and the server's resident memory rose by about 50 MB **with a single worker**, which is what
//! said the cost was the collecting rather than the threads. A bounded channel keeps a couple of
//! files per worker in flight instead, and the batch size then bounds only how long the index lock
//! is held.

use std::{num::NonZero, ops::AddAssign, path::Path, sync::mpsc, thread};

use phrost_model::{FileId, Interner, SourceOrigin};
use phrost_vfs::{FileFingerprint, FileStamp, SourceText};

use crate::skeleton::ProjectSkeleton;

/// The most workers to lower on, whatever the machine reports.
///
/// A cap rather than a share of what is there, because the rest of the machine is somebody's
/// editor -- and because each worker costs resident memory that does not come back.
///
/// Measured on 30 965 files, driving a real server to the point where it answers, cold:
///
/// | workers | ready in | resident, idle |
/// | ---: | ---: | ---: |
/// | 1 | 4.8-5.3 s | 258-259 MB |
/// | 4 | 2.7-3.4 s | 306-313 MB |
/// | 8 | 2.2-2.9 s | 352-384 MB |
///
/// About 12-15 MB a worker, and it is not the batch: claiming results as they arrive rather than
/// collecting them changed it by 8 MB. It is the allocator holding each thread's high-water mark,
/// most of which is the bump arena a parse needs -- one per file, because the tree borrows the
/// arena and the arena has no reset, so a worker cannot reuse one. That is a structural cost of
/// parsing in parallel rather than an oversight, and it is the thing to remove before eight
/// workers is affordable.
///
/// Four rather than eight, then: it takes the larger half of the speed for the smaller half of the
/// memory, and this project's one rule ranks the second above the first. What eight would buy is
/// half a second on a start that happens once per cache generation.
///
/// **A warm start costs nothing either way.** Every start after the first idles at 205-209 MB on
/// one worker and on eight alike, because nothing is parsed -- the whole cost above is the cold
/// path. Confusing a warm run for a cold one is what made this look like a 148 MB regression at
/// first, and it is why the table says which it measured.
const MOST_WORKERS: usize = 4;

/// How many files one batch takes.
///
/// Large enough that thread setup is not paid per handful of files -- eight workers over 64 files
/// is eight files each, and a thirty-thousand-file project would pay setup 484 times for slices
/// that finish almost at once. Small enough to bound what is held read but not yet claimed, which
/// is a batch's worth of file text and HIR.
///
/// The server also releases its index lock between batches, so this doubles as how long a request
/// arriving mid-bootstrap waits. That the two want the same number is a coincidence worth stating
/// rather than hiding: a parallel batch of 512 takes about as long as a serial 64 used to, so the
/// wait is unchanged from when this was chosen for the lock alone.
pub const INGEST_BATCH: usize = 512;

/// One file a batch is to take in.
///
/// The id comes from the caller because the vfs that mints it is not this crate's, and registering
/// is 16 ms of the 2.92 s -- there is nothing to gain by moving it and a boundary to lose.
#[derive(Clone, Copy, Debug)]
pub struct BatchFile<'a> {
    pub file: FileId,
    pub path: &'a Path,
    pub origin: SourceOrigin,
}

/// How many of a batch's files a stored index answered for, and how it knew.
///
/// Two counts rather than one, because they are the instrument for two different failures. A cache
/// that was thrown away reuses nothing; a cache whose fingerprints never match reads every file to
/// confirm it, and takes as long as no cache at all while reporting that everything was reused.
/// Only the split tells them apart.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct Reused {
    /// Rows kept on the file's metadata alone, without reading it.
    pub unread: usize,
    /// Rows kept after reading the file and finding its bytes unchanged.
    pub hashed: usize,
}

impl Reused {
    pub fn total(self) -> usize {
        self.unread + self.hashed
    }
}

impl AddAssign for Reused {
    fn add_assign(&mut self, other: Self) {
        self.unread += other.unread;
        self.hashed += other.hashed;
    }
}

/// What the index holds for a file, as much of it as deciding whether to read the file needs.
#[derive(Clone, Copy)]
struct Held {
    stamp: FileStamp,
    fingerprint: Option<FileFingerprint>,
}

/// What a worker produced for one file, waiting to be claimed.
enum Prepared {
    /// The file's metadata matches the stored row's, so it was not read and there is nothing to do.
    Unread,
    /// The file was read and its bytes are the stored row's; only the fingerprint is new.
    Rehashed {
        fingerprint: Option<FileFingerprint>,
    },
    /// The file could not be read, and is skipped exactly as the serial path skipped it.
    Unreadable,
    /// Read and lowered, ready for the index.
    Lowered {
        text: SourceText,
        fingerprint: Option<FileFingerprint>,
        /// Boxed to keep the enum's own size off the `Vec` holding one per file in the batch.
        hir: Box<phrost_hir::FileHir>,
    },
}

impl ProjectSkeleton {
    /// Reads, lowers and claims a batch of files, lowering them across the machine's cores.
    ///
    /// `reuse` asks each file whether the index already holds a row matching its content, which is
    /// what separates a warm start from a cold one. The question is answered on the worker rather
    /// than here because answering it needs the file's bytes, and reading them is the expensive
    /// half -- so the stamps to compare against are gathered first and sent along.
    ///
    /// Returns how many of the batch were reused rather than lowered, and how that was known.
    pub fn ingest_batch(&mut self, batch: &[BatchFile<'_>], reuse: bool) -> Reused {
        let workers = thread::available_parallelism()
            .map_or(1, NonZero::get)
            .min(MOST_WORKERS)
            .min(batch.len().max(1));

        // What a worker needs of the index, taken before the claim borrows it mutably. Both are
        // cheap: the stamps are a map lookup each, and the attribute set is a handful of short
        // names. Copying them is what lets a worker read while the thread beside it claims.
        let stamps: Vec<Option<Held>> = match reuse {
            true => batch
                .iter()
                .map(|entry| {
                    self.file(entry.file).map(|held| Held {
                        stamp: held.stamp(),
                        fingerprint: held.fingerprint(),
                    })
                })
                .collect(),
            false => vec![None; batch.len()],
        };
        let attributes = self.keyed_attributes.clone();
        let dialects = self.template_dialects();

        if workers <= 1 {
            let mut interner = Interner::new();
            let mut reused = Reused::default();
            for (entry, stamp) in batch.iter().zip(&stamps) {
                prepare_one(&mut interner, &attributes, &dialects, entry, *stamp).claim(
                    self,
                    entry,
                    &mut reused,
                );
            }
            return reused;
        }

        let per = batch.len().div_ceil(workers);
        thread::scope(|scope| {
            // Bounded, and that bound is the whole point of the channel. Collecting a batch's
            // results and claiming them afterwards was measured holding a batch's worth of file
            // text and HIR at once -- 128 files, and the server's resident memory rose by about
            // 50 MB whether or not there was more than one worker, so it was the collecting rather
            // than the threads. Claiming as results arrive keeps a couple of files per worker in
            // flight instead, and the batch size then bounds only how long the index lock is held.
            let (send, receive) = mpsc::sync_channel::<(usize, Prepared)>(workers * 2);
            for (slice, offset) in batch.chunks(per).zip((0..).step_by(per)) {
                let send = send.clone();
                let stamps = &stamps;
                let attributes = &attributes;
                let dialects = &dialects;
                // With a stack, for the reason the sweep's workers have one: a scoped thread gets
                // neither the size this process gave `main` nor the one it gave the tokio workers.
                // Not demonstrated here -- a closed file's bodies are never lowered, so the deep
                // expression trees that abort the sweep are not reached on this path -- and left
                // defaulted it would be the same latent inconsistency, on the path that indexes
                // every file of a project.
                std::thread::Builder::new()
                    .name("phrost-ingest".to_owned())
                    .stack_size(phrost_model::DEEP_RECURSION_STACK)
                    .spawn_scoped(scope, move || {
                        // Per batch and not per run: a table living for the whole bootstrap would hold
                        // most of the project's names once per worker, which is the memory rule broken
                        // for a saving already had -- a `Name` is an `Arc` and outlives the table that
                        // minted it.
                        let mut interner = Interner::new();
                        for (index, entry) in slice.iter().enumerate() {
                            let prepared = prepare_one(
                                &mut interner,
                                attributes,
                                dialects,
                                entry,
                                stamps[offset + index],
                            );
                            // A closed channel means the receiver is gone, which only happens if it
                            // panicked. Stop rather than spin.
                            if send.send((offset + index, prepared)).is_err() {
                                return;
                            }
                        }
                    })
                    .expect("the operating system can start an ingest thread");
            }
            // The senders the workers hold are clones; this one has to go or the loop never ends.
            drop(send);

            let mut reused = Reused::default();
            for (index, prepared) in receive {
                prepared.claim(self, &batch[index], &mut reused);
            }
            reused
        })
    }
}

/// Reads one file and lowers it, unless the index already has that exact content.
///
/// Two ways of knowing, cheapest first. If the row was read under a fingerprint and the file's
/// metadata still matches it, the file is not opened at all. Otherwise it is read and hashed, and
/// a matching hash keeps the row -- with the fingerprint refreshed, so that a `git switch` there
/// and back costs one read rather than one on every start after.
///
/// A free function rather than a method, because the point of this design is that it borrows
/// nothing the claim needs: a worker running this and the thread claiming its neighbour's result
/// are touching disjoint memory.
fn prepare_one(
    interner: &mut Interner,
    attributes: &phrost_lower::KeyedAttributes,
    dialects: &[crate::TemplateDialect],
    entry: &BatchFile<'_>,
    held: Option<Held>,
) -> Prepared {
    if let Some(Held {
        fingerprint: Some(fingerprint),
        ..
    }) = held
        && FileFingerprint::of_path(entry.path) == Some(fingerprint)
    {
        return Prepared::Unread;
    }
    let Ok((source, fingerprint)) = SourceText::read_with_fingerprint(entry.path) else {
        return Prepared::Unreadable;
    };
    // A template is read as the PHP its dialect makes of it, here as everywhere a file's text
    // enters the index -- see `ProjectSkeleton::as_php`.
    let source = match crate::skeleton::template_dialect(dialects, entry.path) {
        Some(to_php) => SourceText::new(to_php(source.text()).0),
        None => source,
    };
    if held.is_some_and(|held| held.stamp == source.stamp()) {
        return Prepared::Rehashed { fingerprint };
    }

    // Bodies for project code only, which is the memory rule. Stated again rather than shared with
    // `ingest`, because the two make the same choice for the same reason and a helper taking a bool
    // would hide which one this is.
    let lowered = if entry.origin.wants_full_analysis() {
        phrost_lower::lower_text_with_bodies(interner, entry.file, source.text(), attributes)
    } else {
        phrost_lower::lower_text(interner, entry.file, source.text(), attributes)
    };
    Prepared::Lowered {
        text: source,
        fingerprint,
        hir: Box::new(lowered.hir),
    }
}

impl Prepared {
    /// Claims this file in the index, counting a stored row that was kept instead.
    ///
    /// The serial half. Everything it touches is shared -- the interner, the catalog, the member
    /// tables -- so it runs on one thread, and the measurement in the module doc says that is 9 % of
    /// a cold start and nothing measurable of a warm one.
    fn claim(self, skeleton: &mut ProjectSkeleton, entry: &BatchFile<'_>, reused: &mut Reused) {
        match self {
            Self::Unread => reused.unread += 1,
            Self::Rehashed { fingerprint } => {
                skeleton.set_fingerprint(entry.file, fingerprint);
                reused.hashed += 1;
            }
            Self::Unreadable => {}
            Self::Lowered {
                text,
                fingerprint,
                hir,
            } => {
                skeleton.ingest_lowered(
                    entry.file,
                    entry.path,
                    entry.origin,
                    text.text(),
                    text.stamp(),
                    &hir,
                );
                skeleton.set_fingerprint(entry.file, fingerprint);
            }
        }
    }
}
