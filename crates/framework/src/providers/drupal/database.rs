//! What Drupal's database layer returns, where its own signature is wrong about it.

use phrost_model::BuiltinTy;

use crate::rule::{ChainedReturn, NonNullReturn, NullFact};

/// Database methods declared `?StatementInterface` that cannot return `NULL` on a path that returns
/// at all.
///
/// Read from core rather than assumed, because a wrong entry here silences a real null:
///
/// - `Connection::query()` ends `return $result ? $statement : NULL;`, and `$result` is falsy only
///   in the `catch` that has already called `exceptionHandler()->handleExecutionException()`, whose
///   whole job is to throw. The `NULL` is the return type's tribute to a branch that does not
///   return.
/// - `Select::execute()` returns `NULL` when `preExecute()` is false; `preExecute()` returns early
///   `TRUE` when already prepared and otherwise falls through to `return TRUE`. Its own docblock says
///   the false case is "validation routines may throw exceptions instead".
///
/// Declared on `Connection` and `SelectInterface` rather than on each driver, because a driver's
/// override inherits the contract with the signature -- which is what makes this two entries instead
/// of two per database backend.
pub(super) static NON_NULL_RETURNS: &[NonNullReturn] = &[
    NonNullReturn {
        class: "Drupal\\Core\\Database\\Connection",
        method: "query",
        null: NullFact::Unreachable,
    },
    NonNullReturn {
        class: "Drupal\\Core\\Database\\Query\\SelectInterface",
        method: "execute",
        null: NullFact::Unreachable,
    },
];

/// What an entity query gives back, which its own signature cannot say.
///
/// `QueryInterface::execute()` declares `int|array` because `count()` decides which: a count query
/// answers a number, and every other query answers entity ids. The union is a fact about the
/// method and says nothing about a call, so a checker believing it reports the branch nobody took.
///
/// Measured on one real project: 246 calls to `execute()`, **not one of them** a count query, and
/// 22 findings about the `int` that never arrives.
pub(super) static CHAINED_RETURNS: &[ChainedReturn] = &[ChainedReturn {
    class: "Drupal\\Core\\Entity\\Query\\QueryInterface",
    method: "execute",
    after: "count",
    then: BuiltinTy::Int,
    otherwise: BuiltinTy::Array,
}];
