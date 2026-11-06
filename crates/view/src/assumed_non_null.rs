//! Where a framework contract took a sentinel off a declaration, and the state it really comes from.
//!
//! Two silences wear one face and only one of them is a proof. `Select::execute()` is declared
//! `?StatementInterface` and cannot return `NULL` on any run, so reporting it would be a false
//! positive and there is nothing to disclose. `FileInterface::getFileUri()` is declared `?string`
//! and `File::create([])->getFileUri()` really is null -- the contract that removes it is a
//! judgement about which states a project's code is in, and the null it hides is real.
//!
//! Which one each entry is, is now a field the contract carries rather than a sentence in a doc
//! comment, and this is what reads it. Only the second kind is here: what is disclosed is what was
//! decided, not what was proved.
//!
//! Off unless a project asks for it, and the measurement the contracts were written from is why --
//! on the reference checkout the null-safety findings they suppress were 118 of 379, "all of them
//! noise", which is the cost that gets a checker turned off. A project chasing PHPStan's level 8
//! wants them anyway, and the point of this file is that it can have them without either of us
//! pretending the null was never there.
//!
//! # Only where the decision changed an answer
//!
//! A contract applies to every call of a contracted method, and at nearly all of them nothing
//! depends on it: `sprintf('%s', $file->getFileUri())`, `'name' => $file->getFilename()`, a
//! concatenation, a `(string)` cast, a `?? ''` the author already wrote. Disclosing there is a
//! disclosure about nothing -- the same complaint the argument filter below was written for, and
//! measured the same way: 130 disclosures on the reference project, of which **PHPStan at level 8
//! reports a null problem at 19 and one more line down at 4**. The other 107 say that a decision
//! was made where no answer would have differed.
//!
//! So a disclosure survives only where the sentinel, had it stayed, would have been **refused** --
//! passed where a parameter does not take one, dereferenced, returned against a declaration that
//! does not admit one, subscripted, iterated, called, or assigned to something that rejects it.
//!
//! That question is not answered here by a second rule about nullability. It is answered by reading
//! the file twice: once as the index reads it, and once through
//! [`IndexView::with_contracts_lifted`], where the declarations say what they say. Whatever the
//! ordinary checks report against the second view and not the first *is* the set the contracts
//! suppressed, by construction -- which is the difference between a filter and a second opinion,
//! and the shape §8 calls two constructors of one rule is what it exists to avoid. A seventh kind
//! of refusal is one line in [`IndexView::sentinel_refusals`] and no new rule anywhere.

use phrost_body::AssignmentTarget;
use phrost_model::{Name, TextRange};
use phrost_sema::MemberKind;

use crate::{IndexView, MemberAccessOutcome, references::SentinelReach};

/// One call whose declared sentinel a contract removed, and why.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct AssumedNonNull {
    /// The member name, which is what a report underlines: the decision is about this call and not
    /// about the receiver that happens to be on its left.
    pub range: TextRange,
    pub name: Name,
    /// The class the contract was written against, which may be above the receiver's own.
    ///
    /// A `Box<str>` and not a `Name`: interning needs the table open for writing and this is read
    /// from a projection, and what the string is for is a sentence rather than an identity.
    pub declarer: Box<str>,
    /// Which sentinel was taken off, as a word a message can use.
    pub sentinel: &'static str,
    /// The state it really arrives in, finishing the sentence "this can be null only ...".
    pub only_when: Box<str>,
}

impl IndexView<'_> {
    /// Every call in the current file whose sentinel a contract decided not to report, and whose
    /// value something would have refused had it stayed.
    ///
    /// Asked of the accesses the view already holds rather than by a walk of its own, and only for
    /// the ones that *resolved*: a contract is keyed by a class, so an access with no class has no
    /// contract on it and nothing was decided.
    pub fn assumed_non_nulls(&self) -> Vec<AssumedNonNull> {
        let candidates = self.contracted_accesses();
        if candidates.is_empty() {
            return Vec::new();
        }

        // The two reads, in the order that lets the commoner file stop after one: a file whose
        // contracted values are all passed somewhere tolerant refuses nothing, and there is nothing
        // to subtract from.
        let lifted = self.with_contracts_lifted().sentinel_refusals();
        if lifted.is_empty() {
            return Vec::new();
        }
        // What is refused with the contracts in force is refused for some other reason -- a `?:`
        // whose other operand is nullable, a string assigned to an `int` property -- and no
        // contract decided it.
        let standing = self.sentinel_refusals();
        let suppressed: Vec<TextRange> = lifted
            .into_iter()
            .filter(|range| !standing.contains(range))
            .collect();

        candidates
            .into_iter()
            .filter(|found| self.reaches_a_refusal(&suppressed, found.range))
            .collect()
    }

    /// The calls a contract spoke about, before asking whether it mattered.
    fn contracted_accesses(&self) -> Vec<AssumedNonNull> {
        let Some(open) = self.current() else {
            return Vec::new();
        };
        let hir = open.hir();
        let mut found = Vec::new();
        for access in self.member_accesses().iter() {
            // A method a string names is not called there, so nothing is read off its result.
            if access.kind != MemberKind::Method || access.named {
                continue;
            }
            let MemberAccessOutcome::Resolved { class } = &access.outcome else {
                continue;
            };
            // Nothing to disclose where the sentinel still arrives: the ordinary check reports it
            // and saying both would be two reports about one value.
            for (sentinel, word, reach) in [
                (phrost_model::BuiltinTy::Null, "null", access.null_reach),
                (phrost_model::BuiltinTy::False, "false", access.false_reach),
            ] {
                if reach != SentinelReach::Impossible {
                    continue;
                }
                let disclosed = self
                    .skeleton
                    .returns()
                    .disclosed_declarers(access.name.as_str(), sentinel)
                    .find(|(declarer, _, _)| {
                        class.as_str().eq_ignore_ascii_case(declarer)
                            || self.descends_from(
                                class.as_str(),
                                declarer,
                                crate::resolve::HIERARCHY_DEPTH,
                            )
                    });
                let Some((declarer, only_when, needs_argument)) = disclosed else {
                    continue;
                };
                // A state the *call* can be read for, and this call is not in it. Disclosing
                // "null only when the call passes NULL_ON_INVALID_REFERENCE" beside a call that
                // plainly passes one argument is a disclosure about nothing -- 233 of 362 on the
                // reference project, every one of them a single-argument `get()`.
                if let Some(position) = needs_argument
                    && !self.call_passes_argument(hir, access.range.end(), position)
                {
                    continue;
                }
                found.push(AssumedNonNull {
                    range: access.range,
                    name: access.name.clone(),
                    declarer: declarer.into(),
                    sentinel: word,
                    only_when: only_when.into(),
                });
            }
        }
        found
    }

    /// Every expression in the current file that a sentinel reaching it would be refused at.
    ///
    /// The seven places one is, each named by the check that owns it, and the ranges are the
    /// expressions those checks underline -- so a range **contains** the call whose value arrived
    /// there. Nothing here decides what a refusal is; it asks the checks, which is the whole point
    /// of reading the file twice rather than writing the rule again.
    ///
    /// Only the sentinel-shaped refusals. `incompatible_assignments` is the one entry that also
    /// speaks about types that are wrong for other reasons, and it is why the caller subtracts what
    /// stands with the contracts applied rather than trusting this list to be about nulls alone.
    fn sentinel_refusals(&self) -> Vec<TextRange> {
        let mut refused: Vec<TextRange> = Vec::new();
        refused.extend(self.sentinel_arguments().iter().map(|found| found.range));
        refused.extend(self.null_returns().iter().map(|found| found.range));
        refused.extend(self.null_subscripts().iter().map(|found| found.range));
        refused.extend(self.not_iterable_loops().iter().map(|found| found.range));
        refused.extend(self.not_callable_values().iter().map(|found| found.range));
        refused.extend(
            self.incompatible_assignments()
                .iter()
                .map(|found| found.range),
        );
        // A dereference: the receiver is the expression the value arrived at, and the member name
        // is where the ordinary check reports.
        refused.extend(
            self.member_accesses()
                .iter()
                .filter(|access| {
                    access.null_reach != SentinelReach::Impossible
                        || access.false_reach != SentinelReach::Impossible
                })
                .map(|access| access.receiver_range),
        );
        refused
    }

    /// Whether the value produced at `at` arrives somewhere that refuses a sentinel.
    ///
    /// One hop, and one only. Directly is the common half -- the call is written inside the
    /// argument, the `return`, the receiver. Through a local is the other, and it is not a rarity
    /// to be waved away: `$uri = $file->getFileUri();` on one line and `is_file($uri)` on the next
    /// is four of the twenty-three the reference project has. Past that the value is somebody
    /// else's -- a second local, a property, an array -- and following it would be inventing the
    /// dataflow this file is written to avoid.
    fn reaches_a_refusal(&self, refused: &[TextRange], at: TextRange) -> bool {
        if refused.iter().any(|range| range.contains_range(at)) {
            return true;
        }
        let Some(open) = self.current() else {
            return false;
        };
        let hir = open.hir();

        phrost_body::assignment_sites(hir)
            .into_iter()
            .filter(|site| site.value_range.contains_range(at))
            .filter_map(|site| match site.target {
                AssignmentTarget::Variable(name) => Some((name, site.target_range)),
                _ => None,
            })
            .any(|(name, target)| {
                phrost_body::local_occurrences(hir, target.start(), name.as_str())
                    .iter()
                    .filter(|occurrence| occurrence.kind != phrost_body::OccurrenceKind::Write)
                    .any(|occurrence| {
                        refused
                            .iter()
                            .any(|range| range.contains_range(occurrence.range))
                    })
            })
    }

    /// Whether the call whose callee ends at this offset wrote an argument at `position`.
    ///
    /// The *end* of the member name, and that is the whole of what makes it the right call:
    /// `call_site_at` refuses a call whose callee has not finished at the offset, so asking from
    /// the name's start walks past this call to whatever encloses it -- and an enclosing
    /// `$this->log(\Drupal::service('x'))` has an argument where the `service('x')` does not.
    /// Measured wrong first: 33 disclosures filtered where 233 were expected, every one of the 200
    /// left over reading the arguments of somebody else's call.
    ///
    /// Presence and not value: reading the value would mean resolving a class constant per call on
    /// a path a project switched on in order to *read*, and presence already separates every real
    /// case from every noisy one -- a call that does not pass the argument cannot be in the state
    /// whatever the argument would have said.
    fn call_passes_argument(
        &self,
        hir: &phrost_hir::FileHir,
        at: phrost_model::TextSize,
        position: u16,
    ) -> bool {
        phrost_body::call_site_at(hir, at)
            .is_some_and(|call| call.arguments.len() > usize::from(position))
    }
}
