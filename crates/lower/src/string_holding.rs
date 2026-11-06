//! What a string the source builds is known to hold, read off the syntax that builds it.
//!
//! `non-empty-string` is a type of its own, and most of the strings a parameter declared that way is
//! handed are built in place: a quoted literal, `"order.{$field}"`, `$prefix . '_id'`. Each says
//! what it holds in its own text -- a literal by its characters, an interpolation by the text around
//! its holes, a `.` by whichever piece holds the most, since joining never empties a string nor
//! shortens one to `'0'`. That is a fact about the syntax and nothing else, so it is read here, where
//! the text is decoded, rather than by a layer that would have to find it again.
//!
//! The answer is a lower bound and always a true one. A piece this cannot read -- a variable, a call
//! -- holds nothing as far as this knows, and the reader that can resolve one is the view, which
//! asks again with types; see `PathRoot::Concatenation`.

use mago_syntax::cst::{CompositeString, Expression, Literal, MagicConstant, StringPart};
use phrost_model::BuiltinTy;

/// What the value of `expr` is known to hold as a piece of a string: one of the three string
/// types, `string` where the syntax says nothing.
///
/// A number counts, as a piece: `.` spells an int or a float in digits, and digits are never empty.
/// A number on its own is not a string at all, which is why this is asked of the operands of a `.`
/// and of a constant's string parts, and never to type a value by itself.
pub(crate) fn piece_holds(expr: &Expression<'_>) -> BuiltinTy {
    match expr {
        Expression::Literal(Literal::String(literal)) => literal
            .value
            .map_or(BuiltinTy::String, BuiltinTy::string_holding),
        Expression::Literal(Literal::Integer(_) | Literal::Float(_)) => BuiltinTy::NonEmptyString,
        Expression::CompositeString(composite) => composite_holds(composite),
        Expression::Binary(binary)
            if matches!(
                binary.operator,
                mago_syntax::cst::BinaryOperator::StringConcat(_)
            ) =>
        {
            piece_holds(binary.lhs).concatenated(piece_holds(binary.rhs))
        }
        Expression::Parenthesized(inner) => piece_holds(inner.expression),
        // A path is never empty and never `'0'`. The other magic constants are empty outside the
        // scope they name -- `__CLASS__` at the top of a file -- and this does not ask where it is.
        Expression::MagicConstant(MagicConstant::File(_) | MagicConstant::Directory(_)) => {
            BuiltinTy::NonFalsyString
        }
        // `Foo::class` is the name of a class, which no empty string and no `'0'` is.
        Expression::Access(mago_syntax::cst::Access::ClassConstant(access))
            if matches!(
                &access.constant,
                mago_syntax::cst::ClassLikeConstantSelector::Identifier(name)
                    if phrost_syntax::identifier_text(name.value).eq_ignore_ascii_case("class")
            ) =>
        {
            BuiltinTy::NonFalsyString
        }
        _ => BuiltinTy::String,
    }
}

/// What an interpolated string, a heredoc or a nowdoc holds: what the text around its holes does.
///
/// Whatever fills a hole, the written text is still there -- `"SELECT * FROM {$t}"` is not empty and
/// not `'0'` -- and text that is exactly `'0'` around holes proves only that something is there. A
/// backtick string runs a command, and what a command prints may be nothing at all.
pub(crate) fn composite_holds(composite: &CompositeString<'_>) -> BuiltinTy {
    let parts = match composite {
        CompositeString::Interpolated(string) => &string.parts,
        CompositeString::Document(document) => &document.parts,
        CompositeString::ShellExecute(_) => return BuiltinTy::String,
    };
    let mut text = Vec::new();
    for part in parts.iter() {
        if let StringPart::Literal(literal) = part {
            // A part mago could not decode is text nobody can read the value of, and is left out
            // rather than guessed at.
            text.extend_from_slice(literal.value.unwrap_or_default());
        }
    }
    BuiltinTy::string_holding(&text)
}
