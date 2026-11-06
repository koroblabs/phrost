//! The set of words one file's text holds, so a project-wide search does not read the project.
//!
//! A reference sweep asks the same question of every file the project owns: could this file
//! mention this name. It used to answer by reading the file -- 1 726 `open()` calls for one
//! `textDocument/references`, which on this platform is 21 us each and was 96 % of the request.
//! Measured per phase: scan 29.5 ms, the reference-finding itself 1.1 ms. Reading on every core
//! took the scan to 19.6 ms and no further, because it divides the cost rather than removing it.
//!
//! What removes it is answering from memory, and the shape is fixed by what the question is. A
//! per-file Bloom filter does not work: a file holds 88 distinct words at the median and 1 175 at
//! the worst, so the 64 bits that would be free are saturated and a filter big enough to be
//! selective is bigger than this. A sorted list of hashes is exact, is 8 bytes a word, and answers
//! by binary search -- 202 925 words over 1 674 files is **1.6 MB**, against an 88 MB index.
//!
//! Kept for the project's own files only, which is the same set a sweep reads: a dependency's
//! references are not something anyone can act on, and 27 000 vendor files would be most of the
//! memory for none of the answers.
//!
//! # Why this is exact rather than a guess
//!
//! The scan it replaces is a case-insensitive **substring** test, and this is a **word** test, so
//! it is the more selective of the two -- a file holding `FooBar` is no longer a candidate for
//! `Foo`. That is sound because of what a needle is: a name somebody asked for the references of,
//! which every real reference to it writes as a whole token. `\App\Foo`, `Foo::class` and
//! `use App\Foo;` all tokenise to a `foo`. A needle spelled with punctuation -- the
//! `parent::__construct` a constructor is also reached by -- is tokenised the same way and every
//! one of its tokens has to be present.
//!
//! What it must never do is say no to a file that holds the name, and the failure direction is
//! chosen for that: a file with no word set is a candidate, and a needle with no words is held by
//! everything.

use lspkit_memsize::MemSize;
use rustc_hash::FxHashSet;

/// The words one file's text holds, hashed and sorted.
///
/// Hashes rather than the words themselves, because the words are not names anybody looks up --
/// they are every run of identifier characters in the file, comments and string bodies included,
/// and interning 19 320 of them would double what the interner holds to answer a question that
/// never needs the text back. A collision costs one file read and cannot lose an answer.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WordSet(Box<[u64]>);

impl WordSet {
    /// Every identifier-shaped run in `text`, lowercased, hashed, sorted and deduplicated.
    pub fn of(text: &str) -> Self {
        let mut words = Vec::new();
        for word in Self::words(text.as_bytes()) {
            words.push(word);
        }
        words.sort_unstable();
        words.dedup();
        Self(words.into_boxed_slice())
    }

    /// Rebuilds one from a cache, which stored exactly what [`Self::of`] produced.
    pub fn from_sorted(words: Box<[u64]>) -> Self {
        Self(words)
    }

    /// Whether this file could hold `needle`, and every word of it.
    ///
    /// `true` for a needle with no words at all, which is not a case any caller has and is the
    /// direction that cannot lose an answer.
    pub fn may_hold(&self, needle: &str) -> bool {
        self.may_hold_words(&Self::needle(needle))
    }

    /// A needle's words, hashed once for a caller asking many sets about it.
    pub(crate) fn needle(needle: &str) -> Box<[u64]> {
        Self::words(needle.as_bytes()).collect()
    }

    /// [`Self::may_hold`] for a needle already hashed by [`Self::needle`].
    pub(crate) fn may_hold_words(&self, words: &[u64]) -> bool {
        words.iter().all(|word| self.0.binary_search(word).is_ok())
    }

    /// Whether this file could hold any one of `needles`.
    ///
    /// From whichever side is smaller: a constructor's needles are one per class that runs it --
    /// two thousand on a test base class -- and a file's words are 88 at the median, so walking the
    /// needles would be two thousand binary searches for a file a few dozen lookups settle.
    pub fn may_hold_any(&self, needles: &WordNeedles) -> bool {
        let single = match needles.single.len() < self.0.len() {
            true => needles
                .single
                .iter()
                .any(|word| self.0.binary_search(word).is_ok()),
            false => self.0.iter().any(|word| needles.single.contains(word)),
        };
        single
            || needles
                .several
                .iter()
                .any(|needle| self.may_hold_words(needle))
    }

    pub fn as_sorted(&self) -> &[u64] {
        &self.0
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// The identifier-shaped runs of `bytes`, hashed with case folded away.
    ///
    /// PHP's own rule for what an identifier may hold, minus the high bytes: a name written with
    /// one is legal and vanishingly rare, and splitting on it only ever makes the filter admit
    /// more files.
    fn words(bytes: &[u8]) -> impl Iterator<Item = u64> + '_ {
        let mut at = 0;
        std::iter::from_fn(move || {
            while at < bytes.len() && !Self::in_word(bytes[at]) {
                at += 1;
            }
            let start = at;
            while at < bytes.len() && Self::in_word(bytes[at]) {
                at += 1;
            }
            (at > start).then(|| Self::hash(&bytes[start..at]))
        })
    }

    const fn in_word(byte: u8) -> bool {
        byte.is_ascii_alphanumeric() || byte == b'_'
    }

    /// One name, folded the way this file folds every other.
    ///
    /// Exposed because a second table keys on names too, and two foldings of one concept is how
    /// one of them comes to disagree with the other.
    pub fn fold(name: &str) -> u64 {
        Self::hash(name.as_bytes())
    }

    /// A class and one of its members as one key.
    ///
    /// Combined rather than concatenated, so no string is built for a question asked of every
    /// member access in the project. A collision costs a member reading as reached, which is the
    /// direction a report about absence has to be wrong in.
    pub fn fold_pair(owner: &str, member: &str) -> u64 {
        Self::fold(owner.trim_start_matches('\\')).wrapping_mul(0x0000_0100_0000_01b3)
            ^ Self::fold(member)
    }

    /// FNV-1a over the lowercased bytes.
    ///
    /// Chosen for being a few lines with no dependency and no per-call setup: this runs over every
    /// byte of every file the project owns, once, while the file is already in hand.
    fn hash(word: &[u8]) -> u64 {
        let mut hash = 0xcbf2_9ce4_8422_2325_u64;
        for byte in word {
            hash ^= u64::from(byte.to_ascii_lowercase());
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
        hash
    }
}

/// Many needles hashed once, for asking every file's set about all of them.
///
/// A needle of one word is looked up by hash; one spelled with punctuation needs every one of its
/// words, as [`WordSet::may_hold`] has it, and is kept whole.
#[derive(Clone, Debug, Default)]
pub struct WordNeedles {
    single: FxHashSet<u64>,
    several: Vec<Box<[u64]>>,
}

impl WordNeedles {
    pub fn of<'a>(needles: impl IntoIterator<Item = &'a str>) -> Self {
        let mut hashed = Self::default();
        for needle in needles {
            match &*WordSet::needle(needle) {
                [word] => {
                    hashed.single.insert(*word);
                }
                words => hashed.several.push(words.into()),
            }
        }
        hashed
    }
}

impl MemSize for WordSet {
    fn heap_size(&self) -> usize {
        self.0.len() * std::mem::size_of::<u64>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_written_any_of_the_ways_php_writes_it_is_found() {
        let set = WordSet::of("<?php use App\\Repo; new \\App\\Repo(); Repo::class;");
        assert!(set.may_hold("Repo"));
        assert!(set.may_hold("repo"), "the scan it replaces ignores case");
        assert!(set.may_hold("App"));
    }

    #[test]
    fn a_needle_spelled_with_punctuation_needs_every_word_of_it() {
        let set =
            WordSet::of("<?php class A extends B { function f() { parent::__construct(); } }");
        assert!(set.may_hold("parent::__construct"));
        assert!(!set.may_hold("parent::__destruct"));
    }

    #[test]
    fn a_word_this_file_only_holds_a_part_of_is_refused() {
        // The one place this is stricter than the substring scan it replaces, and the reason that
        // is sound: a real reference to `Foo` writes `Foo`, never only `FooBar`.
        let set = WordSet::of("<?php $x = new FooBar();");
        assert!(set.may_hold("FooBar"));
        assert!(!set.may_hold("Foo"));
    }

    #[test]
    fn a_needle_with_no_words_is_held_by_everything() {
        let set = WordSet::of("<?php class A {}");
        assert!(set.may_hold("::"));
        assert!(set.may_hold(""));
    }

    #[test]
    fn many_needles_at_once_answer_as_each_would_alone() {
        let set =
            WordSet::of("<?php class A extends B { function f() { parent::__construct(); } }");
        let asked = |needles: &[&str]| set.may_hold_any(&WordNeedles::of(needles.iter().copied()));
        assert!(asked(&["Missing", "b"]));
        assert!(asked(&["parent::__construct"]));
        assert!(!asked(&["parent::__destruct", "Missing"]));
        assert!(!asked(&[]), "no needle is held by nothing");
        assert!(
            asked(&["::"]),
            "a needle with no words is held by everything"
        );
        // More needles than the file has words, which is the other side of the walk.
        let many = (0..100).map(|n| format!("w{n}")).collect::<Vec<_>>();
        let mut names = many.iter().map(String::as_str).collect::<Vec<_>>();
        assert!(!asked(&names));
        names.push("A");
        assert!(asked(&names));
    }

    #[test]
    fn the_words_are_sorted_and_carry_no_repeats() {
        let set = WordSet::of("<?php $a = $a + $a;");
        let sorted = set.as_sorted();
        assert!(sorted.windows(2).all(|pair| pair[0] < pair[1]));
    }
}
