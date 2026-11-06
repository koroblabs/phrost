//! Members a class declares in its docblock.
//!
//! `@method` and `@property` are how a library describes a surface it produces at runtime -- through
//! `__call`, through a generated subclass, through a trait mixed in by a framework. It is not a
//! niche: PHPUnit declares `MockObject::method()` this way, and so do Laravel's facades and
//! Doctrine's repositories. Without them, the most-used method of a mock is an undefined method.
//!
//! Lowered into ordinary [`MemberData`], so nothing above here has a second kind of member to
//! handle. The one thing that marks them is [`MemberData::from_docblock`], which exists because a
//! reader deserves to be told why goto landed inside a comment.
//!
//! # The range is real
//!
//! `@method InvocationStubber method($constraint)` is written at a place in a file, so the name
//! range points at the name inside the tag. Nothing is fabricated -- which is what the
//! survivable-data rule requires, since these ranges have to work for a closed vendor file whose
//! text is not resident.

use std::borrow::Cow;

use lspkit_arena::IdSlice;
use mago_allocator::{Arena as _, LocalArena};
use mago_phpdoc_syntax::{
    PHPDocParser,
    cst::{
        Element, TagValue,
        tag::{MethodTagValue, PropertyTagValue, Visibility as DocVisibility},
        r#type::Type,
    },
};
use mago_span::HasSpan as _;
use phrost_hir::{
    ClassId, ConditionTestData, ContractData, MemberData, MemberId, MemberKind, MemberModifiers,
    ParamData, ParamId, ReturnBranchData, TypeRefId,
};
use phrost_model::{TextSize, Visibility};

use crate::{
    docblock::{
        argument_binding::binds_class_string,
        contract::{as_conditional, collect_branch},
        shift, slice,
        template::{declared_templates, names_a_template},
        type_alias::TypeAliasTable,
        variable_name,
    },
    lowerer::Lowerer,
};

impl Lowerer<'_, '_> {
    /// Emits a member for every `@method` and `@property` in the class's docblock.
    ///
    /// `aliases` is the same class's `@phpstan-type` table: a magic property is as free to write
    /// `@property UserList $users` as a real one is to write `@var UserList $users`.
    pub(crate) fn docblock_members(
        &mut self,
        class: ClassId,
        declaration_start: TextSize,
        out: &mut Vec<MemberId>,
        aliases: &TypeAliasTable,
    ) {
        let Some(docblock) = self.raw_docblock_before(declaration_start) else {
            return;
        };
        // Cheap prefilter: parsing is the expensive part, and a class docblock usually holds prose
        // and nothing else.
        // Without the `@`, so the vendor-prefixed spellings are not filtered out.
        if !docblock.text.contains("method") && !docblock.text.contains("propert") {
            return;
        }

        let arena = LocalArena::new();
        let content = arena.alloc_slice_copy(docblock.text.as_bytes());
        let document = PHPDocParser::parse(
            &arena,
            mago_database::file::FileId::new(b"docblock"),
            content,
        );

        let base = docblock.range.start();
        // The class's own `@template`s, which a `@method` may name as freely as its own.
        let class_templates = declared_templates(&document);
        for element in document.elements.iter() {
            let Element::Tag(tag) = element else {
                continue;
            };

            let member = match &tag.value {
                TagValue::Method(value) => self.docblock_method(
                    class,
                    value,
                    base,
                    docblock.text,
                    aliases,
                    &class_templates,
                ),
                // Read-only and write-only are properties whose access is one-sided; the plain form
                // is both.
                TagValue::Property(value) => {
                    self.docblock_property(class, value, base, docblock.text, None, aliases)
                }
                TagValue::PropertyRead(value) => {
                    self.docblock_property(class, value, base, docblock.text, Some(false), aliases)
                }
                TagValue::PropertyWrite(value) => {
                    self.docblock_property(class, value, base, docblock.text, Some(true), aliases)
                }
                _ => None,
            };

            if let Some(member) = member {
                out.push(self.builder.add_member(member));
            }
        }
    }

    fn docblock_method(
        &mut self,
        class: ClassId,
        value: &MethodTagValue<'_>,
        base: TextSize,
        text: &str,
        aliases: &TypeAliasTable,
        class_templates: &[Cow<'_, str>],
    ) -> Option<MemberData> {
        let name = self
            .interner
            .name(&String::from_utf8_lossy(value.name.value));
        // The tag's own templates -- `make<T of object>(...)` -- beside the class's.
        let templates: Vec<Cow<'_, str>> = value
            .templates
            .into_iter()
            .flat_map(|list| list.entries.iter())
            .map(|entry| String::from_utf8_lossy(entry.template.name.value))
            .chain(class_templates.iter().cloned())
            .collect();
        // A return naming a template names no class, so lowering it would build a reference to
        // something nothing declares; what it names is settled at the call, by the contract below.
        let type_ref = value.return_type.and_then(|returned| {
            if names_a_template(slice(text, returned.span()), &templates) {
                return None;
            }
            self.lower_doc_type(returned, base, text, aliases)
        });

        let params = value
            .parameters
            .entries
            .iter()
            .map(|parameter| {
                let param_type = parameter
                    .r#type
                    .and_then(|written| self.lower_doc_type(written, base, text, aliases));
                // Through the same reader a real `@param` uses, with the node already in hand.
                // A `@method` tag documenting a callback is rare; sharing the reader costs nothing
                // and is what keeps the two from drifting.
                let callback_params_type_ref = parameter
                    .r#type
                    // Every template counted as held: a `@method` tag's callback beside a container
                    // is not a shape anybody writes, and "held" is the reading it always had.
                    .and_then(|written| {
                        self.callback_params_of(written, base, text, aliases, &templates, &|_| true)
                    });
                let range = shift(parameter.parameter.span(), base);
                self.builder.add_param(ParamData {
                    default: None,
                    name: self.interner.name(
                        String::from_utf8_lossy(parameter.parameter.value).trim_start_matches('$'),
                    ),
                    type_ref: param_type,
                    doc_type_ref: None,
                    doc_type_refused: false,
                    has_default: parameter.default.is_some(),
                    is_variadic: parameter.ellipsis.is_some(),
                    by_reference: parameter.ampersand.is_some(),
                    promotion: None,
                    name_range: range,
                    full_range: range,
                    out_type_ref: None,
                    callback_params_type_ref,
                    // A `@method` tag has no tags of its own to say it with.
                    closure_this_type_ref: None,
                })
            })
            .collect::<Vec<_>>();
        let params = self.builder.alloc_param_list(&params);
        let contract = self.method_tag_contract(value, params, base, text, aliases, &templates);

        Some(MemberData {
            has_default: false,
            type_from_body: false,
            doc_type_refused: false,
            returned_path: None,
            throws: lspkit_arena::IdSlice::EMPTY,
            // A tag has no body, so nothing states that it takes more than it names.
            reads_own_arguments: false,
            name,
            kind: MemberKind::Method,
            owner: class,
            visibility: visibility_of(value.visibility.as_ref()),
            set_visibility: None,
            modifiers: {
                let mut modifiers = MemberModifiers::default();
                modifiers.set_static(value.r#static.is_some());
                modifiers
            },
            // A docblock declaration has only a docblock type, so it goes in the native slot: there
            // is no native declaration for it to lose to.
            type_ref,
            doc_type_ref: None,
            params,
            hooks: IdSlice::EMPTY,
            body: None,
            name_range: shift(value.name.span(), base),
            full_range: shift(value.span(), base),
            docblock: None,
            contract,
            from_docblock: true,
            ty_from_value: false,
        })
    }

    /// What a `@method` tag's return type proves about a call, in the two ways a tag can say it.
    ///
    /// A real method's docblock spreads the claim over several tags -- `@template`, `@param`,
    /// `@return` -- and `contract_beside` reads them from the parsed document. A `@method` tag is
    /// the whole declaration in one line, so the same two claims are read off the tag's own parts:
    /// a return conditional on one of its parameters (`@method static ($id is int ? User :
    /// UserCollection) find($id)`, which is how a facade declares a finder), and a template one of
    /// its parameters binds through a `class-string<T>` (`@method T make<T of object>(class-string<T>
    /// $abstract)`, which is how a container declares its factory). Laravel writes both on every
    /// facade; a project using one reads its own code through them.
    fn method_tag_contract(
        &mut self,
        value: &MethodTagValue<'_>,
        params: IdSlice<ParamId>,
        base: TextSize,
        text: &str,
        aliases: &TypeAliasTable,
        templates: &[Cow<'_, str>],
    ) -> Option<ContractData> {
        let returned = value.return_type?;

        if let Some(conditional) = as_conditional(returned) {
            let Type::Variable(subject) = conditional.subject else {
                return None;
            };
            let argument = self.parameter_position(params, variable_name(subject))?;
            // Only a test an argument settles, and not one about what an array holds: see
            // `conditional_return`, whose reasons are the same here.
            let test = self
                .condition_test(conditional.target, base, text)
                .filter(|test| !matches!(test, ConditionTestData::NonEmpty))?;
            let (then, otherwise) = match conditional.is_negated() {
                false => (conditional.then, conditional.r#else),
                true => (conditional.r#else, conditional.then),
            };
            let then = self.method_tag_branch(value, then, base, text, aliases, templates)?;
            let otherwise =
                self.method_tag_branch(value, otherwise, base, text, aliases, templates)?;
            return Some(ContractData::ConditionalReturn {
                argument,
                test,
                then,
                otherwise,
                // A tag's default is a constant expression this layer does not read; an absent
                // argument then settles nothing, which is the answer `conditional_return` gives for
                // a default that is not a literal.
                by_default: None,
            });
        }

        // `@return T`, `?T` or `T|null`, where a parameter binds `T` through a `class-string<T>`.
        let mut named = Vec::new();
        let mut absent = None;
        collect_branch(returned, &mut named, &mut absent);
        let first = named.first()?;
        let written = slice(text, first.span());
        if named.iter().any(|leaf| slice(text, leaf.span()) != written)
            || !names_a_template(written, templates)
        {
            return None;
        }
        let parameter = Self::tag_parameter_binding(value, text, written)?;
        Some(ContractData::ReturnsArgumentClass {
            parameter,
            absent,
            many: false,
            // A `@method` tag's branches are collected one name at a time, so an intersection there
            // is not a shape this reader can reach.
            beside: false,
        })
    }

    /// One branch of a `@method` tag's conditional return: a template a `class-string<T>` parameter
    /// binds, or a plain type -- and nothing for a template nothing binds, which names no class.
    fn method_tag_branch(
        &mut self,
        value: &MethodTagValue<'_>,
        branch: &Type<'_>,
        base: TextSize,
        text: &str,
        aliases: &TypeAliasTable,
        templates: &[Cow<'_, str>],
    ) -> Option<ReturnBranchData> {
        let written = slice(text, branch.span());
        if let Type::Reference(_) = branch
            && let Some(position) = Self::tag_parameter_binding(value, text, written)
        {
            return Some(ReturnBranchData::ArgumentClass(position));
        }
        if names_a_template(written, templates) {
            return None;
        }
        Some(ReturnBranchData::Ty(
            self.lower_doc_type(branch, base, text, aliases)?,
        ))
    }

    /// Which of the tag's own parameters binds `written` through a `class-string<T>`, by position.
    ///
    /// Through a union too, because that is how a container's factory is declared:
    /// `class-string<T>|string $abstract` takes a service id and a class name in one parameter.
    fn tag_parameter_binding(value: &MethodTagValue<'_>, text: &str, written: &str) -> Option<u16> {
        let position = value.parameters.entries.iter().position(|parameter| {
            parameter
                .r#type
                .is_some_and(|ty| binds_class_string(ty, text, written))
        })?;
        u16::try_from(position).ok()
    }

    /// `write_only` is `None` for `@property`, `Some(false)` for read-only, `Some(true)` for
    /// write-only.
    fn docblock_property(
        &mut self,
        class: ClassId,
        value: &PropertyTagValue<'_>,
        base: TextSize,
        text: &str,
        write_only: Option<bool>,
        aliases: &TypeAliasTable,
    ) -> Option<MemberData> {
        let name = self
            .interner
            .name(String::from_utf8_lossy(value.variable.value).trim_start_matches('$'));
        let type_ref: Option<TypeRefId> = value
            .r#type
            .and_then(|written| self.lower_doc_type(written, base, text, aliases));
        let range = shift(value.variable.span(), base);

        // One-sided access expressed the way PHP 8.4 expresses it, so nothing downstream needs a
        // separate notion of a read-only magic property.
        let set_visibility = match write_only {
            Some(false) => Some(Visibility::Private),
            Some(true) | None => None,
        };

        Some(MemberData {
            has_default: false,
            type_from_body: false,
            doc_type_refused: false,
            returned_path: None,
            throws: lspkit_arena::IdSlice::EMPTY,
            // A tag has no body, so nothing states that it takes more than it names.
            reads_own_arguments: false,
            name,
            kind: MemberKind::Property,
            owner: class,
            visibility: Visibility::Public,
            set_visibility,
            modifiers: MemberModifiers::default(),
            type_ref,
            doc_type_ref: None,
            params: IdSlice::EMPTY,
            hooks: IdSlice::EMPTY,
            body: None,
            name_range: range,
            full_range: shift(value.span(), base),
            docblock: None,
            contract: None,
            from_docblock: true,
            ty_from_value: false,
        })
    }
}

fn visibility_of(declared: Option<&DocVisibility<'_>>) -> Visibility {
    match declared {
        Some(DocVisibility::Protected(_)) => Visibility::Protected,
        Some(DocVisibility::Private(_)) => Visibility::Private,
        Some(DocVisibility::Public(_)) | None => Visibility::Public,
    }
}
