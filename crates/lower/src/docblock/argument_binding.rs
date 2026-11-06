//! Which argument a written type resolves back to.
//!
//! Shared by both halves of [`contract`](super::contract): a conditional return's branch may name
//! something only an argument can settle (`holding_the_same`'s callers), and
//! [`Lowerer::returns_a_named_class`] is nothing *but* this -- a `@return T` where `T` is a
//! `@template` bound by an argument in one of four ways: a `class-string<T>` the call named, a
//! container the call passed, a bare parameter of the same type, or a callable's own return type.
//! All four read the parsed docblock rather than the lowered types, because matching a name as
//! written cannot go through a resolution step that would turn `T` into a class nothing declares.
//!
//! [`Lowerer::returns_an_applied_class`] is the same four reads one level in: `@return Bar<T>` names
//! a class outright and leaves only its arguments to an argument, so both halves are lowered here
//! and travel together.

use mago_allocator::{Arena as _, LocalArena};
use mago_phpdoc_syntax::{
    PHPDocParser,
    cst::{Element, TagValue, r#type::Type},
};
use mago_span::HasSpan as _;

use crate::lowerer::Lowerer;

use super::{slice, variable_name};

impl Lowerer<'_, '_> {
    /// Which position a parameter name sits at, counted from zero.
    pub(crate) fn parameter_position(
        &self,
        params: lspkit_arena::IdSlice<phrost_hir::ParamId>,
        name: &str,
    ) -> Option<u16> {
        let position = self
            .builder
            .params_in(params)
            .position(|param| param.name.as_str() == name)?;
        u16::try_from(position).ok()
    }

    /// A `@return` whose templates a parameter binds, in the ways a parameter can.
    ///
    /// The factory shape and its neighbours, and between them they are most of the generics anybody
    /// writes. `class-string<T>` is the factory -- a DI container's `get()`, a test's `createMock()`
    /// -- where the class is named at the call. `array<T>` is every `first()` and `pop()` helper. A
    /// bare `T` is the plainest of them: whatever was passed is what comes back. And `Bar<T>` is
    /// Doctrine's `getRepository()`, where the result is a container of what the call named.
    ///
    /// All of them say the same kind of thing -- the declaration cannot name the type, only the
    /// argument to read it from -- which is why one contract slot carries them.
    pub(crate) fn returns_a_named_class(
        &mut self,
        declaration_start: phrost_model::TextSize,
        params: lspkit_arena::IdSlice<phrost_hir::ParamId>,
        written_params: &mago_syntax::cst::FunctionLikeParameterList<'_>,
    ) -> Option<phrost_hir::ContractData> {
        let docblock = self.raw_docblock_before(declaration_start)?;
        // One word, and almost no docblock has it. Without the `@` for the same reason every other
        // prefilter here drops it: `@phpstan-template` does not contain `@template`.
        if !docblock.text.contains("template") {
            return None;
        }
        let text = docblock.text.to_owned();
        let base = docblock.range.start();
        let arena = LocalArena::new();
        let content = arena.alloc_slice_copy(text.as_bytes());
        let document = PHPDocParser::parse(
            &arena,
            mago_database::file::FileId::new(b"docblock"),
            content,
        );

        let returned = super::tagged_type_in(&document, super::DocTag::Return)?;

        // A bare name is what two of the three shapes need: they read a template *out of* something
        // the argument names, and `list<T>` says more about the result than the argument can answer.
        //
        // A null around it does not. `@return ?T` and the `null|TNode` `NodeFinder` writes are the
        // same swap `@return ?T` beside a native hint already is -- the null belongs to the
        // declaration, and `class-string<T>` says which class and nothing about whether one was
        // found. Looking under it here and *only* here is deliberate: the shapes below match on the
        // whole written text, and stripping the null for them would answer non-null, which is
        // narrower than the truth and reports correct code.
        let (bare, nullable) = under_a_null(returned);
        // And a container around *that*, which is the same claim one level out: `findInstanceOf`
        // beside `findFirstInstanceOf`, Doctrine's `findAll()` beside its `find()`. Only for the
        // class-string shape below -- what a container of an argument's *elements* means is a
        // different question, and `elements_branch` already answers it.
        let (bare, many) = match held_by(bare) {
            Some(element) => (element, true),
            None => (bare, false),
        };
        // And an intersection around it, which is what a mock factory writes: PHPUnit's
        // `createMock(string $type): MockObject` says `@return MockObject&T`, and both halves are
        // true at once. Only the template operand is read here, because the other one is the
        // *declaration's own* return type and the reader composes the two -- see
        // `beside_the_declared_return`. An intersection naming a class the hint does not is a miss
        // rather than a wrong answer, and it is not a shape anybody writes.
        // Both operands, because `MockObject&T` writes two names and only the parameter list says
        // which is the template. The class-string question is asked of each in turn, and the one
        // that is a real class answers nothing.
        let operands = operands_of(bare);
        let beside = operands.len() > 1;
        for bare in operands {
            let Type::Reference(reference) = bare else {
                continue;
            };
            let named = slice(&text, reference.kind.span());
            // The class-string first, because it is the one that answers from what the call *wrote*
            // rather than from what the argument resolves to -- so it also works in a chain, where
            // nothing resolves.
            if let Some(parameter) = self.bound_by_a_class_string(&document, &text, params, named) {
                return Some(phrost_hir::ContractData::ReturnsArgumentClass {
                    parameter,
                    // `under_a_null` reads the one sentinel a bare-name return can carry: a
                    // `T|false` leaves the union standing and never reaches this match.
                    absent: nullable.then_some(phrost_model::Absence::Null),
                    many,
                    beside,
                });
            }
            // The element shape reads a template *out of* an argument's container, which a container
            // on the return says nothing about -- so it is asked only where the return is bare.
            if !many
                && let Some(parameter) = self.bound_by_an_element(&document, &text, params, named)
            {
                return Some(phrost_hir::ContractData::AgreedReturn(
                    phrost_hir::ReturnBranchData::ArgumentElement {
                        parameter,
                        // `under_a_null` reads the one sentinel this shape can be written with: a
                        // `V|false` leaves the union standing and falls past the bare-name match
                        // above, so there is nothing here that could have said `false`.
                        absent: nullable.then_some(phrost_model::Absence::Null),
                    },
                ));
            }
        }

        let written = slice(&text, returned.span());
        if let Some((parameter, evaluated)) =
            self.bound_by_a_parameter_evaluated(&document, &text, params, written)
        {
            return Some(phrost_hir::ContractData::AgreedReturn(
                phrost_hir::ReturnBranchData::ArgumentType {
                    parameter,
                    absent: left_out(&document, &text, written, written_params, parameter),
                    evaluated,
                },
            ));
        }
        // A callable's own return type, next: it is the least direct of the four, and a declaration
        // that binds the same template to a parameter *and* to a callback means the parameter.
        if let Some(parameter) = self.bound_by_a_callback_return(&document, &text, params, written)
        {
            return Some(phrost_hir::ContractData::AgreedReturn(
                phrost_hir::ReturnBranchData::CallbackResult { parameter },
            ));
        }
        // A union whose operands are bound by *different* things, which is why it comes after every
        // shape that reads the whole return as one claim: `@return TDefault|TInput` is Symfony's
        // `InputBag::get`, where one half is an argument's type and the other is what the receiver
        // was written with.
        if let Some(operands) =
            self.returns_one_of(&document, &text, params, written_params, returned, base)
        {
            return Some(operands);
        }
        // Last, because it is the only one of the five that is not a swap: the others hand back
        // something the call already holds, and this builds a value out of a class the declaration
        // named and arguments it could not.
        //
        // Or a container of them, `list<ReflectionAttribute<A>>`, which is what every
        // `getAttributes()` in the reflection API returns.
        let (reference, many) = match (returned, held_by(returned)) {
            (Type::Reference(reference), _) => (reference, false),
            (_, Some(Type::Reference(reference))) => (reference, true),
            _ => {
                return self.returns_one_of_applied(
                    &document,
                    &text,
                    params,
                    Some(written_params),
                    returned,
                    base,
                );
            }
        };
        self.returns_an_applied_class(
            &document,
            &text,
            params,
            Some(written_params),
            reference,
            base,
            many,
        )
    }
}

/// What arrives for a template bound by a parameter the call leaves out.
///
/// The template's own default where the declaration wrote one -- `@template TDefault = never` says
/// the operand is not there, whatever the parameter defaults to -- else the parameter's default
/// value, which is what PHP itself passes. Guzzle's promises, Laravel's `whenHas` and the stub set's
/// `Ds\Map::get` write the first, and reading only the second answered `?TValue` for a call that
/// cannot return null.
fn left_out(
    document: &mago_phpdoc_syntax::cst::Document<'_>,
    text: &str,
    template: &str,
    written_params: &mago_syntax::cst::FunctionLikeParameterList<'_>,
    parameter: u16,
) -> Option<phrost_model::BuiltinTy> {
    super::template::default_keyword(document, text, template.trim())
        .or_else(|| super::contract::default_kind(written_params, parameter))
}

impl Lowerer<'_, '_> {
    /// A `@return A|B` whose operands each need a different thing to settle them.
    ///
    /// One operand may be a template an argument's type binds and another a template the receiver
    /// supplies, and until this existed neither resolver saw the whole tag: one is handed the call and
    /// the other the receiver, and a union spanning both resolved in neither. `InputBag::get` is the
    /// one method in a 300 MB vendor tree that writes it -- built for the shape rather than the catch,
    /// because "two resolvers, each complete for its own kind and blind to the other's" is a form this
    /// codebase has now hit four times.
    ///
    /// `None` unless **every** operand answers, and unless at least one of them needs a call at all: a
    /// union of plain types is a return type the declaration already carries, and recording one here
    /// would put a contract in front of it for nothing.
    ///
    /// An operand that is a bare name nothing binds is kept as its written type rather than refused,
    /// because the reader can still substitute it from the receiver -- which is exactly the half that
    /// was missing.
    fn returns_one_of(
        &mut self,
        document: &mago_phpdoc_syntax::cst::Document<'_>,
        text: &str,
        params: lspkit_arena::IdSlice<phrost_hir::ParamId>,
        written_params: &mago_syntax::cst::FunctionLikeParameterList<'_>,
        returned: &Type<'_>,
        base: phrost_model::TextSize,
    ) -> Option<phrost_hir::ContractData> {
        let written = flat_union(returned);
        if written.len() < 2 {
            return None;
        }
        let mut operands = Vec::with_capacity(written.len());
        let mut wants_the_call = false;
        for operand in written {
            let named = slice(text, operand.span());
            if let Some((parameter, evaluated)) =
                self.bound_by_a_parameter_evaluated(document, text, params, named)
            {
                wants_the_call = true;
                operands.push(phrost_hir::ReturnBranchData::ArgumentType {
                    parameter,
                    absent: left_out(document, text, named, written_params, parameter),
                    evaluated,
                });
                continue;
            }
            // Written out, which covers the `null` of `@return T|null` and a real class beside a
            // template -- and covers the template the *receiver* supplies, whose name lowers to a
            // reference the reader substitutes.
            operands.push(phrost_hir::ReturnBranchData::Ty(self.lower_doc_type(
                operand,
                base,
                text,
                &std::rc::Rc::clone(&self.class_aliases),
            )?));
        }
        wants_the_call.then(|| phrost_hir::ContractData::ReturnsOneOf(operands.into_boxed_slice()))
    }

    /// A `@return Bar<T>` -- a class the declaration names, with arguments only the call can fill.
    ///
    /// The class resolves here like any written name; each argument is read off a parameter the same
    /// four ways [`Self::returns_a_named_class`] reads a bare `@return T`. Both halves have to travel
    /// together, because neither answers on its own -- `Bar` alone loses what it holds, and `T` alone
    /// has nothing to hang off.
    ///
    /// `None` unless a parameter settles at least one argument, and `None` too when any argument is a
    /// template nothing binds: an unanswerable slot would be filled with a class the project does not
    /// declare, which is a wrong answer rather than a partial one.
    #[allow(clippy::too_many_arguments)]
    fn returns_an_applied_class(
        &mut self,
        document: &mago_phpdoc_syntax::cst::Document<'_>,
        text: &str,
        params: lspkit_arena::IdSlice<phrost_hir::ParamId>,
        written: Option<&mago_syntax::cst::FunctionLikeParameterList<'_>>,
        reference: &mago_phpdoc_syntax::cst::r#type::ReferenceType<'_>,
        base: phrost_model::TextSize,
        many: bool,
    ) -> Option<phrost_hir::ContractData> {
        // A bare name is a plain return type, which the declaration already carries.
        reference.parameters.as_ref()?;
        let arguments = self.applied_arguments(document, text, params, written, reference, base)?;
        // Every slot a plain type means the declaration said the whole thing, which the return type
        // already carries. A contract there would be a second spelling of one answer.
        if arguments
            .iter()
            .all(|branch| matches!(branch, phrost_hir::ReturnBranchData::Ty(_)))
        {
            return None;
        }

        let range = super::shift(reference.kind.span(), base);
        let class = self.doc_reference_base(&reference.kind, range, text);
        Some(phrost_hir::ContractData::ReturnsApplied {
            class,
            arguments: arguments.into_boxed_slice(),
            many,
            alternatives: Box::default(),
        })
    }

    /// A `@return A<...>|B<...>` whose operands write one argument list over different classes.
    ///
    /// Eloquent's `Collection::map()` is `Support\Collection<TKey, TMapValue>|static<TKey,
    /// TMapValue>`: the callback decides whether what comes back holds models, and the declaration
    /// says either. The arguments are read once, as for a single class, and the classes travel
    /// beside them -- the reader decides which of them a call's arguments can be.
    ///
    /// Only where every operand writes the same list, character for character. Two different lists
    /// are two contracts, and this one has a slot for one.
    fn returns_one_of_applied(
        &mut self,
        document: &mago_phpdoc_syntax::cst::Document<'_>,
        text: &str,
        params: lspkit_arena::IdSlice<phrost_hir::ParamId>,
        written: Option<&mago_syntax::cst::FunctionLikeParameterList<'_>>,
        returned: &Type<'_>,
        base: phrost_model::TextSize,
    ) -> Option<phrost_hir::ContractData> {
        let operands = flat_union(returned);
        let [first, rest @ ..] = operands.as_slice() else {
            return None;
        };
        if rest.is_empty() {
            return None;
        }
        let written_arguments = |operand: &Type<'_>| match operand {
            Type::Reference(reference) => {
                reference.parameters.as_ref()?;
                text.get(
                    reference.kind.span().end.offset as usize..reference.span().end.offset as usize,
                )
            }
            _ => None,
        };
        let shared = written_arguments(first)?;
        if rest
            .iter()
            .any(|operand| written_arguments(operand) != Some(shared))
        {
            return None;
        }
        let Type::Reference(reference) = first else {
            return None;
        };
        let mut contract =
            self.returns_an_applied_class(document, text, params, written, reference, base, false)?;
        let phrost_hir::ContractData::ReturnsApplied { alternatives, .. } = &mut contract else {
            return None;
        };
        *alternatives = rest
            .iter()
            .filter_map(|operand| match operand {
                Type::Reference(reference) => {
                    let range = super::shift(reference.kind.span(), base);
                    Some(self.doc_reference_base(&reference.kind, range, text))
                }
                _ => None,
            })
            .collect();
        Some(contract)
    }

    /// What fills each type parameter a written `Bar<...>` names, read the way
    /// [`Self::returns_an_applied_class`] reads them -- empty for a `Bar` written with none.
    ///
    /// Shared with the receiver's out type (`@phpstan-self-out self<U>`), which is the same shape
    /// written about a different value: one reading of a slot, so the two cannot disagree about
    /// which argument binds a template.
    ///
    /// `None` when any slot is a template nothing binds, for the reason
    /// [`Self::returns_an_applied_class`] gives.
    pub(super) fn applied_arguments(
        &mut self,
        document: &mago_phpdoc_syntax::cst::Document<'_>,
        text: &str,
        params: lspkit_arena::IdSlice<phrost_hir::ParamId>,
        written: Option<&mago_syntax::cst::FunctionLikeParameterList<'_>>,
        reference: &mago_phpdoc_syntax::cst::r#type::ReferenceType<'_>,
        base: phrost_model::TextSize,
    ) -> Option<Vec<phrost_hir::ReturnBranchData>> {
        let Some(parameters) = reference.parameters.as_ref() else {
            return Some(Vec::new());
        };
        let templates = super::template::declared_templates(document);
        let mut arguments = Vec::new();
        for entry in parameters.entries.iter() {
            arguments.push(self.argument_branch(
                document,
                text,
                params,
                written,
                &entry.inner,
                base,
                &templates,
            )?);
        }
        Some(arguments)
    }

    /// One type argument of a `@return Bar<...>`.
    ///
    /// The same four bindings a bare `@return T` is read through, in the same order and for the same
    /// reasons, plus the case a bare return does not have: an argument the declaration wrote out in
    /// full is a `Ty`, and no call is needed to settle it.
    #[allow(clippy::too_many_arguments)]
    fn argument_branch(
        &mut self,
        document: &mago_phpdoc_syntax::cst::Document<'_>,
        text: &str,
        params: lspkit_arena::IdSlice<phrost_hir::ParamId>,
        written: Option<&mago_syntax::cst::FunctionLikeParameterList<'_>>,
        argument: &Type<'_>,
        base: phrost_model::TextSize,
        templates: &[std::borrow::Cow<'_, str>],
    ) -> Option<phrost_hir::ReturnBranchData> {
        // A conditional in the slot, which the call settles as it would a whole return: Laravel's
        // `groupBy()` keys its groups `($groupBy is string ? array-key : TGroupKey)`. Read before
        // anything that reads the slot as one type -- the test is no type, and the template in one
        // branch refused the whole contract.
        if let Some(conditional) = super::contract::as_conditional(argument) {
            return self.conditional_slot(
                document,
                text,
                params,
                written,
                conditional,
                base,
                templates,
            );
        }
        // A class with arguments in the slot, some of which the call fills: each group of a
        // `groupBy()` is a `static<..., TValue>` of its own. One whose slots are all written out is
        // a plain type, and falls through to be lowered as one.
        if let Type::Reference(reference) = argument
            && reference.parameters.is_some()
            && let Some(arguments) =
                self.applied_arguments(document, text, params, written, reference, base)
            && !arguments
                .iter()
                .all(|branch| matches!(branch, phrost_hir::ReturnBranchData::Ty(_)))
        {
            let range = super::shift(reference.kind.span(), base);
            return Some(phrost_hir::ReturnBranchData::Applied {
                class: self.doc_reference_base(&reference.kind, range, text),
                arguments: arguments.into_boxed_slice(),
            });
        }
        // A bare name is what the first two reads need: they take a template *out of* something an
        // argument names, and `list<T>` says more about the slot than the argument can answer.
        if let Type::Reference(reference) = argument
            && reference.parameters.is_none()
        {
            let named = slice(text, reference.kind.span());
            if let Some(parameter) = self.bound_by_a_class_string(document, text, params, named) {
                return Some(phrost_hir::ReturnBranchData::ArgumentClass(parameter));
            }
            // Its keys as well as its elements, through the one reading both other places a
            // template is reduced to an argument use: `collect()` is `Collection<TKey, TValue>`
            // beside `iterable<TKey, TValue>`, and a slot that learned only the elements refused
            // the key -- and with it the whole contract, so every `collect($rows)` held nothing.
            if let Some(container) =
                self.bound_by_the_container(document, text, params, named, None)
            {
                return Some(container);
            }
        }

        let written = slice(text, argument.span());
        if let Some((parameter, evaluated)) =
            self.bound_by_a_parameter_evaluated(document, text, params, written)
        {
            return Some(phrost_hir::ReturnBranchData::ArgumentType {
                parameter,
                absent: None,
                evaluated,
            });
        }
        if let Some(parameter) = self.bound_by_a_callback_return(document, text, params, written) {
            return Some(phrost_hir::ReturnBranchData::CallbackResult { parameter });
        }
        // Nothing binds it, so it has to be a type the declaration wrote out -- and a template is not
        // one. See [`Self::returns_an_applied_class`] for why one such slot refuses the whole shape.
        if super::template::names_a_template(written, templates) {
            return None;
        }
        Some(phrost_hir::ReturnBranchData::Ty(self.lower_doc_type(
            argument,
            base,
            text,
            &std::rc::Rc::clone(&self.class_aliases),
        )?))
    }

    /// A conditional written as one slot of a returned class: its subject, its test and a branch per
    /// outcome, each branch read as a slot itself -- so a branch may be another conditional, a
    /// template the callback binds, or a class with arguments.
    #[allow(clippy::too_many_arguments)]
    fn conditional_slot(
        &mut self,
        document: &mago_phpdoc_syntax::cst::Document<'_>,
        text: &str,
        params: lspkit_arena::IdSlice<phrost_hir::ParamId>,
        written: Option<&mago_syntax::cst::FunctionLikeParameterList<'_>>,
        conditional: &mago_phpdoc_syntax::cst::r#type::conditional::ConditionalType<'_>,
        base: phrost_model::TextSize,
        templates: &[std::borrow::Cow<'_, str>],
    ) -> Option<phrost_hir::ReturnBranchData> {
        // A subject or a test this cannot read leaves the slot to its bound rather than refusing the
        // contract: Laravel 12's `groupBy()` tests `TGroupKey is \UnitEnum`, a template only the
        // callback binds, inside a slot whose sibling says what each group holds.
        let unfilled = || Some(phrost_hir::ReturnBranchData::Unfilled);
        let Some(argument) =
            self.subject_argument(document, text, params, conditional.subject, templates)
        else {
            return unfilled();
        };
        let Some(test) = self.condition_test(conditional.target, base, text) else {
            return unfilled();
        };
        // `is not` says the same thing with the branches the other way round.
        let (then, otherwise) = match conditional.is_negated() {
            false => (conditional.then, conditional.r#else),
            true => (conditional.r#else, conditional.then),
        };
        let then = self
            .argument_branch(document, text, params, written, then, base, templates)
            .unwrap_or(phrost_hir::ReturnBranchData::Unfilled);
        let otherwise = self
            .argument_branch(document, text, params, written, otherwise, base, templates)
            .unwrap_or(phrost_hir::ReturnBranchData::Unfilled);
        let by_default = written
            .and_then(|written| super::contract::default_satisfies(written, argument, &test));
        Some(phrost_hir::ReturnBranchData::Conditional(Box::new(
            phrost_hir::ConditionalBranchData {
                argument,
                test,
                then,
                otherwise,
                by_default,
            },
        )))
    }

    /// Which parameter is a callable declared to return `written`.
    ///
    /// `@param (callable(V): U) $callback` beside `@return list<U>`: the declaration cannot name `U`
    /// -- the closure written at the call is the only place it appears -- so what it names instead is
    /// the argument to read it off.
    pub(crate) fn bound_by_a_callback_return(
        &self,
        document: &mago_phpdoc_syntax::cst::Document<'_>,
        text: &str,
        params: lspkit_arena::IdSlice<phrost_hir::ParamId>,
        written: &str,
    ) -> Option<u16> {
        let bound = document.elements.iter().find_map(|element| match element {
            Element::Tag(entry) => {
                let TagValue::Param(value) = &entry.value else {
                    return None;
                };
                if !returns_written(value.r#type, text, written) {
                    return None;
                }
                Some(variable_name(value.parameter.as_ref()?))
            }
            Element::Text(_) | Element::Code(_) => None,
        })?;
        self.parameter_position(params, bound)
    }

    /// Which parameter's container type names this template as its *keys*.
    ///
    /// The sibling of [`Self::bound_by_an_element`], and `array_keys` is the only shape in PHP's own
    /// library that needs it: `@param array<K, V> $array` beside `@return list<K>`.
    pub(crate) fn bound_by_the_keys(
        &self,
        document: &mago_phpdoc_syntax::cst::Document<'_>,
        text: &str,
        params: lspkit_arena::IdSlice<phrost_hir::ParamId>,
        written: &str,
    ) -> Option<u16> {
        let bound = document.elements.iter().find_map(|element| match element {
            Element::Tag(entry) => {
                let TagValue::Param(value) = &entry.value else {
                    return None;
                };
                if !keys_element(value.r#type, text, written) {
                    return None;
                }
                Some(variable_name(value.parameter.as_ref()?))
            }
            Element::Text(_) | Element::Code(_) => None,
        })?;
        self.parameter_position(params, bound)
    }

    /// What a single template names in one argument's container -- its elements or its keys --
    /// with the sentinel its leaf set aside travelling beside it.
    ///
    /// One answer for the three places a contract reduces a template to an argument, because the
    /// third of them learned only the elements: `array_key_first`'s `(... ? K : null)` was refused
    /// as a branch, the conditional went with it, and a guard above the call -- or a literal that
    /// plainly holds something -- never took the null away. `array_first` beside it, spelled the
    /// same with a `V`, did not have the bug.
    pub(crate) fn bound_by_the_container(
        &self,
        document: &mago_phpdoc_syntax::cst::Document<'_>,
        text: &str,
        params: lspkit_arena::IdSlice<phrost_hir::ParamId>,
        written: &str,
        absent: Option<phrost_model::Absence>,
    ) -> Option<phrost_hir::ReturnBranchData> {
        if let Some(parameter) = self.bound_by_an_element(document, text, params, written) {
            return Some(phrost_hir::ReturnBranchData::ArgumentElement { parameter, absent });
        }
        let parameter = self.bound_by_the_keys(document, text, params, written)?;
        Some(phrost_hir::ReturnBranchData::ArgumentKey { parameter, absent })
    }

    /// Which parameter's element type binds `written`.
    ///
    /// `@param array<K, V> $array` beside `@return V`: `V` is not a class, it is whatever the array
    /// the call passed holds. Read off the parsed docblock for the reason
    /// [`Self::bound_by_a_class_string`] gives -- the lowering keeps the argument, but matching a
    /// name needs the name as written.
    ///
    /// The last argument, for the reason the iteration bound takes it: every shape in PHP's ecosystem
    /// puts the value there, and `<Key, Value>` is the only pair anyone writes.
    pub(crate) fn bound_by_an_element(
        &self,
        document: &mago_phpdoc_syntax::cst::Document<'_>,
        text: &str,
        params: lspkit_arena::IdSlice<phrost_hir::ParamId>,
        written: &str,
    ) -> Option<u16> {
        let bound = document.elements.iter().find_map(|element| match element {
            Element::Tag(entry) => {
                let TagValue::Param(value) = &entry.value else {
                    return None;
                };
                if !holds_element(value.r#type, text, written) {
                    return None;
                }
                Some(variable_name(value.parameter.as_ref()?))
            }
            Element::Text(_) | Element::Code(_) => None,
        })?;
        self.parameter_position(params, bound)
    }

    /// Which parameter's own type is spelled exactly `written`.
    ///
    /// `@param T $item` beside `@return T`: the declaration says the result is whatever it was given,
    /// and only the call says what that was. `array_filter` is the same claim about a container --
    /// `@param array<K, V> $array` beside `@return array<K, V>` -- and so is every `array_diff`,
    /// `array_intersect` and `array_replace` beside it, which between them are most of how a PHP
    /// array is narrowed.
    ///
    /// Matched on the whole type as written, for the reason
    /// [`bound_by_a_class_string`](Self::bound_by_a_class_string) gives -- and here the match is the
    /// whole of the claim. Two spellings that differ may differ in ways nothing here can compose:
    /// `@param array<K, V>` beside `@return list<V>` is a rearrangement, not an identity.
    ///
    /// A `null|` on the parameter's side is looked through, because it is the one difference that
    /// says nothing about the result: `array_reduce` takes `null|I $initial` and returns `I`, where
    /// the null is the parameter's own default and not something the value carries out.
    pub(super) fn bound_by_a_parameter(
        &self,
        document: &mago_phpdoc_syntax::cst::Document<'_>,
        text: &str,
        params: lspkit_arena::IdSlice<phrost_hir::ParamId>,
        written: &str,
    ) -> Option<u16> {
        self.bound_by_a_parameter_evaluated(document, text, params, written)
            .map(|(parameter, _)| parameter)
    }

    /// The same, and whether the parameter also takes a callable producing the value -- the
    /// `ReturnBranchData::ArgumentType::evaluated` a branch needs to read a closure passed there.
    pub(super) fn bound_by_a_parameter_evaluated(
        &self,
        document: &mago_phpdoc_syntax::cst::Document<'_>,
        text: &str,
        params: lspkit_arena::IdSlice<phrost_hir::ParamId>,
        written: &str,
    ) -> Option<(u16, bool)> {
        let (bound, evaluated) = document.elements.iter().find_map(|element| match element {
            Element::Tag(entry) => {
                let TagValue::Param(value) = &entry.value else {
                    return None;
                };
                if !declares(text, value.r#type, written) {
                    return None;
                }
                let evaluated = flat_union(value.r#type)
                    .iter()
                    .any(|operand| produces(text, operand, written));
                Some((variable_name(value.parameter.as_ref()?), evaluated))
            }
            Element::Text(_) | Element::Code(_) => None,
        })?;
        Some((self.parameter_position(params, bound)?, evaluated))
    }

    /// Which parameter's `class-string<T>` binds `written`, the name an assert tag used.
    ///
    /// Read off the parsed docblock rather than off the lowered types, because the lowering throws
    /// the argument away on purpose: a `class-string<T>` is a string at runtime whatever it names,
    /// and every reader but this one wants the string.
    ///
    /// Matched by the name as written, which is the only spelling that can be compared: `T` is a
    /// `@template` and resolving it against the file's imports produces a class nothing declares.
    /// No `@template` list is consulted and none is needed -- when the name really is a template the
    /// match is the substitution, and when it is a real class, `class-string<Foo>` beside a `Foo`
    /// says the same thing either way.
    pub(crate) fn bound_by_a_class_string(
        &self,
        document: &mago_phpdoc_syntax::cst::Document<'_>,
        text: &str,
        params: lspkit_arena::IdSlice<phrost_hir::ParamId>,
        written: &str,
    ) -> Option<u16> {
        let bound = document.elements.iter().find_map(|element| match element {
            Element::Tag(entry) => {
                let TagValue::Param(value) = &entry.value else {
                    return None;
                };
                if !binds_class_string(value.r#type, text, written) {
                    return None;
                }
                Some(variable_name(value.parameter.as_ref()?))
            }
            Element::Text(_) | Element::Code(_) => None,
        })?;

        self.parameter_position(params, bound)
    }
}

/// Whether a `@param` type binds `written` through a `class-string<T>`.
///
/// Through a union, because that is how the tag is usually written: a factory takes
/// `string|class-string<T>` so that a service id and a class name go to the same parameter, and
/// looking only at the whole type would miss every one of them.
pub(crate) fn binds_class_string(ty: &Type<'_>, text: &str, written: &str) -> bool {
    match ty {
        Type::ClassString(class_string) => class_string
            .parameter
            .as_ref()
            .and_then(|parameter| match &parameter.entry.inner {
                Type::Reference(reference) => Some(slice(text, reference.kind.span()) == written),
                _ => None,
            })
            .unwrap_or(false),
        Type::Union(union) => {
            binds_class_string(union.left, text, written)
                || binds_class_string(union.right, text, written)
        }
        Type::Nullable(nullable) => binds_class_string(nullable.inner, text, written),
        Type::Parenthesized(inner) => binds_class_string(inner.inner, text, written),
        _ => false,
    }
}

/// Whether a parameter's written type is `written`, once a null that only the parameter carries is
/// looked through -- and a callable producing it, which is how a default is written lazily.
///
/// See [`Lowerer::bound_by_a_parameter`]. Over the flat union rather than its text, so `null|I` and
/// `I|null` are one case and `A|B` is not mistaken for either. Exactly one operand is the template;
/// every other has to be the null or a callable returning it: Laravel's `first($callback,
/// TFirstDefault|(\Closure(): TFirstDefault) $default)`, whose value is evaluated if it is a
/// closure, was bound by nothing, so every `->first()` on a collection answered nothing at all.
fn declares(text: &str, declared: &Type<'_>, written: &str) -> bool {
    if slice(text, declared.span()) == written {
        return true;
    }
    match declared {
        Type::Nullable(nullable) => declares(text, nullable.inner, written),
        Type::Parenthesized(inner) => declares(text, inner.inner, written),
        Type::Union(_) => {
            let operands = flat_union(declared);
            let mut found = false;
            for operand in &operands {
                if is_null_type(text, operand) || produces(text, operand, written) {
                    continue;
                }
                if found || slice(text, operand.span()) != written {
                    return false;
                }
                found = true;
            }
            found && operands.len() > 1
        }
        _ => false,
    }
}

/// Whether a written type is a callable whose declared return is `written`: `\Closure(): T`.
fn produces(text: &str, declared: &Type<'_>, written: &str) -> bool {
    match declared {
        Type::Parenthesized(inner) => produces(text, inner.inner, written),
        Type::Callable(callable) => callable
            .specification
            .as_ref()
            .and_then(|specification| specification.return_type.as_ref())
            .is_some_and(|returned| slice(text, returned.return_type.span()) == written),
        _ => false,
    }
}

/// Whether a written type is the null literal.
fn is_null_type(text: &str, declared: &Type<'_>) -> bool {
    slice(text, declared.span()).eq_ignore_ascii_case("null")
}

/// The value type a container writes, as written: the last argument of `array<K, V>`, of `list<V>`,
/// the inner type of `V[]`.
///
/// `None` for anything that is not a container with its contents spelled out. This is what leaves
/// that disagree about the container can still agree about.
pub(crate) fn contained<'a>(ty: &Type<'_>, text: &'a str) -> Option<&'a str> {
    let parameters = match ty {
        Type::Array(array) => array.parameters.as_ref(),
        Type::NonEmptyArray(array) => array.parameters.as_ref(),
        Type::AssociativeArray(array) => array.parameters.as_ref(),
        Type::List(list) => list.parameters.as_ref(),
        Type::NonEmptyList(list) => list.parameters.as_ref(),
        Type::Iterable(iterable) => iterable.parameters.as_ref(),
        Type::Reference(reference) => reference.parameters.as_ref(),
        Type::Slice(shorthand) => return Some(slice(text, shorthand.inner.span())),
        Type::Parenthesized(inner) => return contained(inner.inner, text),
        _ => return None,
    };
    parameters
        .and_then(|parameters| parameters.entries.iter().last())
        .map(|last| slice(text, last.inner.span()))
}

/// The key a container of rows is keyed by in each row, when each row holds `written`.
///
/// `array<array-key, array<K, V>|object>` holding `V` answers `K` -- `array_column`'s rows. Through
/// a union at either level, since the stub writes a list and an array of rows side by side and a
/// row as an array or an object.
fn row_key_holding<'t>(ty: &Type<'_>, text: &'t str, written: &str) -> Option<&'t str> {
    let parameters = match ty {
        Type::Array(array) => array.parameters.as_ref(),
        Type::NonEmptyArray(array) => array.parameters.as_ref(),
        Type::AssociativeArray(array) => array.parameters.as_ref(),
        Type::List(list) => list.parameters.as_ref(),
        Type::NonEmptyList(list) => list.parameters.as_ref(),
        Type::Iterable(iterable) => iterable.parameters.as_ref(),
        Type::Union(union) => {
            return row_key_holding(union.left, text, written)
                .or_else(|| row_key_holding(union.right, text, written));
        }
        Type::Parenthesized(inner) => return row_key_holding(inner.inner, text, written),
        _ => return None,
    }?;
    let row = &parameters.entries.iter().last()?.inner;
    keyed_row(row, text, written)
}

/// The key of a two-argument container holding `written`, through a union.
fn keyed_row<'t>(row: &Type<'_>, text: &'t str, written: &str) -> Option<&'t str> {
    match row {
        Type::Union(union) => {
            keyed_row(union.left, text, written).or_else(|| keyed_row(union.right, text, written))
        }
        Type::Parenthesized(inner) => keyed_row(inner.inner, text, written),
        _ => {
            let parameters = match row {
                Type::Array(array) => array.parameters.as_ref(),
                Type::AssociativeArray(array) => array.parameters.as_ref(),
                _ => return None,
            }?;
            let entries = parameters.entries.iter().collect::<Vec<_>>();
            let [key, value] = entries.as_slice() else {
                return None;
            };
            declares(text, &value.inner, written).then(|| slice(text, key.inner.span()))
        }
    }
}

impl crate::lowerer::Lowerer<'_, '_> {
    /// Which parameters are the rows and the column, for a return that is one column of a container
    /// of rows. See `phrost_hir::ReturnBranchData::ArgumentColumn`.
    ///
    /// The rows are the `@param` whose rows hold `held`; the column is the `@param` written as the
    /// rows' key template, with or without a `null` beside it.
    pub(crate) fn bound_by_a_column(
        &self,
        document: &mago_phpdoc_syntax::cst::Document<'_>,
        text: &str,
        params: lspkit_arena::IdSlice<phrost_hir::ParamId>,
        held: &str,
    ) -> Option<(u16, u16)> {
        let tagged = document
            .elements
            .iter()
            .filter_map(|element| match element {
                Element::Tag(entry) => match &entry.value {
                    TagValue::Param(value) => Some(value),
                    _ => None,
                },
                Element::Text(_) | Element::Code(_) => None,
            })
            .collect::<Vec<_>>();
        let (rows, key) = tagged.iter().find_map(|value| {
            let key = row_key_holding(value.r#type, text, held)?;
            Some((variable_name(value.parameter.as_ref()?), key))
        })?;
        let column = tagged.iter().find_map(|value| {
            let written = slice(text, value.r#type.span());
            let names_the_key = written
                .split('|')
                .map(str::trim)
                .all(|operand| operand == key || operand.eq_ignore_ascii_case("null"))
                && written.split('|').any(|operand| operand.trim() == key);
            match names_the_key {
                true => Some(variable_name(value.parameter.as_ref()?)),
                false => None,
            }
        })?;
        Some((
            self.parameter_position(params, rows)?,
            self.parameter_position(params, column)?,
        ))
    }
}

/// Whether some `@param` in the docblock is a container of exactly `written`.
///
/// Asked of a template in a callback's signature, where it decides what the call site may answer:
/// `usort`'s `callable(T, T)` beside `array<T> $array` is handed elements of the array, and
/// `array_reduce`'s `callable(I, V)` puts an accumulator first that no container holds.
pub(crate) fn a_container_holds(
    document: &mago_phpdoc_syntax::cst::Document<'_>,
    text: &str,
    written: &str,
) -> bool {
    document.elements.iter().any(|element| match element {
        Element::Tag(entry) => {
            matches!(&entry.value, TagValue::Param(value) if holds_element(value.r#type, text, written))
        }
        Element::Text(_) | Element::Code(_) => false,
    })
}

/// The key a two-argument container is written with, when it names one of `templates`.
///
/// `array<K, V>` beside `@param array<K> $keys` and `@param array<V> $values` -- `array_combine` --
/// is keyed by what one argument holds and filled with what another holds, and a branch that reads
/// only the element would hand back a list: every key an `int`. So whoever reduces a return to its
/// element asks this first.
pub(crate) fn keyed_by_a_template<'a>(
    ty: &Type<'_>,
    text: &'a str,
    templates: &[std::borrow::Cow<'_, str>],
) -> Option<&'a str> {
    let parameters = match ty {
        Type::Array(array) => array.parameters.as_ref(),
        Type::NonEmptyArray(array) => array.parameters.as_ref(),
        Type::AssociativeArray(array) => array.parameters.as_ref(),
        Type::Iterable(iterable) => iterable.parameters.as_ref(),
        Type::Reference(reference) => reference.parameters.as_ref(),
        Type::Parenthesized(inner) => return keyed_by_a_template(inner.inner, text, templates),
        _ => return None,
    }?;
    let entries = parameters.entries.iter().collect::<Vec<_>>();
    let [key, _] = entries.as_slice() else {
        return None;
    };
    let key = slice(text, key.inner.span());
    super::template::names_a_template(key, templates).then_some(key)
}

/// Whether a container parameter's *first* type argument is spelled `written`.
///
/// The key half, where [`holds_element`] reads the last argument -- and it is a different question,
/// because `array<K, V>` names both and only the position tells them apart. Two arguments required:
/// `array<V>` and `V[]` and `list<V>` all name what a container holds and say nothing about its keys,
/// so a one-argument generic binds nothing here.
///
/// Through a union for [`holds_element`]'s reason -- a stub writes the same parameter several ways at
/// once: `Traversable<K, V>|array<K, V>` -- and through one in the key argument for the reason
/// given there.
fn keys_element(ty: &Type<'_>, text: &str, written: &str) -> bool {
    let parameters = match ty {
        Type::Array(array) => array.parameters.as_ref(),
        Type::NonEmptyArray(array) => array.parameters.as_ref(),
        Type::AssociativeArray(array) => array.parameters.as_ref(),
        Type::Iterable(iterable) => iterable.parameters.as_ref(),
        Type::Reference(reference) => reference.parameters.as_ref(),
        Type::Union(union) => {
            return keys_element(union.left, text, written)
                || keys_element(union.right, text, written);
        }
        Type::Nullable(nullable) => return keys_element(nullable.inner, text, written),
        Type::Parenthesized(inner) => return keys_element(inner.inner, text, written),
        _ => return false,
    };
    let Some(entries) = parameters.map(|parameters| &parameters.entries) else {
        return false;
    };
    entries.iter().count() > 1
        && entries
            .iter()
            .next()
            .is_some_and(|first| declares(text, &first.inner, written))
}

/// Whether a written parameter type is a callable declared to return `written`.
///
/// Through a union, because a stub writes the same callback several ways at once:
/// `(callable(V): U)|(callable(V, S): U)|null`.
fn returns_written(declared: &Type<'_>, text: &str, written: &str) -> bool {
    match declared {
        Type::Callable(callable) => callable
            .specification
            .as_ref()
            .and_then(|specification| specification.return_type.as_ref())
            .is_some_and(|returns| slice(text, returns.return_type.span()) == written),
        Type::Union(union) => {
            returns_written(union.left, text, written)
                || returns_written(union.right, text, written)
        }
        Type::Nullable(nullable) => returns_written(nullable.inner, text, written),
        Type::Parenthesized(inner) => returns_written(inner.inner, text, written),
        _ => false,
    }
}

/// Whether a `@param` type is a container whose element is `written`.
///
/// Through a union, because that is how a stub writes it: `reset` takes `object|array<K, V>` so that
/// the one signature covers both, and looking only at the whole type would miss every one of them.
///
/// And through a union in the *argument* as well, by [`declares`] -- two levels, two rules, and
/// only the outer one was here. `@param array<K, V|null>` beside `@return V` is how a helper that
/// drops the empties is written, and the null is the parameter's own: it says what may be in the
/// container, not what comes back. Refusing it left `V` bound by `array<K, V>` and by nothing else,
/// which is the same asymmetry [`bound_by_a_parameter`](Lowerer::bound_by_a_parameter) had already
/// been fixed for at the top level. `A|B` is still refused, and by the same helper -- which of the
/// two the element is has no answer.
fn holds_element(ty: &Type<'_>, text: &str, written: &str) -> bool {
    let parameters = match ty {
        Type::Array(array) => array.parameters.as_ref(),
        Type::NonEmptyArray(array) => array.parameters.as_ref(),
        Type::AssociativeArray(array) => array.parameters.as_ref(),
        Type::List(list) => list.parameters.as_ref(),
        Type::NonEmptyList(list) => list.parameters.as_ref(),
        Type::Iterable(iterable) => iterable.parameters.as_ref(),
        // `Traversable<K, V>`, `Collection<Item>` -- a class-like with arguments is a container too.
        Type::Reference(reference) => reference.parameters.as_ref(),
        // `V[]` -- the same container written the other way round.
        Type::Slice(shorthand) => return declares(text, shorthand.inner, written),
        Type::Union(union) => {
            return holds_element(union.left, text, written)
                || holds_element(union.right, text, written);
        }
        Type::Nullable(nullable) => return holds_element(nullable.inner, text, written),
        Type::Parenthesized(inner) => return holds_element(inner.inner, text, written),
        _ => return false,
    };
    parameters
        .and_then(|parameters| parameters.entries.iter().last())
        .is_some_and(|last| declares(text, &last.inner, written))
}

/// A return type with a null taken off it, and whether there was one.
///
/// Three spellings, and a real library writes each of them: `?T`, `T|null`, and the `null|TNode`
/// `NodeFinder` writes -- mago's docblock union is binary, so the null sits on whichever side the
/// author put it. Anything else comes back unchanged with `false`, which is what makes this safe to
/// call in front of a match that only wanted the bare name.
/// The names a return could be reading a template out of: itself, or both sides of an intersection.
///
/// `MockObject&T` is what a mock factory writes -- PHPUnit's `createMock`, Mockery's `mock` -- and
/// both operands are references, so nothing in the *syntax* says which is the template. The
/// parameter list says it, one step further in, and the operand that is a real class simply answers
/// nothing there.
///
/// The other operand is not lost: it is the declaration's own return type as well, and the reader
/// composes the two -- see `IndexView::beside_the_declared_return`.
fn operands_of<'t>(returned: &'t Type<'t>) -> Vec<&'t Type<'t>> {
    match returned {
        Type::Intersection(intersection) => vec![intersection.left, intersection.right],
        other => vec![other],
    }
}

fn under_a_null<'t>(returned: &'t Type<'t>) -> (&'t Type<'t>, bool) {
    match returned {
        Type::Nullable(nullable) => (nullable.inner, true),
        Type::Union(union) => match (union.left, union.right) {
            (Type::Null(_), inner) | (inner, Type::Null(_)) => (inner, true),
            _ => (returned, false),
        },
        _ => (returned, false),
    }
}

/// What a container written as a return type holds.
///
/// The mirror of [`holds_element`], which asks *whether* a container holds a named template; this
/// asks *what* one holds. Kept apart rather than folded together because the two want opposite
/// answers on a union: a parameter matcher takes either side, and a return has to name one thing.
/// Nothing here walks a union or a null -- [`under_a_null`] has already taken the null off, and a
/// union of containers is not a shape this contract can express.
fn held_by<'t>(returned: &'t Type<'t>) -> Option<&'t Type<'t>> {
    let parameters = match returned {
        Type::Array(array) => array.parameters.as_ref(),
        Type::NonEmptyArray(array) => array.parameters.as_ref(),
        Type::AssociativeArray(array) => array.parameters.as_ref(),
        Type::List(list) => list.parameters.as_ref(),
        Type::NonEmptyList(list) => list.parameters.as_ref(),
        Type::Iterable(iterable) => iterable.parameters.as_ref(),
        // `T[]` -- the same container written the other way round, and the commonest of the three.
        Type::Slice(shorthand) => return Some(shorthand.inner),
        _ => return None,
    };
    // The last argument, which is what a container holds however it was keyed: `array<K, V>`,
    // `list<V>` and `V[]` all say `V`.
    parameters.and_then(|parameters| parameters.entries.iter().last().map(|last| &last.inner))
}

/// A docblock union taken apart, however deeply mago nested it.
///
/// The parser builds `A|B|C` as pairs, so a three-operand union is a union whose left is a union.
/// Flattened here rather than walked recursively by the caller, because "every operand has to answer"
/// is a claim about the flat list and a nested one hides an operand inside a shape nothing examines --
/// the degenerate form this codebase has now had to fold out of three separate union constructors.
fn flat_union<'t>(returned: &'t Type<'t>) -> Vec<&'t Type<'t>> {
    match returned {
        Type::Union(union) => {
            let mut flat = flat_union(union.left);
            flat.extend(flat_union(union.right));
            flat
        }
        Type::Parenthesized(inner) => flat_union(inner.inner),
        _ => vec![returned],
    }
}
