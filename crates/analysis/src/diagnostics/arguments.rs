//! A call with the wrong number of arguments for its callee.
//!
//! `ArgumentCountError` at runtime, on every run that reaches the line -- so this is the same kind
//! of claim as an undefined method rather than a style note. It needs the callee's declaration, which
//! is in another file, so it is one of the few checks a per-file linter cannot make.
//!
//! Both directions are implemented, but only the lower bound is always on. See
//! `phrost_view::arguments` for what each one declines and why -- in particular the one thing "too
//! many" needs a fact from lowering to see: a body calling `func_get_args()` accepts more than its
//! signature promises, and a declaration's body is not visible above the boundary. That was 373
//! findings across four corpora until the fact existed, and none at all on the fifth -- the one it
//! was originally measured against, which is why it had been written down as theoretical. What did
//! appear there, 184 times, was a symbol-table collision the check has no
//! way to see through -- see [`ExtraArgument`]'s own doc for what that was. The upper bound is
//! opt-in until that is fixed.

use phrost_view::IndexView;

use super::{Check, Diagnostic, Severity};

pub(crate) const MISSING_ARGUMENT: &str = "missing-argument";
pub(crate) const EXTRA_ARGUMENT: &str = "extra-argument";
pub(crate) const UNINSTANTIABLE_CLASS: &str = "uninstantiable-class";
pub(crate) const NULL_ARGUMENT: &str = "null-argument";
pub(crate) const POSSIBLY_NULL_ARGUMENT: &str = "possibly-null-argument";
pub(crate) const FALSE_ARGUMENT: &str = "false-argument";
pub(crate) const POSSIBLY_FALSE_ARGUMENT: &str = "possibly-false-argument";

pub(super) struct MissingArgument;

/// An argument that may be null where the parameter cannot take one.
/// Named because a quick fix will want to point at it, and because two shapes emit it.
pub(crate) const INCOMPATIBLE_ARGUMENT: &str = "incompatible-argument";
pub(crate) const INCOMPATIBLE_REFERENCE: &str = "incompatible-reference";

pub(super) struct IncompatibleArgument;

impl Check for IncompatibleArgument {
    /// The judgement's own precision rules do the work here; see
    /// `IndexView::incompatible_arguments`, which reports only what can be proven impossible and
    /// leaves everything sentinel-shaped to [`SentinelArgument`].
    fn run(&self, view: &IndexView<'_>, out: &mut Vec<Diagnostic>) {
        for argument in view.incompatible_arguments() {
            out.push(Diagnostic {
                range: argument.range,
                // PHP raises a `TypeError`, so this is a branch that cannot work rather than a
                // style note.
                severity: Severity::Error,
                code: INCOMPATIBLE_ARGUMENT,
                message: match argument.spread {
                    true => format!(
                        "${} takes {}, and this unpacks {}",
                        argument.parameter, argument.declared, argument.given
                    ),
                    false => format!(
                        "${} takes {}, and this is {}",
                        argument.parameter, argument.declared, argument.given
                    ),
                },
                unnecessary: false,
                deprecated: false,
            });
        }
    }
}

pub(super) struct SentinelArgument;

impl Check for SentinelArgument {
    /// Two sentinels and four codes, from one walk. See `IndexView::sentinel_arguments` for why the
    /// `false` half is asked there rather than in a walk of its own.
    fn run(&self, view: &IndexView<'_>, out: &mut Vec<Diagnostic>) {
        for argument in view.sentinel_arguments() {
            let sentinel = match argument.sentinel {
                phrost_model::BuiltinTy::False => "false",
                _ => "null",
            };
            let is_null = argument.sentinel == phrost_model::BuiltinTy::Null;
            out.push(Diagnostic {
                range: argument.range,
                // A null that is certain is not a branch anyone gets away with -- PHP raises a
                // `TypeError` for it whatever the mode -- and one that may be is a branch nobody
                // tested. Same split as a null receiver.
                //
                // A `false` never reaches `Error`, however certain: weak mode converts it to `""`
                // or `0` and the line runs. It is a defect and not a fatal, and the code says which
                // of the two it is -- the same reading `false-iteration` takes, where `foreach` on
                // a `false` is a warning PHP itself only warns about.
                severity: match (is_null, argument.certain) {
                    (true, true) => Severity::Error,
                    _ => Severity::Warning,
                },
                code: match (is_null, argument.certain) {
                    (true, true) => NULL_ARGUMENT,
                    (true, false) => POSSIBLY_NULL_ARGUMENT,
                    (false, true) => FALSE_ARGUMENT,
                    (false, false) => POSSIBLY_FALSE_ARGUMENT,
                },
                message: match argument.certain {
                    true => format!(
                        "Cannot pass {sentinel} as ${}, which takes {}",
                        argument.parameter, argument.declared
                    ),
                    false => format!(
                        "${} takes {}, and this may be {sentinel}",
                        argument.parameter, argument.declared
                    ),
                },
                unnecessary: false,
                deprecated: false,
            });
        }
    }
}

impl Check for MissingArgument {
    fn run(&self, view: &IndexView<'_>, out: &mut Vec<Diagnostic>) {
        for call in view.calls_missing_arguments() {
            let named = call
                .missing
                .iter()
                .map(|name| format!("${name}"))
                .collect::<Vec<_>>()
                .join(", ");

            out.push(Diagnostic {
                range: call.range,
                severity: Severity::Error,
                code: MISSING_ARGUMENT,
                // Both the count and the names: the count says how wrong it is and the names say
                // what to write, and a reader wants the second one.
                message: format!(
                    "{} needs {} argument{}, {} given -- nothing for {named}",
                    call.callee,
                    call.required,
                    match call.required {
                        1 => "",
                        _ => "s",
                    },
                    call.given
                ),
                unnecessary: false,
                deprecated: false,
            });
        }
    }
}

/// A call with more arguments than its callee has parameters for.
///
/// Always on, like its lower-bound sibling -- but it was opt-in for a day, and the reason it could
/// be turned on is worth keeping. Run against a production Drupal checkout it once produced 184
/// findings, every one the same call: `count($x)`, resolving with zero parameters because a
/// dev-dependency's test fixture declares a bare `function count() {}` and out-ranked the real
/// stub. That was a symbol-table defect rather than a mistake in how this counts, and it is fixed
/// -- a dependency's test data now ranks below everything, so it cannot shadow. This check was
/// simply the first that ever trusted an empty parameter list enough to act on it, which is what
/// made the defect visible.
pub(super) struct ExtraArgument;

impl Check for ExtraArgument {
    fn run(&self, view: &IndexView<'_>, out: &mut Vec<Diagnostic>) {
        for call in view.calls_extra_arguments() {
            out.push(Diagnostic {
                // The extra arguments, not the call: everything before them is fine, and
                // underlining the whole call would blame arguments that are correct. See
                // `CLAUDE.md` on why the range is what the done-criterion is about.
                range: call.range,
                severity: Severity::Error,
                code: EXTRA_ARGUMENT,
                message: format!(
                    "{} takes at most {} argument{}, {} given",
                    call.callee,
                    call.allowed,
                    match call.allowed {
                        1 => "",
                        _ => "s",
                    },
                    call.given
                ),
                unnecessary: false,
                deprecated: false,
            });
        }
    }
}

/// A `new` on something PHP will not instantiate.
pub(super) struct UninstantiableNew;

impl Check for UninstantiableNew {
    fn run(&self, view: &IndexView<'_>, out: &mut Vec<Diagnostic>) {
        for found in view.uninstantiable_news() {
            out.push(Diagnostic {
                range: found.range,
                severity: Severity::Error,
                code: UNINSTANTIABLE_CLASS,
                message: format!(
                    "Cannot instantiate {} {}",
                    match found.because {
                        phrost_view::Uninstantiable::Abstract => "abstract class",
                        phrost_view::Uninstantiable::Interface => "interface",
                        phrost_view::Uninstantiable::Trait => "trait",
                    },
                    found.class
                ),
                unnecessary: false,
                deprecated: false,
            });
        }
    }
}

/// A `&` argument naming a property that cannot hold what the callee may write.
///
/// The other direction of [`IncompatibleArgument`], and the reason it is a separate code rather
/// than a wider reading of that one: nothing is wrong with the argument going *in*. An `array`
/// passed to a `?array` parameter is a fine argument, so the read-side judgement accepts it and is
/// right to. What the call can do that the class forbids is put the null back.
///
/// An error rather than a warning. A property declared `array` holding a null is a state the
/// engine refuses -- PHP throws a `TypeError` on the write, not on the read -- so this is not a
/// branch that runs and is wrong.
pub(super) struct WrittenBackArgument;

impl Check for WrittenBackArgument {
    fn run(&self, view: &IndexView<'_>, out: &mut Vec<Diagnostic>) {
        for found in view.written_back_arguments() {
            out.push(Diagnostic {
                range: found.range,
                severity: Severity::Error,
                code: INCOMPATIBLE_REFERENCE,
                // Both promises on one line: what the property is declared to hold, and what the
                // parameter says it may put there. The callee is a lookup away by name.
                message: format!(
                    "{}::${} takes {}, and &${} may write {} into it",
                    found.owner, found.property, found.declared, found.parameter, found.writes
                ),
                unnecessary: false,
                deprecated: false,
            });
        }
    }
}
