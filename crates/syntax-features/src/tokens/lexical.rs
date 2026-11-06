//! What a file's tokens are, before anything knows what they mean.
//!
//! The half a grammar would have coloured: keywords, strings, numbers, comments, modifiers. Needed
//! because an editor told to use these tokens *instead of* its grammar draws nothing for a range
//! nobody claims -- and a file whose `if`, `return` and every string are uncoloured reads worse than
//! one with no semantic tokens at all.
//!
//! Deliberately not every token. Whitespace is most of them and has no colour; operators and
//! punctuation are given the plain foreground by nearly every theme, so emitting one per `;` and `->`
//! would multiply the payload for a difference nobody sees. What is here is what a reader would
//! notice missing.

use mago_syntax::{lexer::Lexer, token::TokenKind as Lexed};
use mago_syntax_core::input::Input;
use phrost_model::{TextRange, TextSize};

use super::{SemanticToken, TokenKind, TokenModifiers};

/// Every lexically-decided token in `text`, in document order, each within one line.
pub(super) fn from_tokens(text: &str) -> Vec<SemanticToken> {
    let mut lexer = Lexer::new(
        Input::new(mago_database::file::FileId::zero(), text.as_bytes()),
        mago_syntax::settings::LexerSettings::default(),
    );
    let mut found = Vec::new();

    // Whether the lexer is between a heredoc's two delimiters. PHP has no nested heredoc, so a flag
    // is the whole of the state -- and the body's own tokens come back as `StringPart`, the same kind
    // a double-quoted string's literal chunks do, so nothing but position tells them apart.
    let mut inside_a_document = false;

    while let Some(Ok(token)) = lexer.advance() {
        if matches!(token.kind, Lexed::DocumentEnd) {
            inside_a_document = false;
        }
        // A heredoc's body is the one string whose contents are routinely another language, and the
        // custom label is the author saying which: `<<<TWIG` is written to get Twig highlighting.
        // Sending one flat string token per line takes that away, because a client applies ours over
        // its own grammar -- so the body is left alone, exactly as inline HTML is.
        let skip = inside_a_document && matches!(token.kind, Lexed::StringPart);
        let kind = match (skip, kind_of(token.kind)) {
            (false, Some(kind)) => kind,
            _ => {
                if matches!(token.kind, Lexed::DocumentStart(_)) {
                    inside_a_document = true;
                }
                continue;
            }
        };
        if matches!(token.kind, Lexed::DocumentStart(_)) {
            inside_a_document = true;
        }
        let start = TextSize::from(token.start.offset);
        let length = TextSize::try_from(token.value.len()).unwrap_or_default();
        let range = TextRange::at(start, length);

        // The protocol cannot encode a token spanning lines, and a comment or a heredoc routinely
        // does. One per line, which is what every other server sends for the same reason.
        for line in lines_of(text, range) {
            found.push(SemanticToken {
                range: line,
                kind,
                modifiers: TokenModifiers::default(),
            });
        }
    }

    found
}

/// Which of ours a lexed token is, or nothing when it is not one worth colouring.
///
/// `public`, `static` and `readonly` come back as keywords, which they are. The protocol has a
/// `modifier` type and they were sent as that at first, on the reasoning that a theme dims the group
/// separately -- which was a guess. Reported from an editor: almost no theme styles `modifier`, so
/// every visibility keyword in the file came out plain while everything around it was coloured.
fn kind_of(lexed: Lexed) -> Option<TokenKind> {
    match lexed {
        Lexed::SingleLineComment
        | Lexed::HashComment
        | Lexed::MultiLineComment
        | Lexed::DocBlockComment => Some(TokenKind::Comment),
        Lexed::LiteralString | Lexed::PartialLiteralString | Lexed::StringPart => {
            Some(TokenKind::Str)
        }
        // A heredoc's own delimiters: `<<<TWIG` and the `TWIG` that ends it. They are the string's
        // quotes, and a quoted literal's are already inside `LiteralString` -- so leaving these out
        // left the two ends of every heredoc plain while its body was coloured, which is what a
        // client showing *only* our tokens renders as the highlighting breaking there.
        Lexed::DocumentStart(_) | Lexed::DocumentEnd => Some(TokenKind::Str),
        Lexed::LiteralInteger | Lexed::LiteralFloat => Some(TokenKind::Number),
        // `<?php` and `?>` are the file's own frame. A keyword is the closest the protocol has, and
        // leaving them plain makes the top of every file look unfinished.
        Lexed::OpenTag | Lexed::ShortOpenTag | Lexed::EchoTag | Lexed::CloseTag => {
            Some(TokenKind::Keyword)
        }
        // A cast names a type, and both of the other tools that colour PHP agree: tree-sitter
        // captures `(cast_type)` as `@type.builtin` and phpantom emits its `TYPE`. Calling it a
        // keyword put `(int)` in the same colour as `if`, which is the one difference from a
        // grammar-highlighted file a reader notices immediately.
        other if other.is_cast() => Some(TokenKind::Type),
        // A magic constant is a keyword that looks like a name: `__CLASS__` resolves to nothing and
        // names nothing.
        other if other.is_magic_constant() => Some(TokenKind::Keyword),
        other if other.is_keyword() => Some(TokenKind::Keyword),
        // Whitespace, operators, punctuation, identifiers, variables, inline HTML. The names are the
        // tree's to describe; the rest is what this deliberately leaves alone.
        _ => None,
    }
}

/// One range per line the input range covers.
fn lines_of(text: &str, range: TextRange) -> Vec<TextRange> {
    let slice = &text[usize::from(range.start())..usize::from(range.end())];
    if !slice.contains('\n') {
        return vec![range];
    }

    let mut ranges = Vec::new();
    let mut at = range.start();
    for piece in slice.split_inclusive('\n') {
        let length = TextSize::try_from(piece.len()).unwrap_or_default();
        // Without the newline itself: a token that ended on it would still cross into the next line
        // as far as the encoder is concerned.
        let visible =
            TextSize::try_from(piece.trim_end_matches(['\n', '\r']).len()).unwrap_or_default();
        if visible > TextSize::from(0) {
            ranges.push(TextRange::at(at, visible));
        }
        at += length;
    }

    ranges
}
