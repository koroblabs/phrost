//! A method `__call` answers for every name that begins with a prefix, handing back the receiver.
//!
//! Laravel's dynamic wheres: `whereEmail($email)` is `where('email', '=', $email)`, read off the
//! name by the query builder. No declaration exists, so without this the call answered `mixed` and
//! the chain after it -- `User::whereEmail($email)->first()` -- answered nothing. See
//! [`phrost_index::FluentPrefixRule`].
//!
//! The rule names the class whose `__call` does it, and every class that forwards to that one
//! answers too: through a `@mixin` the receiver is still the value the call was made on, and
//! through an implied mixin the call runs on the fresh instance the framework creates -- a model's
//! `Post::whereTitle()` is its builder's, so what comes back is `Builder<Post>` and not `Post`.

use phrost_model::Name;
use phrost_sema::MemberKind;

use crate::{resolve::Held, view::IndexView};

impl IndexView<'_> {
    /// What such a call hands back, when the name and the receiver are both the rule's.
    pub(crate) fn fluent_held(
        &self,
        class_fqn: &str,
        kind: MemberKind,
        name: &str,
        called_on: Option<&Name>,
        arguments: &[Held],
    ) -> Option<Held> {
        if kind != MemberKind::Method {
            return None;
        }
        let rule = self
            .skeleton
            .returns()
            .fluent_prefixes()
            .iter()
            .find(|rule| rule.spells(name))?;
        // Forwarded first: a model's table reaches the builder too, through the mixin the framework
        // implies, but the call runs on the builder it creates. One hop, which is all a framework
        // states and all that cannot loop.
        let (class, arguments) = match self.implied_forward_to(class_fqn, called_on, |mixin| {
            self.mixes_in(mixin, &rule.class)
        }) {
            Some((mixin, arguments)) => (mixin, arguments),
            None if self.mixes_in(class_fqn, &rule.class) => {
                (self.members_owner(class_fqn)?, arguments.to_vec())
            }
            None => return None,
        };
        Some(match arguments.is_empty() {
            true => Held::Class(class),
            false => Held::Applied {
                class,
                arguments: arguments.into_boxed_slice(),
            },
        })
    }

    /// The names worth offering on a receiver of `class_fqn`: the prefix and each word the model it
    /// is, or is a builder for, states -- `whereTitle` for the `title` a model lists. Any word is a
    /// call `__call` answers, and these are the ones the project wrote down.
    ///
    /// A builder says which model it is for only in its type arguments, so they are asked for
    /// lazily, and only where the receiver is one.
    pub(crate) fn fluent_methods_answering(
        &self,
        class_fqn: &str,
        held: impl FnOnce() -> Vec<Held>,
    ) -> Vec<crate::FluentCandidate> {
        let rules = self.skeleton.returns().fluent_prefixes();
        let mut held = Some(held);
        let mut out: Vec<crate::FluentCandidate> = Vec::new();
        for rule in rules {
            let model = if self
                .implied_forward_to(class_fqn, None, |mixin| self.mixes_in(mixin, &rule.class))
                .is_some()
            {
                self.members_owner(class_fqn)
            } else if self.mixes_in(class_fqn, &rule.class) {
                let Some(hir) = self.current().map(|open| open.hir()) else {
                    continue;
                };
                let receivers = held.take().map(|held| held()).unwrap_or_default();
                let [receiver] = receivers.as_slice() else {
                    continue;
                };
                self.arguments_of_held(hir, receiver)
                    .first()
                    .and_then(|argument| self.class_named_by(argument))
            } else {
                None
            };
            let Some(model) = model else {
                continue;
            };
            let words = self
                .stated_names(model.as_str())
                .into_iter()
                .map(|(word, _)| word)
                .chain(self.tagged_columns(model.as_str()));
            for word in words {
                let name = format!("{}{}", rule.prefix, super::accessors::studly(&word));
                if out.iter().any(|known| *known.name == *name) {
                    continue;
                }
                out.push(crate::FluentCandidate {
                    name: name.into(),
                    model: model.clone(),
                    word,
                });
            }
        }
        out
    }

    /// The properties a model declares in a `@property` tag that no method backs: a column written
    /// as `@property string $html` rather than in a migration or a cast. A tag a method of the same
    /// name backs is a relation -- `$page->chapter` is `chapter()` loaded -- and `whereChapter` would
    /// ask for a column that does not exist.
    fn tagged_columns(&self, model: &str) -> Vec<Box<str>> {
        let members = self.members(model);
        members
            .members()
            .iter()
            .filter(|member| member.row.kind == MemberKind::Property && member.row.from_docblock)
            .filter(|member| {
                members
                    .find(MemberKind::Method, member.name.as_str())
                    .is_none()
            })
            .map(|member| member.name.as_str().into())
            .collect()
    }

    /// Whether `class_fqn`'s members include `class`'s -- by descent or through a `@mixin`, which
    /// is what the table's contributors record.
    pub(super) fn mixes_in(&self, class_fqn: &str, class: &str) -> bool {
        self.members(class_fqn)
            .contributors()
            .iter()
            .any(|contributor| contributor.as_str().eq_ignore_ascii_case(class))
    }
}
