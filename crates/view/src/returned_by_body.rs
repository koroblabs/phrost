//! The type of a declaration that states none and hands back a chain.
//!
//! `SqlBase::select()` in Drupal core declares no return type, carries no `@return`, and its whole
//! body is `return $this->getDatabase()->select($table, $alias, $options);`. Everything written
//! about it says nothing and the one thing that says anything is the expression it returns -- so a
//! reader following `$this->select('t')->fields(...)` in a migrate source got the first hop and
//! nothing after it.
//!
//! The chain itself is read where the body is, at ingest, and stored as names -- see
//! `ReturnedRow`. What is left here is the half only a project-wide index can do: walking those
//! names into the classes they land in.
//!
//! **`$this` means the class the *caller* reached it through**, not the one that declared it. A
//! subclass calling an inherited `create()` that returns `$this->build()` gets the subclass's
//! `build`, which is what PHP does and what late static binding is for. That is why the stored root
//! is `None` for a receiver rather than the declaring class's name.

use phrost_model::Name;
use phrost_sema::MemberKind;

use crate::{resolve::Held, view::IndexView};

/// How deep one chain may lead into another before the walk gives up.
///
/// A wrapper returning a wrapper is ordinary -- that is what makes this worth having -- and a cycle
/// is something a checkout is entitled to contain. Three is past every chain measured on Drupal
/// core, whose longest is `$this->getSession()->getPage()->getText()`.
const FOLLOW_DEPTH: u8 = 3;

impl IndexView<'_> {
    /// What the body of `name` on `class_fqn` hands back, when nothing written says.
    pub(crate) fn returned_by_body(
        &self,
        class_fqn: &str,
        kind: MemberKind,
        name: &str,
        called_on: Option<Name>,
    ) -> Option<Held> {
        // Only a method. A property has no body, and a constant's value is already its type.
        if kind != MemberKind::Method {
            return None;
        }
        let depth = self.following_returns.get();
        if depth >= FOLLOW_DEPTH {
            return None;
        }

        let declared = self.member_location(class_fqn, MemberKind::Method, name)?;
        let row = self.skeleton.declared_return_path(
            declared.file,
            declared.container.as_ref().map(Name::as_str),
            name,
        )?;

        // The root: a written class, or -- for `$this` -- whichever class the call was made on.
        // `members_owner` rather than the name as written, so the two spellings of one class do not
        // become two answers.
        let mut class = match &row.root {
            Some(root) => root.clone(),
            None => match called_on {
                Some(called) => called,
                None => self.members_owner(class_fqn)?,
            },
        };

        self.following_returns.set(depth + 1);
        let held = self.walk_returned(&mut class, &row.steps, row.key.as_ref());
        self.following_returns.set(depth);
        held
    }

    /// [`Self::walk_returned`] for a chain stored somewhere other than a declaration's row -- a macro's
    /// closure -- under the same depth guard, since a chain may lead back into itself.
    pub(crate) fn walk_returned_guarded(&self, class: &mut Name, steps: &[Name]) -> Option<Held> {
        let depth = self.following_returns.get();
        if depth >= FOLLOW_DEPTH {
            return None;
        }
        self.following_returns.set(depth + 1);
        let held = self.walk_returned(class, steps, None);
        self.following_returns.set(depth);
        held
    }

    /// Walks the member names of a stored chain, class by class.
    ///
    /// A step is a method or a property and the stored name does not say which -- it does not need
    /// to, because one class cannot declare both under one name. The method is tried first because
    /// that is what a wrapper chain is made of.
    fn walk_returned(
        &self,
        class: &mut Name,
        steps: &[Name],
        key: Option<&phrost_index::ReturnedKey>,
    ) -> Option<Held> {
        let mut held = None;
        // What the previous step's value was given, handed to the next: `allRevisions()` is a
        // `HasMany<PageRevision, Page>` and its `where()` returns `static`, which is the relation
        // with those arguments only if they travel with it.
        let mut arguments: Vec<Held> = Vec::new();
        for (at, step) in steps.iter().enumerate() {
            // The key first, and only on the first step -- which is where a catalog contract puts
            // it. `return \Drupal::service('utilities.manager');` names `Drupal::service` in the
            // chain and the class nowhere in it: what the declaration hands back is whatever the
            // container builds for that id, and the same reader a written call goes through
            // answers it. Asked before the member lookup because the member's own declaration is
            // the thing the contract exists to improve on -- `service()` returns `object`.
            //
            // A class written as `Foo::class` is the other contract a first argument settles: a
            // factory's `class-string<T>`, which is how a test helper builds the double it returns.
            let keyed = key.filter(|_| at == 0).and_then(|key| match key {
                phrost_index::ReturnedKey::Text(text) => {
                    self.returned_from_argument(class.as_str(), step.as_str(), Some(text.as_str()))
                }
                phrost_index::ReturnedKey::Class(named) => {
                    self.returned_class_for(class.as_str(), step.as_str(), named.as_str())
                }
            });
            let next = keyed
                .or_else(|| {
                    self.member_held_on(
                        class.as_str(),
                        MemberKind::Method,
                        step.as_str(),
                        None,
                        &arguments,
                    )
                })
                .or_else(|| {
                    self.member_held_on(
                        class.as_str(),
                        MemberKind::Property,
                        step.as_str(),
                        None,
                        &arguments,
                    )
                })?;
            if at + 1 == steps.len() {
                held = Some(next);
                break;
            }
            *class = self.class_of_held_name(&next)?;
            arguments = self.arguments_carried(&next);
        }
        held
    }

    /// The arguments a held value was given, without a tree to read a written type against --
    /// everything here came out of the index already resolved.
    fn arguments_carried(&self, held: &Held) -> Vec<Held> {
        match held {
            Held::Applied { arguments, .. } => arguments.to_vec(),
            Held::Nullable(inner) => self.arguments_carried(inner),
            Held::Stored { ty, scope } => self
                .stored_arguments(*ty)
                .into_iter()
                .map(|ty| Held::Stored {
                    ty,
                    scope: scope.clone(),
                })
                .collect(),
            _ => Vec::new(),
        }
    }

    /// The class a held type names, without a position to resolve one against.
    ///
    /// Apart from `class_of_held`, which takes an offset and a tree because it also reads what a
    /// *written* type meant in the file it was written in. Everything reachable here is already
    /// resolved -- the chain came out of the index -- so the four shapes below are all of it.
    fn class_of_held_name(&self, held: &Held) -> Option<Name> {
        match held {
            Held::Class(name) | Held::Applied { class: name, .. } => Some(name.clone()),
            Held::Nullable(inner) => self.class_of_held_name(inner),
            Held::Stored { ty, scope } => self.class_of_stored(*ty, scope.as_ref()),
            _ => None,
        }
    }
}
