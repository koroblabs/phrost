//! An element read from a value that holds none.
//!
//! `$n['key']` where `$n` is an `int`. PHP 8 warns -- `Trying to access array offset on int` -- and
//! evaluates it as null, so like a read before assignment it is a branch that runs and is wrong
//! rather than one that stops.
//!
//! The sibling of [`NonObjectReceiver`](super::receiver::NonObjectReceiver), and the same division
//! of labour: that one asks whether there is a class at all, this asks whether there are elements at
//! all, and neither is the undefined-member check, which asks what a table holds. A `string` and an
//! `ArrayAccess` implementer are both subscriptable and neither is reported -- see the projection
//! for the whole list.
//!
//! And beside it the null half. `$row['vin']` where `$row` is a `?array` was reported by nobody:
//! this file's rule stepped aside for a possible null on the ground that it "belongs to the null
//! check", and that check walks member accesses -- so `$row->vin()` answered and `$row['vin']` did
//! not. One mistake, two spellings, and only one of them had a rule.

use phrost_view::IndexView;

use super::{Check, Diagnostic, Severity};

pub(crate) const NON_ARRAY_SUBSCRIPT: &str = "non-array-subscript";
pub(crate) const NULL_SUBSCRIPT: &str = "null-subscript";
pub(crate) const POSSIBLY_NULL_SUBSCRIPT: &str = "possibly-null-subscript";
pub(crate) const FALSE_SUBSCRIPT: &str = "false-subscript";
pub(crate) const POSSIBLY_FALSE_SUBSCRIPT: &str = "possibly-false-subscript";

pub(crate) const MISSPELT_KEY: &str = "misspelt-key";
pub(crate) const POSSIBLY_ABSENT_KEY: &str = "possibly-absent-key";
pub(crate) const IMPOSSIBLE_KEY: &str = "impossible-key";

pub(super) struct MisspeltShapeKey;

impl Check for MisspeltShapeKey {
    fn run(&self, view: &IndexView<'_>, out: &mut Vec<Diagnostic>) {
        for found in view.misspelt_keys() {
            out.push(Diagnostic {
                range: found.range,
                // A near miss is evidence rather than proof -- the shape may simply not list the
                // key -- so it is a warning where a kind or a null is an error.
                severity: Severity::Warning,
                code: MISSPELT_KEY,
                message: format!(
                    "'{}' is not declared; '{}' is one edit away",
                    found.key, found.nearest
                ),
                unnecessary: false,
                deprecated: false,
            });
        }
    }
}

/// A key the container itself says may not be there.
///
/// PHPStan reports the *read* and phrost reports the *use*, and for a key that is only ever passed
/// on to something taking `mixed` there is no use to report -- twenty-one findings on one real
/// project, every one of them a key the docblock says may be absent. So this is the read, and only
/// where the declaration states the absence: `array{}` as one arm of the union, or a field written
/// `key?:`. A populated shape that simply does not list a key still says nothing, which is the
/// position `misspelt-key` answers under.
///
/// Asked for rather than always on. A project that writes `array{...}|array{}` and reads the keys
/// without a guard has done nothing PHP objects to -- the read is null and the code may want that
/// -- so this is a precision a project opts into, beside `doc-missing-throws` and the rest.
pub(super) struct PossiblyAbsentKey;

impl Check for PossiblyAbsentKey {
    fn asked_for(&self) -> Option<&'static str> {
        Some(POSSIBLY_ABSENT_KEY)
    }

    fn run(&self, view: &IndexView<'_>, out: &mut Vec<Diagnostic>) {
        for found in view.absent_keys() {
            out.push(Diagnostic {
                range: found.range,
                // A warning, like every other subscript rule: PHP reads an absent key as null and
                // carries on, so this is a branch that runs and is wrong rather than one that stops.
                severity: Severity::Warning,
                code: POSSIBLY_ABSENT_KEY,
                message: match found.absence {
                    phrost_view::Absence::EveryKey => format!(
                        "'{}' may not be there; array{{}} is one of the shapes declared",
                        found.key
                    ),
                    phrost_view::Absence::ThisKey => {
                        format!("'{}' may not be there; it is declared optional", found.key)
                    }
                },
                unnecessary: false,
                deprecated: false,
            });
        }
    }
}

pub(super) struct NonArraySubscript;

impl Check for NonArraySubscript {
    fn run(&self, view: &IndexView<'_>, out: &mut Vec<Diagnostic>) {
        for subscript in view.non_array_subscripts() {
            out.push(Diagnostic {
                range: subscript.range,
                // A warning, not the error its member sibling reports: PHP stops on a method call
                // through a string and carries on with null here, and the severity is the
                // difference between "this cannot run" and "this runs and is wrong".
                severity: Severity::Warning,
                code: NON_ARRAY_SUBSCRIPT,
                message: match &subscript.key {
                    Some(key) => format!(
                        "Cannot read element '{}' of {}",
                        key,
                        subscript.kind.as_str()
                    ),
                    None => format!("Cannot read an element of {}", subscript.kind.as_str()),
                },
                unnecessary: false,
                deprecated: false,
            });
        }
    }
}

/// An element read from a value that may not be there.
///
/// The pair `null-receiver` and `possibly-null-receiver` are to `->`, written for `[...]` -- which
/// is what the rule above always said this case belonged to and what did not exist.
///
/// Both halves are warnings, unlike the receiver pair. PHP fatals on a method call through null and
/// merely warns here -- `Trying to access array offset on value of type null` -- then evaluates the
/// read as null, so even a certain null is a branch that runs and is wrong rather than one that
/// stops. That is the same reading the rule above this one takes of a subscript on an `int`.
pub(super) struct NullableSubscript;

impl Check for NullableSubscript {
    fn run(&self, view: &IndexView<'_>, out: &mut Vec<Diagnostic>) {
        for subscript in view.null_subscripts() {
            let is_null = subscript.sentinel == phrost_model::BuiltinTy::Null;
            let held = subscript.rendered.as_deref().unwrap_or(match is_null {
                true => "a value that may be null",
                false => "a value that may be false",
            });
            out.push(Diagnostic {
                range: subscript.range,
                severity: Severity::Warning,
                code: match (is_null, subscript.certain) {
                    (true, true) => NULL_SUBSCRIPT,
                    (true, false) => POSSIBLY_NULL_SUBSCRIPT,
                    (false, true) => FALSE_SUBSCRIPT,
                    (false, false) => POSSIBLY_FALSE_SUBSCRIPT,
                },
                message: match &subscript.key {
                    Some(key) => format!("Cannot read element '{key}' of {held}"),
                    None => format!("Cannot read an element of {held}"),
                },
                unnecessary: false,
                deprecated: false,
            });
        }
    }
}

/// A literal key the container's declared keys exclude: `$map[0]` on an `array<string, int>`,
/// `[$a, $b] = $map`, `$list['name']`.
///
/// On by default, unlike the absent key above, because nothing the code could have meant reads it:
/// PHP stores every numeric string key as an integer, so a string-keyed array has no `0` on any run.
pub(super) struct ImpossibleKey;

impl Check for ImpossibleKey {
    fn run(&self, view: &IndexView<'_>, out: &mut Vec<Diagnostic>) {
        for found in view.impossible_keys() {
            out.push(Diagnostic {
                range: found.range,
                severity: Severity::Warning,
                code: IMPOSSIBLE_KEY,
                message: format!("{} has no key {}", found.container, found.key),
                unnecessary: false,
                deprecated: false,
            });
        }
    }
}
