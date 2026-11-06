//! What each name in an open buffer is, for the editor to colour.
//!
//! Two conversions and nothing else. The kinds come from `phrost-analysis`, which has no protocol
//! vocabulary; the legend, the indices into it, and the delta encoding are protocol and live here.
//!
//! It does not *wait* for the index, which is the property worth keeping: the tree paints the file
//! and the index only refines what it can, so a buffer opened while the project is still being
//! read is coloured immediately and coloured better a moment later.

use lspkit_runtime::protocol::{jsonrpc::Result, ls_types::*};
use phrost_model::LineIndex;
use phrost_syntax_features::tokens::{SemanticToken as Name, TokenKind, TokenModifiers};

use crate::{convert, server::Backend};

/// The token types this server sends, in the order their indices refer to.
///
/// Order is the contract: a client is told this list once and afterwards receives numbers. Adding a
/// kind means appending here and nowhere else, because [`type_of`] is exhaustive and will not
/// compile until it is given a name.
const TYPES: &[SemanticTokenType] = &[
    SemanticTokenType::NAMESPACE,
    SemanticTokenType::CLASS,
    SemanticTokenType::INTERFACE,
    SemanticTokenType::ENUM,
    SemanticTokenType::ENUM_MEMBER,
    SemanticTokenType::PROPERTY,
    SemanticTokenType::METHOD,
    SemanticTokenType::FUNCTION,
    SemanticTokenType::PARAMETER,
    SemanticTokenType::VARIABLE,
    SemanticTokenType::DECORATOR,
    SemanticTokenType::TYPE,
    SemanticTokenType::KEYWORD,
    SemanticTokenType::COMMENT,
    SemanticTokenType::STRING,
    SemanticTokenType::NUMBER,
];

/// The modifiers this server sends, in bit order.
const MODIFIERS: &[SemanticTokenModifier] = &[
    SemanticTokenModifier::DECLARATION,
    SemanticTokenModifier::STATIC,
    SemanticTokenModifier::READONLY,
    SemanticTokenModifier::ABSTRACT,
];

/// The legend, and with it the set of kinds worth sending.
///
/// A client declares in `initialize` which token types it understands, and it cannot paint one it
/// did not name -- so a token of any other kind is bytes on the wire for nothing. Measured against
/// a client that declared none: the response was 67.6 KB of 6 222 tokens, the single largest
/// message this server sends and the only one that crossed the 64 KB a pipe buffer holds. Honouring
/// the declaration is both what the protocol asks for and the cheapest cut there is.
///
/// The legend is the *intersection*, in this server's own order, so the indices it hands out and
/// the list it declares cannot drift apart. A client that named nothing gets an empty legend and no
/// tokens, which is what it asked for. A client that named nothing *because it said nothing at all*
/// about semantic tokens is a different case and is handled by the caller, which passes `None`.
pub(crate) fn legend(understood: Option<&[SemanticTokenType]>) -> SemanticTokensLegend {
    SemanticTokensLegend {
        token_types: offered(understood).to_vec(),
        token_modifiers: MODIFIERS.to_vec(),
    }
}

/// Which of this server's kinds a client can paint, in this server's order.
fn offered(understood: Option<&[SemanticTokenType]>) -> Vec<SemanticTokenType> {
    match understood {
        // No list at all: the client did not narrow, so nothing is dropped. An older client that
        // reads tokens without advertising a legend is the case this protects.
        None => TYPES.to_vec(),
        Some(named) => TYPES
            .iter()
            .filter(|ours| named.iter().any(|theirs| theirs == *ours))
            .cloned()
            .collect(),
    }
}

impl Backend {
    pub(crate) async fn answer_semantic_tokens(
        &self,
        uri: &Uri,
        within: Option<Range>,
    ) -> Result<Option<SemanticTokensResult>> {
        let Some((_, file)) = self.file_of(uri).await else {
            return Ok(None);
        };
        let workspace = self.workspace.read().await;
        let Some((text, lines)) = workspace.editable_text(file) else {
            return Ok(None);
        };

        let offered = offered(self.understood_tokens.get().map(Vec::as_slice));
        // Through the index, which settles the kinds one file cannot: an interface and an enum
        // are written exactly like a class, and `Status::Draft` is a constant or a case. A name
        // that resolves to nothing keeps what the tree said, so a buffer opened before the
        // project is indexed is coloured now and re-coloured when the index catches up.
        let names = phrost_analysis::semantic_tokens(&workspace.view(file), &text);
        let data = encode(&text, &lines, names, within, &offered);
        Ok(Some(SemanticTokensResult::Tokens(SemanticTokens {
            // No result id, because there is no delta support to use one: a delta answer means
            // holding the previous result per document and invalidating it in step with the buffer,
            // and a stale one miscolours every line after the edit.
            result_id: None,
            data,
        })))
    }
}

/// Turns absolute ranges into the protocol's line-relative encoding.
///
/// Each token is five numbers, and every one of them is relative to the token before it. That makes
/// the order load-bearing in a way nothing else in this protocol is: one token out of place moves
/// every token after it.
fn encode(
    text: &str,
    lines: &LineIndex,
    names: Vec<Name>,
    within: Option<Range>,
    offered: &[SemanticTokenType],
) -> Vec<SemanticToken> {
    let mut data = Vec::with_capacity(names.len());
    let mut previous = Position::new(0, 0);

    for name in names {
        let range = convert::range_of(text, lines, name.range);
        if within.is_some_and(|within| outside(range, within)) {
            continue;
        }

        // A kind the client did not declare cannot be painted, so sending it is bytes for nothing.
        // Skipped before the delta is computed: the encoding is relative to the token *sent* before,
        // and measuring against one that was dropped moves every token after it.
        let Some(token_type) = index_in(offered, name.kind) else {
            continue;
        };

        let on_a_new_line = range.start.line != previous.line;
        data.push(SemanticToken {
            delta_line: range.start.line.saturating_sub(previous.line),
            // Relative to the previous token when they share a line, absolute otherwise.
            delta_start: if on_a_new_line {
                range.start.character
            } else {
                range.start.character.saturating_sub(previous.character)
            },
            // Safe because no token crosses a line, which the producing crate holds as an
            // invariant with a test of its own.
            length: range.end.character.saturating_sub(range.start.character),
            token_type,
            token_modifiers_bitset: bits(name.modifiers),
        });
        previous = range.start;
    }

    data
}

/// Whether a token falls entirely outside the range a client asked about.
fn outside(token: Range, within: Range) -> bool {
    (token.end.line, token.end.character) <= (within.start.line, within.start.character)
        || (token.start.line, token.start.character) >= (within.end.line, within.end.character)
}

/// The protocol's name for one of this crate's kinds.
///
/// One function rather than a hand-written index, because the index now depends on which kinds the
/// client accepted: two lists that had to be kept in step by hand is exactly how a token comes to be
/// painted as the wrong thing.
fn type_of(kind: TokenKind) -> SemanticTokenType {
    match kind {
        TokenKind::Namespace => SemanticTokenType::NAMESPACE,
        TokenKind::Class => SemanticTokenType::CLASS,
        TokenKind::Interface => SemanticTokenType::INTERFACE,
        TokenKind::Enum => SemanticTokenType::ENUM,
        TokenKind::EnumMember => SemanticTokenType::ENUM_MEMBER,
        TokenKind::Property => SemanticTokenType::PROPERTY,
        TokenKind::Method => SemanticTokenType::METHOD,
        TokenKind::Function => SemanticTokenType::FUNCTION,
        TokenKind::Parameter => SemanticTokenType::PARAMETER,
        TokenKind::Variable => SemanticTokenType::VARIABLE,
        TokenKind::Decorator => SemanticTokenType::DECORATOR,
        TokenKind::Type => SemanticTokenType::TYPE,
        TokenKind::Keyword => SemanticTokenType::KEYWORD,
        TokenKind::Comment => SemanticTokenType::COMMENT,
        TokenKind::Str => SemanticTokenType::STRING,
        TokenKind::Number => SemanticTokenType::NUMBER,
    }
}

/// Where a kind sits in the legend that was declared, or `None` when it was not declared at all.
fn index_in(offered: &[SemanticTokenType], kind: TokenKind) -> Option<u32> {
    let wanted = type_of(kind);
    offered
        .iter()
        .position(|ty| *ty == wanted)
        .and_then(|at| u32::try_from(at).ok())
}

fn bits(modifiers: TokenModifiers) -> u32 {
    u32::from(modifiers.is_declaration)
        | u32::from(modifiers.is_static) << 1
        | u32::from(modifiers.is_readonly) << 2
        | u32::from(modifiers.is_abstract) << 3
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_kind_indexes_the_legend_entry_it_means() {
        // The one place a silent mismatch would be invisible: the client is told the list once and
        // then receives numbers, so a wrong index colours every occurrence of one kind as another
        // and nothing reports an error.
        for (kind, expected) in [
            (TokenKind::Namespace, SemanticTokenType::NAMESPACE),
            (TokenKind::Class, SemanticTokenType::CLASS),
            (TokenKind::Interface, SemanticTokenType::INTERFACE),
            (TokenKind::Enum, SemanticTokenType::ENUM),
            (TokenKind::EnumMember, SemanticTokenType::ENUM_MEMBER),
            (TokenKind::Property, SemanticTokenType::PROPERTY),
            (TokenKind::Method, SemanticTokenType::METHOD),
            (TokenKind::Function, SemanticTokenType::FUNCTION),
            (TokenKind::Parameter, SemanticTokenType::PARAMETER),
            (TokenKind::Variable, SemanticTokenType::VARIABLE),
            (TokenKind::Decorator, SemanticTokenType::DECORATOR),
            (TokenKind::Type, SemanticTokenType::TYPE),
            (TokenKind::Keyword, SemanticTokenType::KEYWORD),
            (TokenKind::Comment, SemanticTokenType::COMMENT),
            (TokenKind::Str, SemanticTokenType::STRING),
            (TokenKind::Number, SemanticTokenType::NUMBER),
        ] {
            assert_eq!(type_of(kind), expected, "{kind:?}");
        }
        assert_eq!(TYPES.len(), 16, "a kind was added without a legend entry");
    }

    #[test]
    fn modifiers_are_the_bits_the_legend_declares() {
        assert_eq!(bits(TokenModifiers::default()), 0);
        assert_eq!(
            bits(TokenModifiers {
                is_declaration: true,
                ..TokenModifiers::default()
            }),
            1 << MODIFIERS
                .iter()
                .position(|modifier| *modifier == SemanticTokenModifier::DECLARATION)
                .expect("declaration is declared")
        );
        assert_eq!(
            bits(TokenModifiers {
                is_declaration: true,
                is_static: true,
                is_readonly: true,
                is_abstract: true,
            }),
            0b1111
        );
    }
}
