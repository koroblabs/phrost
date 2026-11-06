//! Calls, PHP 8.1 partial application / first-class callables, and the argument lists both write.

use lspkit_arena::IdSlice;
use mago_span::HasSpan as _;
use mago_syntax::cst::{
    Argument, ArgumentList, Expression, NamedArgument, PartialArgument, PartialArgumentList,
    PositionalArgument,
};
use phrost_hir::{
    ExprId, ImportKind,
    body::{CallableReceiver, ClassTarget, ExprData},
};
use phrost_model::TextRange;
use phrost_syntax::identifier_text;

use crate::lowerer::Lowerer;

impl<'p> Lowerer<'_, 'p> {
    pub(crate) fn call(&mut self, call: &mago_syntax::cst::Call<'p>, range: TextRange) -> ExprData {
        use mago_syntax::cst::Call;

        match call {
            Call::Function(call) => {
                let args = self.arguments(Some(&call.argument_list));
                let literal_arguments = self.literal_arguments(&call.argument_list);
                // A written callee is a name to resolve; anything else is a value to type.
                match call.function {
                    Expression::Identifier(identifier) => ExprData::FunctionCall {
                        name: self.name_ref_of(identifier, ImportKind::Function),
                        args,
                        literal_arguments,
                        range,
                    },
                    callee => ExprData::IndirectCall {
                        callee: self.expression(callee),
                        args,
                        range,
                    },
                }
            }
            Call::Method(call) => {
                let receiver = self.expression(call.object);
                let (name, selector) = self.member_selector(&call.method);
                ExprData::MethodCall {
                    receiver,
                    name,
                    selector,
                    name_range: self.within_the_text(call.method.span()),
                    literal_arguments: self.literal_arguments(&call.argument_list),
                    args: self.arguments(Some(&call.argument_list)),
                    nullsafe: false,
                    range,
                }
            }
            Call::NullSafeMethod(call) => {
                let receiver = self.expression(call.object);
                let (name, selector) = self.member_selector(&call.method);
                ExprData::MethodCall {
                    receiver,
                    name,
                    selector,
                    name_range: self.within_the_text(call.method.span()),
                    literal_arguments: self.literal_arguments(&call.argument_list),
                    args: self.arguments(Some(&call.argument_list)),
                    nullsafe: true,
                    range,
                }
            }
            Call::StaticMethod(call) => {
                let class = self.class_target(call.class);
                let (name, selector) = self.member_selector(&call.method);
                ExprData::StaticCall {
                    class,
                    name,
                    selector,
                    name_range: self.within_the_text(call.method.span()),
                    literal_arguments: self.literal_arguments(&call.argument_list),
                    args: self.arguments(Some(&call.argument_list)),
                    range,
                }
            }
        }
    }

    /// `strlen(...)`, `$repo->find(...)`, `Repo::find(...)` -- PHP 8.1's first-class callables.
    ///
    /// Not lowered as a call, because it is not one: it *builds* a `Closure` and the callee is never
    /// entered. Lowering it as a call would give `$fn = Repo::find(...)` the return type of `find`,
    /// which is a confidently wrong type where there had been none -- and would make every
    /// argument-count and argument-type check fire on a call with no arguments.
    ///
    /// What it must do is put the names in the right symbol tables. Before this, the whole shape
    /// fell through to the generic walk, which found a bare identifier and recorded `Repo` as a
    /// *constant* -- so goto, hover, find-references and rename all searched the wrong table, and
    /// an import used only from `Repo::find(...)` read as unused. PHP 8.1 is not a corner.
    ///
    /// The two member forms get [`ExprData::CallableMember`], which carries the method name, so
    /// `member_site_at` finds it and hover, goto, find-references and rename answer on `find` the
    /// way they do everywhere else. It is deliberately reachable from the member-site walk and not
    /// from the call-site one. The function form gets [`ExprData::CallableFunction`], which carries
    /// only the name reference, because a function has no receiver and so no member site.
    ///
    /// Either way the node *is* the value: the closure it builds hands back whatever the callee
    /// returns, and that is read off the node by the layer with a catalog. Everything downstream of
    /// a first-class callable was dead while this shape had no node at all -- `$fn = $this->find(...)`
    /// then `$fn(1)->title()` reaches the title.
    pub(crate) fn partial_application(
        &mut self,
        partial: &mago_syntax::cst::PartialApplication<'p>,
        range: TextRange,
    ) -> ExprData {
        use mago_syntax::cst::PartialApplication;

        // Only `(...)` is PHP. mago's `PartialApplication` is a superset: it also covers the partial
        // application an RFC proposed and no release adopted -- `Repo::find($id, ?)` -- where real
        // arguments are written between the parentheses. Modelling those as a callable would drop
        // them, so they stay unmodelled with everything they wrote still lowered.
        let first_class = partial.is_first_class_callable();
        let mut inner = Vec::new();

        match partial {
            PartialApplication::Function(application) => match application.function {
                // A written callee is a name in the function table, exactly as it is for a call. No
                // member site: a function has no receiver, and the name reference already puts it
                // where goto and find-references look.
                Expression::Identifier(identifier) => {
                    let name = self.name_ref_of(identifier, ImportKind::Function);
                    if first_class {
                        return ExprData::CallableFunction { name, range };
                    }
                }
                callee => inner.push(self.expression(callee)),
            },
            PartialApplication::Method(application) => {
                let receiver = self.expression(application.object);
                // A computed selector -- `$obj->$name(...)` -- is a value, and is kept either way:
                // it holds a variable, and one nothing can reach is one a rename would not change.
                let (name, selector) = self.member_selector(&application.method);
                if first_class {
                    return ExprData::CallableMember {
                        receiver: CallableReceiver::Value(receiver),
                        name,
                        selector,
                        name_range: self.within_the_text(application.method.span()),
                        range,
                        // `(...)` says outright that a method is named and not called.
                        presumed: false,
                    };
                }
                inner.push(receiver);
                inner.extend(selector);
            }
            PartialApplication::StaticMethod(application) => {
                // The class is recorded class-like whichever form this is, which is the table an
                // import is bound in and the one goto has to search.
                let class = self.class_target(application.class);
                let (name, selector) = self.member_selector(&application.method);
                if first_class {
                    return ExprData::CallableMember {
                        receiver: CallableReceiver::Class(class),
                        name,
                        selector,
                        name_range: self.within_the_text(application.method.span()),
                        range,
                        // `(...)` says outright that a method is named and not called.
                        presumed: false,
                    };
                }
                if let ClassTarget::Expr(expr) = class {
                    inner.push(expr);
                }
                inner.extend(selector);
            }
        }

        inner.extend(self.partial_argument_values(partial.get_argument_list()));

        ExprData::Unmodelled {
            inner: self.builder.alloc_expr_list(&inner),
            range,
        }
    }

    /// `$x |> f(...)`: PHP 8.5's pipe, lowered as the call it makes.
    ///
    /// The operator passes its left operand as the sole argument of the callable on its right and
    /// hands back what that returns, so `$title |> trim(...) |> createDate(...)` *is*
    /// `createDate(trim($title))`. A node of its own would need every reader of a call -- hover,
    /// goto, the argument checks, the chain resolver -- taught a second spelling of one, where the
    /// call node already says everything the pipe does. A first-class callable names its callee and
    /// becomes that call with one argument; anything else on the right is a value being called,
    /// which is [`ExprData::IndirectCall`]'s shape already.
    ///
    /// The input is lowered first, because PHP evaluates it first.
    pub(crate) fn pipe(&mut self, pipe: &mago_syntax::cst::Pipe<'p>, range: TextRange) -> ExprData {
        use mago_syntax::cst::PartialApplication;

        let input = self.expression(pipe.input);
        let args = self.builder.alloc_expr_list(&[input]);
        // Nothing here decides a type off a literal: what is piped is a value, and `'x' |> f(...)`
        // is not a thing anybody writes.
        let literal_arguments = Box::default();

        if let Expression::PartialApplication(partial) = pipe.callable
            && partial.is_first_class_callable()
        {
            match partial {
                PartialApplication::Function(application) => match application.function {
                    Expression::Identifier(identifier) => {
                        return ExprData::FunctionCall {
                            name: self.name_ref_of(identifier, ImportKind::Function),
                            args,
                            literal_arguments,
                            range,
                        };
                    }
                    callee => {
                        return ExprData::IndirectCall {
                            callee: self.expression(callee),
                            args,
                            range,
                        };
                    }
                },
                PartialApplication::Method(application) => {
                    let receiver = self.expression(application.object);
                    let (name, selector) = self.member_selector(&application.method);
                    return ExprData::MethodCall {
                        receiver,
                        name,
                        selector,
                        name_range: self.within_the_text(application.method.span()),
                        literal_arguments,
                        args,
                        nullsafe: false,
                        range,
                    };
                }
                PartialApplication::StaticMethod(application) => {
                    let class = self.class_target(application.class);
                    let (name, selector) = self.member_selector(&application.method);
                    return ExprData::StaticCall {
                        class,
                        name,
                        selector,
                        name_range: self.within_the_text(application.method.span()),
                        literal_arguments,
                        args,
                        range,
                    };
                }
            }
        }

        ExprData::IndirectCall {
            callee: self.expression(pipe.callable),
            args,
            range,
        }
    }

    pub(crate) fn arguments(&mut self, list: Option<&ArgumentList<'p>>) -> IdSlice<ExprId> {
        let Some(list) = list else {
            return IdSlice::EMPTY;
        };

        // Arguments keep their written order. Matching a name to a parameter needs the callee's
        // signature, which is a question for a layer with a catalog and not for lowering.
        let ids = list
            .arguments
            .iter()
            .map(|argument| self.argument(argument))
            .collect::<Vec<_>>();
        self.builder.alloc_expr_list(&ids)
    }

    /// Lowers one argument, wrapping it only when it says more than its expression does.
    fn argument(&mut self, argument: &Argument<'p>) -> ExprId {
        match argument {
            Argument::Positional(positional) => self.positional_argument(positional),
            Argument::Named(named) => self.named_argument(named),
        }
    }

    /// `$foo`, or `...$rest`.
    fn positional_argument(&mut self, positional: &PositionalArgument<'p>) -> ExprId {
        // A plain positional argument is its expression; there is nothing else to record.
        if positional.ellipsis.is_none() {
            return self.expression(positional.value);
        }
        let range = self.within_the_text(positional.span());
        let value = self.expression(positional.value);
        self.builder.add_expr(ExprData::Argument {
            name: None,
            spread: true,
            value,
            range,
        })
    }

    /// `name: $foo`.
    ///
    /// A name is enough on its own: everything that reads this wants to know whether the argument
    /// was written by name, and a named spread -- legal, and about as common as it sounds -- is
    /// already covered by that.
    fn named_argument(&mut self, named: &NamedArgument<'p>) -> ExprId {
        let range = self.within_the_text(named.span());
        let name = self.interner.name(&identifier_text(named.name.value));
        let value = self.expression(named.value);
        self.builder.add_expr(ExprData::Argument {
            name: Some(name),
            spread: false,
            value,
            range,
        })
    }

    /// The arguments of an attribute.
    ///
    /// Separate from [`Self::arguments`] because mago parses an attribute's list as the partial form
    /// that first-class callable syntax also uses. A placeholder cannot be written in an attribute,
    /// so the two forms that can are lowered and the rest are left out rather than invented.
    pub(crate) fn partial_arguments(
        &mut self,
        list: Option<&PartialArgumentList<'p>>,
    ) -> IdSlice<ExprId> {
        let Some(list) = list else {
            return IdSlice::EMPTY;
        };
        let ids = self.partial_argument_values(list);
        self.builder.alloc_expr_list(&ids)
    }

    /// The expressions a partial argument list wrote, in written order.
    ///
    /// Three of the five shapes are placeholders -- `?`, `...`, `name: ?` -- and write no expression
    /// at all, so a first-class callable's list is empty and there is nothing for its node to carry.
    fn partial_argument_values(&mut self, list: &PartialArgumentList<'p>) -> Vec<ExprId> {
        list.arguments
            .iter()
            .filter_map(|argument| match argument {
                PartialArgument::Positional(positional) => {
                    Some(self.positional_argument(positional))
                }
                PartialArgument::Named(named) => Some(self.named_argument(named)),
                _ => None,
            })
            .collect()
    }
}
