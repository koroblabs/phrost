//! Whether a declaration reads arguments its signature does not name.
//!
//! `func_get_args()` and `func_num_args()` are how PHP wrote a variadic before it had syntax for
//! one, and they are still how a function keeps a signature it does not want to change. A
//! declaration that calls either treats its parameter list as a **lower** bound: PHP raises nothing
//! when a caller supplies more, and the extra values come back through the call rather than through
//! a name. Laravel's `PendingRequest::throwIf($condition)` is the shape --
//! `return $condition ? $this->throw(func_get_args()[1] ?? null) : $this;` -- and its own tests pass
//! two arguments to it on purpose.
//!
//! So the fact belongs beside the parameter count, and `phrost_view::arguments` said so before it
//! existed: nothing above lowering can see a body, because a closed file's statements are never
//! retained, and from the parameter table alone such a declaration is indistinguishable from one
//! that really takes what it names.
//!
//! # Why a text scan, and why that is safe here
//!
//! Read off the body's own text rather than by walking the tree. §8 records a text scan being the
//! wrong instrument -- a scan for a syntax read Drupal's documentation *of* that syntax as an
//! instance of it -- and the difference is the direction of the error. That scan **added** a claim,
//! so a false hit removed a working resolution. This one only ever **withholds** a diagnostic, so
//! its false hits are silence: a `func_get_args` written in a comment, in a string, or as part of a
//! longer identifier costs one call site its arity check and can report nothing wrong.
//!
//! Two imprecisions follow from the same direction and are worth stating rather than fixing. A
//! closure written inside the body is inside the body's span, so a `func_get_args()` that is the
//! *closure's* silences the enclosing declaration too -- the mirror of the one thing `saw_yield`
//! gets wrong, arriving with the safe sign. And the scan cannot tell `func_get_args()` from
//! `$this->func_get_args()`; a method of that name is not a thing anybody writes, and if they did,
//! the cost is again silence.

use mago_span::HasSpan;

use crate::lowerer::Lowerer;

/// The two names, and the prefix they share.
///
/// The shared prefix is what the file-level prefilter tests, so a file mentioning neither is
/// rejected by one search rather than two.
pub(crate) const PREFIX: &str = "func_";
const NAMES: [&str; 2] = ["func_get_args", "func_num_args"];

impl<'p> Lowerer<'_, 'p> {
    /// Whether the span of a declaration's body reads its own arguments.
    ///
    /// Gated on the whole file mentioning `func_` at all, which is what keeps this off the path for
    /// every declaration in a project that does not use it -- one search per file rather than one
    /// per body, and the per-body scan runs only inside the few files that could hit.
    pub(crate) fn reads_own_arguments(&self, body: &impl HasSpan) -> bool {
        if !self.file_may_read_own_arguments() {
            return false;
        }
        let source = self.parsed_source();
        let span = body.span();
        let (start, end) = (span.start.offset as usize, span.end.offset as usize);
        // Clamped rather than trusted: a repaired parse can carry a span past the text it was
        // re-parsed from, and slicing on a byte that is not a character boundary panics.
        let Some(text) = source.get(start.min(source.len())..end.min(source.len())) else {
            return false;
        };
        NAMES.iter().any(|name| text.contains(name))
    }
}
