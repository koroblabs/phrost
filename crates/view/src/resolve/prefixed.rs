//! A method called without the prefix it is declared under.
//!
//! Eloquent's local scopes are the case: `scopePublished($query)` is called `Post::published()` and
//! `$query->published()`, and what comes back is the builder the call was made on, or a fresh one
//! for the model. Nothing declares `published`; the model's and the builder's `__call` find the
//! prefixed method. A framework states the rule and the project's own method is what goto lands on.
//! Asked after the member table, as every accessor is.
//!
//! The same rule says one more thing a model can write: a static `@method` returning the builder.
//! `SoftDeletes` writes `@method static Builder<static> withTrashed()`, and the model's
//! `__callStatic` reaches it by making a builder and calling the name on that -- so a builder for
//! the model answers it too, and hands itself back. The tag is the only statement of it: Laravel
//! registers the method as a macro the builder holds at runtime.

use phrost_index::PrefixedMethodRule;
use phrost_model::Name;
use phrost_sema::MemberKind;

use crate::{
    location::SymbolLocation,
    resolve::{HIERARCHY_DEPTH, Held},
    view::IndexView,
};

impl IndexView<'_> {
    /// What such a call hands back: a builder for the model the method is declared on -- or, made
    /// on a class forwarding to the builder, that class itself. `Relation::__call` hands back the
    /// relation where the builder handed back itself, which is what every other builder method
    /// reached through the relation's `@mixin` already answers.
    pub(crate) fn prefixed_method_held(
        &self,
        class_fqn: &str,
        kind: MemberKind,
        name: &str,
        arguments: &[Held],
    ) -> Option<Held> {
        let target = self.prefixed_target(class_fqn, kind, name, arguments)?;
        if target.forwarded {
            let class = self.class_location(class_fqn)?.fqn;
            return Some(match arguments.is_empty() {
                true => Held::Class(class),
                false => Held::Applied {
                    class,
                    arguments: arguments.to_vec().into_boxed_slice(),
                },
            });
        }
        let builder = self.class_location(&target.rule.builder)?.fqn;
        Some(Held::Applied {
            class: builder,
            arguments: Box::new([Held::Class(target.model)]),
        })
    }

    /// What a call to a *marked* method hands back when it goes through the magic method: Laravel
    /// 12's `#[Scope] protected function popular($query)`, called `Post::popular()` from outside.
    ///
    /// Asked only where PHP runs the magic method rather than the declaration -- inside the class
    /// `$this->popular($query)` is a plain call and answers what it declares.
    pub(crate) fn marked_method_held(&self, class_fqn: &str, name: &str) -> Option<Held> {
        let rules = self.skeleton.returns().prefixed_methods();
        let rule = rules.iter().find(|rule| rule.marked)?;
        let table = self.members(class_fqn);
        let member = table.find(MemberKind::Method, name)?;
        if !member.row.modifiers.is_marked() || !self.is_at_or_below(class_fqn, &rule.class) {
            return None;
        }
        let model = self.class_location(class_fqn)?.fqn;
        let builder = self.class_location(&rule.builder)?.fqn;
        Some(Held::Applied {
            class: builder,
            arguments: Box::new([Held::Class(model)]),
        })
    }

    /// The model a builder-side call is answered by, for a cursor on it.
    ///
    /// A location is asked of a class and a name, without the receiver's arguments, so
    /// `Post::query()->published()` could not say which model its builder is for and goto and hover
    /// answered nothing. The receiver is in hand where the cursor's target is built, so the model
    /// is named there instead -- and from then on the call is the model-side one, which
    /// [`Self::prefixed_method_location`] already places.
    pub(crate) fn prefixed_owner(
        &self,
        hir: &phrost_hir::FileHir,
        builder: &str,
        held: &Held,
        name: &str,
    ) -> Option<Name> {
        if self.skeleton.returns().prefixed_methods().is_empty() {
            return None;
        }
        let arguments = self.arguments_of_held(hir, held);
        let model = self
            .prefixed_target(builder, MemberKind::Method, name, &arguments)?
            .model;
        (!model.as_str().eq_ignore_ascii_case(builder)).then_some(model)
    }

    /// Where such a call is declared, which is the prefixed method on the model.
    ///
    /// The model side only: a location is asked without the receiver's arguments, so a builder
    /// cannot say which model it is for -- see [`Self::prefixed_owner`], which names the model
    /// first.
    pub(crate) fn prefixed_method_location(
        &self,
        class_fqn: &str,
        kind: MemberKind,
        name: &str,
    ) -> Option<SymbolLocation> {
        let target = self.prefixed_target(class_fqn, kind, name, &[])?;
        self.member_location(target.model.as_str(), MemberKind::Method, &target.declared)
    }

    /// The model whose prefixed method answers `name` on this receiver, and the rule that says so.
    ///
    /// Cheapest first: no rule asks nothing, and a name with no prefixed method on the model ends at
    /// one lookup before any hierarchy is climbed.
    fn prefixed_target(
        &self,
        class_fqn: &str,
        kind: MemberKind,
        name: &str,
        arguments: &[Held],
    ) -> Option<PrefixedTarget<'_>> {
        if kind != MemberKind::Method || name.is_empty() {
            return None;
        }
        let rules = self.skeleton.returns().prefixed_methods();
        rules.iter().find_map(|rule| {
            let declared = declared_name(&rule.prefix, name);
            // Called on the model itself, statically or not.
            if self
                .members(class_fqn)
                .find(MemberKind::Method, &declared)
                .is_some()
                && self.is_at_or_below(class_fqn, &rule.class)
            {
                return Some(PrefixedTarget {
                    model: self.class_location(class_fqn)?.fqn,
                    rule,
                    declared,
                    forwarded: false,
                });
            }
            // Called on a builder for one, whose model is its first argument -- or on a class that
            // forwards to one through a written `@mixin`, `Relation`'s `Builder<TRelatedModel>`,
            // whose model is what the receiver supplies for it. By the prefixed name, by its own
            // where the rule's marker is on it, or by a tag.
            let forwarded = !self.is_at_or_below(class_fqn, &rule.builder);
            let model = match forwarded {
                false => self.class_named_by(arguments.first()?)?,
                true => self.model_through_mixin(class_fqn, &rule.builder, arguments)?,
            };
            let table = self.members(model.as_str());
            let declared = match table.find(MemberKind::Method, &declared) {
                Some(_) => declared,
                None => {
                    let own = table.find(MemberKind::Method, name)?;
                    let answered = (rule.marked && own.row.modifiers.is_marked())
                        || self.tags_the_builder(own, &rule.builder);
                    answered.then(|| name.to_owned())?
                }
            };
            self.is_at_or_below(model.as_str(), &rule.class)
                .then_some(PrefixedTarget {
                    model,
                    rule,
                    declared,
                    forwarded,
                })
        })
    }

    /// The model a class forwarding to `builder` through a written `@mixin` is for: what the
    /// receiver supplies for the template the tag fills the builder's with.
    ///
    /// Gated on the builder being among the classes the receiver's table was built from, which a
    /// mixin puts there -- a lookup in a list the table already holds, where the walk below is not.
    fn model_through_mixin(
        &self,
        class_fqn: &str,
        builder: &str,
        arguments: &[Held],
    ) -> Option<Name> {
        if arguments.is_empty() || !self.mixes_in(class_fqn, builder) {
            return None;
        }
        let through = self.type_argument(
            class_fqn,
            builder,
            0,
            crate::generics::CHASE,
            crate::generics::Landing::Template,
        )?;
        let supplied = self.supplied_argument_at(class_fqn, class_fqn, through, arguments)?;
        self.class_named_by(&supplied)
    }

    /// Every prefixed method a receiver of `class_fqn` answers: the model's own, or the builder's
    /// model's, which only the receiver's type arguments say -- so they are asked for lazily, and
    /// only where the class is a builder or forwards to one.
    pub(crate) fn prefixed_methods_answering(
        &self,
        class_fqn: &str,
        held: impl FnOnce() -> Vec<Held>,
    ) -> Vec<crate::PrefixedCandidate> {
        let rules = self.skeleton.returns().prefixed_methods();
        let mut held = Some(held);
        let mut out = Vec::new();
        for rule in rules {
            let model = if self.is_at_or_below(class_fqn, &rule.class) {
                self.class_location(class_fqn).map(|location| location.fqn)
            } else {
                let on_builder = self.is_at_or_below(class_fqn, &rule.builder);
                if !on_builder && !self.mixes_in(class_fqn, &rule.builder) {
                    continue;
                }
                let Some(hir) = self.current().map(|open| open.hir()) else {
                    continue;
                };
                let receivers = held.take().map(|held| held()).unwrap_or_default();
                let [receiver] = receivers.as_slice() else {
                    continue;
                };
                let arguments = self.arguments_of_held(hir, receiver);
                match on_builder {
                    true => arguments
                        .first()
                        .and_then(|argument| self.class_named_by(argument)),
                    false => self.model_through_mixin(class_fqn, &rule.builder, &arguments),
                }
            };
            let Some(model) =
                model.filter(|model| self.is_at_or_below(model.as_str(), &rule.class))
            else {
                continue;
            };
            for member in self.members(model.as_str()).members() {
                if member.row.kind != MemberKind::Method {
                    continue;
                }
                // A scope's first parameter is the builder the call passes in; a tag's are all the
                // caller's.
                let (called, passed_in) = match called_name(&rule.prefix, member.name.as_str()) {
                    Some(called) => (called, 1),
                    None if rule.marked && member.row.modifiers.is_marked() => {
                        (member.name.as_str().to_owned(), 1)
                    }
                    None if self.tags_the_builder(member, &rule.builder) => {
                        (member.name.as_str().to_owned(), 0)
                    }
                    None => continue,
                };
                if out
                    .iter()
                    .any(|known: &crate::PrefixedCandidate| *known.name == *called)
                {
                    continue;
                }
                out.push(crate::PrefixedCandidate {
                    name: called.into(),
                    declared: member.name.clone(),
                    model: model.clone(),
                    takes_arguments: member.row.params.len() > passed_in,
                    parameters: member
                        .row
                        .params
                        .iter()
                        .skip(passed_in)
                        .filter(|param| !param.has_default && !param.is_variadic)
                        .map(|param| param.name.clone())
                        .collect(),
                });
            }
        }
        out
    }

    /// Whether a member is a static `@method` tag returning the builder -- see the module note.
    ///
    /// Only one returning the builder: that is what makes it a call the builder answers. A tag
    /// returning anything else may be the model's own instance method reached statically --
    /// `SoftDeletes` tags `restore()`, which is a real method of the model and a different macro of
    /// the builder.
    fn tags_the_builder(&self, member: &phrost_sema::ResolvedMember, builder: &str) -> bool {
        member.row.from_docblock
            && member.row.modifiers.is_static()
            && member
                .row
                .effective_ty(self.skeleton.types())
                .and_then(|ty| self.class_of_stored(ty, None))
                .is_some_and(|returned| self.is_at_or_below(returned.as_str(), builder))
    }

    pub(super) fn class_named_by(&self, held: &Held) -> Option<Name> {
        match held {
            Held::Class(class) | Held::Applied { class, .. } => Some(class.clone()),
            Held::Stored { ty, scope } => self.class_of_stored(*ty, scope.as_ref()),
            _ => None,
        }
    }

    fn is_at_or_below(&self, class_fqn: &str, ancestor: &str) -> bool {
        class_fqn.eq_ignore_ascii_case(ancestor)
            || self.descends_from(class_fqn, ancestor, HIERARCHY_DEPTH)
    }
}

/// `scopePublished` as the name a call writes, `published` -- or `None` for a method the prefix
/// does not begin a word of: `scope` itself, and `scoped`.
fn called_name(prefix: &str, declared: &str) -> Option<String> {
    let rest = declared.get(prefix.len()..)?;
    if !declared[..prefix.len()].eq_ignore_ascii_case(prefix) {
        return None;
    }
    let mut characters = rest.chars();
    let first = characters.next().filter(char::is_ascii_uppercase)?;
    Some(format!(
        "{}{}",
        first.to_ascii_lowercase(),
        characters.as_str()
    ))
}

/// `scope` and `published` as the name a declaration writes: `scopePublished`.
fn declared_name(prefix: &str, called: &str) -> String {
    let mut characters = called.chars();
    let first = characters.next().map(|first| first.to_ascii_uppercase());
    format!(
        "{prefix}{}{}",
        first.map(String::from).unwrap_or_default(),
        characters.as_str()
    )
}

/// Who answers a call no declaration names, and under which name it is declared.
struct PrefixedTarget<'r> {
    model: Name,
    rule: &'r PrefixedMethodRule,
    /// `scopePublished` for a call written `published()`; the name itself for a marked method or a
    /// tag.
    declared: String,
    /// Made on a class forwarding to the builder rather than on the builder or the model.
    forwarded: bool,
}
