//! A function-like's parameters, each with what its docblock says beside its hint.
//!
//! Its own module because a parameter is lowered the same way for a method, a function and a closure,
//! and because what a docblock means beside a parameter has rules of its own: a promoted one may
//! carry a docblock of its own, a by-reference one has a written-back type, and a variadic one may
//! be documented as the array of its arguments.

use lspkit_arena::IdSlice;
use mago_span::HasSpan as _;
use mago_syntax::cst::{FunctionLikeParameter, FunctionLikeParameterList};
use phrost_hir::{ParamData, ParamPromotion};
use phrost_syntax::identifier_text;

use crate::docblock::{DocTag, type_alias::TypeAliasTable};
use crate::lowerer::{Lowerer, set_visibility_of, visibility_of};
use crate::members::member_modifiers;

impl<'p> Lowerer<'_, 'p> {
    /// `declaration_start` is where the docblock search begins: a `@param` belongs to the
    /// function-like's own comment, not to anything nearer the parameter.
    ///
    /// No class context: the one caller outside this file is a free function's parameter list,
    /// which has no `@phpstan-type` table to pass. A method's own parameters go through
    /// [`Self::params_with_aliases`] instead.
    pub(crate) fn params(
        &mut self,
        list: &FunctionLikeParameterList<'p>,
        declaration_start: phrost_model::TextSize,
    ) -> IdSlice<phrost_hir::ParamId> {
        self.params_with_aliases(
            list,
            declaration_start,
            &std::rc::Rc::clone(&self.class_aliases),
        )
    }

    /// [`Self::params`], with the enclosing class's `@phpstan-type` table in hand.
    pub(crate) fn params_with_aliases(
        &mut self,
        list: &FunctionLikeParameterList<'p>,
        declaration_start: phrost_model::TextSize,
        aliases: &TypeAliasTable,
    ) -> IdSlice<phrost_hir::ParamId> {
        let ids = list
            .parameters
            .iter()
            .map(|param| self.param(param, declaration_start, aliases))
            .collect::<Vec<_>>();
        self.builder.alloc_param_list(&ids)
    }

    /// What one argument of a variadic takes, when its `@param` was written as the array of them.
    ///
    /// `@param Row[] ...$rows` is the older way to say each argument is a `Row`, and PHPStan reads it
    /// so: `rows(new Row())` passes and `rows([new Row()])` does not. Read as written, every call
    /// passing one map to `@param array<string, float>[] ...$maps` was reported. A keyless array
    /// only -- `list<Row> ...$rows` really is a list per argument, there and here -- and not where
    /// the hint is itself a container, which already says each argument is one.
    fn each_argument_of(
        &mut self,
        native: Option<phrost_hir::TypeRefId>,
        doc: phrost_hir::TypeRefId,
    ) -> phrost_hir::TypeRefId {
        use phrost_hir::TypeRefData;
        use phrost_model::BuiltinTy;
        let container = |data: &TypeRefData| {
            matches!(
                data,
                TypeRefData::Builtin(BuiltinTy::Array | BuiltinTy::Iterable)
            )
        };
        if let Some(native) = native {
            let hint = match self.builder.type_ref(native) {
                TypeRefData::Nullable(inner) => *inner,
                _ => native,
            };
            if container(self.builder.type_ref(hint)) {
                return doc;
            }
        }
        let (array, nullable) = match self.builder.type_ref(doc) {
            TypeRefData::Nullable(inner) => (*inner, true),
            _ => (doc, false),
        };
        let element = match self.builder.type_ref(array) {
            TypeRefData::Generic {
                base,
                args,
                non_empty: false,
            } if matches!(
                self.builder.type_ref(*base),
                TypeRefData::Builtin(BuiltinTy::Array)
            ) =>
            {
                match self.builder.type_ref_list(*args) {
                    [element] => *element,
                    _ => return doc,
                }
            }
            _ => return doc,
        };
        match nullable {
            true => self.builder.add_type_ref(TypeRefData::Nullable(element)),
            false => element,
        }
    }

    fn param(
        &mut self,
        param: &FunctionLikeParameter<'p>,
        declaration_start: phrost_model::TextSize,
        aliases: &TypeAliasTable,
    ) -> phrost_hir::ParamId {
        self.attribute_names(&param.attribute_lists);
        let written = identifier_text(param.variable.name);
        let short = written.trim_start_matches('$');
        let name = self.interner.name(short);
        let type_ref = param.hint.as_ref().map(|hint| self.hint(hint));
        let doc_type_ref = self.doc_type_beside_with_aliases(
            type_ref,
            declaration_start,
            DocTag::Param(short),
            aliases,
        );
        let doc_type_ref = match param.is_variadic() {
            true => doc_type_ref.map(|doc| self.each_argument_of(type_ref, doc)),
            false => doc_type_ref,
        };
        // The whole parameter's span and not the variable's: a docblock of its own sits above
        // `public array $rules`, and `raw_docblock_before` wants the offset the comment immediately
        // precedes. Asked at the variable it finds nothing, with `public array ` in between -- the
        // same rule §8 records for a method, whose docblock precedes its modifiers.
        //
        // Hoisted because two readings want it, and the second one got it wrong: a `@deprecated`
        // above a promoted property was asked for at the *constructor's* start, where it read the
        // constructor's docblock and never the parameter's.
        let own_start =
            crate::lowerer::declaration_start(&param.attribute_lists, None, param.span());
        let doc_type_ref = doc_type_ref.or_else(|| {
            if !param.is_promoted_property() {
                return None;
            }
            // A promoted parameter may carry a docblock of its own, and that docblock is about the
            // *property* it declares: `__construct(/** @var Constraint[] */ public array
            // $constraints)` is how Symfony's own attributes are written, and PHPStan reads it.
            // Only a promoted one, because an inline `@var` above a parameter that declares no
            // property says nothing anybody has agreed the meaning of.
            //
            // Second, never first: the constructor's own `@param` is the documented spelling and
            // already worked, so where both are written the one on the function wins and this
            // changes nothing. Measured before the change, with `undefined-method` on the element
            // as the instrument -- `@param Rule[]` typed the promoted property and the inline
            // `@var Rule[]` did not.
            self.doc_type_beside_with_aliases(type_ref, own_start, DocTag::Var, aliases)
        });

        // Only for a by-reference parameter, because that is the only kind whose written-back type
        // anybody can observe. `@param-out` on a by-value parameter is a mistake in the docblock
        // rather than a claim, and lowering it would cost a tag lookup on every parameter there is.
        // `is_reference()`, which is what the `by_reference` field below asks. The `ampersand` token
        // is the other spelling of the same question and is not the same answer: a parameter written
        // `?int &$count` does not carry it where one written `&$matches` does, so gating on the token
        // read `preg_match`'s tag and silently skipped `str_replace`'s.
        let out_type_ref = param
            .is_reference()
            .then(|| {
                self.doc_type_beside_with_aliases(
                    type_ref,
                    declaration_start,
                    DocTag::ParamOut(short),
                    aliases,
                )
            })
            .flatten();

        // What the callback this parameter takes will be called with, which only a docblock can
        // say. Asked of every parameter and answered by a substring test for all but the few that
        // document one -- see `doc_callback_params`.
        let callback_params_type_ref =
            self.doc_callback_params(declaration_start, DocTag::Param(short), aliases);

        let closure_this_type_ref = self.doc_closure_this(declaration_start, short, aliases);

        // A `@param` naming a template is refused above, and the empty slot it leaves is
        // indistinguishable from an undocumented parameter unless the refusal is recorded. Asked
        // only where nothing was lowered, so the common parameter pays one branch.
        let doc_type_refused = doc_type_ref.is_none()
            && self.doc_names_a_template(declaration_start, DocTag::Param(short));

        // A promoted parameter can carry hooks too, and they belong to the property it declares.
        let promoted_name = identifier_text(param.variable.name)
            .trim_start_matches('$')
            .to_owned();
        let (hooks, shape) = self.property_hooks(
            param.hooks.as_ref(),
            type_ref,
            declaration_start,
            Some(&promoted_name),
        );
        // A parameter with a visibility modifier is also a property declaration.
        let promotion = param.is_promoted_property().then(|| ParamPromotion {
            visibility: visibility_of(&param.modifiers),
            set_visibility: set_visibility_of(&param.modifiers),
            modifiers: {
                let mut modifiers = member_modifiers(&param.modifiers, shape);
                modifiers.set_deprecated(self.is_deprecated(&param.attribute_lists, own_start));
                modifiers
            },
            hooks,
        });

        self.builder.add_param(ParamData {
            name,
            type_ref,
            doc_type_ref,
            doc_type_refused,
            out_type_ref,
            has_default: param.default_value.is_some(),
            // Sliced from the source rather than lowered: the one reader writes it back into a
            // file, so what it wants is the characters the author typed.
            default: param
                .default_value
                .as_ref()
                .and_then(|default| self.written_between(default.value.span())),
            is_variadic: param.is_variadic(),
            by_reference: param.is_reference(),
            promotion,
            name_range: self.within_the_text(param.variable.span()),
            full_range: self.within_the_text(param.span()),
            callback_params_type_ref,
            closure_this_type_ref,
        })
    }
}
