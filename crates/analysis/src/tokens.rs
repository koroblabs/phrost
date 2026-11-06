//! What the index can say about a colour that the tree could not.
//!
//! The token layer is deliberately syntactic: it answers from one file, so a buffer opened while
//! the project is still indexing is coloured at once. The price is that every class-like name in
//! use is a `class` -- an interface, a trait and an enum are written exactly like one -- and that
//! `Status::Draft` is a `property`, because `Foo::BAR` is a class constant or an enum case and
//! only the declaration says which.
//!
//! This is the second half, and it is a *refinement* rather than a second painter: the tokens are
//! the syntax layer's, and each one the index has something to say about is rewritten in place. A
//! name that resolves to nothing keeps the colour it had, which is what makes an unindexed project
//! behave exactly as it did before rather than losing its colours while it waits.

use phrost_syntax_features::tokens::{SemanticToken, TokenKind, tokens};
use phrost_view::{ClassKind, IndexView};

/// Every name in `text`, with the kinds the index can settle settled.
pub fn semantic_tokens(view: &IndexView<'_>, text: &str) -> Vec<SemanticToken> {
    let mut painted = tokens(text);
    let mut enums = Vec::new();

    // The names first, because what a `Foo::BAR` is depends on what `Foo` is.
    for reference in view.name_references() {
        // A docblock's names are already types, and `@see` is the type colour too.
        if reference.in_docblock {
            continue;
        }
        let Some(resolved) = &reference.resolved else {
            continue;
        };
        let Some(kind) = view.class_kind(resolved.as_str()) else {
            continue;
        };
        let refined = match kind {
            ClassKind::Interface => TokenKind::Interface,
            ClassKind::Enum => TokenKind::Enum,
            // A trait is class-like and the protocol has no name for it, so the syntax layer's
            // answer is already the best one there is.
            ClassKind::Class | ClassKind::Trait => continue,
        };
        // The written path may be qualified, and the token covers its last segment: the prefix is
        // a namespace and has its own. So the two are paired by where they *end*.
        let Some(token) = painted.iter_mut().find(|token| {
            token.kind == TokenKind::Class && token.range.end() == reference.range.end()
        }) else {
            continue;
        };
        token.kind = refined;
        if refined == TokenKind::Enum {
            enums.push(token.range);
        }
    }

    // `self::Draft` inside the enum that declares it: the receiver is a keyword rather than a
    // name, so no reference above covers it -- and it is how an enum names its own cases.
    let receivers: Vec<_> = painted
        .iter()
        .filter(|token| token.kind == TokenKind::Keyword)
        .map(|token| token.range)
        .filter(|range| {
            matches!(
                text.get(usize::from(range.start())..usize::from(range.end())),
                Some("self" | "static")
            )
        })
        .collect();
    for range in receivers {
        let enclosing = view
            .enclosing_class(range.start())
            .and_then(|fqn| view.class_kind(fqn.as_str()));
        if enclosing == Some(ClassKind::Enum) {
            enums.push(range);
        }
    }

    settle_cases(&mut painted, &enums);
    painted
}

/// Turns the constant read off an enum into the case it is.
///
/// By adjacency, and that is exact rather than a guess: `Foo::BAR` is the only way to write one,
/// so the constant's token is the next one after the receiver's with nothing but `::` between
/// them. Matching on the receiver's *resolution* instead would mean resolving every constant
/// access in the file, which is the most expensive question a request can ask.
fn settle_cases(painted: &mut [SemanticToken], enums: &[phrost_model::TextRange]) {
    for index in 0..painted.len() {
        let token = painted[index];
        // The three modifiers a class-constant access is emitted with, and nothing else is.
        let is_constant_access = token.kind == TokenKind::Property
            && token.modifiers.is_static
            && token.modifiers.is_readonly
            && !token.modifiers.is_declaration;
        if !is_constant_access {
            continue;
        }
        let follows_an_enum = index > 0
            && enums.contains(&painted[index - 1].range)
            && painted[index - 1].range.end() + phrost_model::TextSize::new(2)
                == token.range.start();
        if follows_an_enum {
            painted[index].kind = TokenKind::EnumMember;
        }
    }
}
