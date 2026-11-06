use phrost_model::{Name, Visibility};

use crate::member::{MemberKind, MemberRow};

/// Every member a class has, after inheritance and trait application.
#[derive(Clone, Debug, Default)]
pub struct MemberTable {
    members: Vec<ResolvedMember>,
    conflicts: Vec<MemberConflict>,
    /// The classes, interfaces, and traits walked to build this, in the order they were reached.
    ///
    /// Retained so a later invalidation can ask "does this table depend on the class that just
    /// changed" without walking the hierarchy again.
    contributors: Vec<Name>,
    /// True when a cycle or a missing class stopped the walk early, so a caller can tell an
    /// incomplete answer from a complete one instead of trusting a short table.
    incomplete: bool,
    /// The depth on the walk's stack of a class this table stopped short of, because reaching it
    /// again went through a `@mixin`.
    ///
    /// Not incomplete: a mixin that loops back names a class whose table is being built, and that
    /// table is where the members end up. But the table is then about *this walk* -- the class the
    /// loop skipped is missing from it -- so it may not be kept for another. Cleared when the walk
    /// leaves the class it was cut at, which is the table that holds everything.
    cut_at: Option<usize>,
    /// Which members carry each name, case folded, in table order -- built on the first [`find`]
    /// and dropped by anything that changes the members.
    ///
    /// A framework class holds hundreds of members and `find` is asked for every access a file
    /// makes; reading the whole list each time was an eighth of checking a large Laravel test.
    ///
    /// [`find`]: Self::find
    by_name: std::sync::OnceLock<rustc_hash::FxHashMap<Box<str>, Vec<u32>>>,
}

impl MemberTable {
    pub fn members(&self) -> &[ResolvedMember] {
        &self.members
    }

    pub fn conflicts(&self) -> &[MemberConflict] {
        &self.conflicts
    }

    pub fn contributors(&self) -> &[Name] {
        &self.contributors
    }

    pub fn is_incomplete(&self) -> bool {
        self.incomplete
    }

    /// Looks a member up using the case rule of its kind.
    ///
    /// Class constants and enum cases are one namespace: `Stage::APPROVED` is written as a constant
    /// access and PHP resolves it to a case, and no class can declare both under one name. A lookup
    /// that insisted on the exact kind would miss every enum case ever written.
    pub fn find(&self, kind: MemberKind, name: &str) -> Option<&ResolvedMember> {
        let by_name = self.by_name.get_or_init(|| {
            let mut by_name: rustc_hash::FxHashMap<Box<str>, Vec<u32>> =
                rustc_hash::FxHashMap::default();
            for (index, member) in self.members.iter().enumerate() {
                by_name
                    .entry(member.name.as_str().to_ascii_lowercase().into())
                    .or_default()
                    .push(u32::try_from(index).unwrap_or(u32::MAX));
            }
            by_name
        });
        // Every spelling that could match under either case rule, in table order -- so the first
        // that passes the exact test below is the one a scan of the whole list would have found.
        let candidates = by_name
            .get(name.to_ascii_lowercase().as_str())
            .map_or(&[][..], Vec::as_slice);
        candidates
            .iter()
            .map(|&index| &self.members[index as usize])
            .find(|member| {
                member.row.kind.shares_namespace_with(kind)
                    && if kind.is_case_insensitive() {
                        member.name.as_str().eq_ignore_ascii_case(name)
                    } else {
                        member.name.as_str() == name
                    }
            })
    }

    pub(crate) fn push(&mut self, member: ResolvedMember) {
        self.by_name = std::sync::OnceLock::new();
        self.members.push(member);
    }

    pub(crate) fn replace(&mut self, index: usize, member: ResolvedMember) {
        self.by_name = std::sync::OnceLock::new();
        self.members[index] = member;
    }

    pub(crate) fn position(&self, kind: MemberKind, name: &str) -> Option<usize> {
        self.members.iter().position(|member| {
            member.row.kind == kind
                && if kind.is_case_insensitive() {
                    member.name.as_str().eq_ignore_ascii_case(name)
                } else {
                    member.name.as_str() == name
                }
        })
    }

    pub(crate) fn record_conflict(&mut self, conflict: MemberConflict) {
        self.conflicts.push(conflict);
    }

    pub(crate) fn note_contributor(&mut self, class: Name) {
        if !self.contributors.contains(&class) {
            self.contributors.push(class);
        }
    }

    pub(crate) fn mark_incomplete(&mut self) {
        self.incomplete = true;
    }

    pub(crate) fn cut_at(&self) -> Option<usize> {
        self.cut_at
    }

    /// Carries a cut from a table merged into this one; the shallowest wins, since it is the last
    /// to be left.
    pub(crate) fn cut(&mut self, depth: Option<usize>) {
        self.cut_at = match (self.cut_at, depth) {
            (Some(held), Some(new)) => Some(held.min(new)),
            (held, new) => held.or(new),
        };
    }

    pub(crate) fn heal_at(&mut self, depth: usize) {
        if self.cut_at == Some(depth) {
            self.cut_at = None;
        }
    }

    /// Orders members for display and for snapshots: by kind, then by name.
    ///
    /// Declaration order is not usable here — members arrive from several classes, so "source
    /// order" has no single meaning — and hash order would make two builds disagree.
    /// Copies the table so one copy can be filtered while the other stays whole.
    ///
    /// Needed because `insteadof` filters what a trait contributes to the merged table, while an
    /// `as` clause has to be able to reach the method that filtering just removed.
    pub(crate) fn clone_shallow(&self) -> Self {
        Self {
            members: self.members.clone(),
            conflicts: Vec::new(),
            contributors: Vec::new(),
            incomplete: self.incomplete,
            cut_at: self.cut_at,
            by_name: std::sync::OnceLock::new(),
        }
    }

    pub(crate) fn set_members(&mut self, members: Vec<ResolvedMember>) {
        self.by_name = std::sync::OnceLock::new();
        self.members = members;
    }

    pub(crate) fn sort(&mut self) {
        self.by_name = std::sync::OnceLock::new();
        self.members.sort_by(|left, right| {
            (left.row.kind, left.name.as_str()).cmp(&(right.row.kind, right.name.as_str()))
        });
        self.conflicts.sort_by(|left, right| {
            (left.member.as_str(), left.first.as_str())
                .cmp(&(right.member.as_str(), right.first.as_str()))
        });
    }
}

/// One member, with where it came from and what it is called here.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ResolvedMember {
    /// The name under which this class exposes the member. Differs from `row.name` when a trait
    /// `as` clause aliased it.
    pub name: Name,
    pub row: MemberRow,
    pub provenance: MemberProvenance,
    /// Visibility after any trait `as` clause changed it.
    pub visibility: Visibility,
    /// Reached through a `@mixin` rather than declared anywhere above the class.
    ///
    /// Such a member answers a call at runtime -- through `__call`, usually -- and is no part of
    /// the hierarchy PHP checks: a class declaring the same name overrides nothing, implements
    /// nothing and may give it any signature it likes.
    pub forwarded: bool,
    /// Reached from any scope, whatever it is declared as, because the class's `__call` hands the
    /// name straight back to this declaration -- Eloquent's protected `increment()`, which
    /// `Model::__call` runs for a caller outside the model. PHP refuses the declaration and runs
    /// the magic method, and the magic method runs the declaration, so the declaration is what
    /// answers. Carried onto an override, which `__call` reaches the same way.
    pub opened: bool,
    /// The class whose docblock `row.doc_ty` was written in, when the override carried it from the
    /// declaration above rather than writing one -- see `Flattener::insert`.
    ///
    /// A carried type is only half an answer without it: `@return T` is a template of the
    /// interface that wrote it, and read as the override's own it names no template and no class,
    /// so it rendered as a class called `T` and an `@implements Getter<Foo>` above never reached it.
    pub doc_from: Option<Name>,
}

impl ResolvedMember {
    /// Whether a scope that has earned `access` reaches this member through its declaration.
    ///
    /// The one place the opening above is applied: whoever offers a member and whoever resolves
    /// one ask this, so the two cannot disagree about which names a caller reaches.
    pub fn reached_with(&self, access: Visibility) -> bool {
        self.opened || self.visibility.is_visible_from(access)
    }
}

/// Where a member in a flattened table came from.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum MemberProvenance {
    /// Declared by the class itself.
    Own,
    /// Copied in from a trait, which in PHP behaves as if it had been written here.
    Trait(Name),
    /// Inherited from a parent class.
    Inherited(Name),
    /// Required by an interface. Carries no body, and loses to any real declaration.
    Interface(Name),
}

impl MemberProvenance {
    /// The class that wrote the member, when it is not the one whose table holds it.
    pub fn declarer(&self) -> Option<&Name> {
        match self {
            Self::Own => None,
            Self::Trait(fqn) | Self::Inherited(fqn) | Self::Interface(fqn) => Some(fqn),
        }
    }

    pub fn label(&self) -> String {
        match self {
            Self::Own => "own".to_owned(),
            Self::Trait(fqn) => format!("trait {fqn}"),
            Self::Inherited(fqn) => format!("inherited {fqn}"),
            Self::Interface(fqn) => format!("interface {fqn}"),
        }
    }
}

/// Two traits declared the same member and nothing resolved it.
///
/// A fatal error in PHP, so it is reported rather than resolved: picking a winner would make the
/// language server disagree with the runtime, and silently disagreeing is worse than saying so.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct MemberConflict {
    pub member: Name,
    pub kind: MemberKind,
    pub first: Name,
    pub second: Name,
}
