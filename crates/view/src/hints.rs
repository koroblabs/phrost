//! Which parameter each argument at a call site fills.
//!
//! The fact behind a parameter-name hint, and the only way to produce one honestly: it needs the
//! callee's signature, which means resolving the call, which is what this layer is for. Guessing
//! from position alone would label `f($a, $b)` with the wrong names as soon as the callee changed.

use phrost_body::{ArgumentSite, CallSite, CallTarget};
use phrost_model::{Name, TextRange};
use phrost_sema::{MemberKind, ParamRow};

use crate::parameters::Parameter;
use crate::view::{FOLLOW_LIMIT, IndexView};

/// One argument, and the parameter it fills.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ArgumentBinding {
    /// The argument as written, so a caller can decide whether naming it adds anything.
    pub range: TextRange,
    /// The parameter's name, without its `$`.
    pub parameter: Name,
}

/// What a call's receiver supplied for the templates of the class that declared the callee.
///
/// A parameter typed `T` is not a type: judging an argument against the bare name rejects every
/// scalar for not being a class and accepts every class because the name resolves to none, which is
/// wrong in both directions at once. So the check needs what the receiver was written with, and the
/// class whose templates to look for -- and both are known at the one place that resolved the
/// receiver, which is why they travel with the parameters rather than being found again.
#[derive(Clone, Debug)]
pub(crate) struct CalleeTemplates {
    /// The class the value has the member from.
    pub(crate) called: phrost_model::Name,
    /// The class that *declared* it, which is whose `@template` a `T` in it names.
    pub(crate) owner: phrost_model::Name,
    /// What the receiver itself was written with: the `Row` of a `@var Bag<Row>`.
    pub(crate) supplied: Vec<crate::resolve::Held>,
}

impl IndexView<'_> {
    /// Every argument in the current file whose parameter is known, in source order.
    ///
    /// Silent about a call it cannot resolve, and about arguments a name would not describe: past a
    /// named argument the positions no longer line up, a spread stands for however many values are
    /// in the array, and a variadic parameter takes all the rest under one name.
    pub fn argument_bindings(&self) -> Vec<ArgumentBinding> {
        let Some(open) = self.current() else {
            return Vec::new();
        };
        let hir = open.hir();
        let mut out = Vec::new();

        for call in phrost_body::call_sites(hir) {
            let Some(params) = self.parameters_of(hir, &call) else {
                continue;
            };
            bind(&call.arguments, &params, &mut out);
        }

        out.sort_by_key(|binding| binding.range.start());
        out
    }

    /// The callee's parameters, whichever kind of call it is.
    pub(crate) fn parameters_of(
        &self,
        hir: &phrost_hir::FileHir,
        call: &CallSite,
    ) -> Option<Box<[Parameter]>> {
        self.parameters_and_templates_of(hir, call)
            .map(|(params, _)| params)
    }

    /// Whether the callee takes arguments its signature does not name.
    ///
    /// `func_get_args()` / `func_num_args()` in the body, recorded at lowering because nothing
    /// above the boundary can see a body -- see `phrost_sema::MemberRow::reads_own_arguments`.
    ///
    /// Or a method reached only through a `@mixin`, which is not the callee at all: the call lands
    /// in `__call`, which takes whatever it is given, and the tag says what answers rather than how
    /// many arguments it counts. Laravel's `HigherOrderCollectionProxy` is `@mixin Enumerable` for
    /// the type `->map->` hands back, and forwards `->map->toArray($request)` to every *element* --
    /// so `Enumerable::toArray()` taking none is no claim about that call.
    ///
    /// A second resolution of the callee, and deliberately so: the only caller asks it *after*
    /// deciding a call has too many arguments, which is a handful of sites in a project. Folding it
    /// into [`Self::parameters_and_templates_of`] would make every call in every file pay a wider
    /// return for a fact almost none of them need -- the shape §8 records for `NeverReturns`, where
    /// an expensive question asked constantly and answering the same way almost always wants a
    /// cheap gate in front of it rather than a place in the common path.
    ///
    /// `false` where the callee does not resolve, which is the answer that lets the report stand: a
    /// declaration nothing found cannot be shown to accept more than it names.
    pub(crate) fn callee_takes_more_than_it_names(
        &self,
        hir: &phrost_hir::FileHir,
        call: &CallSite,
    ) -> bool {
        match &call.target {
            CallTarget::Member { receiver, name } => {
                let at = call.receiver_at();
                let Some(held) = self.resolve_path(at, hir, receiver, FOLLOW_LIMIT) else {
                    return false;
                };
                let Some(class) =
                    self.declaring_class(at, hir, &held, MemberKind::Method, name.as_str())
                else {
                    return false;
                };
                self.members(class.as_str())
                    .find(MemberKind::Method, name.as_str())
                    .is_some_and(|member| member.row.reads_own_arguments || member.forwarded)
            }
            CallTarget::Function(reference) => {
                let written = hir.name_ref(*reference);
                self.resolve_name(
                    call.range.start(),
                    written.path.as_str(),
                    written.anchor,
                    phrost_hir::ImportKind::Function,
                )
                .is_some_and(|location| self.function_reads_own_arguments(location.fqn.as_str()))
            }
            // Nothing records whether a closure reads `func_get_args()`, and `false` is the answer
            // that lets an extra-argument report stand.
            CallTarget::Invoked(_) => false,
            CallTarget::Constructor(target) => {
                let at = call.range.start();
                let Some(class) = self.class_of_target(at, hir, *target) else {
                    return false;
                };
                self.members(class.as_str())
                    .find(MemberKind::Method, "__construct")
                    .is_some_and(|member| member.row.reads_own_arguments)
            }
        }
    }

    /// The same, with what the receiver supplied for the callee's class templates.
    ///
    /// The two together, because a parameter typed `T` says nothing on its own -- and judging an
    /// argument against the bare template name is wrong in *both* directions at once: a scalar is
    /// rejected for not being a class, and any class is accepted because `T` resolves to none.
    /// Measured on a probe: five findings, all false, and two genuinely wrong classes missed.
    ///
    /// `None` for the context where there is no receiver -- a free function, a `new` -- which is
    /// also where a class template cannot appear.
    ///
    /// Memoized per call for as long as the view lives: six argument checks ask it of every call in
    /// the file, and each answer is a receiver resolution and a member lookup.
    pub(crate) fn parameters_and_templates_of(
        &self,
        hir: &phrost_hir::FileHir,
        call: &CallSite,
    ) -> Option<(Box<[Parameter]>, Option<CalleeTemplates>)> {
        let key = (call.range, call.callee_range);
        if let Some(known) = self.call_parameters.borrow().get(&key) {
            return known.clone();
        }
        let found = self.parameters_and_templates_uncached(hir, call);
        self.call_parameters.borrow_mut().insert(key, found.clone());
        found
    }

    fn parameters_and_templates_uncached(
        &self,
        hir: &phrost_hir::FileHir,
        call: &CallSite,
    ) -> Option<(Box<[Parameter]>, Option<CalleeTemplates>)> {
        match &call.target {
            CallTarget::Member { receiver, name } => {
                let at = call.receiver_at();
                let held = self.resolve_path(at, hir, receiver, FOLLOW_LIMIT)?;
                self.parameters_through(at, hir, &held, name.as_str())
            }
            CallTarget::Function(reference) => {
                let written = hir.name_ref(*reference);
                let location = self.resolve_name(
                    call.range.start(),
                    written.path.as_str(),
                    written.anchor,
                    phrost_hir::ImportKind::Function,
                )?;
                Some((
                    declared(&self.function_params(location.fqn.as_str())?),
                    None,
                ))
            }
            // With the class's templates, which a constructor's parameters name as readily as a
            // method's: `@param T $items` under `@template T of array`. Nothing is supplied -- the
            // `new` is what binds them -- so an argument is judged against the bound, and
            // `new Collection(1)` is outside it. Without them every such argument was `Unknown`.
            CallTarget::Constructor(target) => {
                let at = call.range.start();
                let class = self.class_of_target(at, hir, *target)?;
                let table = self.members(class.as_str());
                let member = table.find(MemberKind::Method, "__construct")?;
                let owner = match &member.provenance {
                    phrost_sema::MemberProvenance::Inherited(fqn)
                    | phrost_sema::MemberProvenance::Interface(fqn) => fqn.clone(),
                    _ => class.clone(),
                };
                Some((
                    declared(&member.row.params),
                    Some(CalleeTemplates {
                        called: class,
                        owner,
                        supplied: Vec::new(),
                    }),
                ))
            }
            // A call through a value, whose parameters exist only where the value is a closure
            // written in the file being read. Nothing is looked up: the literal is an id into this
            // file's arena, exactly as a `@var` is, and its parameters are read straight off it.
            //
            // No templates. A closure declares none of its own that a receiver could fill, and
            // there is no receiver.
            CallTarget::Invoked(value) => {
                let at = call.range.start();
                let held = self.resolve_path(at, hir, value, FOLLOW_LIMIT)?;
                // Two sources and they are read in this order because a closure literal is the more
                // specific answer: a value can be both -- `$fn = new Handler()` where `Handler` also
                // holds a closure is not a shape anybody writes, but `Held::Callable` carrying a
                // literal is a statement about *this* value where a class is a statement about the
                // type.
                if let Some(literal) = self.closure_literal_of(&held)
                    && let phrost_hir::body::ExprData::Closure { params, .. } = hir.expr(literal)
                {
                    return Some((
                        hir.param_list(*params)
                            .iter()
                            .map(|&id| Parameter::Closure(hir.param(id).clone()))
                            .collect(),
                        None,
                    ));
                }
                // An invokable object. `$handler($request)` is `$handler->__invoke($request)` by
                // PHP's own rule, so the parameters are a member lookup like any other -- and they
                // are stored, so they arrive in the vocabulary the checks already read.
                // A first-class callable: `$fn = $this->takes(...)`. The value is a `Closure` made
                // from a declaration, and the path the binding wrote is the call that was never
                // made -- so the callee is reachable even though `Callable` keeps only what
                // invoking it returns.
                //
                // Gated on the value already being a callable, which is what keeps the statement
                // walk below off the common path: an invokable object is a `Held::Class` and skips
                // it entirely, and the two cases are disjoint by construction.
                if matches!(held, crate::resolve::Held::Callable(_))
                    && let Some(of) = self.callable_behind(at, hir, value)
                    && let Some(found) = self.parameters_of_callee(at, hir, &of)
                {
                    return Some(found);
                }
                // An invokable object. `$handler($request)` is `$handler->__invoke($request)` by
                // PHP's own rule, so the parameters are a member lookup like any other -- and they
                // are stored, so they arrive in the vocabulary the checks already read.
                self.parameters_through(at, hir, &held, "__invoke")
            }
        }
    }
}

impl IndexView<'_> {
    /// One method's parameters, reached through a value, with the templates that value supplied.
    ///
    /// Shared by the three ways a method is called -- written with `->`, reached by invoking an
    /// object, and reached through a first-class callable -- so which templates a `T` belongs to is
    /// decided once. Who *declared* the member is what decides it: a `@param T` on an inherited
    /// method is the supertype's `T` and not this class's.
    fn parameters_through(
        &self,
        at: phrost_model::TextSize,
        hir: &phrost_hir::FileHir,
        held: &crate::resolve::Held,
        name: &str,
    ) -> Option<(Box<[Parameter]>, Option<CalleeTemplates>)> {
        // A reduction to one class reads nothing off an intersection, and a trait's `$this` is one
        // where the trait names what composes it -- see `declaring_class`.
        let class = self.declaring_class(at, hir, held, MemberKind::Method, name)?;
        let table = self.members(class.as_str());
        let member = table.find(MemberKind::Method, name)?;
        // A method this scope may not call, on a class with `__call` or `__callStatic`, is a call
        // to the magic method -- PHP runs that and never the declaration, so the declaration's
        // parameters say nothing about the arguments. Laravel's `#[Scope] protected function
        // popular($query)` is called as `Post::popular()`, and was reported a missing argument.
        if self.reaches_through_magic(at, class.as_str(), MemberKind::Method, name) {
            return None;
        }
        let owner = match &member.provenance {
            phrost_sema::MemberProvenance::Inherited(fqn)
            | phrost_sema::MemberProvenance::Interface(fqn) => fqn.clone(),
            _ => class.clone(),
        };
        let templates = CalleeTemplates {
            called: class,
            owner,
            supplied: self.arguments_of_held(hir, held),
        };
        Some((declared(&member.row.params), Some(templates)))
    }

    /// The call a first-class callable stands for, when the value being invoked is one.
    ///
    /// `$fn = $this->takes(...)` and `$fn = strlen(...)` both bind a path whose root is
    /// [`phrost_body::PathRoot::Callable`], wrapping the call that was never made. Written in place
    /// -- `($this->takes(...))(...)` -- the wrapper is the value's own root; bound to a local, which
    /// is how anybody writes it, it is the binding's.
    fn callable_behind(
        &self,
        at: phrost_model::TextSize,
        hir: &phrost_hir::FileHir,
        value: &phrost_body::ValuePath,
    ) -> Option<phrost_body::ValuePath> {
        if !value.steps.is_empty() {
            return None;
        }
        if let phrost_body::PathRoot::Callable(of) = &value.root {
            return Some((**of).clone());
        }
        let phrost_body::PathRoot::Local(name) = &value.root else {
            return None;
        };
        let bound = phrost_body::local_binding(
            hir,
            at,
            name.as_str(),
            &|hir, expr| self.call_never_returns(at, hir, expr),
            &self.asserting(),
        )?;
        match &bound.value.root {
            phrost_body::PathRoot::Callable(of) if bound.value.steps.is_empty() => {
                Some((**of).clone())
            }
            _ => None,
        }
    }

    /// The parameters of the callee a never-made call names.
    ///
    /// Two shapes and they are the two a first-class callable has: a method, whose receiver is the
    /// path with the last step taken off, and a free function, which is the path's root and has no
    /// receiver at all. Anything else -- a property holding a callable, a constant -- names no
    /// declaration to read.
    fn parameters_of_callee(
        &self,
        at: phrost_model::TextSize,
        hir: &phrost_hir::FileHir,
        of: &phrost_body::ValuePath,
    ) -> Option<(Box<[Parameter]>, Option<CalleeTemplates>)> {
        if let Some((phrost_body::PathStep::MethodReturn { name, .. }, before)) =
            of.steps.split_last()
        {
            let receiver = phrost_body::ValuePath {
                root: of.root.clone(),
                steps: before.to_vec(),
                // The receiver is inside the chain; what the whole expression may be says nothing
                // about it.
                may_be_absent: false,
            };
            let held = self.resolve_path(at, hir, &receiver, FOLLOW_LIMIT)?;
            return self.parameters_through(at, hir, &held, name.as_str());
        }
        let phrost_body::PathRoot::FunctionReturn { name, anchor, .. } = &of.root else {
            return None;
        };
        let location =
            self.resolve_name(at, name.as_str(), *anchor, phrost_hir::ImportKind::Function)?;
        Some((
            declared(&self.function_params(location.fqn.as_str())?),
            None,
        ))
    }
}

/// The parameters of a declaration, which are already in the vocabulary the checks read.
fn declared(params: &[ParamRow]) -> Box<[Parameter]> {
    params.iter().cloned().map(Parameter::Declared).collect()
}

/// Lines arguments up with parameters, stopping wherever the correspondence ends.
fn bind(arguments: &[ArgumentSite], params: &[Parameter], out: &mut Vec<ArgumentBinding>) {
    for (position, argument) in arguments.iter().enumerate() {
        // A name is already written; a spread fills however many parameters the array has. Neither
        // has one parameter to point at, and everything after them has lost its position.
        if argument.name.is_some() || argument.spread {
            return;
        }
        let Some(param) = params.get(position) else {
            return;
        };
        // A variadic takes all the rest under one name, and repeating it on each is noise.
        if param.is_variadic() {
            return;
        }

        out.push(ArgumentBinding {
            range: argument.range,
            parameter: param.name().clone(),
        });
    }
}
