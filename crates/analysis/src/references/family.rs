//! Every declaration of one member that a rename has to move at once.
//!
//! `AttachmentProcessorInterface::processBatch` is not one declaration, it is a contract: six classes
//! implement it, and renaming the interface's copy leaves each of them answering the old name --
//! which is a project that no longer runs. Renaming *only* the implementations is the same mistake
//! upside down. So a rename of a member is a rename of the family or nothing.
//!
//! # Both directions, and why the downward walk does not stop at the first class that declares it
//!
//! Upwards is the cheap half: a class's header names its parents, and a declaration above is a
//! declaration this one overrides. It is asked transitively rather than once, because a class can
//! implement two interfaces that both declare the name -- and then renaming the first without the
//! second leaves the class satisfying neither.
//!
//! Downwards costs a pass over the class table per class, and it cannot stop where a subtype fails
//! to redeclare: `B extends A` inheriting quietly still has a `C extends B` that overrides, and
//! stopping at `B` would leave `C::run()` behind. So every subtype is walked and only the ones that
//! declare the name join the family.
//!
//! # A bound, and a refusal rather than a guess
//!
//! The walk gives up past [`MOST_CLASSES`], and giving up means the rename is refused. That is the
//! whole difference between this and a rename that is usually right: a hierarchy too large to
//! enumerate is one nobody can prove safe to rename, and saying so costs a sentence where being
//! wrong costs a working project.

use phrost_model::Name;
use phrost_view::{IndexView, MemberKind};

/// How many classes the walk will look at before giving up.
///
/// Each step downwards is a pass over the project's class table -- the table is scanned, not
/// indexed by supertype -- so this is the price of the request as much as a safety bound. A member
/// declared on an interface a hundred classes implement is not a rename an editor should offer,
/// whatever it costs.
const MOST_CLASSES: usize = 40;

/// How many of one class's direct subtypes *from outside the project* are looked at, matching
/// goto-implementation's own bound. The project's own are all looked at, and [`MOST_CLASSES`]
/// bounds the walk as a whole either way.
const SUBTYPES: usize = 100;

/// The declarations of one member that PHP makes a single contract.
#[derive(Clone, PartialEq, Eq, Debug)]
pub(crate) struct MemberFamily {
    /// Every class declaring it: the one asked about first, then the rest in the order found.
    pub(crate) declarers: Vec<Name>,
    /// Whether the walk finished. See [`MOST_CLASSES`].
    pub(crate) complete: bool,
}

/// Every class that declares this member and is in the same hierarchy as `declared_by`.
///
/// Always at least the class asked about, so a member nothing overrides comes back as a family of
/// one and every caller can be written the same way.
pub(crate) fn member_family(
    view: &IndexView<'_>,
    declared_by: &Name,
    kind: MemberKind,
    name: &str,
) -> MemberFamily {
    // Whichever class actually wrote it, so a cursor on an inherited access and one on the
    // declaration start the walk from the same place.
    let root = view
        .member_location(declared_by.as_str(), kind, name)
        .and_then(|here| here.container)
        .unwrap_or_else(|| declared_by.clone());

    let mut declarers: Vec<Name> = vec![root.clone()];
    // A member PHP will not let a subtype redeclare has no family below it, and a private one is
    // not inherited at all -- a subclass writing the same name declares a different member. Worth
    // asking before the scans rather than after, because a private helper is the commonest thing
    // anybody renames.
    if !can_be_overridden(view, root.as_str(), kind, name) {
        return MemberFamily {
            declarers,
            complete: true,
        };
    }

    // A level at a time, so the subtypes of everything on a level cost one pass over the class
    // table between them -- one per class was forty passes and 470 ms on Laravel's `Model`.
    let mut walked: Vec<Name> = Vec::new();
    let mut level: Vec<Name> = vec![root];

    while !level.is_empty() {
        let mut fresh: Vec<Name> = Vec::new();
        for class in level {
            if walked.iter().any(|seen| same_class(seen, &class)) {
                continue;
            }
            if walked.len() >= MOST_CLASSES {
                return MemberFamily {
                    declarers,
                    complete: false,
                };
            }
            walked.push(class.clone());
            fresh.push(class);
        }

        let mut next: Vec<Name> = Vec::new();
        for class in &fresh {
            for ancestor in ancestors_of(view, class.as_str()) {
                if view.declares_member(ancestor.as_str(), kind, name) {
                    remember(&mut declarers, &ancestor);
                    next.push(ancestor);
                }
            }
        }
        for subtype in view.implementers_of_any(&fresh, SUBTYPES * fresh.len()) {
            if view.declares_member(subtype.as_str(), kind, name) {
                remember(&mut declarers, &subtype);
            }
            // Walked whether or not it declares the name: a subtype that inherits quietly still
            // has subtypes of its own, and one of those may override.
            next.push(subtype);
        }
        level = next;
    }

    MemberFamily {
        declarers,
        complete: true,
    }
}

/// Every class above this one, transitively, through all three ways PHP composes them.
/// The member's own declaration and every class above it that declares it too -- the cheap half of
/// [`member_family`]. See [`super::rename_target_above`].
pub(crate) fn declared_above(
    view: &IndexView<'_>,
    declared_by: &Name,
    kind: MemberKind,
    name: &str,
) -> Vec<Name> {
    let root = view
        .member_location(declared_by.as_str(), kind, name)
        .and_then(|here| here.container)
        .unwrap_or_else(|| declared_by.clone());
    let mut declarers = vec![root.clone()];
    for ancestor in ancestors_of(view, root.as_str()) {
        if view.declares_member(ancestor.as_str(), kind, name) {
            remember(&mut declarers, &ancestor);
        }
    }
    declarers
}

fn ancestors_of(view: &IndexView<'_>, class: &str) -> Vec<Name> {
    let Some(header) = view.class_header(class) else {
        return Vec::new();
    };
    let mut found: Vec<Name> = Vec::new();
    let mut frontier: Vec<Name> = header
        .extends
        .iter()
        .chain(header.implements.iter())
        .chain(header.uses.iter())
        .cloned()
        .collect();

    while let Some(above) = frontier.pop() {
        if found.iter().any(|seen| same_class(seen, &above)) || found.len() >= MOST_CLASSES {
            continue;
        }
        if let Some(header) = view.class_header(above.as_str()) {
            frontier.extend(
                header
                    .extends
                    .iter()
                    .chain(header.implements.iter())
                    .chain(header.uses.iter())
                    .cloned(),
            );
        }
        found.push(above);
    }

    found
}

/// Whether PHP would let a subtype redeclare this member at all.
fn can_be_overridden(
    view: &IndexView<'_>,
    declared_by: &str,
    kind: MemberKind,
    name: &str,
) -> bool {
    view.member_location(declared_by, kind, name)
        .and_then(|location| location.member_modifiers)
        .is_none_or(|modifiers| {
            !modifiers.is_final && modifiers.visibility != phrost_model::Visibility::Private
        })
}

/// Class names, compared the way PHP compares them.
fn same_class(left: &Name, right: &Name) -> bool {
    left.as_str().eq_ignore_ascii_case(right.as_str())
}

fn remember(declarers: &mut Vec<Name>, class: &Name) {
    if !declarers.iter().any(|seen| same_class(seen, class)) {
        declarers.push(class.clone());
    }
}
