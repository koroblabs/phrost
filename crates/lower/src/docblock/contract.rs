//! What a docblock proves about a call, entry point and conditional-return half: `ContractData`.
//!
//! [`Lowerer::contract_beside`] is the entry point every caller uses. Its own shape, and the one
//! this file owns, is a `@return ($p is T ? A : B)` whose test an argument settles
//! ([`Lowerer::conditional_return`], and the branch machinery under it that reads a conditional
//! type, tests it, and flattens or matches its branches). Two other shapes it dispatches to live
//! elsewhere: a `@return T` bound by an argument through a `class-string<T>`, a container, or a
//! bare parameter is [`argument_binding`](super::argument_binding), which this file's own
//! `agreed_return` and `return_branch` also call into when a conditional's branch turns out to be
//! one of those same three shapes rather than a plain type. `@assert` on a parameter is the third
//! shape, and stays in [`assert`](super::assert) because what it yields -- a parameter name beside
//! a type, not a type alone -- has nothing in common with either.

use mago_allocator::{Arena as _, LocalArena};
use mago_phpdoc_syntax::{
    PHPDocParser,
    cst::{
        Element, TagValue,
        r#type::{Type, conditional::ConditionalType},
    },
};
use mago_span::HasSpan as _;
use mago_syntax::cst::{Expression, FunctionLikeParameterList};
use phrost_hir::TypeRefData;
use phrost_model::{Absence, BuiltinTy};

use crate::lowerer::Lowerer;

use super::argument_binding::contained;
use super::template::names_a_template;
use super::type_alias::TypeAliasTable;
use super::{DocTag, slice, variable_name};

impl Lowerer<'_, '_> {
    /// What a method's docblock proves about its receiver when it returns.
    ///
    /// Gated on the word appearing in the raw text, for the same reason the `<` gate exists: locating
    /// the docblock is a lookup and parsing it is not, and nearly no method carries one of these --
    /// seven in the project this was measured on, out of eight thousand methods.
    pub(crate) fn contract_beside(
        &mut self,
        declaration_start: phrost_model::TextSize,
        params: lspkit_arena::IdSlice<phrost_hir::ParamId>,
        written: &FunctionLikeParameterList<'_>,
    ) -> Option<phrost_hir::ContractData> {
        let asserts = self
            .raw_docblock_before(declaration_start)
            .is_some_and(|docblock| docblock.text.contains("assert"));
        if asserts {
            if let Some(ty) = self.doc_type(declaration_start, DocTag::AssertIfTrue) {
                return Some(phrost_hir::ContractData::Receiver {
                    ty,
                    when_true: true,
                });
            }
            if let Some(ty) = self.doc_type(declaration_start, DocTag::AssertIfFalse) {
                return Some(phrost_hir::ContractData::Receiver {
                    ty,
                    when_true: false,
                });
            }
            if let Some(contract) = self.asserted_about_a_parameter(declaration_start, params) {
                return Some(contract);
            }
        }

        // Each has a gate of its own -- a couple of words, and almost no docblock has any of them --
        // so the cheap prefilter for one must not swallow another.
        //
        // A return contract ranks above a self-out because the slot holds one, and the one method
        // in a real tree that writes both -- Laravel's `PendingRequest::async()`, `@return self<T>`
        // beside `@phpstan-self-out self<T>` -- says the same thing twice.
        self.returns_a_named_class(declaration_start, params, written)
            .or_else(|| self.conditional_return(declaration_start, params, written))
            .or_else(|| self.element_return(declaration_start, params))
            .or_else(|| self.constant_entry(declaration_start, params))
            .or_else(|| self.receiver_out(declaration_start, params))
    }

    /// A plain `@return V` naming what an array parameter holds.
    ///
    /// The other half of what [`Self::agreed_return`] does for a conditional, and the reason
    /// `reset($rows)` answered `?Row` while `array_find($rows, $fn)` answered `mixed`. Both stubs
    /// carry `@template V` and `@param array<K, V> $array`; `reset`'s return is written as a
    /// conditional and so reached `ArgumentElement`, and `array_find`'s is the plain `@return V|null`
    /// that nothing produced a branch for. The difference was the spelling, not the claim.
    ///
    /// Gated on the docblock declaring a template, which is what makes the parse affordable: a bare
    /// name bound to an argument's elements is what a template is *for*, and a docblock without one
    /// already lowers `@return Row` to `Row` by the ordinary path. So this adds an answer where
    /// there was none rather than a second route to one there was.
    fn element_return(
        &mut self,
        declaration_start: phrost_model::TextSize,
        params: lspkit_arena::IdSlice<phrost_hir::ParamId>,
    ) -> Option<phrost_hir::ContractData> {
        let docblock = self.raw_docblock_before(declaration_start)?;
        if !super::template::may_declare_a_template(docblock.text) {
            return None;
        }
        let text = docblock.text.to_owned();
        let arena = LocalArena::new();
        let content = arena.alloc_slice_copy(text.as_bytes());
        let document = PHPDocParser::parse(
            &arena,
            mago_database::file::FileId::new(b"docblock"),
            content,
        );

        let returned = super::tagged_type_in(&document, super::DocTag::Return)?;

        // The same reduction the conditional path makes: a sentinel beside the template is set
        // aside rather than treated as a second type, which is what lets `V` and `V|null` agree.
        // *Which* sentinel is kept, because that is the guard a reader has to write.
        let mut named = Vec::new();
        let mut absent = None;
        collect_branch(returned, &mut named, &mut absent);

        let first = named.first()?;
        let written = slice(&text, first.span());
        // Two different names is a union, and a union is what the declaration already said.
        if named
            .iter()
            .any(|leaf| slice(&text, leaf.span()) != written)
        {
            return None;
        }

        if let Some(branch) = self.bound_by_the_container(&document, &text, params, written, absent)
        {
            return Some(phrost_hir::ContractData::AgreedReturn(branch));
        }
        // A container of one column of a container of rows: `array_column`, whose `V` is what a
        // row holds rather than what the argument does.
        if absent.is_some() {
            return None;
        }
        let held = super::argument_binding::contained(first, &text)?;
        if let Some((parameter, column)) = self.bound_by_a_column(&document, &text, params, held) {
            return Some(phrost_hir::ContractData::AgreedReturn(
                phrost_hir::ReturnBranchData::ArgumentColumn { parameter, column },
            ));
        }
        // And every other container a template fills -- what a callback returns, the keys of an
        // argument -- read the way the stubs' conditional spelling of the same claim is. A
        // project's own `@return list<U>` beside `@param callable(V): U $map` is the plain form,
        // and it bound nothing while `array_map`'s four-branch version did.
        //
        // A builtin container only: `@return Collection<TKey, TValue>` is a class the arguments
        // belong to, and reading it as "a list of the elements" made every Laravel `collect()` an
        // array.
        use mago_phpdoc_syntax::cst::r#type::Type as DocType;
        if !matches!(
            first,
            DocType::Array(_)
                | DocType::NonEmptyArray(_)
                | DocType::AssociativeArray(_)
                | DocType::List(_)
                | DocType::NonEmptyList(_)
                | DocType::Iterable(_)
                | DocType::Slice(_)
        ) {
            return None;
        }
        self.holding_the_same(&document, &text, params, &[first])
            .map(phrost_hir::ContractData::AgreedReturn)
    }

    /// A `@return ($p is T ? A : B)` whose test a call can settle.
    ///
    /// The shape PHP's own array functions are written in, and the shape a framework helper is
    /// written in: Laravel's `response()` hands back a factory when it is passed nothing and a
    /// response when it is passed a body, and nothing but the argument says which.
    ///
    /// Only the tests an argument settles. A conditional on what an array *contains* --
    /// `$array is non-empty-array` -- is recorded as no contract at all, because reading it would
    /// take the argument's own type rather than what was written at the call.
    fn conditional_return(
        &mut self,
        declaration_start: phrost_model::TextSize,
        params: lspkit_arena::IdSlice<phrost_hir::ParamId>,
        written: &FunctionLikeParameterList<'_>,
    ) -> Option<phrost_hir::ContractData> {
        let docblock = self.raw_docblock_before(declaration_start)?;
        // Two markers, and a conditional cannot be written without either: the test is spelled
        // `$x is T`, and a `?` separates the branches. Prose has plenty of each and little of both.
        if !docblock.text.contains(" is ") || !docblock.text.contains('?') {
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

        let conditional = as_conditional(returned)?;

        // A branch naming one of the declaration's own `@template` parameters is refused whole. The
        // name resolves to no class, so lowering it produces one nothing declares -- and reporting
        // `iterator_to_array($it)` as an array of `V` objects is worse than reporting the plain
        // `array` its signature already promised. The two exceptions are a template a
        // `class-string<T>` or a container parameter binds, which `return_branch` answers from the
        // argument.
        let templates = super::template::declared_templates(&document);

        let argument =
            self.subject_argument(&document, &text, params, conditional.subject, &templates)?;
        if let Some(cases) = self.literal_cases(
            &document,
            &text,
            params,
            conditional,
            base,
            &templates,
            argument,
        ) {
            return Some(cases);
        }
        if let Some(chain) = self.test_chain(
            &document,
            &text,
            params,
            written,
            conditional,
            base,
            &templates,
            argument,
        ) {
            return Some(chain);
        }
        // A non-empty test is only worth keeping where absence is what the branches disagree about.
        // `array_values` is written `($array is non-empty-array ? non-empty-list<V> : list<V>)`,
        // where both branches are a container of the same thing and the keying is all that differs
        // -- there the agreed reduction below says everything a reader can use, and reading the test
        // instead would answer nothing for every call that has no guard above it. `reset` is
        // `V : V|false`, where the test is the whole of the difference.
        let test = self
            .condition_test(conditional.target, base, &text)
            .filter(|test| {
                !matches!(test, phrost_hir::ConditionTestData::NonEmpty)
                    || absence_is_the_difference(conditional, &text)
            });
        let Some(test) = test else {
            // A test nothing here can settle. The branches may still agree: `array_pop` gives back
            // `V` when the array had elements and `V|null` when it did not, which is what the array
            // held either way.
            return self.agreed_return(&document, &text, params, conditional, base, &templates);
        };
        // `is not` says the same thing with the branches the other way round, so they are swapped
        // here and nothing downstream has to carry a negation.
        let (then, otherwise) = match conditional.is_negated() {
            false => (conditional.then, conditional.r#else),
            true => (conditional.r#else, conditional.then),
        };
        let then = self.return_branch(&document, &text, params, then, base, &templates)?;
        let otherwise =
            self.return_branch(&document, &text, params, otherwise, base, &templates)?;
        Some(phrost_hir::ContractData::ConditionalReturn {
            argument,
            by_default: default_satisfies(written, argument, &test),
            test,
            then,
            otherwise,
        })
    }

    /// Which argument a conditional's subject is: a `$parameter`, or a `@template` one is declared as.
    ///
    /// `(T is 'array' ? ... : ...)` beside `@param T $type` says what `($type is 'array' ? ...)` says
    /// -- the template is whatever the call wrote there -- and Symfony's `prototype()`, which every
    /// configuration tree with a list in it calls, is written the first way. Read only the second,
    /// the branch was never picked and the body's own return answered for every call alike.
    pub(super) fn subject_argument(
        &self,
        document: &mago_phpdoc_syntax::cst::Document<'_>,
        text: &str,
        params: lspkit_arena::IdSlice<phrost_hir::ParamId>,
        subject: &Type<'_>,
        templates: &[std::borrow::Cow<'_, str>],
    ) -> Option<u16> {
        match subject {
            Type::Variable(variable) => self.parameter_position(params, variable_name(variable)),
            Type::Reference(reference) if reference.parameters.is_none() => {
                let named = super::slice(text, reference.kind.span());
                templates
                    .iter()
                    .any(|template| template == named)
                    .then_some(())?;
                self.bound_by_a_parameter(document, text, params, named)
            }
            _ => None,
        }
    }

    /// A conditional whose test nothing here settles, when every branch names the same thing.
    ///
    /// PHP's array API is written this way throughout -- `array_pop`, `array_shift`, `reset`, `end`
    /// all say "the element, or nothing if there was none" -- and the emptiness a test like that turns
    /// on is not something this tracks. What every branch agrees on is the whole of the answer, and
    /// the nulls set aside along the way are what makes it a `?T`.
    fn agreed_return(
        &mut self,
        document: &mago_phpdoc_syntax::cst::Document<'_>,
        text: &str,
        params: lspkit_arena::IdSlice<phrost_hir::ParamId>,
        conditional: &ConditionalType<'_>,
        base: phrost_model::TextSize,
        templates: &[std::borrow::Cow<'_, str>],
    ) -> Option<phrost_hir::ContractData> {
        self.agreed_branch(document, text, params, conditional, base, templates)
            .map(phrost_hir::ContractData::AgreedReturn)
    }

    /// What every branch of a conditional nothing settles agrees on, as one branch.
    ///
    /// [`Self::agreed_return`]'s reading, for a conditional anywhere in a tag: at the top, and as a
    /// branch of one whose own test a call does settle. `array_fill` is the second --
    /// `($start_index is 0 ? ($count is int<1, max> ? non-empty-list<T> : list<T>) : (...))` --
    /// where the inner test is a range nothing reads and both of its branches are a list of the
    /// value. Whichever way it goes the template is carried through, so the branch is that rather
    /// than a refusal of the whole contract over a test whose answer would not have mattered.
    fn agreed_branch(
        &mut self,
        document: &mago_phpdoc_syntax::cst::Document<'_>,
        text: &str,
        params: lspkit_arena::IdSlice<phrost_hir::ParamId>,
        conditional: &ConditionalType<'_>,
        base: phrost_model::TextSize,
        templates: &[std::borrow::Cow<'_, str>],
    ) -> Option<phrost_hir::ReturnBranchData> {
        // A sentinel is set aside rather than treated as a second type, which is what lets `V` and
        // `V|null` agree -- and which one it was is kept, because `null` and `false` need different
        // guards and folding them together made the right one powerless.
        let mut named = Vec::new();
        let mut absent = None;
        collect_leaves(conditional, &mut named, &mut absent);

        // One thing, or nothing doing. Two branches naming two types is a union, and a union is what
        // the declaration already said.
        let first = named.first()?;
        let written = slice(text, first.span());
        if named.iter().any(|leaf| slice(text, leaf.span()) != written) {
            // Unless they still agree about what the container *holds*. `array_values` gives back a
            // `non-empty-list<V>` or a `list<V>` and `array_map` has four spellings of the same
            // claim; the keying is what they disagree about, and the elements are what anybody
            // reading the result is after.
            //
            // Not when a leaf admitted the value being absent: what would be absent is the
            // container, and a branch carries no room to say so about one it is building.
            return absent
                .is_none()
                .then(|| self.holding_the_same(document, text, params, &named))
                .flatten();
        }

        if let Some(branch) = self.bound_by_the_container(document, text, params, written, absent) {
            return Some(branch);
        }
        // Not bound by anything, so it is a type -- unless it is a template, which names no class
        // and is refused for the reason `conditional_return` gives.
        if names_a_template(written, templates) {
            return None;
        }
        // `EMPTY`: a contract's own leaf types are not one of the tags aliases are read for -- see
        // `crates/lower/src/docblock/type_alias.rs`.
        let ty =
            self.lower_doc_type(first, base, text, &std::rc::Rc::clone(&self.class_aliases))?;
        Some(phrost_hir::ReturnBranchData::Ty(
            self.beside_the_sentinel(ty, absent),
        ))
    }

    /// Whether any leaf a conditional ends in names one of the declaration's own templates.
    ///
    /// The leaves and not the whole text: `(T is string ? Foo : Bar)` names a template in its test
    /// and none in what it gives back, and its union is a plain type.
    fn leaves_name_a_template(
        conditional: &ConditionalType<'_>,
        text: &str,
        templates: &[std::borrow::Cow<'_, str>],
    ) -> bool {
        let mut named = Vec::new();
        let mut absent = None;
        collect_leaves(conditional, &mut named, &mut absent);
        named
            .iter()
            .any(|leaf| names_a_template(slice(text, leaf.span()), templates))
    }

    /// Whether a container return is keyed the way `parameter`'s own argument is, which is what
    /// reducing it to that argument's elements takes for granted.
    ///
    /// True for a key that is no template -- `list<V>`, `array<int, V>` -- and for one bound by the
    /// keys of the same parameter, `iterator_to_array`'s `array<K, V>` over a `Traversable<K, V>`.
    /// False for `array_combine`'s `array<K, V>`, keyed by another argument's elements.
    /// The branch for a container of one parameter's elements, keyed as the leaf says.
    ///
    /// The elements' own keys where the leaf names none or names that parameter's; the elements of
    /// a *second* parameter where the leaf's key is a template that one binds -- `array_combine`'s
    /// `array<K, V>` beside `array<K> $keys` and `array<V> $values`. Nothing where the key is bound
    /// some other way, which is a keying this cannot state.
    fn elements_keyed(
        &self,
        document: &mago_phpdoc_syntax::cst::Document<'_>,
        text: &str,
        params: lspkit_arena::IdSlice<phrost_hir::ParamId>,
        leaves: &[&Type<'_>],
        templates: &[std::borrow::Cow<'_, str>],
        parameter: u16,
    ) -> Option<phrost_hir::ReturnBranchData> {
        if leaves
            .iter()
            .all(|leaf| self.keyed_as(document, text, params, leaf, templates, parameter))
        {
            return Some(phrost_hir::ReturnBranchData::ArgumentElements { parameter });
        }
        let mut keys = None;
        for leaf in leaves {
            let key = super::argument_binding::keyed_by_a_template(leaf, text, templates)?;
            let bound = self.bound_by_an_element(document, text, params, key)?;
            if bound == parameter || keys.is_some_and(|keys| keys != bound) {
                return None;
            }
            keys = Some(bound);
        }
        Some(phrost_hir::ReturnBranchData::CombinedElements {
            keys: keys?,
            values: parameter,
        })
    }

    fn keyed_as(
        &self,
        document: &mago_phpdoc_syntax::cst::Document<'_>,
        text: &str,
        params: lspkit_arena::IdSlice<phrost_hir::ParamId>,
        leaf: &Type<'_>,
        templates: &[std::borrow::Cow<'_, str>],
        parameter: u16,
    ) -> bool {
        match super::argument_binding::keyed_by_a_template(leaf, text, templates) {
            None => true,
            Some(key) => self.bound_by_the_keys(document, text, params, key) == Some(parameter),
        }
    }

    /// A container of whatever every leaf holds, when they all hold the same thing.
    ///
    /// The element has to be bound by an argument for this to say anything: a leaf spelled
    /// `list<Foo>` names a type outright and the branches would have agreed textually, so what is
    /// left here is a template -- read either off a container parameter or off a callable's own
    /// return type.
    fn holding_the_same(
        &self,
        document: &mago_phpdoc_syntax::cst::Document<'_>,
        text: &str,
        params: lspkit_arena::IdSlice<phrost_hir::ParamId>,
        leaves: &[&Type<'_>],
    ) -> Option<phrost_hir::ReturnBranchData> {
        let held = contained(leaves.first()?, text)?;
        if leaves
            .iter()
            .any(|leaf| contained(leaf, text) != Some(held))
        {
            return None;
        }
        if let Some(parameter) = self.bound_by_an_element(document, text, params, held) {
            let templates = super::template::declared_templates(document);
            return self.elements_keyed(document, text, params, leaves, &templates, parameter);
        }
        // The keys of the same parameter -- `array_keys`, whose branches are `non-empty-list<K>` and
        // `list<K>` and so agree here rather than reaching `elements_branch`.
        if let Some(parameter) = self.bound_by_the_keys(document, text, params, held) {
            return Some(phrost_hir::ReturnBranchData::ArgumentKeys { parameter });
        }
        // A parameter declared as the template outright -- `array_fill`'s `@param T $value` -- so the
        // container holds whatever the call passed. Before the callback, for the reason a bare
        // `@return T` asks in that order: a template bound both ways means the parameter.
        //
        // Not one the parameter may also *produce* (`T|(\Closure(): T)`): a closure passed there
        // would be read as the element rather than what calling it gives. And only keyed by `int`,
        // which is the one keying the reader's container states.
        if let Some((parameter, evaluated)) =
            self.bound_by_a_parameter_evaluated(document, text, params, held)
        {
            return (!evaluated && leaves.iter().all(|leaf| keyed_by_an_int(leaf)))
                .then_some(phrost_hir::ReturnBranchData::ArgumentTypes { parameter });
        }
        let parameter = self.bound_by_a_callback_return(document, text, params, held)?;
        Some(phrost_hir::ReturnBranchData::CallbackResults { parameter })
    }

    /// What a conditional type tests its subject against, when it is something a call settles.
    /// A shape test, for the one form a call can settle from the array it wrote: a single key that
    /// is certainly there, holding one of a few literals. See [`ConditionTestData::Shape`].
    ///
    /// [`ConditionTestData::Shape`]: phrost_hir::ConditionTestData::Shape
    fn shape_test(
        &mut self,
        shape: &mago_phpdoc_syntax::cst::r#type::shape::ShapeType<'_>,
    ) -> Option<phrost_hir::ConditionTestData> {
        use mago_phpdoc_syntax::cst::r#type::shape::ShapeKey;
        if shape.fields.len() != 1 {
            return None;
        }
        let field = shape.fields.first()?;
        let written = field.key.as_ref()?;
        if written.question_mark.is_some() {
            return None;
        }
        let key = match &written.key {
            ShapeKey::String { value, .. } => self.interner.name(&String::from_utf8_lossy(value)),
            ShapeKey::Integer { value, .. } => self.interner.name(&value.to_string()),
            ShapeKey::ClassLikeConstant { .. } => return None,
        };
        let mut accepts = Vec::new();
        self.shape_literals(field.value, &mut accepts)?;
        Some(phrost_hir::ConditionTestData::Shape {
            key,
            accepts: accepts.into_boxed_slice(),
            sealed: shape.additional_fields.is_none(),
        })
    }

    /// The literals a shape field's type is a union of, or `None` for anything that is not one.
    fn shape_literals(
        &mut self,
        ty: &Type<'_>,
        into: &mut Vec<phrost_hir::ShapeLiteralData>,
    ) -> Option<()> {
        use phrost_hir::ShapeLiteralData as Literal;
        match ty {
            Type::Parenthesized(inner) => return self.shape_literals(inner.inner, into),
            Type::Union(union) => {
                self.shape_literals(union.left, into)?;
                return self.shape_literals(union.right, into);
            }
            Type::LiteralString(literal) => into.push(Literal::String(
                self.interner.name(&String::from_utf8_lossy(literal.value)),
            )),
            Type::LiteralInt(literal) => {
                into.push(Literal::Int(i64::try_from(literal.value).ok()?))
            }
            Type::True(_) => into.push(Literal::True),
            Type::False(_) => into.push(Literal::False),
            Type::Null(_) => into.push(Literal::Null),
            _ => return None,
        }
        Some(())
    }

    pub(crate) fn condition_test(
        &mut self,
        target: &Type<'_>,
        base: phrost_model::TextSize,
        text: &str,
    ) -> Option<phrost_hir::ConditionTestData> {
        use phrost_hir::ConditionTestData as Test;
        Some(match target {
            Type::Null(_) => Test::Null,
            Type::True(_) => Test::True,
            Type::False(_) => Test::False,
            // The whole class-string family, because what they share is the only thing the test
            // needs: that the argument *names* a class rather than being one. And what a
            // `class-string<Foo>` is written over, which the reader compares the named class with --
            // a template there lowers to a class nothing declares, which the reader takes as no bound.
            Type::ClassString(class_string) => Test::ClassString(
                class_string
                    .parameter
                    .as_ref()
                    .filter(|parameter| matches!(parameter.entry.inner, Type::Reference(_)))
                    .and_then(|parameter| {
                        self.lower_doc_type(
                            &parameter.entry.inner,
                            base,
                            text,
                            &std::rc::Rc::clone(&self.class_aliases),
                        )
                    }),
            ),
            Type::ClassLikeString(_)
            | Type::InterfaceString(_)
            | Type::EnumString(_)
            | Type::TraitString(_) => Test::ClassString(None),
            Type::LiteralString(literal) => {
                Test::Literal(self.interner.name(&String::from_utf8_lossy(literal.value)))
            }
            // `($flags is 15 ? array : string)`, which is how the standard library says that a flag
            // decides the shape of what comes back. The call writes the constant rather than the
            // number, so what is compared is the constant's *value* -- see `literal_satisfies`.
            Type::LiteralInt(literal) => Test::Number(i64::try_from(literal.value).ok()?),
            // `-1`, which a docblock writes as a minus in front of a number -- and which
            // `wp_parse_url()` tests its component against to mean "all of it".
            Type::Negated(negated) => match negated.operand {
                Type::LiteralInt(literal) => {
                    Test::Number(i64::try_from(literal.value).ok()?.checked_neg()?)
                }
                _ => return None,
            },
            // What sort of value the argument is, which the reader settles by resolving it. Bare
            // only: `array<Foo>` is a claim about what the array *contains*, and reading it as "an
            // array" would take the then-branch for every array there is.
            //
            // `preg_replace` is the shape, and PHP's string API is full of it: one signature covers a
            // string and an array of them, and which came back is decided by what went in.
            Type::String(_) => Test::OfKind(BuiltinTy::String),
            Type::Int(_) => Test::OfKind(BuiltinTy::Int),
            Type::Float(_) => Test::OfKind(BuiltinTy::Float),
            Type::Bool(_) => Test::OfKind(BuiltinTy::Bool),
            Type::Object(object) if object.properties.is_none() => Test::OfKind(BuiltinTy::Object),
            Type::Array(array) if array.parameters.is_none() => Test::OfKind(BuiltinTy::Array),
            // `array<mixed>` and `array<array-key, mixed>` are the bare kind spelled out: they say
            // nothing about what the array holds, so answering "an array" takes nothing on trust.
            Type::Array(array)
                if array.parameters.as_ref().is_some_and(|parameters| {
                    parameters
                        .entries
                        .iter()
                        .all(|entry| matches!(entry.inner, Type::Mixed(_) | Type::ArrayKey(_)))
                }) =>
            {
                Test::OfKind(BuiltinTy::Array)
            }
            Type::Parenthesized(inner) => return self.condition_test(inner.inner, base, text),
            Type::List(list) if list.parameters.is_none() => Test::OfKind(BuiltinTy::Array),
            Type::Iterable(iterable) if iterable.parameters.is_none() => {
                Test::OfKind(BuiltinTy::Iterable)
            }
            Type::Callable(_) => Test::OfKind(BuiltinTy::Callable),
            Type::Scalar(_) => Test::Scalar,
            // What a string holds, which only the call's own text can say -- see the reader.
            Type::NonEmptyString(_) => Test::NonEmptyString,
            Type::NonFalsyString(_) | Type::TruthyString(_) => Test::NonFalsyString,
            // Whether the argument holds anything, which the reader settles from a guard rather
            // than from a type: an empty array is falsy in PHP, so "not empty" is exactly "truthy",
            // and that is a claim the narrowing machinery already makes. This used to be refused
            // outright, and `reset($rows)` after `if (empty($rows)) { return; }` answered `?V` for
            // a value that cannot be absent.
            //
            // Bare only, like the kinds above: `non-empty-array<Foo>` says something about the
            // elements too, and answering it as "not empty" would take the then-branch on a claim
            // half of which nothing here checked.
            Type::NonEmptyArray(array) if array.parameters.is_none() => Test::NonEmpty,
            Type::NonEmptyList(list) if list.parameters.is_none() => Test::NonEmpty,
            // `list{}` and `array{}`, a shape with nothing in it and nothing beyond what it lists.
            Type::Shape(shape) if shape.fields.is_empty() && shape.additional_fields.is_none() => {
                Test::Empty
            }
            // `array{ fields: 'ids', ... }` -- one key, holding one of the literals listed.
            Type::Shape(shape) => return self.shape_test(shape),
            // A union of tests, which is one test when every operand asks the same question.
            // `($array is non-empty-array|non-empty-list ? V : null)` is how the 8.5 stubs write
            // `array_first` and `array_last`, and reading only single tests left both answering
            // `?V` behind a guard that had already proven the array holds something. Every operand,
            // because a union asking two different questions is two tests and this vocabulary
            // carries one.
            //
            // Or a kind beside one class, which is a test of its own: `Arrayable|array`.
            Type::Union(union) => {
                if let Some(test) = self.kind_or_instance(union.left, union.right, base, text) {
                    return Some(test);
                }
                let left = self.condition_test(union.left, base, text)?;
                let right = self.condition_test(union.right, base, text)?;
                match left == right {
                    true => left,
                    false => return None,
                }
            }
            // An instance of one class: `$closure is \Closure`, `$post is WP_Post`. Bare only, for the
            // reason the kinds above are: `Collection<User>` is a claim about what the collection
            // holds, and answering it from the class would take the then-branch for every one.
            Type::Reference(reference)
                if reference.parameters.is_none()
                    && matches!(
                        reference.kind,
                        mago_phpdoc_syntax::cst::r#type::ReferenceKind::Identifier(_)
                    ) =>
            {
                Test::Instance(self.lower_doc_type(
                    target,
                    base,
                    text,
                    &std::rc::Rc::clone(&self.class_aliases),
                )?)
            }
            // Kept out on purpose: see [`Self::conditional_return`]. The span is unused here, and
            // taking it would only make the refusal look like a parse failure.
            _ => {
                let _ = text;
                return None;
            }
        })
    }

    /// A chain of tests of any kind -- see [`phrost_hir::ContractData::TestChain`]. The same walk
    /// [`Self::literal_cases`] makes, for the chains whose tests are not all strings or not all on
    /// one argument: WordPress tests the shape of `$args`, then a string in `$output`.
    ///
    /// The rest of a chain may be nested on either side. `($a is T ? X : (...))` continues in the
    /// else, and `($a is T ? (...) : X)` in the then, where the link picks `X` when the test fails --
    /// `wpdb::get_row()` is `($query is non-falsy-string ? ($output is 'OBJECT' ? ...) : null)`. A
    /// link with a conditional on both sides is not a chain, and the then side is read as the union
    /// a nested conditional reads as.
    ///
    /// A non-empty test refuses the chain. What settles it is a guard rather than the call, and an
    /// unsettled one is read as both branches at once -- which a chain has no way to say.
    #[allow(clippy::too_many_arguments)]
    fn test_chain(
        &mut self,
        document: &mago_phpdoc_syntax::cst::Document<'_>,
        text: &str,
        params: lspkit_arena::IdSlice<phrost_hir::ParamId>,
        written: &FunctionLikeParameterList<'_>,
        conditional: &ConditionalType<'_>,
        base: phrost_model::TextSize,
        templates: &[std::borrow::Cow<'_, str>],
        argument: u16,
    ) -> Option<phrost_hir::ContractData> {
        let _ = argument;
        let mut cases = Vec::new();
        let mut current = conditional;
        let mut unread = false;
        let otherwise = loop {
            let argument =
                self.subject_argument(document, text, params, current.subject, templates)?;
            // `is not` picks the then branch where the test fails, and a chain continuing in the then
            // picks the else branch there -- so the two flips cancel.
            let (picked, rest, negated) =
                match (as_conditional(current.then), as_conditional(current.r#else)) {
                    (Some(inner), None) => (current.r#else, Some(inner), !current.is_negated()),
                    (_, rest) => (current.then, rest, current.is_negated()),
                };
            // A union of literals is a link per literal, all picking one branch. Not when negated:
            // failing every one of them is not a thing separate links can say.
            let literals = match (negated, current.target) {
                (false, Type::Union(_)) => self.literal_strings(current.target),
                _ => None,
            };
            if let Some(literals) = literals {
                let then = self.return_branch(document, text, params, picked, base, templates)?;
                for literal in literals {
                    let test = phrost_hir::ConditionTestData::Literal(literal);
                    cases.push(phrost_hir::TestCaseData {
                        argument,
                        by_default: default_satisfies(written, argument, &test),
                        test,
                        negated,
                        then: then.clone(),
                    });
                }
                match rest {
                    Some(next) => {
                        current = next;
                        continue;
                    }
                    None => {
                        break self.return_branch(
                            document,
                            text,
                            params,
                            current.r#else,
                            base,
                            templates,
                        )?;
                    }
                }
            }
            // A link this cannot test ends the chain rather than refusing it: whatever that link
            // decides, the value is one of the branches below it -- the union a nested conditional
            // inside a contract already reads as. `paginate_comments_links()` ends on `$args is
            // ''|array`, which settles nothing here, after two tests that settle every call.
            let Some(test) = self
                .condition_test(current.target, base, text)
                .filter(|test| !matches!(test, phrost_hir::ConditionTestData::NonEmpty))
            else {
                if cases.is_empty() {
                    return None;
                }
                // What the rest agrees on, where its leaves carry a template -- `array_fill`'s
                // `($count is int<1, max> ? non-empty-array<int, T> : array<int, T>)` is a container
                // of the value either way, and its union would name a class called `T`. That is an
                // ordinary else rather than an unread rest, so one link before it is an ordinary
                // conditional, whose reader answers a call that settles nothing with both branches.
                if Self::leaves_name_a_template(current, text, templates)
                    && let Some(agreed) =
                        self.agreed_branch(document, text, params, current, base, templates)
                {
                    break agreed;
                }
                unread = true;
                break phrost_hir::ReturnBranchData::Ty(
                    self.conditional_union(current, base, text)?,
                );
            };
            let then = self.return_branch(document, text, params, picked, base, templates)?;
            let by_default = default_satisfies(written, argument, &test);
            cases.push(phrost_hir::TestCaseData {
                argument,
                test,
                negated,
                then,
                by_default,
            });
            match rest {
                Some(next) => current = next,
                None => {
                    break self.return_branch(
                        document,
                        text,
                        params,
                        current.r#else,
                        base,
                        templates,
                    )?;
                }
            }
        };
        // One link with an ordinary else is an ordinary conditional, and left to that reader. One
        // link and an unreadable rest is worth keeping as a chain: the test it does settle is the
        // one that matters, and the rest is their union.
        (cases.len() >= 2 || unread)
            .then_some(phrost_hir::ContractData::TestChain { cases, otherwise })
    }

    /// A chain of string tests on one argument, when there are at least two -- see
    /// [`phrost_hir::ContractData::LiteralCases`]. One test is an ordinary conditional and is left to
    /// it; anything in the chain that is not a string test on the same argument refuses the whole.
    #[allow(clippy::too_many_arguments)]
    fn literal_cases(
        &mut self,
        document: &mago_phpdoc_syntax::cst::Document<'_>,
        text: &str,
        params: lspkit_arena::IdSlice<phrost_hir::ParamId>,
        conditional: &ConditionalType<'_>,
        base: phrost_model::TextSize,
        templates: &[std::borrow::Cow<'_, str>],
        argument: u16,
    ) -> Option<phrost_hir::ContractData> {
        let mut cases = Vec::new();
        let mut current = conditional;
        let otherwise = loop {
            if current.is_negated() {
                return None;
            }
            if self.subject_argument(document, text, params, current.subject, templates)?
                != argument
            {
                return None;
            }
            let literals = self.literal_strings(current.target)?;
            let then = self.return_branch(document, text, params, current.then, base, templates)?;
            cases.extend(literals.into_iter().map(|literal| (literal, then.clone())));
            match as_conditional(current.r#else) {
                Some(next) => current = next,
                None => {
                    break self.return_branch(
                        document,
                        text,
                        params,
                        current.r#else,
                        base,
                        templates,
                    )?;
                }
            }
        };
        (cases.len() >= 2).then_some(phrost_hir::ContractData::LiteralCases {
            argument,
            cases,
            otherwise,
        })
    }

    /// The strings a test names: one literal, or a union of them.
    ///
    /// `get_post_field()` is `($field is 'ID'|'post_parent'|'menu_order' ? int|'' : ...)`, and a
    /// union of literals is the same branch reached from each -- so it is read as one case per
    /// literal, which every chain reader already settles, rather than as a test of its own.
    fn literal_strings(&mut self, target: &Type<'_>) -> Option<Vec<phrost_model::Name>> {
        match target {
            Type::LiteralString(literal) => Some(vec![
                self.interner.name(&String::from_utf8_lossy(literal.value)),
            ]),
            Type::Parenthesized(inner) => self.literal_strings(inner.inner),
            Type::Union(union) => {
                let mut literals = self.literal_strings(union.left)?;
                literals.extend(self.literal_strings(union.right)?);
                Some(literals)
            }
            _ => None,
        }
    }

    /// `Arrayable|array`, in either order: a kind test beside a class written as one.
    fn kind_or_instance(
        &mut self,
        left: &Type<'_>,
        right: &Type<'_>,
        base: phrost_model::TextSize,
        text: &str,
    ) -> Option<phrost_hir::ConditionTestData> {
        let (kind, class) = match (left, right) {
            (Type::Reference(_), other) => (other, left),
            (other, Type::Reference(_)) => (other, right),
            _ => return None,
        };
        let phrost_hir::ConditionTestData::OfKind(kind) = self.condition_test(kind, base, text)?
        else {
            return None;
        };
        let class =
            self.lower_doc_type(class, base, text, &std::rc::Rc::clone(&self.class_aliases))?;
        Some(phrost_hir::ConditionTestData::OfKindOrInstance { kind, class })
    }

    /// One branch of a conditional return type.
    fn return_branch(
        &mut self,
        document: &mago_phpdoc_syntax::cst::Document<'_>,
        text: &str,
        params: lspkit_arena::IdSlice<phrost_hir::ParamId>,
        branch: &Type<'_>,
        base: phrost_model::TextSize,
        templates: &[std::borrow::Cow<'_, str>],
    ) -> Option<phrost_hir::ReturnBranchData> {
        // A bare name a `class-string<T>` parameter binds is not a class, and asking before lowering
        // is the point: lowering resolves a name against the file's imports, and `T` resolves to a
        // class nothing declares.
        if let Type::Reference(reference) = branch
            && let Some(position) = self.bound_by_a_class_string(
                document,
                text,
                params,
                slice(text, reference.kind.span()),
            )
        {
            return Some(phrost_hir::ReturnBranchData::ArgumentClass(position));
        }
        // A branch that reduces to one of the declaration's own templates names no class, so
        // lowering it builds a reference to something nothing declares. What it does name is an
        // argument's elements, wherever a `@param` binds it -- and that is asked before the refusal
        // below rather than after, because `(V|false)` is not spelled `V` and so never reaches it.
        if let Some(element) = self.element_branch(document, text, params, branch, templates) {
            return Some(element);
        }
        // And a *container* of one, which is the same claim one level out: `iterator_to_array`'s
        // `array<K, V>` hands back what the argument held, keyed differently. Lowered as written it
        // becomes a generic over two class names nothing declares, so the result read as a bare
        // `array` -- an answer shaped right and empty.
        if let Some(elements) = self.elements_branch(document, text, params, branch, templates) {
            return Some(elements);
        }
        // And the template a parameter is declared as outright: `@param TValue $value` beside
        // `($callback is null ? HigherOrderTapProxy : TValue)` is whatever the call passed. The
        // reader of a `@return Bar<TValue>` slot asked this all along and the reader of a branch
        // did not, so Laravel's `tap()` refused its whole contract over the branch it is for.
        if let Some(passed) = self.passed_branch(document, text, params, branch, templates) {
            return Some(passed);
        }
        // And the template a callable parameter is declared to return: Laravel's `optional()` is
        // `($callback is null ? Optional : ...TReturn)` beside `(callable(TValue): TReturn)|null`.
        // The bare return and a `Bar<TReturn>` slot read this already; refused here, it took the
        // `Optional` branch down with it -- the one every call that passes no callback settles.
        if let Some(parameter) =
            self.bound_by_a_callback_return(document, text, params, slice(text, branch.span()))
        {
            return Some(phrost_hir::ReturnBranchData::CallbackResult { parameter });
        }
        // A conditional inside the branch whose own test nothing reads, and whose leaves carry a
        // template: whichever way that test goes the branch is one of them, so what they agree on is
        // the branch. Lowered as their union instead, each template is a class nothing declares.
        if let Some(nested) = as_conditional(branch)
            && Self::leaves_name_a_template(nested, text, templates)
            && let Some(agreed) =
                self.agreed_branch(document, text, params, nested, base, templates)
        {
            return Some(agreed);
        }
        if names_a_template(slice(text, branch.span()), templates) {
            return self
                .bounded_branch(document, text, params, branch, base, templates)
                .or_else(|| self.erased_container(document, text, params, branch, templates));
        }
        Some(phrost_hir::ReturnBranchData::Ty(
            self.branch_type(branch, base, text)?,
        ))
    }

    /// A branch as a type: itself, or -- for a branch that is another conditional this did not
    /// take apart -- the union of the branches it ends in.
    ///
    /// Laravel's `app()` is `($abstract is class-string<T> ? T : ($abstract is null ? Application :
    /// mixed))`. The inner conditional has no written-PHP spelling, so it lowered to nothing, the
    /// outer branch with it, and the whole contract: `app()->has(...)` and `app(Vite::class)`
    /// answered nothing at all. Whichever way the inner test goes it is one of its branches, so
    /// their union is what that branch says -- and the outer test, which a call does settle, still
    /// picks between it and `T`. Mockery's `expects()` is the same shape, and its expectation chain
    /// is most of a Laravel test.
    ///
    /// The same reading a conditional at the top of a tag gets as the declared type, where it is what
    /// a call nothing settles gets back -- see `type_lowering`.
    fn branch_type(
        &mut self,
        branch: &Type<'_>,
        base: phrost_model::TextSize,
        text: &str,
    ) -> Option<phrost_hir::TypeRefId> {
        match branch {
            Type::Parenthesized(inner) => self.branch_type(inner.inner, base, text),
            Type::Conditional(nested) => self.conditional_union(nested, base, text),
            _ => self.lower_doc_type(branch, base, text, &std::rc::Rc::clone(&self.class_aliases)),
        }
    }

    /// A conditional as the union of the branches it ends in.
    fn conditional_union(
        &mut self,
        conditional: &ConditionalType<'_>,
        base: phrost_model::TextSize,
        text: &str,
    ) -> Option<phrost_hir::TypeRefId> {
        let then = self.branch_type(conditional.then, base, text)?;
        let otherwise = self.branch_type(conditional.r#else, base, text)?;
        let parts = self.builder.alloc_type_ref_list(&[then, otherwise]);
        Some(self.builder.add_type_ref(TypeRefData::Union(parts)))
    }

    /// A branch with a template inside it that no argument binds, lowered with each template
    /// standing for its declared bound.
    ///
    /// `substr_replace` is `($string is string ? string : array<K, string>)` beside `@template K as
    /// array-key`: the key of an array that is not the argument's own, and nothing above reads that.
    /// Refusing the branch refused the whole conditional, so every call -- on a string, which is
    /// nearly all of them -- read as the declared `array|string`, and `strlen()` around it was an
    /// `incompatible-argument`. `array<array-key, string>` is what the declaration promises for
    /// whichever `K` it is, which is a weaker claim and a true one.
    ///
    /// Not a template on its own: a bare `T` is a value whose type is the template, and its bound is
    /// what every argument has in common rather than what this one is -- the refusal above stands
    /// for it. Nor a template declared with no bound, which stands for nothing that could be lowered.
    ///
    /// And not a template an argument binds the way the call-site substitution reads one -- a
    /// `class-string<T>`, a `T $value`, a `T[] $items`. Refusing the branch is what hands those to
    /// it: Messenger's `Envelope::all()` is `(... ? ... : list<TStamp>)` beside `class-string<TStamp>`,
    /// and its bound read every stamp as the bare `StampInterface`, where the substitution had
    /// answered the class the call names. Bound by an argument's *keys*, as `substr_replace`'s `K`
    /// is, is a binding nothing downstream reads, so the bound is the best answer there is.
    fn bounded_branch(
        &mut self,
        document: &mago_phpdoc_syntax::cst::Document<'_>,
        text: &str,
        params: lspkit_arena::IdSlice<phrost_hir::ParamId>,
        branch: &Type<'_>,
        base: phrost_model::TextSize,
        templates: &[std::borrow::Cow<'_, str>],
    ) -> Option<phrost_hir::ReturnBranchData> {
        let written = slice(text, branch.span()).trim();
        if templates
            .iter()
            .any(|template| template.as_ref() == written)
        {
            return None;
        }
        let mut standing = Vec::new();
        for element in document.elements.iter() {
            let Element::Tag(tag) = element else {
                continue;
            };
            let TagValue::Template(value) = &tag.value else {
                continue;
            };
            let name = String::from_utf8_lossy(value.name.value);
            if !names_a_template(written, std::slice::from_ref(&name)) {
                continue;
            }
            if self
                .bound_by_a_class_string(document, text, params, &name)
                .or_else(|| self.bound_by_a_parameter(document, text, params, &name))
                .or_else(|| self.bound_by_an_element(document, text, params, &name))
                .is_some()
            {
                return None;
            }
            let bound = value.bound.and_then(|bound| {
                self.lower_doc_type(
                    bound.r#type,
                    base,
                    text,
                    &std::rc::Rc::clone(&self.class_aliases),
                )
            })?;
            standing.push((self.interner.name(&name), bound));
        }
        let table = TypeAliasTable::standing_for(standing);
        Some(phrost_hir::ReturnBranchData::Ty(
            self.lower_doc_type(branch, base, text, &table)?,
        ))
    }

    /// A builtin container whose arguments say something about a template nothing here reads, as
    /// the container alone.
    ///
    /// `wp_unslash()` hands an array back as `array<key-of<T>, (value-of<T> is string ? string :
    /// value-of<T>)>`: an array of what the argument held, which no branch reader takes apart. Refused,
    /// the branch took its whole contract with it, and a call on a string answered nothing; `array` is
    /// what the branch says however `T` was bound, and a weaker claim than the truth rather than a
    /// different one.
    ///
    /// Not where a `class-string<T>` binds a template the branch names: the call-site substitution
    /// reads those, and Messenger's `all(Foo::class)` is a `list<Foo>` there -- erased here it would
    /// be an `array`, answered first and confidently.
    fn erased_container(
        &mut self,
        document: &mago_phpdoc_syntax::cst::Document<'_>,
        text: &str,
        params: lspkit_arena::IdSlice<phrost_hir::ParamId>,
        branch: &Type<'_>,
        templates: &[std::borrow::Cow<'_, str>],
    ) -> Option<phrost_hir::ReturnBranchData> {
        let written = slice(text, branch.span());
        let bound_by_a_class = templates.iter().any(|template| {
            names_a_template(written, std::slice::from_ref(template))
                && self
                    .bound_by_a_class_string(document, text, params, template)
                    .is_some()
        });
        if bound_by_a_class {
            return None;
        }
        let kind = container_kind(branch)?;
        Some(phrost_hir::ReturnBranchData::Ty(
            self.builder.add_type_ref(TypeRefData::Builtin(kind)),
        ))
    }

    /// A container of what an argument settles -- its elements, its keys, the argument itself, or a
    /// callback's result -- when that is what a branch is written as.
    ///
    /// [`Self::holding_the_same`] makes this reduction where a conditional's branches disagree only
    /// about keying; a branch of a conditional whose test *is* settled is the second place it is
    /// needed, and `iterator_to_array` is the shape -- `($preserve_keys is true ? array<K, V> :
    /// list<V>)`, whose then-branch wins by the declaration's own default.
    ///
    /// Only a template, for [`Self::element_branch`]'s reason: `list<Foo>` names a type outright and
    /// lowers to it, and reading that as the call site's element would trade an answer for a guess.
    fn elements_branch(
        &self,
        document: &mago_phpdoc_syntax::cst::Document<'_>,
        text: &str,
        params: lspkit_arena::IdSlice<phrost_hir::ParamId>,
        branch: &Type<'_>,
        templates: &[std::borrow::Cow<'_, str>],
    ) -> Option<phrost_hir::ReturnBranchData> {
        let held = contained(branch, text)?;
        if !names_a_template(held, templates) {
            return None;
        }
        // One leaf of what [`Self::holding_the_same`] reads for several, and the same reading:
        // the elements, the keys, the argument itself, a callback's result. This had its own copy
        // of the first two and so learned neither of the others -- `array_fill`'s list of the value
        // it was given read as nothing here while the agreed path beside it answered.
        self.holding_the_same(document, text, params, &[branch])
    }

    /// The elements -- or the keys -- of the argument a branch's single template names, with absence
    /// set aside.
    ///
    /// The third place the same reduction is needed. [`Self::agreed_return`] makes it when a
    /// conditional's branches agree and [`Self::element_return`] makes it for a plain `@return V`;
    /// a *branch* of a conditional whose test an argument does settle is the one that was missing,
    /// which is the whole of why `current($rows)` answered `mixed` while `reset($rows)` answered
    /// `?Row`. Both stubs bind `V` to the array's elements; `current`'s
    /// `($array is object ? mixed : (V|false))` merely writes the claim inside a branch.
    ///
    /// Only a template is rescued this way. A branch naming a real type already lowers to that type,
    /// and an array parameter that happens to hold it would turn an answer into whatever the call
    /// site's array is typed as -- more precise where it is typed and a miss where it is not.
    /// A branch that is exactly one template a parameter is declared as -- `TValue`, and not
    /// `TValue|null`, which would be a second claim beside what the call passed and has no slot
    /// here. What a left-out argument's default says is not read either: the parameter a branch
    /// hands back is the one a call writes.
    fn passed_branch(
        &self,
        document: &mago_phpdoc_syntax::cst::Document<'_>,
        text: &str,
        params: lspkit_arena::IdSlice<phrost_hir::ParamId>,
        branch: &Type<'_>,
        templates: &[std::borrow::Cow<'_, str>],
    ) -> Option<phrost_hir::ReturnBranchData> {
        let written = slice(text, branch.span());
        if !names_a_template(written, templates) {
            return None;
        }
        let (parameter, evaluated) =
            self.bound_by_a_parameter_evaluated(document, text, params, written)?;
        Some(phrost_hir::ReturnBranchData::ArgumentType {
            parameter,
            absent: None,
            evaluated,
        })
    }

    fn element_branch(
        &self,
        document: &mago_phpdoc_syntax::cst::Document<'_>,
        text: &str,
        params: lspkit_arena::IdSlice<phrost_hir::ParamId>,
        branch: &Type<'_>,
        templates: &[std::borrow::Cow<'_, str>],
    ) -> Option<phrost_hir::ReturnBranchData> {
        // A sentinel is the value not being there rather than a second type, which is what lets
        // `V` and `V|false` reduce to the same name -- and which sentinel it was travels with the
        // branch, because `reset` says `false` and a reader has to test for `false`.
        let mut named = Vec::new();
        let mut absent = None;
        collect_branch(branch, &mut named, &mut absent);

        let first = named.first()?;
        let written = slice(text, first.span());
        // Two different names is a union, and a union is what the declaration already said.
        if named.iter().any(|leaf| slice(text, leaf.span()) != written) {
            return None;
        }
        if !names_a_template(written, templates) {
            return None;
        }

        self.bound_by_the_container(document, text, params, written, absent)
    }

    /// A leaf type with the sentinel its branch also admitted put back beside it.
    ///
    /// The one place that turns an [`Absence`] into a type, so the rule that `null` wraps and
    /// `false` is an operand lives once. `?T` for a null because that is the spelling PHP has, and a
    /// union for the false because there is no other -- which is the same split the interned union
    /// constructor makes one layer up.
    fn beside_the_sentinel(
        &mut self,
        ty: phrost_hir::TypeRefId,
        absent: Option<Absence>,
    ) -> phrost_hir::TypeRefId {
        let Some(absent) = absent else {
            return ty;
        };
        let ty = match absent.admits_false() {
            true => {
                let no = self
                    .builder
                    .add_type_ref(TypeRefData::Builtin(BuiltinTy::False));
                let parts = self.builder.alloc_type_ref_list(&[ty, no]);
                self.builder.add_type_ref(TypeRefData::Union(parts))
            }
            false => ty,
        };
        match absent.admits_null() {
            true => self.builder.add_type_ref(TypeRefData::Nullable(ty)),
            false => ty,
        }
    }
}

/// The builtin a written container is, whatever it was written to hold.
fn container_kind(ty: &Type<'_>) -> Option<BuiltinTy> {
    match ty {
        Type::Parenthesized(inner) => container_kind(inner.inner),
        Type::Array(_)
        | Type::NonEmptyArray(_)
        | Type::AssociativeArray(_)
        | Type::List(_)
        | Type::NonEmptyList(_)
        | Type::Shape(_) => Some(BuiltinTy::Array),
        Type::Iterable(_) => Some(BuiltinTy::Iterable),
        _ => None,
    }
}

/// Whether a written container is keyed by `int`: a list, or an array whose key is spelled `int`.
///
/// `T[]` and `array<T>` state no key at all, and an `iterable` may be keyed by anything, so none of
/// them is -- reading one as a list would claim `int` keys the declaration never wrote.
fn keyed_by_an_int(ty: &Type<'_>) -> bool {
    let parameters = match ty {
        Type::Parenthesized(inner) => return keyed_by_an_int(inner.inner),
        Type::List(_) | Type::NonEmptyList(_) => return true,
        Type::Array(array) => array.parameters.as_ref(),
        Type::NonEmptyArray(array) => array.parameters.as_ref(),
        _ => return false,
    };
    let Some(parameters) = parameters else {
        return false;
    };
    let entries = parameters.entries.iter().collect::<Vec<_>>();
    matches!(entries.as_slice(), [key, _] if matches!(key.inner, Type::Int(_)))
}

/// The conditional a `@return` names, through however many parentheses it was written in.
pub(crate) fn as_conditional<'arena>(
    ty: &'arena Type<'arena>,
) -> Option<&'arena mago_phpdoc_syntax::cst::r#type::conditional::ConditionalType<'arena>> {
    match ty {
        Type::Conditional(conditional) => Some(conditional),
        Type::Parenthesized(inner) => as_conditional(inner.inner),
        _ => None,
    }
}

/// What a parameter's literal default states, as a type.
///
/// The sibling of [`default_satisfies`], and it reads the same two things off the same node: a
/// keyword and a string. Its own function because the questions differ -- that one compares a default
/// to a *test*, this one names its type -- and folding them together would mean one of the two
/// answering in the other's vocabulary.
///
/// `None` for a parameter with no default and for one whose default is an expression. A literal is
/// what a declaration can be read for, and `= null` is nearly every one of these.
pub(super) fn default_kind(
    params: &FunctionLikeParameterList<'_>,
    position: u16,
) -> Option<BuiltinTy> {
    let param = params.parameters.iter().nth(usize::from(position))?;
    let value = param.default_value.as_ref()?.value;
    let Expression::Literal(literal) = value else {
        return None;
    };
    if let mago_syntax::cst::Literal::String(_) = literal {
        return Some(BuiltinTy::String);
    }
    match crate::expression::keyword_of(literal)? {
        "null" => Some(BuiltinTy::Null),
        "true" => Some(BuiltinTy::True),
        "false" => Some(BuiltinTy::False),
        _ => None,
    }
}

/// Whether the declaration's own default for one parameter satisfies a test.
///
/// `None` when there is no default or when it is not a literal: `$x = self::DEFAULT` names a value
/// this layer cannot read, and an absent argument then settles nothing -- which is a different answer
/// from settling that the test is false.
pub(super) fn default_satisfies(
    params: &FunctionLikeParameterList<'_>,
    position: u16,
    test: &phrost_hir::ConditionTestData,
) -> Option<bool> {
    use phrost_hir::ConditionTestData as Test;

    let param = params.parameters.iter().nth(usize::from(position))?;
    // A default holding the key is not a thing anybody writes: an empty array, a scalar and `null`
    // all leave it out, and only a non-empty array literal would need reading.
    if matches!(test, Test::Shape { .. }) {
        return match param.default_value.as_ref()?.value {
            Expression::Array(array) => array.elements.is_empty().then_some(false),
            Expression::LegacyArray(array) => array.elements.is_empty().then_some(false),
            Expression::Literal(_) => Some(false),
            _ => None,
        };
    }
    // A variadic parameter a call writes nothing for holds nothing, and so does a `[]` default --
    // the one array default anybody writes. Neither is a literal, so this is asked first.
    if matches!(test, Test::Empty) {
        if param.ellipsis.is_some() {
            return Some(true);
        }
        return match param.default_value.as_ref()?.value {
            Expression::Array(array) => Some(array.elements.is_empty()),
            Expression::LegacyArray(array) => Some(array.elements.is_empty()),
            _ => None,
        };
    }
    let value = param.default_value.as_ref()?.value;
    // A number, asked before the literal is required because `-1` is not one: it is a minus in
    // front of one. `wp_parse_url($url, $component = -1)` is the shape, and every call that leaves
    // the component out wants the whole URL.
    if let Test::Number(wanted) = test {
        return Some(written_integer(value)? == *wanted);
    }
    let number = written_number_kind(value);
    let (keyword, string) = match value {
        Expression::Literal(literal) => (
            crate::expression::keyword_of(literal),
            match literal {
                mago_syntax::cst::Literal::String(written) => {
                    Some(String::from_utf8_lossy(written.value?).into_owned())
                }
                _ => None,
            },
        ),
        _ => (None, None),
    };
    if keyword.is_none() && string.is_none() && number.is_none() {
        return None;
    }
    Some(match test {
        Test::Null => keyword == Some("null"),
        Test::True => keyword == Some("true"),
        Test::False => keyword == Some("false"),
        Test::Literal(wanted) => string.as_deref() == Some(wanted.as_str()),
        // A literal default has a kind of its own: `$subject = ''` is a string and `$flags = 0` is
        // not -- and is an int, which `range()`'s `$step is float` asks of every call that leaves the
        // step out. An instance is never a literal, so only the kind half can be what a default is.
        // An object is never written as a default -- `new` in one is an expression this does not
        // read, and was refused above -- so a literal one is no instance.
        Test::Instance(_) => false,
        // Every literal default is a scalar but `null`.
        Test::Scalar => keyword != Some("null"),
        Test::OfKind(kind) | Test::OfKindOrInstance { kind, .. } => match kind {
            BuiltinTy::String => string.is_some(),
            BuiltinTy::Bool => matches!(keyword, Some("true" | "false")),
            BuiltinTy::Int | BuiltinTy::Float => number == Some(*kind),
            _ => false,
        },
        // A default written as `Foo::class` is legal and nobody writes one, so a literal default is
        // reason enough to say the argument names no class.
        Test::ClassString(_) => false,
        // A literal default is a scalar or an empty array -- `$array = []` is the one written -- and
        // neither holds anything. A call that leaves the argument out passed nothing to be non-empty.
        Test::NonEmpty => false,
        // Answered above, before the literal was required.
        Test::Empty | Test::Shape { .. } => return None,
        // `$query = null` is no string at all, and `$separator = ''` is the one string default
        // anybody writes.
        Test::NonEmptyString => string.as_deref().is_some_and(|written| !written.is_empty()),
        Test::NonFalsyString => string
            .as_deref()
            .is_some_and(|written| !written.is_empty() && written != "0"),
        // Answered above, before the literal was required.
        Test::Number(_) => return None,
    })
}

/// Which sort of number an expression writes, sign and all: `1` and `-1` are ints, `0.5` a float.
fn written_number_kind(value: &Expression<'_>) -> Option<BuiltinTy> {
    match value {
        Expression::Literal(mago_syntax::cst::Literal::Integer(_)) => Some(BuiltinTy::Int),
        Expression::Literal(mago_syntax::cst::Literal::Float(_)) => Some(BuiltinTy::Float),
        Expression::UnaryPrefix(prefix) => match prefix.operator {
            mago_syntax::cst::UnaryPrefixOperator::Negation(_)
            | mago_syntax::cst::UnaryPrefixOperator::Plus(_) => written_number_kind(prefix.operand),
            _ => None,
        },
        Expression::Parenthesized(inner) => written_number_kind(inner.expression),
        _ => None,
    }
}

/// The integer an expression writes, with the sign in front of it if there is one.
///
/// Only a written number. A flag's default is the constant that names it -- `$flags = PATHINFO_ALL`
/// -- and what that stands for is in whichever file declares it, which nothing at lowering can read.
/// So a call that leaves the flag out settles nothing rather than settling it wrongly.
fn written_integer(value: &Expression<'_>) -> Option<i64> {
    match value {
        Expression::Literal(mago_syntax::cst::Literal::Integer(written)) => {
            i64::try_from(written.value?).ok()
        }
        Expression::UnaryPrefix(prefix) => match prefix.operator {
            mago_syntax::cst::UnaryPrefixOperator::Negation(_) => {
                written_integer(prefix.operand)?.checked_neg()
            }
            _ => None,
        },
        Expression::Parenthesized(inner) => written_integer(inner.expression),
        _ => None,
    }
}

/// Whether a conditional's branches name the same thing and differ only in admitting its absence.
///
/// What tells `reset`'s `($array is non-empty-list ? V : ($array is non-empty-array ? V : V|false))`
/// from `array_values`'s `($array is non-empty-array ? non-empty-list<V> : list<V>)`: the first says
/// the value may not be there and the second says the same container twice.
fn absence_is_the_difference(conditional: &ConditionalType<'_>, text: &str) -> bool {
    let mut named = Vec::new();
    let mut absent = None;
    collect_leaves(conditional, &mut named, &mut absent);

    let Some(first) = named.first() else {
        return false;
    };
    // Which sentinel it was does not matter here -- the question is only whether the branches differ
    // by one at all.
    absent.is_some()
        && named
            .iter()
            .all(|leaf| slice(text, leaf.span()) == slice(text, first.span()))
}

/// Every leaf a conditional's branches end at, with nested conditionals and unions flattened.
///
/// Flattened because a stub writes them nested: `reset` is
/// `($array is non-empty-list ? V : ($array is non-empty-array ? V : V|false))`, which is three
/// leaves that agree and one that says the value may be absent.
fn collect_leaves<'arena>(
    conditional: &'arena ConditionalType<'arena>,
    into: &mut Vec<&'arena Type<'arena>>,
    absent: &mut Option<Absence>,
) {
    for branch in [conditional.then, conditional.r#else] {
        collect_branch(branch, into, absent);
    }
}

/// One branch, with the ways of saying "not there" set aside as they are met.
///
/// `absent` rather than a leaf of its own, because `?T` has no null node to point at and a `T|null`
/// and a `?T` must reduce the same way. It carries *which* way, though: a `false` guarded with
/// `=== NULL` is not guarded at all.
pub(crate) fn collect_branch<'arena>(
    branch: &'arena Type<'arena>,
    into: &mut Vec<&'arena Type<'arena>>,
    absent: &mut Option<Absence>,
) {
    match branch {
        Type::Conditional(nested) => collect_leaves(nested, into, absent),
        Type::Parenthesized(inner) => collect_branch(inner.inner, into, absent),
        Type::Union(union) => {
            collect_branch(union.left, into, absent);
            collect_branch(union.right, into, absent);
        }
        Type::Nullable(nullable) => {
            met(absent, Absence::Null);
            collect_branch(nullable.inner, into, absent);
        }
        Type::Null(_) => met(absent, Absence::Null),
        Type::False(_) => met(absent, Absence::False),
        // `V|void` is not a thing anybody writes, and if they did they would mean the value is not
        // there -- which is what `null` spells.
        Type::Void(_) => met(absent, Absence::Null),
        other => into.push(other),
    }
}

/// One more sentinel met while walking a branch's operands.
///
/// A free function because both collectors recurse and neither owns the accumulator -- and because
/// `V|null|false` really is written (the DOM, mysqli and intl stubs each do), so meeting a second
/// one has to widen rather than overwrite.
fn met(absent: &mut Option<Absence>, one: Absence) {
    *absent = Some(match *absent {
        Some(already) => already.and(one),
        None => one,
    });
}
