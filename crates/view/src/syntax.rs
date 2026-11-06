//! What did not parse.
//!
//! The one projection that is about the text rather than about its meaning, and the only one whose
//! answer exists for a file precisely because something is wrong with it.

use phrost_model::TextRange;

use crate::view::IndexView;

/// One place the parser could not make sense of.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SyntaxProblem {
    pub range: TextRange,
    pub message: String,
}

impl IndexView<'_> {
    /// Everything the parser reported about the current file, in source order.
    ///
    /// Converted into this crate's own shape rather than handed out as the parser's error type. A
    /// feature layer that could name that type would have the parser in its reach, which is the one
    /// thing the crate graph is checked to prevent.
    ///
    /// Empty when the file is not open: a closed file's text is not parsed, and a project's worth of
    /// parse errors is not something anyone asked for.
    pub fn syntax_problems(&self) -> Vec<SyntaxProblem> {
        let Some(open) = self.current() else {
            return Vec::new();
        };

        let mut problems = open
            .errors()
            .iter()
            .map(|error| SyntaxProblem {
                range: error.range,
                message: error.message.clone(),
            })
            .collect::<Vec<_>>();
        problems.sort_by_key(|problem| problem.range.start());
        problems
    }
}
