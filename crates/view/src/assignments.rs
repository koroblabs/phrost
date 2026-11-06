//! An assignment whose value cannot be what its target declares.
//!
//! `$this->total = 'x';` where the property is `int`. PHP checks a typed property on every write,
//! under weak mode as well as strict, so this is a `TypeError` on every run that reaches the line
//! rather than a style note. The same judgement the argument check makes, from the other side of an
//! `=`.
//!
//! # Two kinds of target, and the expectation is spelled differently for each
//!
//! A property and a static property are rows in a member table, so what they declare is a
//! [`TypeId`] interned in the project's table. A local's only declaration is a `/** @var T */`,
//! which is a `TypeRefId` into the file's own arena: never interned, and not internable from a
//! read-only view, because interning takes the type table by `&mut`. Both are judged, and
//! [`crate::compat::Expectation`] is what carries the difference -- so the rules about what the
//! value was *written* as are one rule reached by both, rather than one per spelling.
//!
//! What the annotated target does not get is a report about a null. A `@var` is an assertion and
//! PHP enforces nothing, and the reason the tag is usually written at all is to take a null off a
//! value the signature could not promise: `/** @var Node $node */ $node = Node::load($id);`.
//! Reporting that is arguing with the author about the one thing the annotation exists to say --
//! measured at 30 of 35 findings on a real project, every one of them a `?T` annotated as `T`.
//!
//! # What stays quiet
//!
//! Everything the index cannot account for, on the rule the whole judgement is built on: reject only
//! what can be proven wrong. Beyond [`Fit::Unknown`], which is most of what real code produces:
//!
//! - a target whose receiver has no type, or no single class that owns the property;
//! - a receiver that may be null, which the null-receiver check owns -- saying both "this may be
//!   null" and "this cannot hold that" about one write is two reports for one mistake;
//! - a property nothing declares a type for, which takes anything;
//! - a member declared by an `@property` tag, because the write goes to `__set` and PHP enforces
//!   nothing about what a docblock claimed;
//! - a property PHP would not let this scope write directly, for the same reason -- `__set`
//!   answers it, and if nothing does then the finding is the visibility one;
//! - a value with no path, a target this layer cannot name, and a compound assignment, each of
//!   which `phrost_body::assignment_sites` has already declined.

#[cfg(test)]
mod tests;

use phrost_body::AssignmentTarget;
use phrost_hir::FileHir;
use phrost_model::{BuiltinTy, Name, TextRange, TextSize};
use phrost_sema::MemberKind;
use phrost_ty::{TypeId, compat::Fit};

use crate::compat::Expectation;
use crate::variables::Naming;
use crate::{
    references::SentinelReach,
    view::{FOLLOW_LIMIT, IndexView},
};

/// What an assignment writes to, named the way a report should name it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum AssignedTo {
    /// A declared property. The class is the one being written *through* -- what PHP's own error
    /// names and what the reader has in front of them -- which is not always the class that
    /// declared it, and the declaration is usually in another file.
    Property { class: Name, property: Name },
    /// A local carrying a `/** @var T */`. The declaration is three lines up, so naming the
    /// variable is enough and naming a file would be noise.
    Annotated(Name),
}

/// An assignment whose value no value of the target's declared type can be.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct IncompatibleAssignment {
    /// The value as written, which is the expression to go and change. The target is named in the
    /// message instead: it is where the type was declared, and it is often in another file.
    pub range: TextRange,
    /// What the type was declared on, as a message should name it.
    pub target: AssignedTo,
    /// The type the property declares, rendered.
    pub declared: String,
    /// The type the value has, rendered.
    pub given: String,
}

/// A write into a typed property of a value that may be `false` where the property does not take it.
///
/// The half of a sentinel [`IncompatibleAssignment`] cannot see. A null into a non-nullable property
/// is a `TypeError` on the runs that carry it, and the judgement already reports that; a `false`
/// is converted in weak mode -- written as `""` into a `string`, `0` into an `int` -- so PHP accepts
/// it, the judgement accepts it with PHP, and nothing said that the write stores the empty string
/// where the read failed. `$this->content = fread($handle, 8)` is the shape, and PHPStan reports it
/// at level 8 as `assign.propertyType`. The argument side has had `possibly-false-argument` for the
/// same reason.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SentinelAssignment {
    /// The value as written, which is the expression to go and guard.
    pub range: TextRange,
    /// The class written through and the property, as the message names them.
    pub class: Name,
    pub property: Name,
    /// The type the property declares, rendered.
    pub declared: String,
    /// Whether the value is `false` on every run or only on some.
    pub certain: bool,
}

/// A target's declaration, once the property behind it has been found.
struct Declared {
    class: Name,
    property: Name,
    ty: TypeId,
}

impl IndexView<'_> {
    /// Every assignment in the current file whose value the target's declared type cannot hold.
    ///
    /// Only what the judgement can *prove* wrong; see the module docs for what that leaves out and
    /// the policy in `crates/ty/src/compat/mod.rs` for why.
    pub fn incompatible_assignments(&self) -> Vec<IncompatibleAssignment> {
        let Some(open) = self.current() else {
            return Vec::new();
        };
        let hir = open.hir();
        let types = self.skeleton.types();
        let mut out = Vec::new();

        // And what a `clone()` writes on its copy, which is the same write to the same class.
        let cloned = phrost_body::cloned_property_writes(hir);
        for site in phrost_body::assignment_sites(hir).into_iter().chain(cloned) {
            let Some(path) = &site.value else {
                continue;
            };
            // The value's own position, so `self` and `static` in whatever it resolves through mean
            // the class this assignment is written in.
            let at = site.value_range.start();
            let Some(held) = self.resolve_path(at, hir, path, FOLLOW_LIMIT) else {
                continue;
            };

            // Two shapes, and they differ only in how the expectation is spelled: a property's type
            // is interned, and an annotation is an index into this file's own arena.
            let (target, declared) = match (&site.target, site.annotation) {
                (AssignmentTarget::Variable(name), Some(annotation)) => {
                    // A `@var` is an assertion and not a constraint: PHP enforces nothing, and the
                    // reason the tag is written at all is usually to take a null off a value the
                    // signature could not promise -- `/** @var Node $node */ $node =
                    // Node::load($id);`. Reporting that would be arguing with the author about the
                    // one thing the annotation exists to say. Measured: it is 30 of 35 findings on
                    // a real project, all of them `?T` annotated as `T`.
                    if self.null_reach_of(hir, &held) != SentinelReach::Impossible {
                        continue;
                    }
                    // And for the same reason, an annotation narrower than the value is the author
                    // saying which of its branches this one is: `/** @var object|false $item */`
                    // over Drupal's `bool|object` `claimItem()`. Only an annotation no branch of
                    // the value can be contradicts it.
                    let expected = Expectation::Written(annotation);
                    // A string annotated as one that holds something is the same narrowing, made
                    // inside the kind: `/** @var non-empty-string $name */` over a `string` says
                    // which strings, and contradicts nothing.
                    let held =
                        match self.string_held(hir, &crate::resolve::Held::Written(annotation)) {
                            Some(holding) if holding != phrost_model::BuiltinTy::String => {
                                self.with_strings_holding(at, hir, held.clone(), holding)
                            }
                            _ => held.clone(),
                        };
                    if !self.alternatives(hir, &held).iter().all(|one| {
                        self.fit_written_value(at, hir, site.value_expr, one, expected)
                            == Fit::Rejected
                    }) {
                        continue;
                    }
                    (
                        AssignedTo::Annotated(name.clone()),
                        phrost_hir::render::render_type_ref(hir, annotation),
                    )
                }
                _ => {
                    let Some(property) = self.declared_property(hir, &site) else {
                        continue;
                    };
                    if self.fit_written_value(
                        at,
                        hir,
                        site.value_expr,
                        &held,
                        Expectation::Stored(property.ty),
                    ) != Fit::Rejected
                    {
                        continue;
                    }
                    (
                        AssignedTo::Property {
                            class: property.class,
                            property: property.property,
                        },
                        types.render_qualified(property.ty),
                    )
                }
            };

            out.push(IncompatibleAssignment {
                range: site.value_range,
                target,
                declared,
                given: self
                    .render_held(site.value_range.start(), hir, &held, Naming::Short)
                    .unwrap_or_else(|| "a value of another type".to_owned()),
            });
        }

        out
    }

    /// Every write into a typed property of a value that may be `false` where the property does not
    /// take it.
    ///
    /// Properties only. A `@var` on a local is an assertion PHP enforces nothing about, and the
    /// usual reason to write one is to take a null off -- see the module docs. A property's own
    /// declaration is found exactly as [`Self::incompatible_assignments`] finds it, so a write
    /// through `__set`, through a receiver that may itself be null, or into an `@property` tag is
    /// silent here for the same reasons.
    ///
    /// A certain `false` is reported too, as a certain one is for an argument: weak mode accepts it,
    /// so nothing else says it either.
    pub fn sentinel_assignments(&self) -> Vec<SentinelAssignment> {
        let Some(open) = self.current() else {
            return Vec::new();
        };
        let hir = open.hir();
        let types = self.skeleton.types();
        let mut out = Vec::new();

        let cloned = phrost_body::cloned_property_writes(hir);
        for site in phrost_body::assignment_sites(hir).into_iter().chain(cloned) {
            let Some(path) = &site.value else {
                continue;
            };
            if matches!(site.target, AssignmentTarget::Variable(_)) {
                continue;
            }
            let Some(property) = self.declared_property(hir, &site) else {
                continue;
            };
            if self.expectation_admits(hir, Expectation::Stored(property.ty), BuiltinTy::False) {
                continue;
            }
            let at = site.value_range.start();
            let Some(held) = self.resolve_path(at, hir, path, FOLLOW_LIMIT) else {
                continue;
            };
            // A value that may also be null is the judgement's report already, about the null; one
            // report per write.
            if self.null_reach_of(hir, &held) != SentinelReach::Impossible {
                continue;
            }
            let certain = match self.false_reach_of(hir, &held) {
                SentinelReach::Impossible => continue,
                SentinelReach::Possible => false,
                SentinelReach::Certain => true,
            };
            out.push(SentinelAssignment {
                range: site.value_range,
                class: property.class,
                property: property.property,
                declared: types.render_qualified(property.ty),
                certain,
            });
        }

        out
    }

    /// The property one assignment writes to, and the type it declares.
    ///
    /// `None` wherever the target is not a property the index can account for -- which includes the
    /// annotated-local shape the module docs describe, and every case where naming a type would be
    /// a guess about somebody else's class.
    fn declared_property(
        &self,
        hir: &FileHir,
        site: &phrost_body::AssignmentSite,
    ) -> Option<Declared> {
        let at = site.target_range.start();
        let (class, property) = match &site.target {
            AssignmentTarget::Property { receiver, name } => {
                let held = self.resolve_path(at, hir, receiver, FOLLOW_LIMIT)?;
                // A write through a value that may be null is the null check's finding, and the
                // property it names is not reachable to be judged anyway.
                if self.null_reach_of(hir, &held) != SentinelReach::Impossible {
                    return None;
                }
                let class =
                    self.owner_of_member(at, hir, &held, MemberKind::Property, name.as_str())?;
                (class, name.clone())
            }
            AssignmentTarget::StaticProperty { class, name } => {
                (self.class_of_target(at, hir, *class)?, name.clone())
            }
            // The `@var` shape. See the module docs: the annotation is a type in this file's arena
            // and the judgement's expected side takes an interned one.
            AssignmentTarget::Variable(_) => return None,
        };

        self.declared_on(at, &class, &property).map(|ty| Declared {
            class,
            property,
            ty,
        })
    }

    /// The type a class declares for one of its properties, when PHP would enforce it on a write
    /// made here.
    fn declared_on(&self, at: TextSize, class: &Name, property: &Name) -> Option<TypeId> {
        let table = self.members(class.as_str());
        let member = table.find(MemberKind::Property, property.as_str())?;

        // An `@property` tag describes what `__set` does with a name the class never declared, so
        // the runtime enforces nothing and a rejection here would be an argument with a comment.
        if member.row.from_docblock {
            return None;
        }
        // Out of reach from here, so either `__set` takes the write -- and the declared type says
        // nothing about it -- or the mistake is the access, which the visibility check owns.
        if self
            .inaccessible_member(at, class.as_str(), MemberKind::Property, property.as_str())
            .is_some()
        {
            return None;
        }

        member.row.effective_ty(self.skeleton.types())
    }
}
