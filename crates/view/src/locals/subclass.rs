//! A value narrowed to a subclass, with the type arguments it already had carried down to it.
//!
//! `@param Enumerable<int, User> $items` and `if ($items instanceof Collection)` is a
//! `Collection<int, User>`: `Collection` writes `@implements Enumerable<TKey, TValue>`, so each of
//! its templates is the argument the interface was given at the same place. Narrowed to the bare
//! class, `$items->first()` inside the guard read `TValue` with nothing bound -- the guard took away
//! what the declaration had said.

use phrost_hir::FileHir;
use phrost_model::Name;

use crate::{
    generics::{CHASE, Landing},
    resolve::Held,
    view::IndexView,
};

impl IndexView<'_> {
    /// `class`, which descends from `ancestor`, with each of its templates bound to what `held` --
    /// a value of `ancestor` -- gave `ancestor` where `class` passes that template up.
    ///
    /// A template `class` passes up as something other than itself (`@extends Base<list<T>>`), or
    /// does not pass up at all, stays unbound, which is what it was before.
    pub(crate) fn subclass_keeping_arguments(
        &self,
        hir: &FileHir,
        held: &Held,
        ancestor: &Name,
        class: &Name,
    ) -> Held {
        let given = self.arguments_of_held(hir, held);
        let templates = self.class_templates(class.as_str()).unwrap_or_default();
        if given.is_empty() || templates.is_empty() {
            return Held::Class(class.clone());
        }
        // `Enumerable<User>` gives a two-template interface one argument, and by the convention every
        // container follows it is the value -- the last of them. Any other shortfall lines up nowhere.
        let declared = self
            .class_templates(ancestor.as_str())
            .map_or(0, <[_]>::len);
        let offset = match (given.len(), declared) {
            (given, declared) if given == declared => 0,
            (1, declared) if declared > 1 => declared - 1,
            _ => return Held::Class(class.clone()),
        };
        let mut filled = vec![None; templates.len()];
        for (index, argument) in given.into_iter().enumerate() {
            let Some(passed) = self.type_argument(
                class.as_str(),
                ancestor.as_str(),
                index + offset,
                CHASE,
                Landing::Template,
            ) else {
                continue;
            };
            if let Some(at) = self.template_position(class.as_str(), passed)
                && filled[at].is_none()
            {
                filled[at] = Some(argument);
            }
        }
        self.applied_instance_of(class.clone(), filled)
            .unwrap_or_else(|| Held::Class(class.clone()))
    }
}
