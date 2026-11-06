//! `use T { greet as welcome; }` and `use A, B { A::m insteadof B; }`, as places that name a member.
//!
//! A method name written in one of these clauses is a reference to a *declaration* like any call
//! is, and PHP checks it when the class is declared: a clause naming a method no used trait has is
//! a fatal error -- `An alias (welcome) was defined for method missing(), but this method does not
//! exist`, asked of 8.5 rather than assumed.
//!
//! So a rename that leaves one behind produces a class that no longer loads, which is the worst
//! thing a rename can do. Renaming `Greets::greet` used to edit its declaration and its call sites
//! and step straight over the `greet as welcome;` between them.

use phrost_model::{Name, TextRange, TextSize};
use phrost_sema::MemberKind;

use crate::view::IndexView;

impl IndexView<'_> {
    /// Every trait-adaptation clause in the current file that names `declared_by::name`.
    ///
    /// Matched by the declaration the clause reaches, which is the rule
    /// [`member_uses`](crate::IndexView::member_accesses_named) applies to a call: an adaptation
    /// written `greet` and one written `Greets::greet` name the same method, and a search keyed on
    /// the spelling would find one of them.
    pub fn trait_adaptation_uses(&self, declared_by: &str, name: &str) -> Vec<TextRange> {
        let Some(open) = self.current() else {
            return Vec::new();
        };
        let hir = open.hir();
        let mut out = Vec::new();

        for (_, class) in hir.classes() {
            for &id in hir.adaptation_list(class.trait_adaptations) {
                let adaptation = hir.trait_adaptation(id);
                if !adaptation.method.as_str().eq_ignore_ascii_case(name) {
                    continue;
                }
                // `A::m` names its trait; a bare `m` is only valid when exactly one used trait
                // declares it, so every one of them is a candidate and the member lookup decides.
                let named = match adaptation.trait_name {
                    Some(reference) => vec![reference],
                    None => hir.name_ref_list(class.uses).to_vec(),
                };
                let reaches = named.into_iter().any(|reference| {
                    let written = hir.name_ref(reference);
                    self.resolve_written_name(
                        written.range.start(),
                        written.path.as_str(),
                        written.anchor,
                    )
                    .and_then(|location| {
                        self.member_location(location.fqn.as_str(), MemberKind::Method, name)
                    })
                    .and_then(|location| location.container)
                    .is_some_and(|owner| owner.as_str() == declared_by)
                });
                if reaches {
                    out.push(adaptation.method_range);
                }
            }
        }

        out.sort_by_key(|range| range.start());
        out.dedup();
        out
    }
}

/// A name a `use` clause gives a trait's method: `greet` in `use Greets { hello as greet; }`.
///
/// Its own thing, the way an import alias is: not the trait's `hello` -- renaming that leaves the
/// alias standing -- and not a declaration anywhere either. A search or a rename asked at `greet`
/// is about the clause that wrote it and the calls that use it, and reading it as `hello` renamed
/// the trait's method, the class's own `hello` and the clause's left half, and left every call to
/// `greet` behind.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TraitAlias {
    /// The class-like whose `use` clause writes the alias.
    pub class: Name,
    pub alias: Name,
}

impl IndexView<'_> {
    /// The trait alias under the cursor: the alias in a `use` clause, or a call that reaches one.
    pub fn trait_alias_at(&self, offset: TextSize) -> Option<TraitAlias> {
        self.trait_alias_clause_at(offset)
            .or_else(|| self.trait_alias_reached_at(offset))
    }

    /// The alias a `use` clause writes, when the cursor is on it. Read off the file's own clauses,
    /// so it costs nothing anywhere else.
    pub fn trait_alias_clause_at(&self, offset: TextSize) -> Option<TraitAlias> {
        let hir = self.current()?.hir();
        for (_, class) in hir.classes() {
            for &id in hir.adaptation_list(class.trait_adaptations) {
                let phrost_hir::TraitAdaptationKind::Alias {
                    alias: Some(alias),
                    alias_range: Some(range),
                    ..
                } = &hir.trait_adaptation(id).kind
                else {
                    continue;
                };
                if range.contains_inclusive(offset) {
                    return Some(TraitAlias {
                        class: self.enclosing_class(offset)?,
                        alias: alias.clone(),
                    });
                }
            }
        }
        None
    }

    /// The alias a call under the cursor reaches, for a caller that has already resolved the call
    /// to `declared` -- the name its declaration has.
    ///
    /// The tell is free: a call reaching a method under a name that is not the declaration's is a
    /// call through an alias, and every other call is settled by comparing two words. Asked from a
    /// search, which a highlight runs at every cursor movement, so the resolution below is paid
    /// only where the answer is yes.
    pub fn trait_alias_called_at(&self, offset: TextSize, declared: &str) -> Option<TraitAlias> {
        let site = phrost_body::member_site_at(self.current()?.hir(), offset)?;
        if site.name.as_str().eq_ignore_ascii_case(declared) {
            return None;
        }
        self.trait_alias_reached_at(offset)
    }

    fn trait_alias_reached_at(&self, offset: TextSize) -> Option<TraitAlias> {
        let access = self
            .member_accesses_in(TextRange::empty(offset))
            .into_iter()
            .find(|access| {
                access.kind == MemberKind::Method && access.range.contains_inclusive(offset)
            })?;
        let crate::MemberAccessOutcome::Resolved { class } = &access.outcome else {
            return None;
        };
        Some(TraitAlias {
            class: self.alias_declarer(class.as_str(), access.name.as_str())?,
            alias: access.name,
        })
    }

    /// Every place the current file writes `alias.alias` meaning that alias: the clause that
    /// writes it, and each call reaching it -- through the class or anything inheriting it.
    pub fn trait_alias_occurrences(&self, alias: &TraitAlias) -> Vec<TextRange> {
        let Some(open) = self.current() else {
            return Vec::new();
        };
        let hir = open.hir();
        let mut found = Vec::new();
        for (_, class) in hir.classes() {
            for &id in hir.adaptation_list(class.trait_adaptations) {
                let phrost_hir::TraitAdaptationKind::Alias {
                    alias: Some(written),
                    alias_range: Some(range),
                    ..
                } = &hir.trait_adaptation(id).kind
                else {
                    continue;
                };
                if written.as_str().eq_ignore_ascii_case(alias.alias.as_str())
                    && self
                        .enclosing_class(range.start())
                        .is_some_and(|fqn| fqn == alias.class)
                {
                    found.push(*range);
                }
            }
        }
        found.extend(
            self.member_accesses_named(alias.alias.as_str(), MemberKind::Method)
                .into_iter()
                .filter(|access| match &access.outcome {
                    crate::MemberAccessOutcome::Resolved { class } => self
                        .alias_declarer(class.as_str(), access.name.as_str())
                        .is_some_and(|declarer| declarer == alias.class),
                    _ => false,
                })
                .map(|access| access.range),
        );
        found.sort_by_key(|range| range.start());
        found.dedup();
        found
    }

    /// The class-like whose `use` clause gave `class_fqn` the method `name` under that name, when
    /// the method it answers to is an alias.
    ///
    /// The member table says *that* it is one -- the name it is found under is not the name its
    /// declaration has -- and not *where*: a subclass inherits the alias with the trait still
    /// recorded as its declarer, which is right for goto and says nothing about the clause. So the
    /// clause is looked for, nearest class first, up the parents and the traits used.
    fn alias_declarer(&self, class_fqn: &str, name: &str) -> Option<Name> {
        let table = self.members(class_fqn);
        let member = table.find(MemberKind::Method, name)?;
        if member
            .name
            .as_str()
            .eq_ignore_ascii_case(member.row.name.as_str())
        {
            return None;
        }

        let mut queue = std::collections::VecDeque::from([(class_fqn.to_owned(), 0_u8)]);
        let mut seen = Vec::<String>::new();
        while let Some((fqn, depth)) = queue.pop_front() {
            if seen.iter().any(|done| done.eq_ignore_ascii_case(&fqn)) {
                continue;
            }
            seen.push(fqn.clone());
            let Some(class) = self.class_skeleton(&fqn) else {
                continue;
            };
            let writes_it = class.adaptations().iter().any(|adaptation| {
                matches!(
                    &adaptation.kind,
                    phrost_sema::TraitAdaptationKind::Alias { alias: Some(alias), .. }
                        if alias.as_str().eq_ignore_ascii_case(name)
                )
            });
            if writes_it {
                return Some(class.fqn().clone());
            }
            if depth < crate::resolve::HIERARCHY_DEPTH {
                for next in class.extends().iter().chain(class.uses()) {
                    queue.push_back((next.fqn.as_str().to_owned(), depth + 1));
                }
            }
        }
        None
    }

    /// A class below the one that writes the alias that declares a method of that name itself.
    ///
    /// The alias is a method of the class like any other, so a subclass's method of the same name
    /// overrides it -- PHP 8.5 calls `Child::greet` through `$this->greet()` written in the parent.
    /// A rename of the alias that left the override behind would quietly stop it overriding, which
    /// is why a rename asks this first. Walked a level at a time over the class table, the way
    /// [`Self::classes_reaching_constructor`] walks down.
    pub fn trait_alias_overridden_by(&self, alias: &TraitAlias) -> Option<Name> {
        let mut level = vec![alias.class.as_str().to_owned()];
        let mut seen = Vec::<String>::new();
        for _ in 0..crate::resolve::HIERARCHY_DEPTH {
            if level.is_empty() {
                break;
            }
            let below = self.skeleton.implementers_matching(|named| {
                level.iter().any(|class| class.eq_ignore_ascii_case(named))
            });
            let mut next = Vec::new();
            for (_, class) in below {
                if !class.extends().iter().any(|parent| {
                    level
                        .iter()
                        .any(|above| above.eq_ignore_ascii_case(parent.fqn.as_str()))
                }) {
                    continue;
                }
                if class.members().iter().any(|member| {
                    member.kind == MemberKind::Method
                        && member
                            .name
                            .as_str()
                            .eq_ignore_ascii_case(alias.alias.as_str())
                }) {
                    return Some(class.fqn().clone());
                }
                let fqn = class.fqn().as_str().to_owned();
                if !seen.iter().any(|done| done.eq_ignore_ascii_case(&fqn)) {
                    seen.push(fqn.clone());
                    next.push(fqn);
                }
            }
            level = next;
        }
        None
    }

    /// The skeleton of the class the index ranks first for `fqn`.
    fn class_skeleton(&self, fqn: &str) -> Option<&phrost_index::ClassSkeleton> {
        let location = self.class_location(fqn)?;
        self.skeleton
            .file(location.file)?
            .classes()
            .iter()
            .find(|class| class.fqn() == &location.fqn)
    }
}
