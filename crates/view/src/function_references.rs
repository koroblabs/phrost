//! Every free function the current file calls by name, and whether it names anything.
//!
//! The function-table counterpart of [`name_references`](IndexView::name_references), and separate
//! from it for the reason PHP keeps the tables separate: `Repo()` and `new Repo` can name different
//! things in the same file, so one projection answering for both would resolve a call against the
//! classes and find a declaration that has nothing to do with what was written.
//!
//! # Only a call whose callee is a written name
//!
//! `$f()`, `($f)()` and `foo()()` are [`IndirectCall`](phrost_hir::body::ExprData::IndirectCall) --
//! they hold a value, not a name, and lowering never records a function-table reference for one.
//! `call_user_func('x')` and `array_map('x', ...)` are the same: the callee is a string literal
//! argument, and a string is not a name. So this list contains direct calls and PHP 8.1's
//! `strlen(...)` first-class callables, which is exactly the set where a missing declaration is a
//! fatal error rather than a value nothing here can follow.

use phrost_hir::ImportKind;
use phrost_model::{Name, TextRange};

use crate::view::IndexView;

/// One function name written in the file.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct FunctionReference {
    /// The name alone -- what a cursor lands on and what a diagnostic underlines.
    pub range: TextRange,
    /// The path as written, without a leading separator.
    pub path: Name,
    /// The declaration it resolves to, fully qualified. `None` when it resolves to nothing.
    pub resolved: Option<Name>,
    /// Written in a docblock rather than in code.
    ///
    /// Carried for the reason a class-like reference carries it: in code a missing name is a fatal
    /// error, in a docblock it is a wrong annotation, and the two cannot be judged by one rule.
    pub in_docblock: bool,
    /// The file asks somewhere whether this function is declared.
    ///
    /// `function_exists('ssh2_connect')` is the only way PHP has to say "this extension may not be
    /// installed", and the code behind it is correct whether or not it is. See
    /// [`phrost_body::existence_tests`].
    pub existence_tested: bool,
}

impl IndexView<'_> {
    /// Every free-function name written in the current file, in source order.
    ///
    /// Resolution goes through the file's scope at the name's own offset, so it gets PHP's two
    /// function-lookup rules for free: a `use function` import (aliased or not) supplies the prefix
    /// when one matches, and an unqualified name inside a namespace falls back to global when
    /// `App\strlen` is declared nowhere. Getting that second rule wrong would make every `count()`
    /// in a namespaced file resolve to nothing.
    pub fn function_references(&self) -> Vec<FunctionReference> {
        let Some(open) = self.current() else {
            return Vec::new();
        };
        let hir = open.hir();
        let tested = phrost_body::existence_tests(hir);
        let mut out = Vec::new();

        for (_, reference) in hir.name_refs() {
            if reference.kind != ImportKind::Function {
                continue;
            }
            out.push(FunctionReference {
                range: reference.range,
                path: reference.path.clone(),
                resolved: self
                    .resolve_name(
                        reference.range.start(),
                        reference.path.as_str(),
                        reference.anchor,
                        ImportKind::Function,
                    )
                    .map(|location| location.fqn),
                in_docblock: reference.in_docblock,
                existence_tested: tested.tests_function(reference.path.as_str()),
            });
        }

        out.sort_by_key(|reference| reference.range.start());
        out
    }

    /// Where the current file writes `use function <fqn>;`.
    ///
    /// Beside the calls rather than in them: an import is not a call, so the projection above is
    /// right to leave it out -- and a rename that leaves it out produces a file importing a
    /// function that no longer exists. Matched case-insensitively, because PHP's function names
    /// are.
    pub fn function_imports_of(&self, fqn: &str) -> Vec<TextRange> {
        let Some(open) = self.current() else {
            return Vec::new();
        };
        open.hir()
            .imports()
            .filter(|(_, import)| {
                import.kind == ImportKind::Function
                    && import.path.as_str().eq_ignore_ascii_case(fqn)
            })
            .map(|(_, import)| import.path_range)
            .collect()
    }
}
