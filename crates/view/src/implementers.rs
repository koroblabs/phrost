//! Which classes in a file inherit from a named one.
//!
//! The reverse of the hierarchy walk everything else here does, and it cannot be answered from one
//! class: a class knows what it extends and nothing knows what extends it. So the answer is swept for
//! -- one file at a time, over the files that mention the name at all -- and this is the per-file
//! half of it.
//!
//! Direct inheritance only. `EntityStorageBase implements EntityStorageInterface` is where the
//! relationship is *written*, and a class three levels below it says nothing about the interface; a
//! transitive answer would need the reverse edges of the whole project held in memory, which is a
//! table that grows with `vendor/` and buys a list nobody reads to the bottom of.

use phrost_model::Name;
use phrost_sema::MemberKind;
use rustc_hash::FxHashSet;

use crate::{location::SymbolLocation, view::IndexView};

/// The one member PHP calls without ever writing its name.
const CONSTRUCTOR: &str = "__construct";

/// How a class names the one it was asked about.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Inherits {
    /// `class C extends T` or `interface I extends T`.
    Extends,
    Implements,
    /// `use T;` inside a class body. A trait is not a supertype, but goto on one wants its users for
    /// exactly the reason goto on an interface wants its implementers.
    Uses,
}

/// One class that inherits from the name asked about.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Implementer {
    pub location: SymbolLocation,
    pub how: Inherits,
}

impl IndexView<'_> {
    /// Every class in the project that directly names `fqn` as a supertype or a trait.
    ///
    /// Project-wide and not per-file, unlike references: the answer comes from the class table, which
    /// already holds resolved supertypes for every file. No file is read and no buffer has to be
    /// open, which is also what makes the answer independent of what the editor happens to have open.
    ///
    /// Direct inheritance only. `EntityStorageBase implements EntityStorageInterface` is where the
    /// relationship is *written*; a class three levels below says nothing about the interface. A
    /// transitive answer would need the reverse edges of the whole project held in memory -- a table
    /// that grows with `vendor/` -- and would produce a list nobody reads to the bottom of.
    ///
    /// Every one the project itself declares, plus at most `elsewhere` from a dependency or a stub.
    ///
    /// A framework's base interface has hundreds of implementers and a jump-to list is not a place
    /// to put them all -- but the ones a reader came for are their own, and a flat cap does not
    /// keep that promise: Drupal's `UnitTestCase` has 732 subclasses in core and contributed
    /// modules and 155 in one checkout's own, so a list of 100 held two thirds of nobody's code
    /// and dropped 55 classes the person wrote. The split is the same one
    /// [`candidates`](crate::candidates) makes for a completion page, and for the same reason: the
    /// bound belongs on what `vendor/` contributes, never on what the project does.
    pub fn implementers_of(&self, fqn: &str, elsewhere: usize) -> Vec<Implementer> {
        let mut filler = 0_usize;
        let mut found = Vec::new();
        for (file, class) in self.skeleton.implementers_of(fqn) {
            let Some(how) = [
                (class.extends(), Inherits::Extends),
                (class.implements(), Inherits::Implements),
                (class.uses(), Inherits::Uses),
            ]
            .into_iter()
            .find_map(|(refs, how)| {
                refs.iter()
                    .any(|named| named.fqn.as_str().eq_ignore_ascii_case(fqn))
                    .then_some(how)
            }) else {
                continue;
            };
            // Counted before the location is built, so a dependency past the bound costs the test
            // and nothing else -- the lookup below is what a long list is actually paying for.
            if !self.is_the_projects_own(file) {
                if filler >= elsewhere {
                    continue;
                }
                filler += 1;
            }
            // In the file the sweep found it in, and not by looking the name up again: two files
            // declaring one class have a single winner in the catalog, so the name lookup rendered
            // that winner once per claimant and left the other out of the list entirely. psl's two
            // `comparison-*.php` examples each declare a `Money`, and `Equable`'s implementers held
            // one of them twice.
            if let Some(location) = self.declared_in(file, class.fqn()) {
                found.push(Implementer { location, how });
            }
        }
        found
    }

    /// Every class directly below any of `classes` -- the set form of [`Self::implementers_of`], for
    /// a walk downwards a level at a time.
    ///
    /// One pass over the class table per call, where taking the names one at a time is one per
    /// name: a member family walked class by class on Laravel's `Model` was forty passes and 470 ms
    /// for a `prepareRename`. The project's own subtypes are all kept and at most `elsewhere` from
    /// outside it, as the single form does; ordered the project's own first and then by name, so
    /// two walks agree.
    pub fn implementers_of_any(&self, classes: &[Name], elsewhere: usize) -> Vec<Name> {
        let wanted = classes
            .iter()
            .map(|class| class.as_str().to_ascii_lowercase())
            .collect::<FxHashSet<_>>();
        let mut folded = String::new();
        let mut found = self
            .skeleton
            .implementers_matching(|named| {
                folded.clear();
                folded.extend(named.chars().map(|letter| letter.to_ascii_lowercase()));
                wanted.contains(&folded)
            })
            .into_iter()
            .map(|(file, class)| (!self.is_the_projects_own(file), class.fqn().clone()))
            .collect::<Vec<_>>();
        found.sort_by(|(left_foreign, left), (right_foreign, right)| {
            left_foreign
                .cmp(right_foreign)
                .then_with(|| left.as_str().cmp(right.as_str()))
        });
        found.dedup_by(|left, right| left.1.as_str().eq_ignore_ascii_case(right.1.as_str()));
        let mut outside = 0_usize;
        found
            .into_iter()
            .filter(|(foreign, _)| {
                if !foreign {
                    return true;
                }
                outside += 1;
                outside <= elsewhere
            })
            .map(|(_, class)| class)
            .collect()
    }

    /// Whether a file is code this project wrote, rather than a dependency's or a stub's.
    fn is_the_projects_own(&self, file: phrost_model::FileId) -> bool {
        self.skeleton
            .file(file)
            .is_some_and(|entry| entry.origin() == phrost_model::SourceOrigin::User)
    }

    /// Every class *below* `fqn` whose `new` runs the constructor `fqn` declares.
    ///
    /// Down to -- and not past -- a class that declares a constructor of its own, since `new` on
    /// that one runs *its* constructor and everything under it inherits from there instead. `fqn`
    /// itself is left out: a caller asking about a class already has its name.
    ///
    /// Here because a project-wide search for a constructor's callers cannot be spelled any other
    /// way. `new Child()` runs `Base::__construct` and writes neither `Base` nor `__construct`, so
    /// a sweep looking for the two written spellings of the call never makes that file a candidate
    /// -- and a caller list comes back missing every subclass instantiation, which on real code is
    /// most of them.
    ///
    /// Transitive, unlike [`Self::implementers_of`], because inheritance is: a grandchild that
    /// declares nothing reaches the same constructor as its parent.
    pub fn classes_reaching_constructor(&self, fqn: &str) -> Vec<Name> {
        let mut reached = Vec::new();
        let mut frontier = FxHashSet::from_iter([fqn.to_ascii_lowercase()]);
        let mut walked = frontier.clone();
        // Reused across every comparison of the scan: folding a name per class per level would
        // allocate once for each of a project's forty thousand classes.
        let mut folded = String::new();

        while !frontier.is_empty() {
            let matched = self.skeleton.implementers_matching(|named| {
                folded.clear();
                folded.extend(named.chars().map(|letter| letter.to_ascii_lowercase()));
                frontier.contains(&folded)
            });

            let mut next = FxHashSet::default();
            for (_, class) in matched {
                let name = class.fqn();
                let key = name.as_str().to_ascii_lowercase();
                // Against a hierarchy that names itself, which a half-written file really does
                // produce -- and which would otherwise be walked forever.
                if !walked.insert(key.clone()) {
                    continue;
                }
                if self.declares_member(name.as_str(), MemberKind::Method, CONSTRUCTOR) {
                    continue;
                }
                reached.push(name.clone());
                next.insert(key);
            }
            frontier = next;
        }
        reached
    }
}
