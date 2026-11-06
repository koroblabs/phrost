//! Writes PHP refuses at runtime because of how the property was declared.
//!
//! Two declarations say a property may not be written from here, and both are ordinary modern PHP:
//! `readonly`, which admits exactly one write and only from inside the class that declared it, and
//! PHP 8.4's asymmetric visibility (`public private(set) string $name`), which says the read half
//! and the write half have different scopes.
//!
//! Both are `Error` at runtime -- not a warning, not a coercion -- so a write this reports is a line
//! that cannot run. What is reported is only what the declaration settles on its own:
//!
//! - A `readonly` property written from **outside** the class that declared it. PHP allows the one
//!   initialising write only from inside that class, whatever the property's visibility says, so
//!   this needs no flow analysis to be certain of.
//! - A `readonly` property **promoted** in a constructor, written anywhere but that constructor.
//!   Promotion *is* the initialising write, so every later one is the second.
//! - A property whose write scope excludes the writer: `private(set)` from outside, `protected(set)`
//!   from outside the hierarchy. The same rule `inaccessible_member` applies to reads, asked of the
//!   other half of the declaration.
//! - A property whose hooks are a *virtual* `get` alone. PHP 8.4 calls that read-only and refuses
//!   the write from everywhere -- inside the declaring class as much as outside -- which is the one
//!   rule here with no scope in it at all. Virtual and not merely get-only: a hook that uses
//!   `$this->propName` makes the property backed, and a backed one holds a slot that takes writes.
//!
//! What is **not** reported is a second write inside the declaring class to a property that is not
//! promoted: `$this->id = $id;` in a method other than the constructor is how a lazily initialised
//! readonly property is written, and whether it has already been written is a question about which
//! path ran rather than about the declaration.

use phrost_body::AssignmentTarget;
use phrost_model::{Name, TextRange, Visibility};
use phrost_sema::MemberKind;

use crate::{references::SentinelReach, view::FOLLOW_LIMIT, view::IndexView};

/// One write PHP's own rules refuse.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct RefusedWrite {
    /// The target as written, which is what a report underlines.
    pub range: TextRange,
    /// The class that declares the property, fully qualified.
    pub class: Name,
    pub property: Name,
    pub reason: RefusedBecause,
}

/// Why the declaration refuses the write.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RefusedBecause {
    /// `readonly`, written from outside the class that declared it.
    ReadonlyFromOutside,
    /// `readonly` and promoted, so the constructor already wrote it.
    ReadonlyAlreadyPromoted,
    /// `private(set)` or `protected(set)`, written from a scope the set half excludes.
    SetScope(Visibility),
    /// Hooked with a virtual `get` and no `set`, which PHP calls read-only.
    OnlyAGetHook,
    /// A `@property-read` tag, written from outside the class: the class's `__set` refuses it,
    /// or the tag says it should.
    ReadOnlyTag,
}

impl IndexView<'_> {
    /// Every write in the current file that the property's own declaration refuses.
    pub fn refused_writes(&self) -> Vec<RefusedWrite> {
        let Some(open) = self.current() else {
            return Vec::new();
        };
        let hir = open.hir();

        phrost_body::assignment_sites(hir)
            .into_iter()
            .filter_map(|site| self.refused_write(hir, &site))
            .collect()
    }

    fn refused_write(
        &self,
        hir: &phrost_hir::FileHir,
        site: &phrost_body::AssignmentSite,
    ) -> Option<RefusedWrite> {
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
            // A static property is never `readonly` -- PHP refuses the combination outright, with
            // `Static property C::$x cannot be readonly` -- but it does carry a set visibility, and
            // that half was missed here for as long as this arm existed. PHP 8.5.10 was asked: a
            // subclass writing a `private(set)` static gets `Cannot modify private(set) property
            // Tally::$n from scope Kid`, and the whole table is the one instance properties follow.
            AssignmentTarget::StaticProperty { class, name } => {
                let class = self.class_of_target(at, hir, *class)?;
                (class, name.clone())
            }
            AssignmentTarget::Variable(_) => return None,
        };

        let table = self.members(class.as_str());
        let found = table.find(MemberKind::Property, property.as_str())?;
        // The class that wrote the declaration, which is the one whose scope decides -- the same
        // rule `inaccessible_member` follows for reads.
        let owner = match &found.provenance {
            phrost_sema::MemberProvenance::Own | phrost_sema::MemberProvenance::Trait(_) => {
                class.clone()
            }
            phrost_sema::MemberProvenance::Inherited(fqn)
            | phrost_sema::MemberProvenance::Interface(fqn) => fqn.clone(),
        };
        let here = self.visible_from(at, owner.as_str());

        // PHP 8.2's `readonly class` says it once for every property, so the modifier is read off
        // the class as well as off the declaration -- and off the class that *declares* the
        // property, since that is the declaration the word applies to. A static property is never
        // any of this: `readonly` is refused on one, so only the set-visibility rule below applies.
        if found.row.modifiers.is_static() {
            let set = found.row.set_visibility?;
            return (!set.is_visible_from(here)).then_some(RefusedWrite {
                range: site.target_range,
                class: owner,
                property,
                reason: RefusedBecause::SetScope(set),
            });
        }

        let is_readonly = found.row.modifiers.is_readonly()
            || self
                .class_header(owner.as_str())
                .is_some_and(|header| header.is_readonly);

        if is_readonly {
            if here != Visibility::Private {
                return Some(RefusedWrite {
                    range: site.target_range,
                    class: owner,
                    property,
                    reason: RefusedBecause::ReadonlyFromOutside,
                });
            }
            // Promotion is the initialising write, so a second one is refused even inside the
            // class. Anywhere but the constructor itself, which is where promotion happens.
            if self.is_promoted(owner.as_str(), property.as_str())
                && !self.inside_constructor_of(at, owner.as_str())
            {
                return Some(RefusedWrite {
                    range: site.target_range,
                    class: owner,
                    property,
                    reason: RefusedBecause::ReadonlyAlreadyPromoted,
                });
            }
            return None;
        }

        // A *virtual* `get` with no `set` is read-only, and unlike everything above it nothing
        // about the writer's scope enters into it. Virtual is the load-bearing word: a hook that
        // uses `$this->propName` makes the property backed, and a backed one takes a write --
        // Symfony's `ViewEvent` caches into a get-only property from its own constructor, and it
        // runs. Suppressed when any class in the project declares a
        // property of that name with a `set`, because a subclass may override the property and add
        // one -- and a write through the parent's type then runs, which PHP 8.5.10 confirms.
        if found.row.modifiers.hooks.refuse_a_write()
            && !self.skeleton.is_set_hooked(property.as_str())
        {
            return Some(RefusedWrite {
                range: site.target_range,
                class: owner,
                property,
                reason: RefusedBecause::OnlyAGetHook,
            });
        }

        let set = found.row.set_visibility?;
        (!set.is_visible_from(here)).then_some(RefusedWrite {
            range: site.target_range,
            class: owner,
            property,
            // A tag is lowered as `private(set)`, which is what it means for who may write -- and
            // not what it says, so the message names the tag rather than a modifier nobody wrote.
            reason: match found.row.from_docblock {
                true => RefusedBecause::ReadOnlyTag,
                false => RefusedBecause::SetScope(set),
            },
        })
    }

    /// Whether the class declares this property by promoting a constructor parameter.
    ///
    /// Read off the constructor's parameter list rather than the property row, because promotion is
    /// a fact about the parameter -- it is what *makes* the property -- and the row carries what the
    /// property is rather than where it came from.
    fn is_promoted(&self, class_fqn: &str, property: &str) -> bool {
        let table = self.members(class_fqn);
        let Some(constructor) = table.find(MemberKind::Method, "__construct") else {
            return false;
        };
        constructor.row.params.iter().any(|param| {
            param.promotion.is_some() && param.name.as_str().eq_ignore_ascii_case(property)
        })
    }

    /// Whether the position sits in the named class's own constructor.
    ///
    /// Where a promoted property's one write happens. Asked of the *declaring* class, because a
    /// subclass constructor is a different function and PHP refuses the write there too.
    fn inside_constructor_of(&self, at: phrost_model::TextSize, class_fqn: &str) -> bool {
        let Some(open) = self.current() else {
            return false;
        };
        let hir = open.hir();
        if !self
            .enclosing_class(at)
            .is_some_and(|here| here.as_str().eq_ignore_ascii_case(class_fqn))
        {
            return false;
        }
        hir.classes()
            .flat_map(|(_, class)| hir.member_list(class.members))
            .map(|&id| hir.member(id))
            .filter(|member| member.full_range.contains_inclusive(at))
            .min_by_key(|member| member.full_range.len())
            .is_some_and(|member| member.name.as_str().eq_ignore_ascii_case("__construct"))
    }
}
