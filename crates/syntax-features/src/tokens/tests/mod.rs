//! What each name is reported as, rendered whole.
//!
//! One file per group of arms. A shared expected-output file would be touched by every change to
//! any arm, and there are thirty of them.

mod array_shapes;
mod declarations;
mod docblocks;
mod heredoc;
mod invariants;
mod named_arguments;
mod names;
mod partial_application;
mod references;
mod trait_adaptations;
mod variables;

use expect_test::Expect;

use crate::tokens::{SemanticToken, TokenKind, TokenModifiers, tokens};

/// Renders every token as its position, the text it covers, its kind, and its modifiers.
///
/// The covered text is in there because a wrong span is the failure mode that matters: a token one
/// character to the left still has a plausible kind, and reading the position alone will not show it.
pub(super) fn check(source: &str, expected: Expect) {
    let rendered = tokens(source)
        .into_iter()
        .map(|token| {
            let (line, column) = position(source, usize::from(token.range.start()));
            let at = format!("{line}:{column}");
            format!(
                "{at:<8} {:<16} {:<10} {}\n",
                &source[usize::from(token.range.start())..usize::from(token.range.end())],
                name(token.kind),
                modifiers(token.modifiers),
            )
        })
        .collect::<String>();

    expected.assert_eq(&rendered);
}

fn name(kind: TokenKind) -> &'static str {
    match kind {
        TokenKind::Namespace => "namespace",
        TokenKind::Class => "class",
        TokenKind::Interface => "interface",
        TokenKind::Enum => "enum",
        TokenKind::EnumMember => "enumMember",
        TokenKind::Property => "property",
        TokenKind::Type => "type",
        TokenKind::Method => "method",
        TokenKind::Function => "function",
        TokenKind::Parameter => "parameter",
        TokenKind::Variable => "variable",
        TokenKind::Decorator => "decorator",
        TokenKind::Keyword => "keyword",
        TokenKind::Comment => "comment",
        TokenKind::Str => "string",
        TokenKind::Number => "number",
    }
}

fn modifiers(modifiers: TokenModifiers) -> String {
    [
        ("declaration", modifiers.is_declaration),
        ("static", modifiers.is_static),
        ("readonly", modifiers.is_readonly),
        ("abstract", modifiers.is_abstract),
    ]
    .into_iter()
    .filter(|(_, set)| *set)
    .map(|(name, _)| name)
    .collect::<Vec<_>>()
    .join(" ")
}

/// One-based line, zero-based column in characters.
fn position(source: &str, offset: usize) -> (usize, usize) {
    let before = &source[..offset];
    let column = before
        .rsplit_once('\n')
        .map_or(before, |(_, last)| last)
        .chars()
        .count();
    (before.matches('\n').count() + 1, column)
}

/// The tokens of `source`, for the invariant tests, which do not care how they render.
pub(super) fn all(source: &str) -> Vec<SemanticToken> {
    tokens(source)
}
