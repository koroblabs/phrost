//! `define('NAME', ...)`, which declares a constant that no `const` statement mentions.
//!
//! PHP's older spelling, and the one WordPress is written in: 636 calls against 560 `const`
//! declarations on that checkout, where Laravel has two and Symfony seven. Without this a goto on
//! `ABSPATH` answers nothing, and so does every question after it.
//!
//! # The name is the string, whole
//!
//! Asked of PHP 8.5 rather than assumed, because the rule is the opposite of the one `const`
//! follows: `namespace App; define('MAX', 1);` declares the **global** `MAX`, and `defined('App\MAX')`
//! is false. A name with a separator in it declares exactly that name. So the constant carries no
//! namespace at all and the string is its fully-qualified name.
//!
//! # Only a literal
//!
//! `define($name, $value)` declares something this cannot know, and a guess there is a name the
//! index holds and the code does not.

use mago_span::HasSpan;
use mago_syntax::cst::{Argument, Call, Expression, ExpressionStatement};
use phrost_hir::ConstantData;
use phrost_model::{TextRange, TextSize};

use crate::lowerer::{Lowerer, string_literal};

impl<'p> Lowerer<'_, 'p> {
    /// Records the constant a `define('NAME', ...)` statement declares, when that is what it is.
    pub(crate) fn defined_constant(&mut self, statement: &ExpressionStatement<'p>) {
        let Expression::Call(Call::Function(call)) = statement.expression else {
            return;
        };
        let Expression::Identifier(identifier) = call.function else {
            return;
        };
        let written = phrost_syntax::identifier_text(identifier.value());
        if !written
            .trim_start_matches('\\')
            .eq_ignore_ascii_case("define")
        {
            return;
        }
        // Two arguments, the first positional and a literal string. A spread names nothing this can
        // read, and `define()` with one argument does not run.
        let [Argument::Positional(first), rest @ ..] = call.argument_list.arguments.as_slice()
        else {
            return;
        };
        if first.ellipsis.is_some() || rest.is_empty() {
            return;
        }
        let Some(name) = string_literal(first.value) else {
            return;
        };
        let name = name.trim_start_matches('\\');
        if name.is_empty() {
            return;
        }

        // A `define` carries no attribute list and no modifier, so its declaration starts where
        // the statement does -- and a docblock above it is the one a reader wrote for the constant.
        let full_range = self.within_the_text(statement.span());
        let docblock = self.docblock(full_range.start());
        let is_ignored = self.doc_is_ignored(full_range.start());
        // A `define` takes no attribute list, so the docblock is the whole of what can say it.
        let is_deprecated = self.doc_names_tag(full_range.start(), "deprecated");
        let name = self.interner.name(name);
        // The value is the second argument, positional or not -- `define('X', value: 1)` is legal.
        let written = match rest.first() {
            Some(Argument::Positional(value)) if value.ellipsis.is_none() => Some(value.value),
            Some(Argument::Named(value)) => Some(value.value),
            _ => None,
        };
        let ty = written
            .and_then(|value| self.constant_value_type(value))
            // And a `@var` above the call where the value names nothing, as for `const`.
            .or_else(|| self.doc_type(full_range.start(), crate::docblock::DocTag::Var));
        let value = match ty {
            Some(_) => None,
            None => written.and_then(|value| self.constant_value_parts(value)),
        };
        self.builder.add_constant(ConstantData {
            name,
            // None, and that is the rule rather than a simplification -- see the module docs.
            namespace: None,
            name_range: inside_the_quotes(self.within_the_text(first.value.span())),
            full_range,
            docblock,
            // A `define` inside `if (!defined('X'))` is how a project ships a fallback, and the
            // walk that reached this statement has already said so.
            is_conditional: self.conditional,
            is_ignored,
            is_deprecated,
            ty,
            value,
        });
    }
}

/// The span of a quoted string without its quotes, which is where the name a reader clicks is.
///
/// Lexical rather than measured against the decoded text: a constant's name holds no escape, and a
/// range one character wide of the truth would only ever cover a quote -- which `rename_range`
/// drops anyway, since it takes the last segment of what is written.
fn inside_the_quotes(span: TextRange) -> TextRange {
    let one = TextSize::from(1);
    match span.len() > TextSize::from(2) {
        true => TextRange::new(span.start() + one, span.end() - one),
        false => span,
    }
}
