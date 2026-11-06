//! Splitting a written name into the part that is a namespace and the part that is not.
//!
//! `App\Nested\Repo` is one identifier in the tree and two different things to a reader. A grammar
//! colours the whole thing one way; splitting it is one of the few places where this crate can say
//! something an editor cannot work out for itself.

use mago_span::Span;

use super::{SemanticToken, TokenKind, TokenModifiers};

/// Emits a written name as its namespace prefix and its final segment.
///
/// The prefix carries no modifiers of its own — it is a namespace either way, and the modifiers
/// describe the thing being named.
///
/// **The separators belong to the prefix**, the leading one included. They used to belong to
/// nothing: the prefix stopped one character short of the `\` that ends it and a leading `\` was
/// stripped before anything was measured, so `Drupal\app_core\Supplier\Product\ProductType`
/// came out with three of its four separators coloured as part of the namespace run and the fourth
/// left grey, which reads as the feature failing on the last one. What made the old rule look right
/// is that it is right about the *name*: that token has to start exactly where the name does, or it
/// sits one character to the left of what it names. Ending the prefix at the separator keeps that
/// and makes the whole path one colour.
///
/// `\Repo` is then a one-character prefix and no more, which is what a leading separator means: the
/// root namespace, named by writing nothing in it.
pub(super) fn qualified(
    span: Span,
    source: &str,
    kind: TokenKind,
    modifiers: TokenModifiers,
    found: &mut Vec<SemanticToken>,
) {
    let Some(text) = source.get(span.start.offset as usize..span.end.offset as usize) else {
        return;
    };
    if text.is_empty() {
        return;
    }
    let start = span.start.offset as usize;

    match text.rfind('\\') {
        Some(cut) => {
            found.push(token(
                start,
                start + cut + 1,
                TokenKind::Namespace,
                TokenModifiers::default(),
            ));
            if cut + 1 < text.len() {
                found.push(token(start + cut + 1, start + text.len(), kind, modifiers));
            }
        }
        None => found.push(token(start, start + text.len(), kind, modifiers)),
    }
}

/// Emits a name that has no namespace to split off: a declared name, or a member selector.
pub(super) fn plain(
    span: Span,
    kind: TokenKind,
    modifiers: TokenModifiers,
    found: &mut Vec<SemanticToken>,
) {
    if span.start.offset >= span.end.offset {
        return;
    }
    found.push(token(
        span.start.offset as usize,
        span.end.offset as usize,
        kind,
        modifiers,
    ));
}

/// A variable's name as written, without the `$`, for matching against a parameter list.
pub(super) fn variable_name(name: &[u8]) -> &str {
    std::str::from_utf8(name)
        .unwrap_or_default()
        .trim_start_matches('$')
}

fn token(start: usize, end: usize, kind: TokenKind, modifiers: TokenModifiers) -> SemanticToken {
    SemanticToken {
        range: phrost_model::TextRange::new(
            phrost_model::TextSize::try_from(start).unwrap_or_default(),
            phrost_model::TextSize::try_from(end).unwrap_or_default(),
        ),
        kind,
        modifiers,
    }
}
