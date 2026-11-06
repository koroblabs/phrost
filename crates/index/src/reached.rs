//! What the project's own code reaches, resolved — the exact half of "is this declaration dead".
//!
//! [`WordSet`](crate::WordSet) answers *is this name written in another file*, which is what makes
//! the editor's unused check affordable and is wrong in three ways one word test cannot tell
//! apart. A class called `StoreClient` counts as a use of a method called `storeClient`. A
//! name in one namespace counts for the same name in another. And `StockDataManager::importStockData`
//! being called keeps `AppCommands::importStockData` alive, which is a different method entirely.
//!
//! The exact question needs the *receiver* resolved, and that is a full analysis of a file rather
//! than a scan of it. So it is not asked per keystroke: every file the project owns is analysed
//! anyway — the project-wide pass does the closed ones and the buffer check the open ones — and
//! this records what that analysis already worked out. Nothing new is computed; what was missing
//! is that the answer was thrown away.
//!
//! # Why two sets
//!
//! An access whose receiver has no type resolves to nothing, and reading that as *nobody calls
//! this member* is how a report about absence earns a false positive. So a name that could not be
//! attributed to an owner is kept as a bare **name**, and any declaration of that name counts as
//! reached. The coarse rule survives, applied only where the precise one has nothing to say —
//! which on the reference project is the 1.6 % of accesses that do not resolve, rather than all of
//! them. A name written inside a string is the same case for the same reason: `[$this, 'refresh']`
//! is a real call and nothing about it says whose.
//!
//! # Keyed, never scanned
//!
//! A file's contribution is withdrawn by the keys it claimed, the way a catalog's are — see the
//! note in `skeleton.rs`. A count per key rather than a set, because two files reaching one member
//! is ordinary and the first of them closing must not withdraw the other's claim.

use std::collections::HashMap;

use lspkit_memsize::MemSize;
use phrost_model::FileId;

use crate::WordSet;

/// What one file reaches, ready to be recorded.
#[derive(Clone, Default, PartialEq, Eq, Debug)]
pub struct Reaches {
    pairs: Box<[u64]>,
    classes: Box<[u64]>,
    loose: Box<[u64]>,
}

impl Reaches {
    /// Sorted and deduplicated, because a file calling one method twice is one claim on it.
    pub fn of(mut pairs: Vec<u64>, mut classes: Vec<u64>, mut loose: Vec<u64>) -> Self {
        for set in [&mut pairs, &mut classes, &mut loose] {
            set.sort_unstable();
            set.dedup();
        }
        Self {
            pairs: pairs.into_boxed_slice(),
            classes: classes.into_boxed_slice(),
            loose: loose.into_boxed_slice(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.pairs.is_empty() && self.classes.is_empty() && self.loose.is_empty()
    }
}

/// Every member the project's own code was seen to reach, and by how many files.
#[derive(Default, Debug)]
pub struct Reached {
    pairs: HashMap<u64, u32>,
    classes: HashMap<u64, u32>,
    loose: HashMap<u64, u32>,
    by_file: HashMap<FileId, Reaches>,
    complete: bool,
}

impl Reached {
    /// Takes on what one file reaches, replacing whatever it claimed before.
    pub fn record(&mut self, file: FileId, reaches: Reaches) {
        self.withdraw(file);
        for &pair in &reaches.pairs {
            *self.pairs.entry(pair).or_default() += 1;
        }
        for &class in &reaches.classes {
            *self.classes.entry(class).or_default() += 1;
        }
        for &name in &reaches.loose {
            *self.loose.entry(name).or_default() += 1;
        }
        // Recorded even when empty, so re-analysing a file that used to reach something
        // withdraws what it claimed rather than leaving it standing.
        self.by_file.insert(file, reaches);
    }

    /// Drops a file's claims, which is what closing or deleting it means.
    pub fn forget(&mut self, file: FileId) {
        self.withdraw(file);
        self.by_file.remove(&file);
    }

    fn withdraw(&mut self, file: FileId) {
        let Some(had) = self.by_file.get(&file) else {
            return;
        };
        // One helper would need the map and the list borrowed from the same `self`; three loops
        // over three pairs of fields is the shape that compiles and reads the same.
        for pair in &had.pairs {
            if let Some(count) = self.pairs.get_mut(pair) {
                *count -= 1;
                if *count == 0 {
                    self.pairs.remove(pair);
                }
            }
        }
        for class in &had.classes {
            if let Some(count) = self.classes.get_mut(class) {
                *count -= 1;
                if *count == 0 {
                    self.classes.remove(class);
                }
            }
        }
        for name in &had.loose {
            if let Some(count) = self.loose.get_mut(name) {
                *count -= 1;
                if *count == 0 {
                    self.loose.remove(name);
                }
            }
        }
    }

    /// Whether anything reaches this member of this class.
    ///
    /// The owner is the class that *declares* it, because that is what a declaration is asked
    /// about; an access on a subclass is recorded against the declaration it resolved to.
    pub fn holds(&self, owner: &str, name: &str) -> bool {
        self.pairs.contains_key(&WordSet::fold_pair(owner, name))
            || self.loose.contains_key(&WordSet::fold(name))
    }

    /// Whether anything outside its own file names this class.
    ///
    /// By fully-qualified name, which is the half a word test cannot do: `App\\Repo` and
    /// `Vendor\\Repo` are two classes and one word. A reference that resolved to nothing falls
    /// back to the word, for the reason an unresolved member access does.
    pub fn holds_class(&self, fqn: &str) -> bool {
        let short = fqn.rsplit('\\').next().unwrap_or(fqn);
        self.classes
            .contains_key(&WordSet::fold(fqn.trim_start_matches('\\')))
            || self.loose.contains_key(&WordSet::fold(short))
    }

    /// Whether every file the project owns has contributed, so an absence means something.
    pub fn is_complete(&self) -> bool {
        self.complete
    }

    /// Whether this was the pass that completed it, so a caller republishes exactly once.
    pub fn mark_complete(&mut self) -> bool {
        let changed = !self.complete;
        self.complete = true;
        changed
    }

    /// Everything, for an index being rebuilt: what a file reaches is derived from analysis, and
    /// analysis of the old tree says nothing about the new one.
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    /// How many distinct members and loose names are held, for the memory report.
    pub fn counts(&self) -> (usize, usize, usize) {
        (self.pairs.len(), self.classes.len(), self.loose.len())
    }
}

impl MemSize for Reached {
    fn heap_size(&self) -> usize {
        let entry = std::mem::size_of::<(u64, u32)>();
        self.pairs.len() * entry
            + self.classes.len() * entry
            + self.loose.len() * entry
            + self
                .by_file
                .values()
                .map(|reaches| {
                    (reaches.pairs.len() + reaches.classes.len() + reaches.loose.len())
                        * std::mem::size_of::<u64>()
                        + std::mem::size_of::<(FileId, Reaches)>()
                })
                .sum::<usize>()
    }
}
