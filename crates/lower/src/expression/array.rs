//! Array literals and their elements.

use lspkit_arena::IdSlice;
use mago_span::HasSpan as _;
use mago_syntax::cst::{ArrayElement, Expression};
use phrost_hir::{
    ExprId,
    body::{ArrayElementKind, CallableReceiver, ExprData},
};
use phrost_model::{Name, TextRange};

use crate::lowerer::Lowerer;

impl<'p> Lowerer<'_, 'p> {
    /// The key an index is written with, when it is written as a literal.
    ///
    /// Both kinds PHP really keys an array by. Anything else -- a variable, a constant, a
    /// concatenation -- is a slot this layer cannot name, and naming it wrongly would let a claim
    /// about one key answer for another.
    pub(crate) fn literal_key(&mut self, index: &Expression<'p>) -> Option<Name> {
        Some(self.interner.name(&literal_key_text(index)?))
    }

    /// The method a two-element array names, when that is what the array is.
    ///
    /// `[$this, 'refresh']` is how PHP wrote a first-class callable before it had `(...)`, and it
    /// is still how a framework is handed one: a Twig filter, an event subscriber, a `usort`
    /// comparator. Left as a literal, the method name is a *string* -- so `references` on the
    /// method finds nothing, `rename` leaves the string spelling a method that no longer exists,
    /// and goto on it goes nowhere. Measured on one real checkout: 245 written with `$this` and 9
    /// with `Foo::class`, none of them reachable by any of the three.
    ///
    /// Lowered as [`ExprData::CallableMember`], which is the shape PHP 8.1's `(...)` already
    /// lowers to and says exactly the same thing: it names a method, it does not call it. Nothing
    /// downstream needs teaching -- `member_sites` has read that shape since it existed.
    ///
    /// # Two receivers, and only one of them is certain
    ///
    /// `[$value, 'text']` is a callable *and* an ordinary pair of data, and the syntax cannot say
    /// which. `$this` has no such reading: a pair of the object you are inside and a string is a
    /// callable, and nothing on five corpora writes it as data.
    ///
    /// `[Foo::class, 'bar']` looks equally unmistakable and is not, which those corpora said and
    /// I did not believe until they did. It is how a **list of class names** is written when one
    /// is spelled with `::class` and the next is not: Symfony's
    /// `setAllowedTypes('active_at', [\DateTimeInterface::class, 'null'])`, its
    /// `in_array($name, [TranslatableMessage::class, 'TranslatableMessage'])`, its
    /// `'required_badges' => [CsrfTokenBadge::class, 'RememberMeBadge']`. Read as callables those
    /// were seventeen undefined methods on Symfony and ten on Laravel, on classes that never
    /// claimed one -- and the counts alone looked like a feature finding real mistakes.
    ///
    /// So that form is *presumed* rather than settled: kept for the readers a false one costs
    /// nothing -- references and rename match what a name resolves to, and a list of class names
    /// resolves to no method -- and dropped before the readers it would make lie. See
    /// [`ExprData::CallableMember::presumed`]. Deciding it properly needs the parameter the pair
    /// is passed to, which is resolution and not lowering.
    fn callable_pair(
        &mut self,
        elements: &[ArrayElement<'p>],
        range: TextRange,
    ) -> Option<ExprData> {
        let [ArrayElement::Value(first), ArrayElement::Value(second)] = elements else {
            return None;
        };
        let name = match second.value {
            Expression::Literal(mago_syntax::cst::Literal::String(literal)) => {
                String::from_utf8_lossy(literal.value?).into_owned()
            }
            _ => return None,
        };
        // A method name, not a sentence: a two-element array holding a class and the word "and" is
        // data, and the quotes are the only thing the two shapes have in common.
        if name.is_empty()
            || !name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
            || name.as_bytes()[0].is_ascii_digit()
        {
            return None;
        }
        // Past the opening quote, so what a reader lands on and a rename rewrites is the name and
        // not the string around it.
        let quoted = self.within_the_text(second.value.span());
        let name_range = TextRange::new(
            quoted.start() + phrost_model::TextSize::from(1),
            quoted.end().checked_sub(phrost_model::TextSize::from(1))?,
        );

        let (receiver, presumed) = match first.value {
            // `$this` names the class the code is written in, and a pair of it and a string is
            // not something anybody writes as data.
            Expression::Variable(mago_syntax::cst::Variable::Direct(variable))
                if String::from_utf8_lossy(variable.name).as_ref() == "$this" =>
            {
                (CallableReceiver::Value(self.expression(first.value)), false)
            }
            // `Foo::class`, `self::class`, `static::class` -- a class named outright, which is
            // also how the first half of a list of class names is written.
            Expression::Access(mago_syntax::cst::Access::ClassConstant(access)) => {
                let mago_syntax::cst::ClassLikeConstantSelector::Identifier(constant) =
                    &access.constant
                else {
                    return None;
                };
                if !phrost_syntax::identifier_text(constant.value).eq_ignore_ascii_case("class") {
                    return None;
                }
                (
                    CallableReceiver::Class(self.class_target(access.class)),
                    true,
                )
            }
            _ => return None,
        };

        Some(ExprData::CallableMember {
            receiver,
            name: Some(self.interner.name(&name)),
            selector: None,
            name_range,
            range,
            presumed,
        })
    }

    /// An array literal, or the method a two-element one names -- see [`Self::callable_pair`].
    pub(crate) fn array_or_callable(
        &mut self,
        elements: &[ArrayElement<'p>],
        range: TextRange,
    ) -> ExprData {
        if let Some(callable) = self.callable_pair(elements, range) {
            return callable;
        }
        ExprData::ArrayLiteral {
            elements: self.array_elements(elements),
            range,
        }
    }

    /// The elements of an array literal, in written order.
    ///
    /// Element by element rather than through [`Self::nested_expressions`], which is what the two
    /// spellings used to go through: that walk yields every expression written inside the literal as
    /// one flat list, so `['a' => $x]` and `[$x, 'a']` came out the same and nothing downstream could
    /// say which id was a key.
    ///
    /// The same expressions reach the arena, minus the literal keys -- whose text moves onto the
    /// element that used to sit beside them. So the commonest literal in PHP comes out the size it
    /// went in: measured over 41 559 Drupal files, the whole change is 8 736 more expressions in
    /// 20.2 million.
    pub(crate) fn array_elements(&mut self, elements: &[ArrayElement<'p>]) -> IdSlice<ExprId> {
        let ids = elements
            .iter()
            .filter_map(|element| self.array_element(element))
            .collect::<Vec<_>>();
        self.builder.alloc_expr_list(&ids)
    }

    /// The elements of a *destructuring target*, each carrying the slot it takes.
    ///
    /// `[$first, , $third] = $row` binds `$third` to `$row[2]`, and what says so is counting -- holes
    /// included, and a hole lowers to nothing at all. So a target's positional elements carry their
    /// index the way a keyed element carries its key, which is the same fact said the same way, and
    /// every reader of a keyed element knows them for free.
    ///
    /// Only a target. Keying the elements of a *value* literal would be a wrapper node per element in
    /// every list in the project, which is the memory [`ArrayElementKind::LiteralKey`] exists to
    /// avoid -- and a value literal has no holes, because `[1, , 3]` is not a thing PHP parses
    /// outside an assignment.
    ///
    /// PHP refuses to mix keyed and unkeyed entries in an assignment, so an element written with a
    /// key keeps it and nothing beside it needs an index.
    pub(crate) fn destructuring_elements(
        &mut self,
        elements: &[ArrayElement<'p>],
    ) -> IdSlice<ExprId> {
        let mut ids = Vec::with_capacity(elements.len());
        for (slot, element) in elements.iter().enumerate() {
            match element {
                ArrayElement::Value(value) => {
                    let range = self.within_the_text(value.span());
                    let name = self.interner.name(&slot.to_string());
                    // Through the target lowering again, because a nested target destructures too:
                    // `[[$a, $b], $c] = $rows` binds `$a` to `$rows[0][0]`, and the inner literal
                    // has to carry its own slots for that to be readable.
                    let value = self.destructuring_target(value.value);
                    ids.push(self.builder.add_expr(ExprData::ArrayElement {
                        kind: ArrayElementKind::LiteralKey { name, range },
                        value,
                        range,
                    }));
                }
                // A hole takes its slot and binds nothing, which is the whole reason the index has
                // to be written down rather than counted by a reader.
                ArrayElement::Missing(_) => {}
                keyed => ids.extend(self.array_element(keyed)),
            }
        }
        self.builder.alloc_expr_list(&ids)
    }

    /// Lowers one element, wrapping it only when it says more than the value it holds.
    ///
    /// The same rule as [`Self::argument`], and for the same reason: a bare `[$a, $b]` element *is*
    /// its expression, and a wrapper around every one of them would be a node per element in every
    /// list literal in the project for nothing a reader asks.
    fn array_element(&mut self, element: &ArrayElement<'p>) -> Option<ExprId> {
        match element {
            ArrayElement::Value(element) => Some(self.expression(element.value)),
            ArrayElement::KeyValue(pair) => {
                let range = self.within_the_text(pair.span());
                let kind = match self.literal_key(pair.key) {
                    // The text is the whole of what a literal key says, so it goes on the element
                    // and the key gets no node -- which is what keeps the wrapper free.
                    Some(name) => ArrayElementKind::LiteralKey {
                        name,
                        range: self.within_the_text(pair.key.span()),
                    },
                    None => ArrayElementKind::ComputedKey {
                        key: self.expression(pair.key),
                    },
                };
                let value = self.expression(pair.value);
                Some(
                    self.builder
                        .add_expr(ExprData::ArrayElement { kind, value, range }),
                )
            }
            ArrayElement::Variadic(spread) => {
                let range = self.within_the_text(spread.span());
                let value = self.expression(spread.value);
                Some(self.builder.add_expr(ExprData::ArrayElement {
                    kind: ArrayElementKind::Spread,
                    value,
                    range,
                }))
            }
            // `[1, , 3]` -- a stray comma, which is a span and holds nothing to lower.
            ArrayElement::Missing(_) => None,
        }
    }
}

/// The text of a key written as a literal -- see [`Lowerer::literal_key`], which interns it. Apart so
/// that a reader holding a tree of another lifetime, a constant's initialiser, reads keys the same way.
pub(crate) fn literal_key_text(index: &Expression<'_>) -> Option<String> {
    match index {
        Expression::Literal(mago_syntax::cst::Literal::String(literal)) => {
            Some(String::from_utf8_lossy(literal.value?).into_owned())
        }
        Expression::Literal(mago_syntax::cst::Literal::Integer(literal)) => {
            Some(literal.value?.to_string())
        }
        _ => None,
    }
}
