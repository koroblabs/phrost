//! A call to a function that is declared nowhere.
//!
//! `array_mapp([], [])` -- a typo in a builtin's name -- is a fatal error the moment the line runs,
//! and it is the defect a person catches by hand most often, because nothing else in a PHP toolchain
//! looks at it. Unlike a missing method there is nothing dynamic about the name itself: it either
//! resolves through this file's `use function` imports, its namespace, and the global table, or it
//! resolves to nothing.
//!
//! # What is not reported, and why it is structural rather than a filter here
//!
//! Only a call whose callee is a written name reaches
//! [`function_references`](phrost_view::IndexView::function_references) at all. `$f()`, `($f)()`,
//! `call_user_func('x')` and `array_map('strlen', ..)` hold a *value*: lowering records an
//! [`IndirectCall`](phrost_hir::body::ExprData::IndirectCall) or a plain string argument, and
//! neither carries a function-table reference. So the exclusion is a property of the IR rather than
//! a rule written here, which is what makes it impossible to get wrong later.
//!
//! A name written in a docblock is skipped for the reason
//! [`UndefinedType`](super::name::UndefinedType) skips one: `@see helper()` is prose about code,
//! not code.
//!
//! # The two shapes that used to make this cry wolf
//!
//! Both are ordinary PHP, and neither is hoisted:
//!
//! ```php
//! if (!function_exists('dump')) { function dump(mixed ...$vars): mixed { /* .. */ } }
//! function outer(): void { function inner(): void {} }
//! ```
//!
//! Lowering used to record a function only from the statement walk over a file's top level, a
//! namespace, or a bare block, so a declaration reached through a conditional or through a body was
//! in no index and a call to one resolved to nothing exactly as a typo does -- one finding on a
//! 1 662-file checkout, and it was symfony/var-dumper's guarded `dump()`. One wrong report is what
//! teaches a person to stop reading a list, so the check waited rather than spending that trust.
//!
//! Both shapes are lowered now, marked as reached only when the code around them runs, and ranked
//! below every unguarded declarer including the bundled stubs -- so a polyfill answers when nothing
//! else declares the name and shadows nothing when something does.

use phrost_view::IndexView;

use super::{Check, Diagnostic, Severity};

pub(crate) const UNDEFINED_FUNCTION: &str = "undefined-function";

pub(super) struct UndefinedFunction;

impl Check for UndefinedFunction {
    fn run(&self, view: &IndexView<'_>, out: &mut Vec<Diagnostic>) {
        for reference in view.function_references() {
            // A name the file tests for is one the author has already said may be absent.
            if reference.resolved.is_some() || reference.in_docblock || reference.existence_tested {
                continue;
            }
            // A call a template's dialect wrote in place of a tag: it renders a template, and is a
            // function in nothing but the PHP the template was read as.
            if view.template_writes(reference.path.as_str()) {
                continue;
            }

            out.push(Diagnostic {
                range: reference.range,
                severity: Severity::Error,
                code: UNDEFINED_FUNCTION,
                message: format!("Undefined function {}()", reference.path),
                unnecessary: false,
                deprecated: false,
            });
        }
    }
}
