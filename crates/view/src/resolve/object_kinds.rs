//! Whether an object of a class is callable, iterable or usable as a string.
//!
//! The three kinds an object answers by what its class declares rather than by being an object:
//! `Traversable` for `iterable`, `__invoke` for `callable`, `__toString` for a weak-mode `string`.
//! Two readers ask. The compatibility judgement asks whether a value fits a declaration, and a
//! kind guard asks whether `is_callable($x)` can hold of an operand -- and when the two answered
//! separately, `HandlerDescriptor|callable` inside `if (is_callable($h))` kept a class the judgement
//! then refused as a callable.
//!
//! Three answers, because a class that lacks the marker today is one a subclass may still supply.
//! Only a class nothing can extend -- `final`, or an enum -- answers no, and not one the runtime
//! declares: an extension class can convert itself without saying so in PHP. `GMP` is final, has
//! no `__toString`, and is a perfectly good `string` argument, because its cast handler is C. A stub
//! describes the methods and not the handlers.

use phrost_model::BuiltinTy;
use phrost_sema::MemberKind;

use crate::{
    resolve::{Answers, HIERARCHY_DEPTH},
    view::IndexView,
};

/// PHP's own marker for something a `foreach` accepts beside an array.
const TRAVERSABLE: &str = "Traversable";
/// PHP's own marker for an object usable where a string is wanted, in weak mode.
const STRINGABLE: &str = "Stringable";
/// PHP's own marker for an object `[...]` can be read off.
const ARRAY_ACCESS: &str = "ArrayAccess";

impl IndexView<'_> {
    /// Whether an object of `class` is of `kind`, for the three kinds a class decides.
    ///
    /// `Maybe` for every other kind and for a class the index does not hold, which is the answer
    /// that rules nothing out.
    pub(crate) fn class_answers_kind(&self, class: &str, kind: BuiltinTy) -> Answers {
        let (marker, method) = match kind {
            // `iterable` is `array|Traversable`, and an object half of that is the interface.
            BuiltinTy::Iterable => (TRAVERSABLE, None),
            BuiltinTy::String => (STRINGABLE, Some("__toString")),
            // A `Closure` is the usual one, but any object declaring `__invoke` is callable.
            BuiltinTy::Callable => ("Closure", Some("__invoke")),
            _ => return Answers::Maybe,
        };
        let Some(header) = self.class_header(class) else {
            return Answers::Maybe;
        };
        // Both the marker and the method, because PHP does not require the interface: `Stringable`
        // is implied for any class with a `__toString`, and a project predating it often declares
        // neither.
        if class.eq_ignore_ascii_case(marker) || self.descends_from(class, marker, HIERARCHY_DEPTH)
        {
            return Answers::Yes;
        }
        // The flattened table, so an inherited one counts.
        if method.is_some_and(|method| {
            self.members(class)
                .find(MemberKind::Method, method)
                .is_some()
        }) {
            return Answers::Yes;
        }
        match header.is_final && !self.declared_by_the_runtime(class) {
            true => Answers::No,
            false => Answers::Maybe,
        }
    }

    /// Whether `$object[$key]` can be read off an object of `class`: whether it is an `ArrayAccess`.
    ///
    /// The same three answers for the same reason. A runtime class is `Maybe` here too, and that
    /// is load-bearing: `SimpleXMLElement` is not an `ArrayAccess` and `$xml['attr']` reads an
    /// attribute, because its dimension handler is C.
    pub(crate) fn class_takes_a_subscript(&self, class: &str) -> Answers {
        let Some(header) = self.class_header(class) else {
            return Answers::Maybe;
        };
        if class.eq_ignore_ascii_case(ARRAY_ACCESS)
            || self.descends_from(class, ARRAY_ACCESS, HIERARCHY_DEPTH)
        {
            return Answers::Yes;
        }
        match header.is_final && !self.declared_by_the_runtime(class) {
            true => Answers::No,
            false => Answers::Maybe,
        }
    }

    /// Whether the class is one of the runtime's own, declared by a stub rather than by PHP source.
    fn declared_by_the_runtime(&self, class: &str) -> bool {
        self.class_location(class).is_some_and(|location| {
            self.skeleton
                .file(location.file)
                .is_some_and(|file| file.origin() == phrost_model::SourceOrigin::Stub)
        })
    }
}
