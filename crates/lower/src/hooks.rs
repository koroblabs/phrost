//! PHP 8.4 property hooks, lowered as the function-likes they are.
//!
//! A hook has a body, `set` has a parameter, and `$this` inside one is the declaring class. So a
//! hook is a scope, and everything above this layer answers about a name written in one only
//! because the hook is lowered here rather than swallowed by the property.
//!
//! The generic children walk still steps around every `PropertyHookBody` -- see
//! [`Lowerer::nested_expressions`](crate::lowerer::Lowerer::nested_expressions). That is not an
//! oversight left over from before: a hook body reached both ways would be lowered twice, once into
//! the hook and once into the body of whatever unmodelled statement encloses the class, and nothing
//! would fail because the duplicates are identical.

use lspkit_arena::IdSlice;
use mago_span::HasSpan as _;
use mago_syntax::cst::{
    PropertyHook, PropertyHookBody, PropertyHookConcreteBody, PropertyHookList,
};
use phrost_hir::{
    BodyId, HookData, HookId, HookKind, Hooks, ParamData, TypeRefId,
    body::{BodyData, StmtData},
};
use phrost_model::{TextRange, TextSize};
use phrost_syntax::identifier_text;

use crate::lowerer::Lowerer;

impl<'p> Lowerer<'_, 'p> {
    /// Every hook written after a property's name, or an empty list when none were.
    ///
    /// `declared` is the property's own type, which is what an implicit `set` parameter receives --
    /// PHP says so, and the source does not.
    ///
    /// `backing` is the property's own name, which is what decides whether the hooks make it a
    /// backed property or a virtual one. Returns the list and the [`Hooks`] shape, which is what a
    /// reader wants without following the ids.
    pub(crate) fn property_hooks(
        &mut self,
        list: Option<&PropertyHookList<'p>>,
        declared: Option<TypeRefId>,
        declaration_start: TextSize,
        backing: Option<&str>,
    ) -> (IdSlice<HookId>, Hooks) {
        let Some(list) = list else {
            return (IdSlice::EMPTY, Hooks::None);
        };

        let span = self.within_the_text(list.span());
        let written = &self.parsed_source()[span.start().into()..span.end().into()];
        let shape = if list
            .hooks
            .iter()
            .any(|hook| hook_kind(hook) == HookKind::Set)
        {
            Hooks::Settable
        } else if !list
            .hooks
            .iter()
            .any(|hook| !matches!(hook.body, PropertyHookBody::Abstract(_)))
        {
            // A hook with no body says nothing about backing: `public string $n { get; }` in an
            // interface or an abstract class is a *requirement*, and whoever implements it decides
            // whether the property holds a slot.
            Hooks::DeclaredGet
        } else if backing.is_some_and(|name| mentions_backing(written, name)) {
            Hooks::BackedGet
        } else {
            Hooks::VirtualGet
        };
        let ids = list
            .hooks
            .iter()
            .map(|hook| self.property_hook(hook, declared, declaration_start))
            .collect::<Vec<_>>();
        (self.builder.alloc_hook_list(&ids), shape)
    }

    fn property_hook(
        &mut self,
        hook: &PropertyHook<'p>,
        declared: Option<TypeRefId>,
        declaration_start: TextSize,
    ) -> HookId {
        self.attribute_names(&hook.attribute_lists);

        let name_range = self.within_the_text(hook.name.span());
        let kind = hook_kind(hook);

        let params = match &hook.parameter_list {
            Some(list) => self.params(list, declaration_start),
            None => self.implicit_set_parameter(kind, declared, name_range),
        };

        let body = self.hook_body(kind, &hook.body);
        self.builder.add_hook(HookData {
            kind,
            params,
            body,
            returns_by_reference: hook.ampersand.is_some(),
            name_range,
            full_range: self.within_the_text(hook.span()),
        })
    }

    /// The `$value` a `set` written without a parameter list still receives.
    ///
    /// `set { $this->raw = $value; }` is the shorter of the two legal spellings and binds `$value`
    /// with the property's own type. Nothing in the text says so, so the parameter is synthesized --
    /// and it takes the `set` keyword's range, because that keyword is what introduces the name and
    /// a range pointing nowhere would be a goto target that lands at `0:0`.
    fn implicit_set_parameter(
        &mut self,
        kind: HookKind,
        declared: Option<TypeRefId>,
        name_range: TextRange,
    ) -> IdSlice<phrost_hir::ParamId> {
        if kind != HookKind::Set {
            return IdSlice::EMPTY;
        }

        let name = self.interner.name("value");
        let id = self.builder.add_param(ParamData {
            default: None,
            name,
            type_ref: declared,
            doc_type_ref: None,
            doc_type_refused: false,
            has_default: false,
            is_variadic: false,
            by_reference: false,
            promotion: None,
            name_range,
            full_range: name_range,
            out_type_ref: None,
            // Synthesised from the property's own type: there is no docblock naming it, so there is
            // nothing that could say what it is called with.
            callback_params_type_ref: None,
            closure_this_type_ref: None,
        });
        self.builder.alloc_param_list(&[id])
    }

    /// A hook's body, in whichever of PHP's three forms it was written.
    ///
    /// `get;` in an interface has none. `get => expr` becomes a body holding one `return`, the same
    /// normalization an arrow function gets, so everything reading a body has one shape -- but
    /// `set => expr` is not a return: the expression is evaluated for its effect and the hook hands
    /// nothing back, so it stays a plain expression statement.
    fn hook_body(&mut self, kind: HookKind, body: &PropertyHookBody<'p>) -> Option<BodyId> {
        match body {
            PropertyHookBody::Abstract(_) => None,
            PropertyHookBody::Concrete(PropertyHookConcreteBody::Block(block)) => self.body(block),
            PropertyHookBody::Concrete(PropertyHookConcreteBody::Expression(written)) => {
                if !self.lower_bodies {
                    return None;
                }
                match kind {
                    HookKind::Get => Some(self.expression_body(written.expression)),
                    HookKind::Set => Some(self.effect_body(written.expression)),
                }
            }
        }
    }

    /// A body holding a single expression statement, for `set => $this->raw = $value;`.
    fn effect_body(&mut self, expression: &mago_syntax::cst::Expression<'p>) -> BodyId {
        let range = self.within_the_text(expression.span());
        let outer = std::mem::take(&mut self.saw_yield);
        let expr = self.expression(expression);
        let is_generator = std::mem::replace(&mut self.saw_yield, outer);
        let statement = self.builder.add_stmt(StmtData::Expr { expr, range });
        let statements = self.builder.alloc_stmt_list(&[statement]);
        self.builder.add_body(BodyData {
            statements,
            range,
            is_generator,
        })
    }
}

/// Which of the two a hook's keyword names.
///
/// Anything but `set` is a `get`, and not only a literal `get`: while the keyword is being typed
/// the recovered name is a prefix of one, and lowering the body anyway is what lets a completion
/// inside it answer.
fn hook_kind(hook: &PropertyHook<'_>) -> HookKind {
    match identifier_text(hook.name.value).eq_ignore_ascii_case("set") {
        true => HookKind::Set,
        false => HookKind::Get,
    }
}

/// Whether hook source mentions `$this-><name>`.
///
/// Read off the written span rather than the lowered body, because a hook's body is lowered only
/// for the project's own files -- and the rule has to hold for a dependency's class as much as for
/// one's own. Read as text rather than as a tree for the same reason, and the direction it is wrong
/// in is the safe one: a mention inside a string or a comment makes the property read as *backed*,
/// which withdraws a report rather than inventing one.
///
/// Exactly, because PHP compares property names exactly: `$name` and `$Name` are two properties.
fn mentions_backing(written: &str, name: &str) -> bool {
    let mut rest = written;
    while let Some(at) = rest.find("$this") {
        rest = &rest[at + "$this".len()..];
        let after = rest.trim_start();
        let Some(after) = after
            .strip_prefix("?->")
            .or_else(|| after.strip_prefix("->"))
        else {
            continue;
        };
        let after = after.trim_start();
        if let Some(tail) = after.strip_prefix(name)
            && !tail.starts_with(|ch: char| ch.is_alphanumeric() || ch == '_')
        {
            return true;
        }
    }
    false
}
