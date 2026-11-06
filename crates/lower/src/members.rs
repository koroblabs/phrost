use crate::docblock::{DocTag, type_alias::TypeAliasTable};
use lspkit_arena::IdSlice;
use mago_span::HasSpan as _;
use mago_syntax::cst::{
    ClassLikeConstant, ClassLikeMember, EnumCase, EnumCaseItem, Method, MethodBody, Modifier,
    Property, PropertyItem, Sequence, TraitUse, TraitUseAdaptation, TraitUseMethodReference,
    TraitUseSpecification,
};
use phrost_hir::{
    ClassId, Hooks, MemberData, MemberId, MemberKind, MemberModifiers, TraitAdaptationData,
    TraitAdaptationKind,
};
use phrost_model::Visibility;
use phrost_syntax::identifier_text;

use crate::lowerer::{Lowerer, declaration_start, set_visibility_of, visibility_of};
use crate::yields::Yielding;

impl<'p> Lowerer<'_, 'p> {
    /// Lowers every member of one class-like declaration, in source order.
    pub(crate) fn class_members(
        &mut self,
        class: ClassId,
        members: &Sequence<'p, ClassLikeMember<'p>>,
        declaration_start: phrost_model::TextSize,
        aliases: &crate::docblock::type_alias::TypeAliasTable,
    ) {
        let mut member_ids = Vec::new();
        let mut trait_uses = Vec::new();
        let mut adaptations = Vec::new();
        let mut use_arguments = Vec::new();

        // The cases, before any constant is read. `const Default = self::Draft;` inside an enum is
        // the enum, and telling it from `const B = self::A;` -- where `A` is an ordinary constant
        // and the type is whatever `A` holds -- takes knowing which names are cases. They may be
        // written below the constant that names one, so the list is gathered first.
        let cases = members
            .iter()
            .filter_map(|member| match member {
                ClassLikeMember::EnumCase(case) => Some(match &case.item {
                    mago_syntax::cst::EnumCaseItem::Unit(unit) => unit.name,
                    mago_syntax::cst::EnumCaseItem::Backed(backed) => backed.name,
                }),
                _ => None,
            })
            .map(|name| identifier_text(name.value).to_string())
            .collect::<Vec<_>>();

        // Before the real ones, which is both source order -- the docblock is written above the
        // body -- and the precedence PHP has: flattening lets a later declaration of a name replace
        // an earlier one, so a written method wins over a comment claiming the same name. It is the
        // written one that runs.
        self.docblock_members(class, declaration_start, &mut member_ids, aliases);

        for member in members.iter() {
            match member {
                ClassLikeMember::Method(method) => {
                    self.method(class, method, &mut member_ids, aliases);
                }
                ClassLikeMember::Property(property) => {
                    self.property(class, property, &mut member_ids, aliases);
                }
                ClassLikeMember::Constant(constant) => {
                    self.class_constant(class, constant, &mut member_ids, &cases);
                }
                ClassLikeMember::EnumCase(case) => {
                    self.enum_case(class, case, &mut member_ids);
                }
                ClassLikeMember::TraitUse(trait_use) => {
                    self.trait_use(trait_use, &mut trait_uses, &mut adaptations);
                    // `/** @use BuildsQueries<TModel> */` above the `use` itself, which is where
                    // PHPStan's docs and Laravel both write it. Read only off the class's own
                    // docblock, Eloquent's builder supplied its trait nothing, and every `first()`
                    // on a query read `TValue`.
                    let start = phrost_model::TextSize::from(trait_use.span().start.offset);
                    let written = self.docblock_supertype_arguments(start, aliases);
                    use_arguments.extend_from_slice(self.builder.type_ref_list(written));
                }
            }
        }

        let members = self.builder.alloc_member_list(&member_ids);
        self.builder.set_class_members(class, members);

        if !use_arguments.is_empty() {
            self.builder
                .add_class_supertype_arguments(class, &use_arguments);
        }
        let uses = self.builder.alloc_name_ref_list(&trait_uses);
        let adaptations = self.builder.alloc_adaptation_list(&adaptations);
        self.builder.set_class_traits(class, uses, adaptations);
    }

    fn method(
        &mut self,
        class: ClassId,
        method: &Method<'p>,
        out: &mut Vec<MemberId>,
        aliases: &TypeAliasTable,
    ) {
        self.attribute_names(&method.attribute_lists);
        let name = self.interner.name(&identifier_text(method.name.value));
        let start = declaration_start(
            &method.attribute_lists,
            Some(&method.modifiers),
            method.function.span(),
        );
        let params = self.params_with_aliases(&method.parameter_list, start, aliases);
        let type_ref = self.return_type(method.return_type_hint.as_ref());
        // Only when the native declaration is silent: a hint is what the runtime enforces, and
        // skipping the parse when there is one is also what keeps this affordable.
        let doc_type_ref =
            self.doc_type_beside_with_aliases(type_ref, start, DocTag::Return, aliases);
        let throws = self.doc_throws(start);
        let contract = self.contract_beside(start, params, &method.parameter_list);
        let deprecated = self.is_deprecated(&method.attribute_lists, start);
        let docblock = self.docblock(start);

        let body = match &method.body {
            MethodBody::Concrete(block) => {
                // A method body declares nothing that is hoisted, and `function helper() {}` inside
                // one is still a name the global table gets once the method runs -- see
                // `conditional.rs`.
                self.body_declarations(block);
                self.declared_body(block, start)
            }
            // Abstract and interface-declared methods have no body to lower, which is a different
            // fact from "nobody asked" -- see `FileHir::has_bodies`.
            MethodBody::Abstract(_) => None,
        };
        // Read off the tree rather than the body, which a dependency's methods do not have.
        if let MethodBody::Concrete(block) = &method.body
            && self.keyed_attributes.keeps_constructions_in(name.as_str())
        {
            self.keep_constructions(class, block);
        }
        if let Some(body) = body
            && self.keyed_attributes.mixes_in_return_of(name.as_str())
            && let Some(mixin) = self.class_the_body_returns(body)
        {
            let ty = self
                .builder
                .add_type_ref(phrost_hir::TypeRefData::Named(mixin));
            self.builder.add_class_mixins(class, &[ty]);
        }
        // What the body hands back, when neither the signature nor a docblock says. Last, so the
        // two that are written win without this being computed at all.
        let written_doc_type_ref = doc_type_ref;
        // Whether a `yield` makes all of that a question about a generator instead. Asked first
        // because both readings below depend on the answer: one is replaced by it, and the other
        // has to stay silent -- a `return` inside a generator goes to `Generator::getReturn()` and
        // never to the caller.
        let yielding = match (doc_type_ref, &method.body) {
            (None, MethodBody::Concrete(block)) => self.yielded_from_body(block, type_ref),
            _ => Yielding::No,
        };
        let doc_type_ref = match yielding {
            Yielding::Produces(yielded) => Some(yielded),
            Yielding::Unreadable => None,
            // Where every written source says nothing a class would not say better. That is both
            // of them absent -- the reading this has always done -- and either of them written as
            // `object`, which is the widest object type PHP has: every class satisfies it, so a
            // class read off the body gives nothing up.
            //
            // `refined` already ranks a *docblock* above an `object` hint for exactly that reason
            // -- "`object` admits any instance, so a docblock narrows it only by naming a class" --
            // and the body was the third source with no such rule. Drupal is written this way:
            // `/** @return object */ protected function utilitiesManager(): object { return
            // \Drupal::service('x'); }`, and `$this->utilitiesManager()->` then resolved to nothing
            // at all -- no goto, no hover, no completion, and every call through it unjudged.
            Yielding::No => match (type_ref, doc_type_ref, &method.body) {
                (hint, doc, MethodBody::Concrete(block))
                    if hint.is_none_or(|id| self.hints_only_object(id))
                        && doc.is_none_or(|id| self.hints_only_object(id)) =>
                {
                    self.returned_from_body(block, &method.parameter_list)
                        .or(doc_type_ref)
                }
                _ => doc_type_ref,
            },
        };
        // Which of the two said it, for the one reader that has to tell them apart: an override
        // inherits the declaration it overrides, and an inherited *claim* beats this file's body.
        //
        // Only where the signature is silent too. A generator's element type is read off the body
        // and written onto what the signature already named, so the pair is one declaration with
        // its arguments filled in -- and handing it to the climb would trade `\Generator<Row>` for
        // whatever bare container an ancestor happened to declare.
        let type_from_body =
            type_ref.is_none() && written_doc_type_ref.is_none() && doc_type_ref.is_some();
        // A `@return` naming a template is refused above, and the empty slot it leaves is
        // indistinguishable from no `@return` at all unless the refusal is recorded. Asked only
        // where nothing was lowered, so the common method pays one branch.
        let doc_type_refused =
            written_doc_type_ref.is_none() && self.doc_names_a_template(start, DocTag::Return);
        // And the other half: a chain that *describes* a type without naming one. Only where
        // nothing above found anything, so the cheap readings win and this walk is not taken.
        let returned_path = match (type_ref, doc_type_ref, &method.body) {
            // The same "says nothing beyond `object`" test the body read above uses, and it has to
            // be here as well: a body that *names* no class can still describe one, and
            // `return \Drupal::service('utilities.manager');` is that -- the class is in the
            // service catalog, which is the view's and not lowering's to read.
            (hint, doc, MethodBody::Concrete(block))
                if matches!(yielding, Yielding::No)
                    && hint.is_none_or(|id| self.hints_only_object(id))
                    && doc.is_none_or(|id| self.hints_only_object(id)) =>
            {
                self.returned_path_from_body(block, &method.parameter_list)
                    .map(Box::new)
            }
            // A class named bare, which may be a generic one the body fills in: Eloquent's
            // `author(): BelongsTo { return $this->belongsTo(User::class); }` says *what* comes back
            // and only the call says of *which* model. Lowering cannot tell a generic class from
            // any other, so the row is kept where the call has an argument a contract reads -- the
            // one shape that can fill a template -- and the view decides whether it refines.
            // Narrower than the gate above on purpose: `return $this->repository;` beside a hint
            // would otherwise be a row on most getters in a project.
            (Some(hint), None, MethodBody::Concrete(block))
                if matches!(yielding, Yielding::No)
                    && self.hints_a_class(hint)
                    && Self::returns_a_call_on_this(block) =>
            {
                // Or a chain of several calls, whose first may fill nothing and still hand the rest
                // a filled relation -- one call with no key is a wrapper, and the row would buy
                // nothing a hint does not already say.
                self.returned_path_from_body(block, &method.parameter_list)
                    .filter(|path| path.key.is_some() || path.steps.len() > 1)
                    .map(Box::new)
            }
            // A chain built from a class, which is how a repository answers: `run():
            // LengthAwarePaginator` whose body is `$query = User::query()->...; return
            // $query->paginate($count);` hands back a page of users, and only the chain says so.
            // Every step is a call, so a getter through a collaborator is not one of these -- and
            // the view still takes the row only where it fills a generic the hint left bare. A class
            // a docblock names bare is the same claim, written where PHP has no syntax for it.
            (Some(written), None, MethodBody::Concrete(block))
            | (None, Some(written), MethodBody::Concrete(block))
                if matches!(yielding, Yielding::No)
                    && self.hints_a_class(written)
                    && self.returns_a_built_chain(block, &method.parameter_list) =>
            {
                self.returned_path_from_body(block, &method.parameter_list)
                    .map(Box::new)
            }
            // The same for a class a *docblock* names bare, where the call is one on `$this` and
            // the step alone may fill the templates: Laravel's factory stubs write `@return
            // Factory` over `return $this->state(...)`, whose `static` is the subclass and its
            // `@extends Factory<User>`. One step only -- a chain through other objects is the
            // getter shape the gate above keeps out.
            (None, Some(doc), MethodBody::Concrete(block))
                if matches!(yielding, Yielding::No)
                    && self.hints_a_class(doc)
                    && Self::returns_a_call_on_this(block) =>
            {
                self.returned_path_from_body(block, &method.parameter_list)
                    .filter(|path| {
                        matches!(path.root, phrost_hir::ReturnRootData::Receiver)
                            && path.steps.len() == 1
                    })
                    .map(Box::new)
            }
            _ => None,
        };

        // Only a concrete body can read them; an abstract or interface declaration has none, and
        // an implementation reading `func_get_args()` says nothing about the calls made through
        // the interface it satisfies.
        let reads_own_arguments = match &method.body {
            MethodBody::Concrete(block) => self.reads_own_arguments(block),
            MethodBody::Abstract(_) => false,
        };

        let id = self.builder.add_member(MemberData {
            has_default: false,
            type_from_body,
            doc_type_refused,
            returned_path,
            throws,
            name,
            kind: MemberKind::Method,
            reads_own_arguments,
            owner: class,
            visibility: visibility_of(&method.modifiers),
            set_visibility: set_visibility_of(&method.modifiers),
            modifiers: {
                let mut modifiers = member_modifiers(&method.modifiers, Hooks::None);
                modifiers.set_no_discard(crate::lowerer::names_an_attribute(
                    &method.attribute_lists,
                    NO_DISCARD,
                ));
                modifiers.set_override(crate::lowerer::names_an_attribute(
                    &method.attribute_lists,
                    OVERRIDE,
                ));
                modifiers.set_marked(self.keyed_attributes.marks(&method.attribute_lists));
                modifiers.set_deprecated(deprecated);
                modifiers
            },
            type_ref,
            doc_type_ref,
            params,
            hooks: IdSlice::EMPTY,
            body,
            name_range: self.within_the_text(method.name.span()),
            full_range: self.within_the_text(method.span()),
            docblock,
            contract,
            from_docblock: false,
            ty_from_value: false,
        });
        out.push(id);

        // Constructor property promotion declares properties as a side effect of declaring
        // parameters. Emitting them here means member lookup never has to know that.
        self.promoted_properties(class, params, out);
    }

    /// Emits a property for each promoted constructor parameter.
    fn promoted_properties(
        &mut self,
        class: ClassId,
        params: IdSlice<phrost_hir::ParamId>,
        out: &mut Vec<MemberId>,
    ) {
        let promoted = self
            .builder
            .params_in(params)
            .filter_map(|param| {
                param.promotion.as_ref().map(|promotion| {
                    (
                        param.name.clone(),
                        param.type_ref,
                        param.doc_type_ref,
                        param.name_range,
                        param.full_range,
                        *promotion,
                    )
                })
            })
            .collect::<Vec<_>>();

        for (name, type_ref, doc_type_ref, name_range, full_range, promotion) in promoted {
            let id = self.builder.add_member(MemberData {
                has_default: false,
                type_from_body: false,
                doc_type_refused: false,
                returned_path: None,
                throws: lspkit_arena::IdSlice::EMPTY,
                name,
                kind: MemberKind::Property,
                // A property is not called, so nothing can pass it too many arguments.
                reads_own_arguments: false,
                owner: class,
                visibility: promotion.visibility,
                set_visibility: promotion.set_visibility,
                modifiers: promotion.modifiers,
                type_ref,
                doc_type_ref,
                params: IdSlice::EMPTY,
                hooks: promotion.hooks,
                body: None,
                name_range,
                full_range,
                docblock: None,
                // Only a method returns a boolean anything could be asserted from.
                contract: None,
                from_docblock: false,
                ty_from_value: false,
            });
            out.push(id);
        }
    }

    fn property(
        &mut self,
        class: ClassId,
        property: &Property<'p>,
        out: &mut Vec<MemberId>,
        aliases: &TypeAliasTable,
    ) {
        let attribute_lists = match property {
            Property::Plain(plain) => &plain.attribute_lists,
            Property::Hooked(hooked) => &hooked.attribute_lists,
        };
        self.attribute_names(attribute_lists);
        let (modifiers, hint, items, hook_list, full_range): (_, _, Vec<&PropertyItem<'p>>, _, _) =
            match property {
                Property::Plain(plain) => (
                    &plain.modifiers,
                    plain.hint.as_ref(),
                    plain.items.iter().collect(),
                    None,
                    self.within_the_text(plain.span()),
                ),
                Property::Hooked(hooked) => (
                    &hooked.modifiers,
                    hooked.hint.as_ref(),
                    vec![&hooked.item],
                    Some(&hooked.hook_list),
                    self.within_the_text(hooked.span()),
                ),
            };

        // A property has no keyword of its own to measure from, and its span already begins at its
        // attributes or modifiers -- whichever comes first -- which is exactly where the docblock
        // search has to start.
        let docblock = self.docblock(full_range.start());
        let type_ref = hint.map(|hint| self.hint(hint));
        let doc_type_ref =
            self.doc_type_beside_with_aliases(type_ref, full_range.start(), DocTag::Var, aliases);
        let visibility = visibility_of(modifiers);
        let set_visibility = set_visibility_of(modifiers);
        // The property's own name, for the backed-or-virtual test. PHP forbids hooks on a
        // multi-property declaration, so a hooked list has exactly one and the first is it.
        let backing = hook_list
            .is_some()
            .then(|| property_name(items[0]))
            .flatten();
        // After the type, because an implicit `set($value)` receives the property's own type.
        let (hooks, shape) =
            self.property_hooks(hook_list, type_ref, full_range.start(), backing.as_deref());
        let mut member_modifiers = member_modifiers(modifiers, shape);
        member_modifiers.set_deprecated(self.is_deprecated(attribute_lists, full_range.start()));
        // PHP 8.5 lets a property say it replaces one above it, with the method's rule.
        member_modifiers.set_override(crate::lowerer::names_an_attribute(
            attribute_lists,
            OVERRIDE,
        ));

        // `public int $a, $b;` declares two properties. Each becomes its own member so nothing
        // downstream has to know they shared a declaration.
        for item in items {
            // The variable's own text includes the `$`; names are stored without it so a property
            // name matches how it is written at a use site (`$obj->name`).
            let variable = match item {
                PropertyItem::Abstract(abstract_item) => &abstract_item.variable,
                PropertyItem::Concrete(concrete) => &concrete.variable,
            };
            let Some(written) = property_name(item) else {
                continue;
            };
            let name = self.interner.name(&written);
            if let PropertyItem::Concrete(concrete) = item
                && self.keyed_attributes.keeps_default_of(&written)
            {
                self.keep_default(class, name.clone(), concrete.value);
            }

            let id = self.builder.add_member(MemberData {
                has_default: matches!(item, PropertyItem::Concrete(_)),
                type_from_body: false,
                doc_type_refused: false,
                returned_path: None,
                throws: lspkit_arena::IdSlice::EMPTY,
                name,
                kind: MemberKind::Property,
                // Not called, so nothing can pass it too many arguments.
                reads_own_arguments: false,
                owner: class,
                visibility,
                set_visibility,
                modifiers: member_modifiers,
                type_ref,
                doc_type_ref,
                params: IdSlice::EMPTY,
                hooks,
                body: None,
                name_range: self.within_the_text(variable.span()),
                full_range,
                docblock,
                // Only a method returns a boolean anything could be asserted from.
                contract: None,
                from_docblock: false,
                ty_from_value: false,
            });
            out.push(id);
        }
    }

    /// A property's array or string default, kept for the framework that reads it.
    ///
    /// Entries a literal spells -- a quoted key, a quoted list entry -- and nothing else: a computed
    /// key names no attribute this can say anything about, and a spread brings entries from
    /// somewhere no lowering of one file sees. A string is kept whole, as written between its quotes.
    fn keep_default(
        &mut self,
        class: ClassId,
        property: phrost_model::Name,
        value: &mago_syntax::cst::Expression<'p>,
    ) {
        let elements = match value {
            mago_syntax::cst::Expression::Array(array) => &array.elements,
            mago_syntax::cst::Expression::LegacyArray(array) => &array.elements,
            mago_syntax::cst::Expression::Literal(mago_syntax::cst::Literal::String(text)) => {
                if let Some(written) = text.value {
                    self.builder.add_kept_text(phrost_hir::KeptTextData {
                        class,
                        property,
                        text: String::from_utf8_lossy(written).into(),
                        range: self.inside_the_quotes(text.span()),
                    });
                }
                return;
            }
            // `protected $model = User::class;`: one class, named.
            mago_syntax::cst::Expression::Access(mago_syntax::cst::Access::ClassConstant(_)) => {
                if let phrost_hir::KeptValueData::Class(named) = self.kept_value(value) {
                    self.builder.add_kept_class(phrost_hir::KeptClassData {
                        class,
                        property,
                        named,
                    });
                }
                return;
            }
            _ => return,
        };
        let mut entries = Vec::new();
        for element in elements.iter() {
            match element {
                mago_syntax::cst::ArrayElement::KeyValue(pair) => {
                    let mago_syntax::cst::Expression::Literal(mago_syntax::cst::Literal::String(
                        key,
                    )) = pair.key
                    else {
                        continue;
                    };
                    let Some(written) = key.value else {
                        continue;
                    };
                    entries.push(phrost_hir::KeptEntryData {
                        name: self.interner.name(&String::from_utf8_lossy(written)),
                        range: self.inside_the_quotes(key.span()),
                        value: Some(self.kept_value(pair.value)),
                    });
                }
                mago_syntax::cst::ArrayElement::Value(entry) => {
                    let mago_syntax::cst::Expression::Literal(mago_syntax::cst::Literal::String(
                        listed,
                    )) = entry.value
                    else {
                        continue;
                    };
                    let Some(written) = listed.value else {
                        continue;
                    };
                    entries.push(phrost_hir::KeptEntryData {
                        name: self.interner.name(&String::from_utf8_lossy(written)),
                        range: self.inside_the_quotes(listed.span()),
                        value: None,
                    });
                }
                _ => {}
            }
        }
        self.builder.add_kept_table(phrost_hir::KeptTableData {
            class,
            property,
            from_attribute: false,
            entries: entries.into_boxed_slice(),
        });
    }

    /// What a kept entry's value says, as far as a literal says it.
    fn kept_value(
        &mut self,
        value: &mago_syntax::cst::Expression<'p>,
    ) -> phrost_hir::KeptValueData {
        match value {
            mago_syntax::cst::Expression::Literal(mago_syntax::cst::Literal::String(text)) => {
                match text.value {
                    Some(written) => {
                        phrost_hir::KeptValueData::Text(String::from_utf8_lossy(written).into())
                    }
                    None => phrost_hir::KeptValueData::Other,
                }
            }
            mago_syntax::cst::Expression::Access(mago_syntax::cst::Access::ClassConstant(
                access,
            )) => {
                let mago_syntax::cst::ClassLikeConstantSelector::Identifier(constant) =
                    &access.constant
                else {
                    return phrost_hir::KeptValueData::Other;
                };
                match (
                    identifier_text(constant.value).eq_ignore_ascii_case("class"),
                    access.class,
                ) {
                    (true, mago_syntax::cst::Expression::Identifier(named)) => {
                        phrost_hir::KeptValueData::Class(self.name_ref(named))
                    }
                    _ => phrost_hir::KeptValueData::Other,
                }
            }
            _ => phrost_hir::KeptValueData::Other,
        }
    }

    /// The class a body consisting of `return X::class;` names, as the reference its lowering
    /// already made -- a second one would put the name in the file twice.
    fn class_the_body_returns(&self, body: phrost_hir::BodyId) -> Option<phrost_hir::NameRefId> {
        let [statement] = self.builder.body_statements(body) else {
            return None;
        };
        let phrost_hir::body::StmtData::Return {
            value: Some(value), ..
        } = self.builder.stmt(*statement)
        else {
            return None;
        };
        match self.builder.expr(*value) {
            phrost_hir::body::ExprData::ClassConstAccess {
                class: phrost_hir::body::ClassTarget::Named(named),
                name: Some(name),
                ..
            } if name.as_str().eq_ignore_ascii_case("class") => Some(*named),
            _ => None,
        }
    }

    /// A string literal's range without its quotes, where it has a pair.
    pub(crate) fn inside_the_quotes(&self, span: mago_span::Span) -> phrost_model::TextRange {
        let range = self.within_the_text(span);
        let one = phrost_model::TextSize::from(1);
        match range.len() >= phrost_model::TextSize::from(2) {
            true => phrost_model::TextRange::new(range.start() + one, range.end() - one),
            false => range,
        }
    }

    /// `self[]` for a constant listing cases of the enum it is written in: `const ALL = [self::A,
    /// self::B]`, which is how an enum names a subset of itself -- the ones a form offers, the ones
    /// a state may move to. Every value has to be one; a list mixing a case with anything else is
    /// left to the builtin reading, which refuses it.
    fn own_cases_listed(
        &mut self,
        value: &mago_syntax::cst::Expression<'p>,
        cases: &[String],
    ) -> Option<phrost_hir::TypeRefId> {
        let elements = match value {
            mago_syntax::cst::Expression::Array(array) => &array.elements,
            mago_syntax::cst::Expression::LegacyArray(array) => &array.elements,
            _ => return None,
        };
        if elements.is_empty() {
            return None;
        }
        for element in elements.iter() {
            let value = match element {
                mago_syntax::cst::ArrayElement::Value(value) => value.value,
                mago_syntax::cst::ArrayElement::KeyValue(pair) => pair.value,
                _ => return None,
            };
            if !Self::names_an_own_case(value, cases) {
                return None;
            }
        }
        let own = self.builder.add_type_ref(phrost_hir::TypeRefData::Relative(
            phrost_model::RelativeTy::SelfTy,
        ));
        let array = self.builder.add_type_ref(phrost_hir::TypeRefData::Builtin(
            phrost_model::BuiltinTy::Array,
        ));
        let args = self.builder.alloc_type_ref_list(&[own]);
        Some(self.builder.add_type_ref(phrost_hir::TypeRefData::Generic {
            base: array,
            args,
            non_empty: false,
        }))
    }

    /// Whether a constant's value is `self::Case` for a case of the enum it is written in.
    ///
    /// Only the relative spellings. `Other::Case` names another enum's case, which is a real type
    /// this cannot express as `self` -- and naming the wrong one is worse than naming none.
    fn names_an_own_case(value: &mago_syntax::cst::Expression<'p>, cases: &[String]) -> bool {
        let mago_syntax::cst::Expression::Access(mago_syntax::cst::Access::ClassConstant(access)) =
            value
        else {
            return false;
        };
        if !matches!(
            access.class,
            mago_syntax::cst::Expression::Self_(_) | mago_syntax::cst::Expression::Static(_)
        ) {
            return false;
        }
        let mago_syntax::cst::ClassLikeConstantSelector::Identifier(name) = &access.constant else {
            return false;
        };
        let written = identifier_text(name.value);
        cases.iter().any(|case| case.as_str() == written)
    }

    fn class_constant(
        &mut self,
        class: ClassId,
        constant: &ClassLikeConstant<'p>,
        out: &mut Vec<MemberId>,
        cases: &[String],
    ) {
        self.attribute_names(&constant.attribute_lists);
        // PHP 8.3 typed class constants: `const string VERSION = '1';`
        let type_ref = constant.hint.as_ref().map(|hint| self.hint(hint));
        let visibility = visibility_of(&constant.modifiers);
        let mut modifiers = member_modifiers(&constant.modifiers, Hooks::None);
        let constant_start = declaration_start(
            &constant.attribute_lists,
            Some(&constant.modifiers),
            constant.r#const.span(),
        );
        modifiers.set_deprecated(self.is_deprecated(&constant.attribute_lists, constant_start));
        let full_range = self.within_the_text(constant.span());
        let docblock = self.docblock(constant_start);
        // Whether `static::X` or `$object::X` can reach a declaration other than this one. An enum
        // cannot be extended, and a private or final constant cannot be redeclared below it.
        let owner = self.builder.class(class);
        let overridable = !owner.modifiers.is_final
            && owner.kind != phrost_hir::ClassKind::Enum
            && !modifiers.is_final()
            && visibility != phrost_model::Visibility::Private;

        for item in constant.items.iter() {
            let name = self.interner.name(&identifier_text(item.name.value));
            // With no hint written, the value beside the name is the only thing that says what the
            // constant is -- which is the ordinary case, because typed constants arrived in 8.3.
            // Read per item and not once above: `const A = 1, B = 'two';` is one declaration and
            // two values.
            let refined =
                type_ref.and_then(|hint| self.constant_hint_refined(hint, item.value, overridable));
            let from_value = match type_ref {
                Some(_) => None,
                // A case of the enclosing enum first: the value *is* an instance of it, which no
                // builtin can say. `self::Draft` and `static::Draft` both reach it, and the class
                // is whichever one the constant is written in -- which is what `Relative` means.
                None => match Self::names_an_own_case(item.value, cases) {
                    true => Some(self.builder.add_type_ref(phrost_hir::TypeRefData::Relative(
                        phrost_model::RelativeTy::SelfTy,
                    ))),
                    false => self
                        .own_cases_listed(item.value, cases)
                        .or_else(|| self.constant_value_type(item.value)),
                },
            };
            let id = self.builder.add_member(MemberData {
                has_default: false,
                type_from_body: false,
                doc_type_refused: false,
                returned_path: None,
                throws: lspkit_arena::IdSlice::EMPTY,
                name,
                kind: MemberKind::ClassConstant,
                // Not called, so nothing can pass it too many arguments.
                reads_own_arguments: false,
                owner: class,
                visibility,
                set_visibility: None,
                modifiers,
                type_ref: type_ref.or(from_value),
                // `@var` on a class constant is not a thing PHPDoc defines; what refines a hint here
                // is the value.
                doc_type_ref: refined,
                params: IdSlice::EMPTY,
                hooks: IdSlice::EMPTY,
                body: None,
                name_range: self.within_the_text(item.name.span()),
                full_range,
                docblock,
                // Only a method returns a boolean anything could be asserted from.
                contract: None,
                from_docblock: false,
                ty_from_value: from_value.is_some(),
            });
            out.push(id);
        }
    }

    fn enum_case(&mut self, class: ClassId, case: &EnumCase<'p>, out: &mut Vec<MemberId>) {
        self.attribute_names(&case.attribute_lists);
        let name_identifier = match &case.item {
            EnumCaseItem::Unit(unit) => &unit.name,
            EnumCaseItem::Backed(backed) => &backed.name,
        };
        let name = self.interner.name(&identifier_text(name_identifier.value));
        let case_start = declaration_start(&case.attribute_lists, None, case.case.span());
        let deprecated = self.is_deprecated(&case.attribute_lists, case_start);
        let docblock = self.docblock(case_start);

        let id = self.builder.add_member(MemberData {
            // Whether the case writes a value, which is what decides whether the enum's own
            // header agrees with it: a backed enum requires one and a plain one refuses it.
            has_default: matches!(case.item, EnumCaseItem::Backed(_)),
            type_from_body: false,
            doc_type_refused: false,
            returned_path: None,
            throws: lspkit_arena::IdSlice::EMPTY,
            name,
            kind: MemberKind::EnumCase,
            // Not called, so nothing can pass it too many arguments.
            reads_own_arguments: false,
            owner: class,
            // Cases are always publicly readable; PHP has no visibility syntax for them.
            visibility: Visibility::Public,
            set_visibility: None,
            modifiers: {
                let mut modifiers = MemberModifiers::default();
                modifiers.set_deprecated(deprecated);
                modifiers
            },
            type_ref: None,
            doc_type_ref: None,
            params: IdSlice::EMPTY,
            hooks: IdSlice::EMPTY,
            body: None,
            name_range: self.within_the_text(name_identifier.span()),
            full_range: self.within_the_text(case.span()),
            docblock,
            // Only a method returns a boolean anything could be asserted from.
            contract: None,
            from_docblock: false,
            ty_from_value: false,
        });
        out.push(id);
    }

    fn trait_use(
        &mut self,
        trait_use: &TraitUse<'p>,
        uses: &mut Vec<phrost_hir::NameRefId>,
        adaptations: &mut Vec<phrost_hir::TraitAdaptationId>,
    ) {
        for name in trait_use.trait_names.iter() {
            let id = self.name_ref(name);
            uses.push(id);
        }

        let TraitUseSpecification::Concrete(concrete) = &trait_use.specification else {
            return;
        };

        for adaptation in concrete.adaptations.iter() {
            let id = self.trait_adaptation(adaptation);
            adaptations.push(id);
        }
    }

    fn trait_adaptation(
        &mut self,
        adaptation: &TraitUseAdaptation<'p>,
    ) -> phrost_hir::TraitAdaptationId {
        let data = match adaptation {
            TraitUseAdaptation::Precedence(precedence) => {
                let trait_name = self.name_ref(&precedence.method_reference.trait_name);
                let method = self.interner.name(&identifier_text(
                    precedence.method_reference.method_name.value,
                ));
                let excluded_ids = precedence
                    .trait_names
                    .iter()
                    .map(|name| self.name_ref(name))
                    .collect::<Vec<_>>();
                let excluded = self.builder.alloc_name_ref_list(&excluded_ids);

                TraitAdaptationData {
                    trait_name: Some(trait_name),
                    method,
                    method_range: self
                        .within_the_text(precedence.method_reference.method_name.span()),
                    kind: TraitAdaptationKind::Precedence { excluded },
                }
            }
            TraitUseAdaptation::Alias(alias) => {
                // `A::m as n` names its trait; a bare `m as n` does not, and is only valid when
                // exactly one used trait declares `m` -- which is the semantic layer's problem.
                let (trait_name, method_name) = match &alias.method_reference {
                    TraitUseMethodReference::Absolute(absolute) => (
                        Some(self.name_ref(&absolute.trait_name)),
                        &absolute.method_name,
                    ),
                    TraitUseMethodReference::Identifier(identifier) => (None, identifier),
                };
                let method = self.interner.name(&identifier_text(method_name.value));
                let alias_name = alias
                    .alias
                    .as_ref()
                    .map(|alias| self.interner.name(&identifier_text(alias.value)));
                let alias_range = alias
                    .alias
                    .as_ref()
                    .map(|alias| self.within_the_text(alias.span()));

                TraitAdaptationData {
                    trait_name,
                    method,
                    method_range: self.within_the_text(method_name.span()),
                    kind: TraitAdaptationKind::Alias {
                        alias: alias_name,
                        alias_range,
                        visibility: alias.modifier.as_ref().and_then(single_visibility),
                    },
                }
            }
        };

        self.builder.add_trait_adaptation(data)
    }

    /// Whether a return hint says only that the value is an object.
    ///
    /// `object` is the widest object type PHP has: every class satisfies it, so a class read off
    /// the body is strictly more precise and nothing is given up by preferring it. Exactly the
    /// hint and not a nullable or a union of one -- `?object` still says the value may be absent,
    /// which the body cannot take back.
    /// Whether a hint names one class and nothing else -- no `?`, no union, no keyword.
    fn hints_a_class(&self, hint: phrost_hir::TypeRefId) -> bool {
        matches!(
            self.builder.type_ref(hint),
            phrost_hir::TypeRefData::Named(_)
        )
    }

    fn hints_only_object(&self, hint: phrost_hir::TypeRefId) -> bool {
        matches!(
            self.builder.type_ref(hint),
            phrost_hir::TypeRefData::Builtin(phrost_model::BuiltinTy::Object)
        )
    }
}

pub(crate) fn member_modifiers(
    modifiers: &Sequence<'_, Modifier<'_>>,
    hooks: Hooks,
) -> MemberModifiers {
    let mut result = MemberModifiers::default();
    result.hooks = hooks;

    for modifier in modifiers.iter() {
        match modifier {
            Modifier::Static(_) => result.set_static(true),
            Modifier::Abstract(_) => result.set_abstract(true),
            Modifier::Final(_) => result.set_final(true),
            Modifier::Readonly(_) => result.set_readonly(true),
            _ => {}
        }
    }

    result
}

/// The visibility a single `as` modifier expresses, if it is one.
fn single_visibility(modifier: &Modifier<'_>) -> Option<Visibility> {
    match modifier {
        Modifier::Public(_) => Some(Visibility::Public),
        Modifier::Protected(_) => Some(Visibility::Protected),
        Modifier::Private(_) => Some(Visibility::Private),
        _ => None,
    }
}

/// A property item's name, without the `$` a declaration writes it with.
///
/// `None` for a name the tree could not make out, which is a half-written line rather than a
/// property.
fn property_name(item: &PropertyItem<'_>) -> Option<String> {
    let variable = match item {
        PropertyItem::Abstract(abstract_item) => &abstract_item.variable,
        PropertyItem::Concrete(concrete) => &concrete.variable,
    };
    let written = identifier_text(variable.name);
    let written = written.trim_start_matches('$');
    (!written.is_empty()).then(|| written.to_owned())
}

/// PHP 8.3's attribute for a declaration that claims to replace one above it.
pub(crate) const OVERRIDE: &str = "Override";

/// PHP 8.5's attribute for a call whose result is the point of making it.
pub(crate) const NO_DISCARD: &str = "NoDiscard";

/// PHP 8.4's attribute for a declaration that still works and should not be reached for.
pub(crate) const DEPRECATED: &str = "Deprecated";
