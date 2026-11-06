//! The objects a named method builds to declare entries: Twig's `new TwigFilter('upper', ...)`.
//!
//! Read from the syntax tree directly, because the bodies these are in are mostly a dependency's,
//! which are not lowered: Twig declares its own filters in `CoreExtension::getFilters()` under
//! `vendor/`, Drupal its `t` in core. So only the methods a rule names are walked, and only for
//! constructions of the classes it names -- one comparison per method in every other file.

use mago_span::HasSpan as _;
use mago_syntax::cst::{Argument, ArrayElement, Block, Call, Expression, Statement, Variable};
use phrost_hir::{ClassId, KeptCallableData, KeptConstructionData};

use crate::lowerer::{Lowerer, string_literal};

impl<'p> Lowerer<'_, 'p> {
    /// Keeps every construction of a kept class the body of a method in `class` writes where its
    /// result is returned or collected: in a `return`, an array, an assignment, an `array_merge`.
    pub(crate) fn keep_constructions(&mut self, class: ClassId, block: &Block<'p>) {
        for statement in block.statements.iter() {
            self.constructions_in_statement(class, statement);
        }
    }

    fn constructions_in_statement(&mut self, class: ClassId, statement: &Statement<'p>) {
        match statement {
            Statement::Return(node) => {
                if let Some(value) = node.value {
                    self.constructions_in(class, value);
                }
            }
            Statement::Expression(node) => self.constructions_in(class, node.expression),
            Statement::Block(node) => {
                for inner in node.statements.iter() {
                    self.constructions_in_statement(class, inner);
                }
            }
            _ => {}
        }
    }

    fn constructions_in(&mut self, class: ClassId, expression: &Expression<'p>) {
        match expression {
            Expression::Parenthesized(inner) => self.constructions_in(class, inner.expression),
            Expression::Array(array) => self.constructions_among(class, array.elements.as_slice()),
            Expression::LegacyArray(array) => {
                self.constructions_among(class, array.elements.as_slice());
            }
            Expression::Assignment(assignment) => self.constructions_in(class, assignment.rhs),
            Expression::Call(Call::Function(call)) => {
                for argument in call.argument_list.arguments.iter() {
                    self.constructions_in(class, argument.value());
                }
            }
            Expression::Instantiation(instantiation) => {
                self.keep_construction(class, instantiation);
            }
            _ => {}
        }
    }

    fn constructions_among(&mut self, class: ClassId, elements: &[ArrayElement<'p>]) {
        for element in elements {
            match element {
                ArrayElement::Value(value) => self.constructions_in(class, value.value),
                ArrayElement::KeyValue(pair) => self.constructions_in(class, pair.value),
                _ => {}
            }
        }
    }

    fn keep_construction(
        &mut self,
        class: ClassId,
        instantiation: &mago_syntax::cst::Instantiation<'p>,
    ) {
        let Expression::Identifier(written) = instantiation.class else {
            return;
        };
        if !self
            .keyed_attributes
            .keeps_construction_of(&phrost_syntax::identifier_text(written.value()))
        {
            return;
        }
        let Some(arguments) = instantiation.argument_list.as_ref() else {
            return;
        };
        let mut positional = arguments.arguments.iter().map(Argument::value);
        let Some(first) = positional.next() else {
            return;
        };
        let Some(key) = string_literal(first) else {
            return;
        };
        let key_range = self.inside_the_quotes(first.span());
        let callable = positional
            .next()
            .map_or(KeptCallableData::Unnamed, |second| {
                self.kept_callable(second)
            });
        let constructed = self.name_ref(written);
        let key = self.interner.name(&key);
        self.builder.add_kept_construction(KeptConstructionData {
            class,
            constructed,
            key,
            key_range,
            callable,
        });
    }

    /// What a callable argument names: a function by its string, a method by its pair or its
    /// `Class::method` string.
    fn kept_callable(&mut self, value: &Expression<'p>) -> KeptCallableData {
        if let Some(written) = string_literal(value) {
            let written = written.trim_start_matches('\\');
            return match written.split_once("::") {
                Some((class, method)) if is_identifier(class) && is_identifier(method) => {
                    KeptCallableData::NamedMethod {
                        class: self.interner.name(class),
                        method: self.interner.name(method),
                    }
                }
                _ if is_identifier(written) => {
                    KeptCallableData::Function(self.interner.name(written))
                }
                _ => KeptCallableData::Unnamed,
            };
        }
        let elements = match value {
            Expression::Array(array) => array.elements.as_slice(),
            Expression::LegacyArray(array) => array.elements.as_slice(),
            _ => return KeptCallableData::Unnamed,
        };
        let [ArrayElement::Value(first), ArrayElement::Value(second)] = elements else {
            return KeptCallableData::Unnamed;
        };
        let Some(method) = string_literal(second.value) else {
            return KeptCallableData::Unnamed;
        };
        let class = match first.value {
            Expression::Variable(Variable::Direct(variable))
                if String::from_utf8_lossy(variable.name).as_ref() == "$this" =>
            {
                None
            }
            Expression::Access(mago_syntax::cst::Access::ClassConstant(access)) => {
                match access.class {
                    Expression::Self_(_) | Expression::Static(_) => None,
                    Expression::Identifier(named) => Some(self.name_ref(named)),
                    _ => return KeptCallableData::Unnamed,
                }
            }
            _ => return KeptCallableData::Unnamed,
        };
        KeptCallableData::Method {
            class,
            method: self.interner.name(&method),
        }
    }
}

fn is_identifier(written: &str) -> bool {
    !written.is_empty()
        && written
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'\\')
}
