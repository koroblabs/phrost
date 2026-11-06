use mago_span::HasSpan as _;
use mago_syntax::error::ParseError;
use phrost_model::TextRange;

/// One syntax error, detached from the parser's types.
///
/// A phrost value rather than a re-export: the diagnostics layer must be able to report syntax
/// errors without being allowed to name a mago type. The message is rendered eagerly because
/// errors are rare, and keeping one alive would tie the diagnostic to the parse arena's lifetime.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SyntaxError {
    pub message: String,
    pub range: TextRange,
}

impl SyntaxError {
    pub(crate) fn from_parse_error(error: &ParseError) -> Self {
        Self {
            message: error.to_string(),
            range: crate::text_range(error.span()),
        }
    }
}
