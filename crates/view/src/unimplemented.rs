//! A concrete class that leaves a method something above it requires undeclared.
//!
//! `class Foo implements Bar {}` where nothing declares `Bar::baz()` is a fatal error at *class
//! load*, before any of the class runs -- the same kind of claim as [`bad_overrides`] and
//! answerable in the same one place, because what the class has to provide is written in other
//! files.
//!
//! # Why the flattened table answers this
//!
//! [`IndexView::members`] merges interfaces, then parents, then traits, then the class's own
//! declarations, and the last writer wins. So a method that is *still* an interface's when the merge
//! finishes is a method no class or trait anywhere in the hierarchy declared, and an abstract row
//! that survived to the end is one nothing concrete replaced. Neither needs a second walk, and both
//! are exactly what PHP checks.
//!
//! # What it declines to report, and why
//!
//! - **An incomplete table.** A parent, trait, or interface this index does not hold truncates the
//!   walk, and a short table looks identical to a satisfied one. A vendor interface phrost never
//!   indexed is the common case, and reporting there would be reporting on a gap here.
//! - **A requirement no supertype was written for.** `@mixin` contributes members and an enum is
//!   given `UnitEnum` without asking, and both arrive with the provenance of a requirement. Only a
//!   class named in an `extends` or `implements` -- at any depth -- can require anything, so that is
//!   the closure a requirement has to come from. Without it every enum in a project is reported for
//!   not declaring `cases()`, which PHP supplies.
//! - **What the language supplies, however the requirement arrived.** The closure above is not
//!   enough on its own: PHP forbids `enum X implements UnitEnum` but an interface may *extend* it,
//!   and `interface HasCases extends UnitEnum` with `enum Stage implements HasCases` is valid PHP
//!   that puts `UnitEnum` into the written set. So `cases()`, `from()` and `tryFrom()` are excluded
//!   by name for an enum -- see [`supplied_by_the_language`].
//! - **A trait's abstract method.** A trait's requirement is satisfiable by an inherited
//!   declaration, and the merge order puts the trait's abstract row *over* the parent's concrete
//!   one -- so the table cannot tell "unmet" from "met by the parent" here. Silence is the safe
//!   direction.
//! - **A name a second file also declares.** The report belongs on the declaration that wins, and
//!   its range comes from there; a class declared twice is a different defect.
//!
//! `__call` is deliberately *not* an excuse. PHP checks the declarations at class load and a magic
//! method does not stand in for one, so a class relying on `__call` to answer an interface is a
//! class that never loads.
//!
//! [`bad_overrides`]: IndexView::bad_overrides

use phrost_model::{Name, TextRange, short_name};
use phrost_sema::{ClassKind, MemberKind, MemberProvenance};
use rustc_hash::FxHashSet;

use crate::{symbols::INHERIT_DEPTH, view::IndexView};

/// One class PHP would refuse to load, and everything it is missing.
///
/// Per class rather than per method: the class is what fails to load, and the repair is one edit
/// even when four names are missing.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct UnimplementedMethods {
    /// The class name alone, which is what a report underlines.
    pub range: TextRange,
    pub class: Name,
    /// What is missing, in a fixed order so two runs over one file agree.
    pub missing: Vec<RequiredMethod>,
}

/// One method a supertype requires and nothing declares.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct RequiredMethod {
    pub name: Name,
    /// The interface or abstract class whose declaration asks for it.
    pub required_by: Name,
}

impl IndexView<'_> {
    /// Every class in the current file that does not declare what its supertypes require.
    pub fn unimplemented_methods(&self) -> Vec<UnimplementedMethods> {
        let Some(file) = self.current_file() else {
            return Vec::new();
        };
        let Some(skeleton) = self.skeleton.file(file) else {
            return Vec::new();
        };
        let mut out = Vec::new();

        for class in skeleton.classes() {
            // An interface, a trait, and an abstract class are all allowed to leave a requirement
            // for somebody below them. Only something instantiable has to answer for it.
            if !matches!(class.kind(), ClassKind::Class | ClassKind::Enum) || class.is_abstract() {
                continue;
            }
            // Nothing written above it, so nothing can require anything of it. Asked before the
            // flatten because it is a length check against work proportional to a hierarchy, and it
            // is exact rather than approximate: a trait's requirement is never reported and a
            // `@mixin` never makes one.
            if class.extends().is_empty() && class.implements().is_empty() {
                continue;
            }
            // The declaration that wins is where the report belongs, and where its range comes from.
            let Some(location) = self.class_location(class.fqn().as_str()) else {
                continue;
            };
            if location.file != file {
                continue;
            }

            let table = self.members(class.fqn().as_str());
            if table.is_incomplete() {
                continue;
            }

            let unmet = table
                .members()
                .iter()
                .filter(|member| member.row.kind == MemberKind::Method)
                // An `@method` tag is a claim about what answers at runtime rather than a
                // declaration -- but it is also the only thing in the source saying so, and a class
                // carrying one is a class whose author has thought about the name.
                .filter(|member| !member.row.from_docblock)
                // Nor is a mixin's: forwarding a call to another class obliges this one to nothing.
                .filter(|member| !member.forwarded)
                .filter_map(|member| match &member.provenance {
                    MemberProvenance::Interface(fqn) => Some((member, fqn)),
                    MemberProvenance::Inherited(fqn) if member.row.modifiers.is_abstract() => {
                        Some((member, fqn))
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();

            // Every class that is correct leaves this empty -- an implemented method carries the
            // provenance of whatever implemented it -- so the hierarchy walk below is paid for only
            // by a class that is about to be reported or by one holding a member the language
            // supplied.
            if unmet.is_empty() {
                continue;
            }

            let required_by = self.written_supertypes(class.fqn().as_str());
            let missing = unmet
                .into_iter()
                // Written into the class's own hierarchy, rather than merged in by a tag or by the
                // language.
                .filter(|(_, declared_by)| required_by.contains(&fold(declared_by.as_str())))
                // And not something the language itself supplies. An enum has `cases()` -- and a
                // backed one `from()` and `tryFrom()` -- whoever asks for them, so a requirement
                // coming from `UnitEnum` or `BackedEnum` is met by the declaration being an enum.
                //
                // Not covered by `written_supertypes` leaving the implicit interfaces out, which is
                // what the comment there says and what is true of `enum X {}`: PHP forbids writing
                // `implements UnitEnum` directly, but an interface may *extend* it, and
                // `interface HasCases extends UnitEnum` with `enum Stage implements HasCases` is
                // valid PHP that put `UnitEnum` into the written set. Three findings on a probe of
                // it, all false.
                .filter(|(_, declared_by)| !supplied_by_the_language(class.kind(), declared_by))
                .map(|(member, declared_by)| RequiredMethod {
                    name: member.name.clone(),
                    required_by: declared_by.clone(),
                })
                .collect::<Vec<_>>();

            if missing.is_empty() {
                continue;
            }
            out.push(UnimplementedMethods {
                range: location.name_range,
                class: location.fqn,
                // The table is already sorted by kind then name, so the order is the table's.
                missing,
            });
        }

        out.sort_by_key(|found| found.range.start());
        out
    }

    /// Every class-like reachable from `fqn` through an `extends` or `implements`, folded.
    ///
    /// The supertypes the *source* names, which is what can impose a requirement. A `@mixin` and an
    /// enum's implicit `UnitEnum` both reach the flattened table and neither is written here, which
    /// is the whole reason this closure exists rather than the table's own contributor list.
    ///
    /// Trait `use` is left out on purpose: a trait's abstract method is not reported, so a trait
    /// cannot be the reason for a finding.
    fn written_supertypes(&self, fqn: &str) -> FxHashSet<String> {
        let mut seen = FxHashSet::default();
        let mut queue = vec![(fqn.to_owned(), 0_u32)];

        while let Some((name, depth)) = queue.pop() {
            // A cycle is already ruled out by the visited set; the bound is for a hierarchy so deep
            // that walking it costs more than the finding is worth.
            if depth > u32::from(INHERIT_DEPTH) {
                continue;
            }
            let Some(header) = self.class_header(&name) else {
                continue;
            };
            for above in header.extends.iter().chain(header.implements.iter()) {
                if seen.insert(fold(above.as_str())) {
                    queue.push((above.as_str().to_owned(), depth + 1));
                }
            }
        }

        seen
    }
}

/// PHP compares class names case-insensitively, and so must a set of them.
fn fold(fqn: &str) -> String {
    fqn.to_ascii_lowercase()
}

/// Whether a requirement is one PHP itself answers for this declaration.
///
/// Only an enum, and only its own two interfaces. `cases()`, `from()` and `tryFrom()` exist on every
/// enum without being written anywhere -- which is why the member table absorbs them from the real
/// `UnitEnum` rather than synthesizing them, and why a check that walks that table has to know they
/// are not a gap.
fn supplied_by_the_language(kind: ClassKind, required_by: &Name) -> bool {
    kind == ClassKind::Enum
        && matches!(
            short_name(required_by.as_str()),
            phrost_sema::UNIT_ENUM | phrost_sema::BACKED_ENUM
        )
}
