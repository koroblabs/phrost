//! Where the source is inside a string.
//!
//! From the token stream rather than the tree, and that is the whole point. A string being typed is
//! not closed yet, so it does not parse -- and the recovered tree has no literal for it, which is
//! exactly the moment a completion list must not appear. The lexer has a token for it
//! (`PartialLiteralString`), so asking the lexer answers for both the finished and the half-written
//! case with one mechanism.
//!
//! Interpolation is left out on purpose: in `"text $name"` the parts around the variable are string,
//! and `$name` is not. Someone typing there is naming a variable and should be offered one.

use mago_database::file::FileId as MagoFileId;
use mago_syntax::{lexer::Lexer, token::TokenKind};
use mago_syntax_core::input::Input;
use phrost_model::{TextRange, TextSize};

/// Every range of the text that is inside a string literal, in source order.
///
/// A second pass over the bytes, so it is asked for only where it is needed: the open buffer whose
/// body was lowered. Lexing is a fraction of parsing, and the alternative -- keeping the token
/// stream alive -- would mean holding the parse arena, which nothing above the parser may do.
pub fn string_ranges(text: &str) -> Vec<TextRange> {
    lex(text, false).strings
}

/// Where the source is inside a string and where it is inside a comment, from the same one pass.
///
/// The comments for the reader that scans characters backwards: completion reads the receiver of
/// a chain out of the text before the cursor, and a `)` in a comment inside a closure's body --
/// `// expected :)` -- unbalanced the parentheses it counts, so `})->` after it offered nothing.
pub fn strings_and_comments(text: &str) -> Lexed {
    lex(text, true)
}

/// What one pass of the lexer found.
#[derive(Debug)]
pub struct Lexed {
    pub strings: Vec<TextRange>,
    /// Every comment, docblocks included, delimiters included, in source order.
    pub comments: Vec<TextRange>,
}

fn lex(text: &str, comments: bool) -> Lexed {
    let mut lexer = Lexer::new(
        Input::new(MagoFileId::zero(), text.as_bytes()),
        mago_syntax::settings::LexerSettings::default(),
    );
    let mut found = Lexed {
        strings: Vec::new(),
        comments: Vec::new(),
    };

    while let Some(Ok(token)) = lexer.advance() {
        let into = match token.kind {
            TokenKind::LiteralString | TokenKind::PartialLiteralString | TokenKind::StringPart => {
                &mut found.strings
            }
            kind if comments && kind.is_comment() => &mut found.comments,
            _ => continue,
        };
        let start = TextSize::from(token.start.offset);
        let length = TextSize::try_from(token.value.len()).unwrap_or_default();
        into.push(TextRange::at(start, length));
    }

    found
}
