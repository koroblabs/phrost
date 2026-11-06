//! What a filtering call leaves in the array it was handed.
//!
//! `array_filter($rows)` with no callback drops every falsy element, so a `list<?string>` comes
//! back holding no nulls at all. The declaration cannot say it: `@return array<K, V>` is a true
//! statement about the two-argument form and the only one the type language can make, and a
//! docblock has no way to write "V, minus the values PHP calls falsy".
//!
//! So `array_values(array_unique(array_filter(array_map($cb, $types))))` -- which is how PHP spells
//! *map and drop the misses* -- answered `?string[]` where the author had declared `string[]`, and
//! the return check reported it. A false positive on an idiom written 70 times in Symfony, 89 in
//! WordPress and 38 in Laravel.
//!
//! # Every falsy value it can name, and only where the keys were already integers
//!
//! Null and `false` are taken off, an empty shape with them, and a string left behind is a
//! `non-falsy-string` -- the same reading `if ($x)` gives, through the same call, since the filter
//! keeps exactly what that test would. `0` and `[]` go too and are not taken off, because the type
//! language has no operand to remove for them.
//!
//! The key rule is the part worth stating. `array_filter` *preserves* keys, and the only shape this
//! layer has for "an array of one thing" is [`Held::ListOf`], which answers `int` when asked what
//! iterating it yields. Rebuilding an `array<string, ?Foo>` as one would turn a correct string key
//! into a wrong int key -- so the transform is refused unless the container already said `int`,
//! which every `array_map` over a list does. A miss rather than a wrong answer, which is the same
//! direction `literal_returns` takes beside it.

use phrost_hir::FileHir;
use phrost_model::{BuiltinTy, TextSize};

use crate::{resolve::Held, view::IndexView};

/// The functions that drop falsy elements when handed nothing but the array.
///
/// One entry, and it is a list rather than a comparison so that the next one -- PHP grows these --
/// is a line here instead of a second rule somewhere else.
const DROPS_FALSY_ELEMENTS: &[&str] = &["array_filter"];

impl IndexView<'_> {
    /// `held` with the falsy values taken off its elements, when the call was one that removes them.
    ///
    /// The same value back for every other call, which is nearly all of them: the name is compared
    /// first, so a project that never writes one of these pays one string comparison per free
    /// function call and nothing else.
    pub(crate) fn without_filtered_elements(
        &self,
        offset: TextSize,
        hir: &FileHir,
        callee: &str,
        call: phrost_body::CallSource,
        held: Held,
    ) -> Held {
        if !DROPS_FALSY_ELEMENTS
            .iter()
            .any(|name| callee.eq_ignore_ascii_case(name))
        {
            return held;
        }
        // The callback is what makes the declaration honest: with one, PHP keeps whatever the
        // callback said yes to, and nothing here knows what that was.
        if arguments_written(hir, call) != Some(1) {
            return held;
        }
        // See the module note: `ListOf` is the only container this can build and it says `int`.
        // Through `kind_of_held`, which is the reader that knows a builtin's three spellings: a
        // `list<?Row>` written in this file answers its key as a `Held::Written` pointing at the
        // docblock's own `int`, and comparing against `Held::Builtin` matched none of them.
        let key = self.iteration_key_of_held(offset, hir, &held);
        if key.and_then(|key| self.kind_of_held(hir, &key)) != Some(BuiltinTy::Int) {
            return held;
        }
        let Some(element) = self.element_of_held(offset, hir, &held) else {
            return held;
        };
        match self.without_sentinels(
            offset,
            hir,
            element.clone(),
            phrost_body::Refused::EveryFalsy,
        ) {
            Some(kept) if kept != element => Held::ListOf(Box::new(kept)),
            // Nothing falsy to take off, so the declaration stands as it was written.
            Some(_) => held,
            // Every element was null, so the filter leaves nothing -- which is a claim about the
            // array's length rather than about what it holds, and not one this can make.
            None => held,
        }
    }
}

/// How many arguments the call wrote, or nothing when the reader could not tell.
fn arguments_written(hir: &FileHir, call: phrost_body::CallSource) -> Option<usize> {
    let phrost_hir::body::ExprData::FunctionCall { args, .. } = hir.expr(call.expr()?) else {
        return None;
    };
    Some(hir.expr_list(*args).len())
}
