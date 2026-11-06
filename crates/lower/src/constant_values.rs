//! The type a class constant's initialiser gives it, when the declaration writes none.
//!
//! PHP only let a class constant carry a type in 8.3, so a written hint is the rare case: almost
//! every constant in the standard library, in `vendor/`, and in a framework's own interfaces is
//! `const PUBLISHED = 1;` with nothing beside it. Reading the value is what makes
//! `NodeInterface::PUBLISHED` an `int` rather than nothing at all.
//!
//! This belongs in lowering rather than in inference because it is not inference. A constant's
//! value is written beside its name, in the same declaration, and cannot change -- so it is the same
//! kind of fact as a hint, arriving through a different piece of syntax.

use mago_span::HasSpan;
use mago_syntax::cst::{ArrayElement, BinaryOperator, Expression, Literal, UnaryPrefixOperator};
use phrost_hir::{ConstantValueData, TypeRefData, TypeRefId, body::ArithmeticOp};
use phrost_model::BuiltinTy;

use crate::lowerer::Lowerer;

impl Lowerer<'_, '_> {
    /// The builtin a constant's value is, or `None` to leave the declaration untyped.
    ///
    /// Only the shapes whose result type is settled by the **syntax alone**, with nothing known
    /// about what any operand holds. That is a deliberately short list, because the alternative is
    /// a wrong type -- and a wrong type on a constant thousands of call sites pass around is worse
    /// than no type, which is what these declarations had before this existed.
    ///
    /// What is refused, and why, since the near misses all look decidable:
    ///
    /// - **Arithmetic.** `1 + 2` is an `int`, `1 + 1.5` is a `float`, and `[1] + [2]` is an
    ///   `array`. `/` gives an `int` or a `float` depending on whether the division is exact.
    /// - **Bitwise `&`, `|`, `^`.** An `int` for integer operands and a `string` for string ones,
    ///   which is exactly what an operand naming another constant does not say.
    /// - **A constant naming another constant**, a `new` in an initialiser, an enum case, a magic
    ///   constant. Each has a type, and finding it means resolving a name -- which is the def-map's
    ///   job and not something a lowerer can do.
    ///
    /// A refusal costs nothing: the declaration stays untyped, which is where it was.
    pub(crate) fn constant_value_type(&mut self, value: &Expression<'_>) -> Option<TypeRefId> {
        if let Some(array) = self.constant_array_type(value) {
            return Some(array);
        }
        let builtin = Self::builtin_of_value(value)?;
        Some(self.builder.add_type_ref(TypeRefData::Builtin(builtin)))
    }

    /// What a typed constant's value says beyond its written hint, where it says more.
    ///
    /// Read as a refinement of the hint, the way a docblock refines a native one: the hint stays the
    /// declaration's, and what is read is the narrower type. Two hints a value refines:
    ///
    /// - a bare `array`, by the table it is: `const array TYPES = ['order' => Order::class]`;
    /// - `string`, by which strings the text rules out: `const string CERT = __DIR__ . '/a.crt'`
    ///   is a `non-falsy-string`, which is what handing it to a `non-empty-string` parameter asks.
    ///   PHPStan reads the value, and without this the hint made every such call a finding.
    ///
    /// The second only where nothing can redeclare the constant -- `overridable` false, for a
    /// private or final one or one in a final class or an enum. Elsewhere the hint is the contract a
    /// subclass keeps and the value is only this class's: `$resource::STATUS` reaches whichever
    /// class the object is, and a subclass writing `const string STATUS = ''` is still a `string`.
    /// The reader does not tell `self::X` from `static::X`, so the declaration decides for both.
    ///
    /// Not `bool` by a `true` or `false` value. A flag constant is written to be flipped by editing
    /// it, and typing it as its value makes every branch on it one-sided -- which is the complaint
    /// PHPStan's own "always true" reports on such constants draw. `int` has nothing narrower here to
    /// be refined to.
    pub(crate) fn constant_hint_refined(
        &mut self,
        hint: TypeRefId,
        value: &Expression<'_>,
        overridable: bool,
    ) -> Option<TypeRefId> {
        let TypeRefData::Builtin(written) = *self.builder.type_ref(hint) else {
            return None;
        };
        let refinable = match written {
            BuiltinTy::Array => true,
            BuiltinTy::String => !overridable,
            _ => false,
        };
        if !refinable {
            return None;
        }
        let refined = self.constant_value_type(value)?;
        let refines = match (written, self.builder.type_ref(refined)) {
            // A builtin is no more than the hint already says: `array` again.
            (BuiltinTy::Array, TypeRefData::Builtin(_)) => false,
            (BuiltinTy::Array, _) => true,
            (BuiltinTy::String, TypeRefData::Builtin(value)) => {
                matches!(value, BuiltinTy::NonEmptyString | BuiltinTy::NonFalsyString)
            }
            _ => false,
        };
        refines.then_some(refined)
    }

    /// An array literal whose every value is one builtin: `array<K, V>`, or `V[]` where the keys
    /// are not all of one kind.
    ///
    /// `const SIZES = ['thumb' => 200, 'full' => 1200]` is how a lookup table is written, and
    /// `self::SIZES[$size]` is read far more often than the table is: 125 such reads and 23
    /// `foreach`es on the reference project, every one of them untyped while the constant said only
    /// `array`. The rule is the syntax's own, as for the scalars: a value this cannot settle -- a
    /// constant naming another, a nested literal of mixed kinds, a spread -- leaves the whole
    /// literal the plain `array` it was, rather than a union guessed operand by operand. Values that
    /// are each settled but of different kinds are no one element at all, and are the shape of what
    /// each key holds -- see [`Self::constant_shape_type`].
    fn constant_array_type(&mut self, value: &Expression<'_>) -> Option<TypeRefId> {
        let elements = match value {
            Expression::Array(array) => &array.elements,
            Expression::LegacyArray(array) => &array.elements,
            Expression::Parenthesized(inner) => return self.constant_array_type(inner.expression),
            _ => return None,
        };
        let mut element: Option<BuiltinTy> = None;
        // The classes a table of `Foo::class` names, in written order and without repeats.
        let mut classes: Vec<(String, phrost_hir::NameRefId)> = Vec::new();
        // `None` once two keys disagree, which a `V[]` states nothing about.
        let mut key: Option<Option<BuiltinTy>> = None;
        for one in elements.iter() {
            let (this_key, this_value) = match one {
                ArrayElement::Value(value) => (Some(BuiltinTy::Int), value.value),
                ArrayElement::KeyValue(pair) => (Self::key_kind(pair.key), pair.value),
                ArrayElement::Variadic(_) | ArrayElement::Missing(_) => return None,
            };
            // A class named with `::class` is a `class-string` of it, and a table of them is how
            // PHP writes a factory's map: `const TYPES = ['order' => OrderApplication::class, ...]`
            // and then `new (self::TYPES[$kind])`. A table mixing names with anything else is
            // neither, and says nothing.
            if let Some(named) = Self::class_named(this_value) {
                if element.is_some() {
                    return None;
                }
                let written = String::from_utf8_lossy(named.value()).to_ascii_lowercase();
                if !classes.iter().any(|(seen, _)| *seen == written) {
                    let id = self.name_ref(named);
                    classes.push((written, id));
                }
            } else {
                if !classes.is_empty() {
                    return None;
                }
                // `true` and `false` are both a bool, which is what a table of flags holds.
                let this_value = match Self::builtin_of_value(this_value)? {
                    BuiltinTy::True | BuiltinTy::False => BuiltinTy::Bool,
                    BuiltinTy::Null => return None,
                    other => other,
                };
                // Two strings are one element type, holding what both do -- the fold `true` and
                // `false` get above, for the other refinement of a builtin.
                let this_value = match element.and_then(|seen| seen.string_join(this_value)) {
                    Some(joined) => {
                        element = Some(joined);
                        joined
                    }
                    None => this_value,
                };
                // Values of different kinds are no one element type, but every key is still
                // written out -- which is what a shape states.
                if element.is_some_and(|seen| seen != this_value) {
                    return self.constant_shape_type(&elements.iter().collect::<Vec<_>>());
                }
                element = Some(this_value);
            }
            key = Some(match key {
                None => this_key,
                Some(seen) if seen == this_key => seen,
                // Two string keys are string keys, holding what both do.
                Some(Some(seen)) => this_key.and_then(|this_key| seen.string_join(this_key)),
                Some(None) => None,
            });
        }
        let element = match classes.as_slice() {
            [] => self.builder.add_type_ref(TypeRefData::Builtin(element?)),
            named => {
                let names = named
                    .iter()
                    .map(|(_, id)| self.builder.add_type_ref(TypeRefData::Named(*id)))
                    .collect::<Vec<_>>();
                let argument = match names.as_slice() {
                    [one] => *one,
                    many => {
                        let list = self.builder.alloc_type_ref_list(many);
                        self.builder.add_type_ref(TypeRefData::Union(list))
                    }
                };
                let string = self
                    .builder
                    .add_type_ref(TypeRefData::Builtin(BuiltinTy::String));
                let args = self.builder.alloc_type_ref_list(&[argument]);
                self.builder.add_type_ref(TypeRefData::Generic {
                    base: string,
                    args,
                    non_empty: false,
                })
            }
        };
        let base = self
            .builder
            .add_type_ref(TypeRefData::Builtin(BuiltinTy::Array));
        let arguments = match key.flatten() {
            Some(key) => {
                let key = self.builder.add_type_ref(TypeRefData::Builtin(key));
                self.builder.alloc_type_ref_list(&[key, element])
            }
            None => self.builder.alloc_type_ref_list(&[element]),
        };
        Some(self.builder.add_type_ref(TypeRefData::Generic {
            base,
            args: arguments,
            non_empty: false,
        }))
    }

    /// A table whose values differ in kind, as the sealed shape of what each key holds:
    /// `['immutable' => 1, 'mutable' => 'two']` is `array{immutable: int, mutable: string}`.
    ///
    /// No single element type can say that, and the plain `array` it was says nothing -- while the
    /// one thing such a table is for is being read by key: `ID_TABLE['mutable']`, and the
    /// `@return ID_TABLE[T]` a lookup function declares, are a `string` and not an `int|string`.
    /// Sealed, because a constant is the whole statement of what it holds and cannot change.
    ///
    /// Every key a literal and every value one the syntax settles, by the rule the uniform table
    /// above follows; positional entries only in a list, where their keys are `0` upwards. Anything
    /// else -- a computed key, a repeat, a value naming a class or another constant -- leaves the
    /// table the plain `array`. And short: a shape is a field per entry kept for every declaration,
    /// and a table this long is a dataset rather than a lookup anybody reads by a literal key.
    fn constant_shape_type(&mut self, elements: &[&ArrayElement<'_>]) -> Option<TypeRefId> {
        const SHAPE_ENTRIES: usize = 32;
        if elements.len() > SHAPE_ENTRIES {
            return None;
        }
        let list = elements
            .iter()
            .all(|one| matches!(one, ArrayElement::Value(_)));
        let mut keys: Vec<phrost_model::Name> = Vec::with_capacity(elements.len());
        let mut fields = Vec::with_capacity(elements.len());
        for (position, one) in elements.iter().enumerate() {
            // A key PHP stores as written: an integer's digits or a string's text, which for a
            // canonical decimal is that integer's -- one key either way.
            let (key, value) = match one {
                ArrayElement::Value(value) if list => {
                    (self.interner.name(&position.to_string()), value.value)
                }
                ArrayElement::KeyValue(pair) => (
                    self.interner
                        .name(&crate::expression::array::literal_key_text(pair.key)?),
                    pair.value,
                ),
                _ => return None,
            };
            if keys.contains(&key) {
                return None;
            }
            // A null refuses the table here as it does above, so the answer does not turn on
            // whether the null was written before the first value of another kind or after it.
            let builtin = match Self::builtin_of_value(value)? {
                BuiltinTy::True | BuiltinTy::False => BuiltinTy::Bool,
                BuiltinTy::Null => return None,
                other => other,
            };
            let ty = self.builder.add_type_ref(TypeRefData::Builtin(builtin));
            fields.push(self.builder.add_type_ref(TypeRefData::ShapeField {
                key: key.clone(),
                optional: false,
                ty,
            }));
            keys.push(key);
        }
        let fields = self.builder.alloc_type_ref_list(&fields);
        Some(self.builder.add_type_ref(TypeRefData::Shape {
            fields,
            open: false,
            list,
        }))
    }

    /// The class a `Foo::class` names, when that is what the value is -- a name written out, since
    /// `self::class` and `static::class` mean the class the constant is reached through.
    fn class_named<'v>(value: &'v Expression<'_>) -> Option<&'v mago_syntax::cst::Identifier<'v>> {
        let Expression::Access(mago_syntax::cst::Access::ClassConstant(access)) = value else {
            return None;
        };
        let mago_syntax::cst::ClassLikeConstantSelector::Identifier(constant) = &access.constant
        else {
            return None;
        };
        if !phrost_syntax::identifier_text(constant.value).eq_ignore_ascii_case("class") {
            return None;
        }
        match access.class {
            Expression::Identifier(identifier) => Some(identifier),
            _ => None,
        }
    }

    /// What kind a literal key is once PHP has stored it.
    ///
    /// A string that spells a canonical decimal integer is stored as that integer -- `['1' => x]`
    /// has the key `1` -- and `'01'` is not canonical and stays a string. Anything but a literal
    /// is a key this cannot name.
    fn key_kind(key: &Expression<'_>) -> Option<BuiltinTy> {
        match key {
            Expression::Literal(Literal::Integer(_)) => Some(BuiltinTy::Int),
            Expression::Literal(Literal::String(literal)) => {
                let text = literal.value.map(String::from_utf8_lossy)?;
                let canonical = text
                    .parse::<i64>()
                    .is_ok_and(|number| number.to_string() == text);
                Some(match canonical {
                    true => BuiltinTy::Int,
                    false => BuiltinTy::string_holding(text.as_bytes()),
                })
            }
            _ => None,
        }
    }

    /// Recursive so that the wrappers -- parentheses, a sign -- reach the value inside them.
    fn builtin_of_value(value: &Expression<'_>) -> Option<BuiltinTy> {
        match value {
            Expression::Literal(literal) => Some(match literal {
                // Which string is known from the text -- see `string_holding`.
                Literal::String(_) => crate::string_holding::piece_holds(value),
                Literal::Integer(_) => BuiltinTy::Int,
                Literal::Float(_) => BuiltinTy::Float,
                Literal::True(_) => BuiltinTy::True,
                Literal::False(_) => BuiltinTy::False,
                Literal::Null(_) => BuiltinTy::Null,
            }),
            // A heredoc, a nowdoc, or a double-quoted string mago kept in parts. Interpolation
            // produces a string whatever it interpolates, and the text around the holes says what
            // it holds.
            Expression::CompositeString(composite) => {
                Some(crate::string_holding::composite_holds(composite))
            }
            // `['a', 'b']` and `array('a')`. What it holds is a separate question this does not
            // answer; that it is an array is not in doubt.
            Expression::Array(_) | Expression::LegacyArray(_) => Some(BuiltinTy::Array),
            // A parenthesised value is the value.
            Expression::Parenthesized(inner) => Self::builtin_of_value(inner.expression),
            Expression::UnaryPrefix(unary) => Self::of_unary(unary.operator.clone(), unary.operand),
            // `.` holds what its pieces do, which the pieces' own syntax says.
            Expression::Binary(binary)
                if matches!(binary.operator, BinaryOperator::StringConcat(_)) =>
            {
                Some(crate::string_holding::piece_holds(value))
            }
            Expression::Binary(binary) => Self::of_binary(&binary.operator),
            _ => None,
        }
    }

    /// A sign in front of a number, which is how `const LIMIT = -1;` is written -- mago parses the
    /// minus as an operator rather than folding it into the literal.
    ///
    /// The operand has to be a number already. `-$x` cannot appear in a constant initialiser, but
    /// `-self::A` can, and unary minus on a *string* yields an `int` or a `float` depending on what
    /// the string spells -- so an operand this cannot read leaves the whole thing unread. Every
    /// other prefix operator is refused: `~` is an `int` for integers and a `string` for strings,
    /// and the casts are so rare in an initialiser that a branch for them would be untested code.
    fn of_unary(operator: UnaryPrefixOperator<'_>, operand: &Expression<'_>) -> Option<BuiltinTy> {
        if !matches!(
            operator,
            UnaryPrefixOperator::Negation(_) | UnaryPrefixOperator::Plus(_)
        ) {
            return None;
        }
        match Self::builtin_of_value(operand)? {
            numeric @ (BuiltinTy::Int | BuiltinTy::Float) => Some(numeric),
            _ => None,
        }
    }

    /// What a value [`Self::constant_value_type`] refuses is built from, when every part of it is
    /// something this can carry: another constant, a part the syntax settles, and the arithmetic and
    /// bitwise operators joining them. See [`ConstantValueData`].
    ///
    /// Refused whole at the first part it cannot carry -- a call, a variable, a class constant -- so
    /// what is kept is never half a value.
    pub(crate) fn constant_value_parts(
        &mut self,
        value: &Expression<'_>,
    ) -> Option<ConstantValueData> {
        if let Some(settled) = self.constant_value_type(value) {
            return Some(ConstantValueData::Settled(settled));
        }
        let joined = |operator: Option<ArithmeticOp>, left, right| match operator {
            Some(operator) => ConstantValueData::Arithmetic {
                operator,
                operands: Box::new([left, right]),
            },
            None => ConstantValueData::Bitwise(Box::new([left, right])),
        };
        match value {
            Expression::Parenthesized(inner) => self.constant_value_parts(inner.expression),
            Expression::ConstantAccess(access) => Some(self.named_constant(&access.name)),
            Expression::Identifier(identifier) => Some(self.named_constant(identifier)),
            Expression::UnaryPrefix(unary) => {
                let operator = match unary.operator {
                    UnaryPrefixOperator::Negation(_) => Some(ArithmeticOp::Subtract),
                    UnaryPrefixOperator::Plus(_) => Some(ArithmeticOp::Add),
                    UnaryPrefixOperator::BitwiseNot(_) => None,
                    _ => return None,
                };
                let operand = self.constant_value_parts(unary.operand)?;
                let left = match operator {
                    Some(_) => ConstantValueData::Settled(
                        self.builder
                            .add_type_ref(TypeRefData::Builtin(BuiltinTy::Int)),
                    ),
                    None => operand.clone(),
                };
                Some(joined(operator, left, operand))
            }
            Expression::Binary(binary) => {
                let operator = match binary.operator {
                    BinaryOperator::Addition(_) => Some(ArithmeticOp::Add),
                    BinaryOperator::Subtraction(_) => Some(ArithmeticOp::Subtract),
                    BinaryOperator::Multiplication(_) => Some(ArithmeticOp::Multiply),
                    BinaryOperator::Division(_) => Some(ArithmeticOp::Divide),
                    BinaryOperator::Exponentiation(_) => Some(ArithmeticOp::Power),
                    BinaryOperator::BitwiseAnd(_)
                    | BinaryOperator::BitwiseOr(_)
                    | BinaryOperator::BitwiseXor(_) => None,
                    _ => return None,
                };
                let left = self.constant_value_parts(binary.lhs)?;
                let right = self.constant_value_parts(binary.rhs)?;
                Some(joined(operator, left, right))
            }
            _ => None,
        }
    }

    fn named_constant(
        &mut self,
        identifier: &mago_syntax::cst::Identifier<'_>,
    ) -> ConstantValueData {
        let (anchor, path) = self.written_name(identifier);
        ConstantValueData::Constant {
            path,
            anchor,
            at: self.within_the_text(identifier.span()).start(),
        }
    }

    /// The operators whose result type does not depend on either operand.
    ///
    /// `.` is PHP's string concatenation and produces a string however it is fed -- an operand it
    /// cannot concatenate is a `TypeError` rather than another type. A comparison and the logical
    /// connectives are a `bool` by definition, `<=>` an `int`, and a shift an `int` because PHP 8
    /// refuses to shift anything else. Everything else depends on operands and is refused; see the
    /// list on [`Self::constant_value_type`].
    fn of_binary(operator: &BinaryOperator<'_>) -> Option<BuiltinTy> {
        match operator {
            BinaryOperator::StringConcat(_) => Some(BuiltinTy::String),
            BinaryOperator::Equal(_)
            | BinaryOperator::NotEqual(_)
            | BinaryOperator::Identical(_)
            | BinaryOperator::NotIdentical(_)
            | BinaryOperator::AngledNotEqual(_)
            | BinaryOperator::LessThan(_)
            | BinaryOperator::LessThanOrEqual(_)
            | BinaryOperator::GreaterThan(_)
            | BinaryOperator::GreaterThanOrEqual(_)
            | BinaryOperator::And(_)
            | BinaryOperator::Or(_)
            | BinaryOperator::LowAnd(_)
            | BinaryOperator::LowOr(_)
            | BinaryOperator::LowXor(_)
            | BinaryOperator::Instanceof(_) => Some(BuiltinTy::Bool),
            BinaryOperator::Spaceship(_) => Some(BuiltinTy::Int),
            // `%` reads both sides as ints, as a shift does, and refuses what it cannot.
            BinaryOperator::LeftShift(_)
            | BinaryOperator::RightShift(_)
            | BinaryOperator::Modulo(_) => Some(BuiltinTy::Int),
            _ => None,
        }
    }
}
