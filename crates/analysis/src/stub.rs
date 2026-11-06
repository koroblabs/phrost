//! The declaration a class writes for a method something above it left to be written.
//!
//! One writer, because there are two places a person asks for the same text: typing the name and
//! accepting a completion, or standing on a class the checker says does not load and asking for all
//! of them at once. The second is the first repeated, and a second generator would be a second
//! answer to what a stub looks like.
//!
//! What may be written at all is [`IndexView::overridable_methods`](phrost_view::IndexView), which
//! refuses a method whose declaration this could not write back exactly -- a **promoted**
//! parameter, which declares a property as well. This only lays out what that hands over.

use phrost_model::TextSize;
use phrost_view::OverridableMethod;

/// How far in the declaration sits, and how much further its body goes.
pub(crate) struct Layout {
    /// The whitespace the cursor's line begins with. The first line of the stub is written *at* the
    /// cursor, so only the lines under it need this.
    indent: String,
    /// One step further in.
    step: &'static str,
}

impl Layout {
    /// Read off the line the cursor is on.
    ///
    /// The step is inferred from that line rather than configured: a file indented with tabs gets a
    /// tab and everything else gets PSR-12's four spaces. Nothing in the index says what a project
    /// indents with, and the line the person is typing on is the closest thing to an answer.
    pub(crate) fn at(text: &str, offset: TextSize) -> Self {
        let at = usize::from(offset).min(text.len());
        let line = text[..at].rfind('\n').map_or(0, |end| end + 1);
        let indent = text[line..at]
            .chars()
            .take_while(|ch| *ch == ' ' || *ch == '\t')
            .collect::<String>();
        let step = match indent.contains('\t') {
            true => "\t",
            false => "    ",
        };
        Self { indent, step }
    }

    /// A layout for text written at a known indentation rather than at a cursor.
    ///
    /// What an action has and a completion does not: the position a stub goes to is decided by the
    /// class rather than by where somebody's caret happens to be.
    pub(crate) fn inside(indent: &str) -> Self {
        Self {
            indent: indent.to_owned(),
            step: match indent.contains('\t') {
                true => "\t",
                false => "    ",
            },
        }
    }

    /// The declaration, its braces, and where inside them a body goes.
    ///
    /// The brace goes on its own line, which is what PSR-12 asks for and what the PHP a person is
    /// completing into almost always already does.
    ///
    /// No `parent::` call in the body. For an abstract or interface method there is nothing above to
    /// call and writing one is a fatal error; for a concrete one it is a guess about intent, and a
    /// line somebody deletes on most acceptances is the same mistake as a `;` in the wrong place.
    pub(crate) fn stub(&self, method: &OverridableMethod) -> (String, usize) {
        let mut text = String::new();
        // The parent's visibility, never a narrower one: PHP allows an override to widen and refuses
        // to load a class that narrows. Widening is a decision, and this makes none.
        text.push_str(method.visibility.as_str());
        text.push(' ');
        if method.is_static {
            text.push_str("static ");
        }
        text.push_str("function ");
        text.push_str(&method.signature);

        text.push('\n');
        text.push_str(&self.indent);
        text.push('{');
        text.push('\n');
        text.push_str(&self.indent);
        text.push_str(self.step);
        let cursor = text.len();
        text.push('\n');
        text.push_str(&self.indent);
        text.push('}');

        (text, cursor)
    }
}
