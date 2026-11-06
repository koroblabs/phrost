//! The literal values a conditional-return-type or catalog-lookup check reads out of an
//! argument list.

use mago_syntax::cst::{Argument, ArgumentList, CompositeString, Expression, StringPart};
use phrost_model::Name;
use phrost_syntax::identifier_text;

use crate::lowerer::Lowerer;

impl<'p> Lowerer<'_, 'p> {
    /// The first argument's value, when it is written as a plain string literal.
    ///
    /// mago decodes the string for us, so escapes are already resolved and `value` is `None` for one
    /// it could not decode -- an unterminated string being typed, most of all, where guessing a
    /// value would type the expression from half a name.
    ///
    /// A named or spread first argument is not a first argument: `service(id: 'x')` puts the id
    /// somewhere this does not look, and answering as if it were positional would be a guess.
    pub(crate) fn literal_arguments(
        &mut self,
        list: &ArgumentList<'p>,
    ) -> Box<[phrost_hir::body::LiteralArgument]> {
        // Every argument the type of a call may depend on, each with where it was written. All of
        // them and not the first, because which one decides is the callee's business: a catalog
        // contract means the first argument and a `class-string<T>` parameter can be anywhere, and
        // `bar('name', Repo::class)` has one of each.
        let mut found = Vec::new();
        for (position, argument) in list.arguments.iter().enumerate() {
            // A named argument fills whichever parameter it names, and which one that is takes the
            // callee's signature to say -- so the name is carried and the reader matches it. A
            // spread fills several and names none, which is nothing this can read.
            let (name, written) = match argument {
                Argument::Positional(written) if written.ellipsis.is_none() => {
                    (None, written.value)
                }
                Argument::Named(written) => (
                    Some(self.interner.name(&identifier_text(written.name.value))),
                    written.value,
                ),
                Argument::Positional(_) => continue,
            };
            let Ok(position) = u16::try_from(position) else {
                break;
            };
            if let Some((value, kind)) = self.literal_value(written) {
                found.push(phrost_hir::body::LiteralArgument {
                    position,
                    name,
                    value,
                    kind,
                });
            }
        }
        found.into_boxed_slice()
    }

    /// A string an argument is, when it is one at all.
    fn literal_value(
        &mut self,
        value: &Expression<'p>,
    ) -> Option<(Name, phrost_hir::body::ArgumentLiteral)> {
        match value {
            Expression::Literal(mago_syntax::cst::Literal::String(literal)) => Some((
                self.interner.name(&String::from_utf8_lossy(literal.value?)),
                phrost_hir::body::ArgumentLiteral::Text,
            )),
            // An integer, as the digits: `pathinfo($path, 4)`. A conditional return type spells the
            // same thing `($flags is 4 ? ... )`, and comparing the two is the reader's business.
            Expression::Literal(mago_syntax::cst::Literal::Integer(literal)) => Some((
                self.interner.name(small_number(literal.value?)?),
                phrost_hir::body::ArgumentLiteral::Number,
            )),
            // `null`, `true`, `false` -- the only three keywords whose value a conditional return
            // type is ever written about. Kept apart from a string by its kind, because `f('null')`
            // is not `f(null)`.
            Expression::Literal(literal) => keyword_of(literal).map(|keyword| {
                (
                    self.interner.name(keyword),
                    phrost_hir::body::ArgumentLiteral::Keyword,
                )
            }),
            // `Foo::class` -- the class the argument names, which is what a factory reads back.
            //
            // The same slot as a string literal, because that is what it *is* at runtime and the two
            // uses cannot collide: a catalog has no key spelled like a class, and a class name is
            // not a service id. Kept as the name the author wrote, so the reader resolves it against
            // the imports of the file it was written in.
            Expression::Access(mago_syntax::cst::Access::ClassConstant(access)) => {
                let mago_syntax::cst::ClassLikeConstantSelector::Identifier(constant) =
                    &access.constant
                else {
                    return None;
                };
                if !identifier_text(constant.value).eq_ignore_ascii_case("class") {
                    return None;
                }
                let Expression::Identifier(named) = access.class else {
                    return None;
                };
                Some((
                    self.interner.name(&identifier_text(named.value())),
                    phrost_hir::body::ArgumentLiteral::Text,
                ))
            }
            // A global constant: `pathinfo($path, PATHINFO_EXTENSION)`, which is how a flag is
            // written and never as the number it stands for. The name travels and the reader looks
            // the value up, for the reason [`ArgumentLiteral::Constant`] gives.
            Expression::ConstantAccess(access) => Some((
                self.interner.name(&identifier_text(access.name.value())),
                phrost_hir::body::ArgumentLiteral::Constant,
            )),
            // `"-$i months"` -- kept as a skeleton, because the text *around* the holes is a fact
            // about the call that nothing else carries: `strtotime()` cannot fail for any integer
            // `$i` there, and only the string says so. The holes are left where every other nested
            // expression goes, since what one holds takes types to say.
            //
            // The double-quoted spelling only. A heredoc means the same thing and arrives with its
            // indentation still on, a backtick string runs a command, and neither is where anybody
            // writes a date.
            Expression::CompositeString(CompositeString::Interpolated(string)) => {
                interpolation_skeleton(string.parts.iter()).map(|skeleton| {
                    (
                        self.interner.name(&skeleton),
                        phrost_hir::body::ArgumentLiteral::Interpolated,
                    )
                })
            }
            _ => None,
        }
    }
}

/// An interpolated string as its literal text, with one hole marker per expression in it.
///
/// `None` twice over. For a string with nothing interpolated into it, which arrives as a plain
/// literal anyway and which a reader of a skeleton has nothing to do with. And for one mago could
/// not decode -- a string being typed, most of all -- for the reason [`Lowerer::literal_value`]
/// gives about guessing a value from half a name: half a skeleton describes a string nobody wrote.
fn interpolation_skeleton<'a>(parts: impl Iterator<Item = &'a StringPart<'a>>) -> Option<String> {
    let mut skeleton = String::new();
    let mut holes = 0_usize;
    for part in parts {
        match part {
            StringPart::Literal(literal) => {
                skeleton.push_str(&String::from_utf8_lossy(literal.value?));
            }
            StringPart::Expression(_) | StringPart::BracedExpression(_) => {
                skeleton.push(phrost_hir::body::INTERPOLATION_HOLE);
                holes += 1;
            }
        }
    }
    (holes > 0).then_some(skeleton)
}

/// The digits of a small number, without allocating any.
///
/// Only the small ones, and the reason is a measurement rather than a taste: recording *every*
/// integer argument cost `phrost check` 4 % on a real project -- one string allocation and one
/// interner lookup for the second argument of every `substr`, `round` and `str_pad` in it -- and
/// what reads them is a conditional return type testing a flag. A flag is a small number; a
/// declaration that turns on `4096` is not something anybody writes.
fn small_number(value: u64) -> Option<&'static str> {
    const WRITTEN: [&str; 16] = [
        "0", "1", "2", "3", "4", "5", "6", "7", "8", "9", "10", "11", "12", "13", "14", "15",
    ];

    WRITTEN.get(usize::try_from(value).ok()?).copied()
}

/// The keyword a literal is, for the three whose identity a conditional return type reads.
pub(crate) fn keyword_of(literal: &mago_syntax::cst::Literal<'_>) -> Option<&'static str> {
    match literal {
        mago_syntax::cst::Literal::Null(_) => Some("null"),
        mago_syntax::cst::Literal::True(_) => Some("true"),
        mago_syntax::cst::Literal::False(_) => Some("false"),
        _ => None,
    }
}
