//! What a local is bound to at a given offset, and which construct did the binding.
//!
//! A narrowing and an assignment can both apply to the same name, and whichever was written later
//! in the text wins over the other -- so this is the layer that ranks them, rather than reporting
//! either alone.

use phrost_hir::{
    ExprId, FileHir,
    body::{BindKind, ExprData, StmtData},
};
use phrost_model::{BuiltinTy, Name, TextSize};

use crate::{
    binding::{Bound, argument_bindings, bound_by, destructured_names, destructured_slot},
    facts::BodyFacts,
    narrowing::Subject,
    path::{Otherwise, PathRoot, PathStep, ValuePath, path_of},
    reach::Reach,
    scope::{Scope, ScopeKind},
    walk::{visit_catches, visit_foreach_bindings, visit_foreach_targets, visit_statements},
};

/// The interned name a local is written under.
///
/// A caller holding a string has no way to reach it otherwise: a skeleton interns declarations, and
/// a variable's name is interned by whatever lowered the file it is written in.
///
/// Whether or not anything binds it, which is the point. A guard can be the only claim about a name
/// -- `if (!$x instanceof T) { break; }` says what `$x` is for the rest of the loop even where
/// nothing ever assigned it -- so a search restricted to bound names would lose exactly the case
/// worth having.
///
/// Case-sensitively, as PHP compares variable names and unlike everything else it compares.
pub fn local_name(hir: &FileHir, offset: TextSize, written: &str) -> Option<Name> {
    if let Some(bound) = locals_in_scope(hir, offset)
        .into_iter()
        .find(|name| name.as_str() == written)
    {
        return Some(bound);
    }

    let mut found = None;
    Scope::visit_every_expression(hir, &mut |hir, expr| {
        if found.is_some() {
            return;
        }
        if let phrost_hir::body::ExprData::Variable { name, .. } = hir.expr(expr)
            && name.as_str() == written
        {
            found = Some(name.clone());
        }
    });
    found
}

/// The interned `Name` a literal array key was written under, anywhere in this file.
///
/// The element-key counterpart of [`local_name`], and it exists for the same reason: a reader that
/// has *text* and needs the `Name` a path is spelled with has nowhere to intern one, so it finds
/// the one lowering already made.
///
/// # Why the project's interner is the wrong place to look
///
/// An array key is **data**, not a declaration. `$constants['C']` says nothing about the project
/// having declared a `C`, so looking the key up among the project's names asks a question with no
/// bearing on the answer -- and gets `None` for every key no class or member happens to share a
/// spelling with. A key lives only inside a *body*, and a closed file's body is never lowered, so
/// the project's interner will not hold one however large the project is.
///
/// Measured before this existed: `$constants['C']->` lost its whole path in a running server --
/// and with it both the guard branch and the `@assert` branch that depend on having one -- while
/// `$constants['image']->` kept it, because `image` happened to be a method name somebody
/// declared. The in-process test harness held every fixture's bodies in the project's own
/// interner, so it answered correctly and hid the bug.
///
/// The first match by text, and duplicates cost nothing: every occurrence of one key in one file
/// interns to the same `Name`, so which one is found cannot change the answer.
pub fn element_key_name(hir: &FileHir, written: &str) -> Option<Name> {
    let mut found = None;
    Scope::visit_every_expression(hir, &mut |hir, expr| {
        if found.is_some() {
            return;
        }
        if let ExprData::Index { key: Some(key), .. } = hir.expr(expr)
            && key.as_str() == written
        {
            found = Some(key.clone());
        }
    });
    found
}

/// What binds a local, and what it holds.
///
/// The same search as [`local_value`], keeping which kind of binding won. Worth reporting because it
/// answers a different question for a reader than the type does: whether a name is an input, a local
/// that was assigned, or one a guard refined.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct LocalBinding {
    pub value: ValuePath,
    pub kind: BindingKind,
    /// Where the value was written, when an expression wrote it.
    ///
    /// The names inside a value's path mean what they meant *there*, not what they mean at the
    /// position asking. `$one = $one->find()` is the case that forces it: reading the receiver at
    /// the asking position finds this same assignment and goes round again, while reading it where
    /// it is written finds the assignment above it.
    ///
    /// `None` for a binding that is a declaration rather than an expression -- a parameter or a
    /// `catch` -- whose value is a written type and holds no name to resolve. A `global` is the one
    /// declaration that carries a position, because what it imports was written somewhere else: at
    /// the file's own top level, where the names in its value mean what they mean there.
    pub written_at: Option<TextSize>,
}

/// One write to a name or a slot from a branch the position is not in.
#[derive(Clone)]
pub(crate) struct MaybeWrite<'f> {
    pub(crate) at: TextSize,
    pub(crate) expr: ExprId,
    /// The write as the body's facts hold it, which is what knows the branches it is in -- see
    /// [`crate::facts::BodyFacts::overwrites`].
    pub(crate) site: &'f crate::facts::WriteSite,
    /// Where the outermost branch it is in begins. See [`Scope::maybe_written`].
    pub(crate) since: TextSize,
    /// The slot of the assigned value this write took, for a destructuring target: `[$a, $b] = $row`
    /// gives `$b` the keys `["1"]`. Empty for the ordinary write, which takes the whole value.
    pub(crate) slot: Vec<phrost_model::Name>,
    /// Whether this write is *below* the read and reaches it round a loop.
    ///
    /// It ran on the pass before, so anything that rebinds the name between the loop's start and
    /// the read has run since -- and then the carried value is gone rather than joined. See where
    /// this is filtered in [`Scope::assignment_before`].
    pub(crate) carried: bool,
    /// Whether the write is inside a loop the read is outside of.
    ///
    /// Then the path that skipped it includes one where the loop body never ran, so no guard
    /// written inside describes it -- see [`crate::facts::BodyFacts::skipped_a_loop`].
    pub(crate) repeated: bool,
    /// Where the branch it is in ends, for the guard that may stand between the two.
    ///
    /// See [`crate::facts::BodyFacts::branch_end`]: the operand this write contributes is what the
    /// name holds where control leaves the branch.
    pub(crate) ends: Option<TextSize>,
    /// Whether a `break` or `continue` beside the write skips the read too -- see
    /// [`crate::facts::BodyFacts::same_pass`].
    pub(crate) same_pass: bool,
}

/// Which construct introduced the value a name holds here.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BindingKind {
    Parameter,
    Assignment,
    /// `foreach ($items as $one)`.
    Foreach,
    /// `catch (Throwable $e)`.
    Catch,
    /// An `instanceof` guard or an `assert` refined a name something else introduced.
    Narrowing,
    /// `/** @var T $x */` written on a line of its own.
    Annotated,
    /// `static $x;` or `global $x;` -- the only way a local is introduced without being written to.
    Declared,
    /// Passed to a call that wrote it back: `preg_match($p, $s, $matches)`.
    ///
    /// The one binding that is not written anywhere in this body as a write. Which argument is by
    /// reference is the callee's business and no layer below the index can say -- so this is
    /// reached only when nothing else bound the name at all, where the alternatives are that the
    /// parameter is by reference or that the code reads a variable nothing set, and the second is
    /// what `unbound-variable` is for.
    Argument,
}

impl BindingKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Parameter => "parameter",
            Self::Assignment => "assigned",
            Self::Foreach => "loop variable",
            Self::Catch => "caught",
            Self::Narrowing => "narrowed by a guard",
            Self::Annotated => "annotated",
            Self::Declared => "declared",
            Self::Argument => "written back through a call",
        }
    }
}

/// Where the local named `name` at `offset` got its value, and what introduced it.
///
/// Searched innermost scope outward, and within a scope most recent binding first: an assignment
/// before the cursor beats the parameter it overwrote, and a closure's own parameter beats an outer
/// one of the same name.
///
/// Position decides which of several writes is the latest; whether a write is in effect at all is a
/// question about *branches*, and [`crate::reach`] answers it. A write control had to go through
/// replaces what came before it. One written in a branch the position is not in only *may* have
/// run, so it joins what the name already held instead of replacing it -- `if ($c) { $x = new A(); }
/// $x->` still names A, because nothing else ever wrote `$x`, while `if ($c) { $v = trim($v); } else
/// { $v-> }` leaves the parameter's own type where the branch does not reach.
pub fn local_binding(
    hir: &FileHir,
    offset: TextSize,
    name: &str,
    never: crate::walk::NeverReturns<'_>,
    asserts: crate::walk::AssertsArguments<'_>,
) -> Option<LocalBinding> {
    let chain = Scope::chain_at(hir, offset);

    for (index, scope) in chain.iter().enumerate().rev() {
        // The three constructs that can bind a name more than once in one scope, ranked by where
        // each is written. Position and not precedence: a narrowing counts as a binding, so
        // `if ($x instanceof Repo) { $x = new Other(); $x-> }` is an `Other` -- and by the same rule
        // `$offer = $this->selected(); ... foreach ($offers as $offer)` is the loop's element inside
        // the loop, where ranking the loop below any assignment used to give the whole body the
        // wrong type.
        //
        // Ties go to the later entry in this list, which is why the narrowing is last: the one way
        // two of these land on the same position is `if ($file = $this->load())`, where the guard is
        // about the value the assignment produced and is the more specific claim.
        let ranked = [
            scope
                .foreach_binding(hir, offset, name)
                .map(|(at, value)| (at, value, BindingKind::Foreach)),
            scope
                .assignment_before(hir, offset, name, never, asserts)
                .map(|(at, value)| (at, value, BindingKind::Assignment)),
            // A loop over the name by reference writes it without an assignment, and is ranked
            // where the loop ends: an assignment after it replaces what it wrote, and one before it
            // is what it read.
            scope
                .rewritten_by_reference(hir, offset, name, never)
                .map(|(at, value)| (at, value, BindingKind::Assignment)),
            scope
                .narrowing_at(hir, offset, Subject::Local(name), never, asserts)
                .map(|(at, value)| (at, value, BindingKind::Narrowing)),
            // Last in the list, so that a `/** @var T $x */` written directly above an assignment
            // wins the tie: the annotation is the author saying the assignment's own type is not
            // what they mean. Both give the same answer when the assignment consumed the same
            // nameless tag, so the tie only matters for a named one.
            scope
                .annotation_before(hir, offset, name)
                .map(|(at, value)| (at, value, BindingKind::Annotated)),
        ];
        let mut best: Option<(TextSize, ValuePath, BindingKind)> = None;
        for candidate in ranked.into_iter().flatten() {
            if best.as_ref().is_none_or(|(at, ..)| *at <= candidate.0) {
                best = Some(candidate);
            }
        }
        if let Some((at, value, kind)) = best {
            return Some(LocalBinding {
                value,
                kind,
                written_at: Some(at),
            });
        }

        if let Some(bound) = scope.catch_binding(hir, offset, name) {
            return bound
                .map(PathRoot::Written)
                .map(ValuePath::of)
                .map(|value| LocalBinding {
                    value,
                    kind: BindingKind::Catch,
                    written_at: None,
                });
        }
        if let Some((declared, is_variadic)) = scope.parameter(hir, name) {
            // A callback's parameter, where its own declaration says nothing useful about what it
            // receives and the call it was passed to does. Any position, not only the first: the
            // callee's declared signature speaks about every one of them, and the container
            // heuristic that only speaks about the first says so itself by carrying no siblings
            // past it.
            if !is_variadic
                && declared.is_none_or(|id| is_a_bare_container(hir, id))
                && matches!(
                    scope.kind(),
                    ScopeKind::Closure { .. } | ScopeKind::ArrowFunction
                )
                && let Some(position) = scope.parameter_position(hir, name)
            {
                // Called where it is written: the parameter holds that call's argument, read as of
                // the call, where the argument's own names are bound.
                if declared.is_none()
                    && let Some((value, at)) = invoked_argument(hir, scope, position)
                {
                    return Some(LocalBinding {
                        value,
                        kind: BindingKind::Parameter,
                        written_at: Some(at),
                    });
                }
                if let Some(value) = callback_parameter(hir, scope, position, declared) {
                    return Some(LocalBinding {
                        value,
                        kind: BindingKind::Parameter,
                        written_at: None,
                    });
                }
            }
            // A method's parameter with no type of its own: what it holds is whatever the
            // declaration it overrides says, and that is a question for a layer with a hierarchy.
            // Only a declaration's own scope, because a closure overrides nothing.
            if declared.is_none()
                && !is_variadic
                && matches!(scope.kind(), ScopeKind::Declaration)
                && let Some(position) = scope.parameter_position(hir, name)
                && let Ok(position) = u16::try_from(position)
            {
                return Some(LocalBinding {
                    value: ValuePath::of(PathRoot::InheritedParameter { position }),
                    kind: BindingKind::Parameter,
                    written_at: None,
                });
            }
            return declared
                .map(match is_variadic {
                    true => PathRoot::VariadicValues,
                    false => PathRoot::Written,
                })
                .map(ValuePath::of)
                .map(|value| LocalBinding {
                    value,
                    kind: BindingKind::Parameter,
                    written_at: None,
                });
        }
        // `global $config;` imports the file's own top-level variable, and the top level is the
        // only thing in reach that says what it holds. Last, because a write inside the scope is a
        // write to that same global and the searches above already found it.
        if let Some(bound) = scope.imported_global(hir, offset, name, never, asserts) {
            return Some(bound);
        }
        // Nothing here binds it, so whether it can be the enclosing scope's is the capture rule.
        if index == 0 || !scope.reaches_outward(hir, name) {
            break;
        }
    }

    // A call that wrote through a by-reference parameter. After every scope has been tried, because
    // `foo($bar)` is how PHP writes an out-parameter and the variable exists *because* of the call --
    // there is no assignment, no parameter and no loop for the search above to find. A call that
    // turns out to take its argument by value says nothing, so ranking this with the assignments
    // would let it hide one.
    //
    // Once, and not once per scope: the search above visits a scope per level, and a captured name
    // is unbound in every one of them. Walking there cost 1.8 ms of the diagnostics budget for an
    // answer that cannot differ between levels.
    crate::narrowing::passed_by_reference(hir, offset, name).map(|value| LocalBinding {
        value,
        kind: BindingKind::Assignment,
        written_at: None,
    })
}

/// What a callback's parameter receives, read off the call the callback was passed to.
///
/// Both sources the reader ranks, gathered here because both are facts about the same call: which
/// argument the callback was written as, and what the other arguments hold. See
/// [`PathRoot::CallbackParameter`] for why they travel together rather than as two roots.
fn callback_parameter(
    hir: &FileHir,
    scope: &Scope,
    parameter: usize,
    declared: Option<phrost_hir::TypeRefId>,
) -> Option<ValuePath> {
    let inside = hir.body(scope.body()).range;
    let call = crate::calls::enclosing_call(hir, inside)?;
    // The argument this scope is written inside, which is the callback itself.
    let (argument, written_as) = call
        .arguments
        .iter()
        .enumerate()
        .find(|(_, argument)| hir.expr(argument.value).range().contains_range(inside))?;
    // Every other argument, whichever parameter is being asked about: with several containers the
    // written order is what pairs them with the callback's parameters, and only the reader can tell
    // which of them is a container.
    let siblings = call
        .arguments
        .iter()
        .filter(|other| !hir.expr(other.value).range().contains_range(inside))
        .filter_map(|other| crate::path::value_path_of(hir, other.value))
        .collect::<Vec<_>>();

    Some(ValuePath::of(PathRoot::CallbackParameter {
        parameter,
        callee: Box::new(callee_of_target(call.target)?),
        argument,
        argument_name: written_as.name.clone(),
        siblings,
        at: call.range.start(),
        declared,
    }))
}

/// What a closure called where it is written -- `(function ($rows) { ... })($this->rows())` -- is
/// handed as a parameter: the call's argument at that position, and where the call starts.
///
/// Positional arguments only. A named or spread argument before the position makes which argument
/// lands there a question of the closure's parameter names or of an array's length, and answering
/// it by position would hand the parameter somebody else's value.
fn invoked_argument(
    hir: &FileHir,
    scope: &Scope,
    parameter: usize,
) -> Option<(ValuePath, phrost_model::TextSize)> {
    let inside = hir.body(scope.body()).range;
    let call = crate::calls::enclosing_call(hir, inside)?;
    if !matches!(call.target, crate::calls::CallTarget::Invoked(_))
        || !call.callee_range.contains_range(inside)
    {
        return None;
    }
    let passed = call.arguments.get(..=parameter)?;
    if passed
        .iter()
        .any(|argument| argument.name.is_some() || argument.spread)
    {
        return None;
    }
    let value = crate::path::value_path_of(hir, passed[parameter].value)?;
    Some((value, call.range.start()))
}

/// The same callee, described the way a declaration lookup wants it.
///
/// Two vocabularies for one thing, and this is the seam between them: [`crate::calls::CallTarget`]
/// is how a call reads to anything walking the file, and [`crate::narrowing::Callee`] is what the
/// three roots that need a callee's *declaration* carry. Kept a conversion rather than a shared
/// type because the two are asked at different moments -- a call site exists whether or not
/// anybody will look the callee up.
/// `None` for a call through a value: `Callee` is what a *declaration lookup* carries, and there is
/// no name to look one up by. A closure's parameter list is reachable, but only from the view and
/// only for one written in the same file -- which is a different question from the one these roots
/// ask.
pub(crate) fn callee_of_target(
    target: crate::calls::CallTarget,
) -> Option<crate::narrowing::Callee> {
    Some(match target {
        crate::calls::CallTarget::Member { receiver, name } => {
            crate::narrowing::Callee::Member { receiver, name }
        }
        crate::calls::CallTarget::Function(reference) => {
            crate::narrowing::Callee::Function(reference)
        }
        crate::calls::CallTarget::Constructor(class) => {
            crate::narrowing::Callee::Constructed(class)
        }
        crate::calls::CallTarget::Invoked(_) => return None,
    })
}

/// `array` or `iterable` with nothing said about what is in it.
///
/// A callback parameter written that way says only that it takes a container, which the call site
/// can say more precisely: `array_map(fn (array $row) => ..., $rows)` where `$rows` is a
/// `list<array{id: int}>` receives the shape.
fn is_a_bare_container(hir: &FileHir, id: phrost_hir::TypeRefId) -> bool {
    match hir.type_ref(id) {
        phrost_hir::TypeRefData::Builtin(
            phrost_model::BuiltinTy::Array | phrost_model::BuiltinTy::Iterable,
        ) => true,
        phrost_hir::TypeRefData::Nullable(inner) => is_a_bare_container(hir, *inner),
        _ => false,
    }
}

/// Every local bound at `offset`, nearest scope first.
///
/// What completion after a `$` offers. Names only: a type is a separate question and most of these
/// do not have one — an untyped parameter is still a variable someone can finish typing.
///
/// Order is scope by scope from the inside out, and within a scope in the order names appear, so a
/// closure's own parameters come before what it captured. Duplicates are dropped, keeping the
/// innermost: an inner binding is the one that name means here.
pub fn locals_in_scope(hir: &FileHir, offset: TextSize) -> Vec<Name> {
    let mut names = Vec::new();

    for (index, scope) in Scope::chain_at(hir, offset).iter().enumerate().rev() {
        for &id in hir.param_list(scope.params()) {
            push_unique(&mut names, hir.param(id).name.clone());
        }
        scope.bound_names(hir, offset, &mut names);

        // Past a closure that captures nothing, an outer name is not in scope at all.
        if index > 0 && !scope.reaches_anything() {
            break;
        }
    }

    names
}

fn push_unique(names: &mut Vec<Name>, name: Name) {
    if !names.contains(&name) {
        names.push(name);
    }
}

impl Scope {
    /// What a `global $name;` in this scope imports, read off the file's own top level.
    ///
    /// `global $config;` binds the name to the *global* variable of that name, and in one file that
    /// is the one the script's top level wrote -- `$config = new Config();` in a bootstrap, read by
    /// every function below it. Without this the import bound a name that held nothing, so
    /// `$config->` inside the function offered nothing at all.
    ///
    /// The value is taken from the end of the file scope rather than from before the `global`: the
    /// function runs when it is called, which is after the whole script has been read, so a
    /// top-level assignment written below the function still reaches it.
    ///
    /// `None` for the file scope itself, where a `global` says nothing -- the name is already this
    /// scope's, and looking it up here would be the same search going round again.
    fn imported_global(
        &self,
        hir: &FileHir,
        offset: TextSize,
        name: &str,
        never: crate::walk::NeverReturns<'_>,
        asserts: crate::walk::AssertsArguments<'_>,
    ) -> Option<LocalBinding> {
        if self.is_file_scope(hir) || !self.declares_global(hir, offset, name) {
            return None;
        }

        let file = Self::file_scope(hir)?;
        let end = hir.body(file.body()).range.end();
        // Ranked the way the in-scope search ranks them: whichever was written later wins, and an
        // annotation wins a tie because it is written precisely when the assignment's own type is
        // not what the author means.
        let assigned = file.assignment_before(hir, end, name, never, asserts);
        let annotated = file.annotation_before(hir, end, name);
        let (at, value) = match (assigned, annotated) {
            (Some(assigned), Some(annotated)) => match annotated.0 >= assigned.0 {
                true => annotated,
                false => assigned,
            },
            (found, None) | (None, found) => found?,
        };

        Some(LocalBinding {
            value,
            kind: BindingKind::Declared,
            // Where the top level wrote it, not where the import reads it: the names inside the
            // value's path mean what they meant at file scope.
            written_at: Some(at),
        })
    }

    /// Whether a `global $name;` in this scope's own statements introduces the name before `offset`.
    fn declares_global(&self, hir: &FileHir, offset: TextSize, name: &str) -> bool {
        hir.stmt_list(hir.body(self.body()).statements)
            .iter()
            .any(|&id| {
                let StmtData::Bind {
                    kind: BindKind::Global,
                    bindings,
                    range,
                } = hir.stmt(id)
                else {
                    return false;
                };
                range.end() <= offset
                    && hir
                        .expr_list(*bindings)
                        .iter()
                        .any(|&id| bound_by(hir, id).is_some_and(|b| b.name().as_str() == name))
            })
    }

    /// `Some(type)` when a parameter of this scope declares the name; the inner option is absent
    /// for an unannotated one, which still binds the name and so still stops the search.
    fn parameter(
        &self,
        hir: &FileHir,
        name: &str,
    ) -> Option<(Option<phrost_hir::TypeRefId>, bool)> {
        hir.param_list(self.params())
            .iter()
            .map(|&id| hir.param(id))
            .find(|param| param.name.as_str() == name)
            // Native first, then the docblock: on code that predates PHP's own types the docblock is
            // the only thing that says anything at all -- except over a bare `array`, where the two
            // are halves of one claim rather than competing ones.
            .map(|param| {
                (
                    phrost_hir::refined_type_ref(hir, param.type_ref, param.doc_type_ref),
                    param.is_variadic,
                )
            })
    }

    /// The type a `catch` clause gives its variable, when the offset is inside that clause's body.
    ///
    /// Scoped to the clause, unlike a parameter: `catch (Foo $e) {}` binds `$e` for the block, and
    /// treating it as a scope-wide binding would answer about it from code that runs before the try.
    fn catch_binding(
        &self,
        hir: &FileHir,
        offset: TextSize,
        name: &str,
    ) -> Option<Option<phrost_hir::TypeRefId>> {
        let mut found = None;
        visit_catches(hir, hir.body(self.body()).statements, &mut |hir, catch| {
            let data = hir.catch(catch);
            if !data.range.contains_inclusive(offset) {
                return;
            }
            if data
                .variable
                .as_ref()
                .is_some_and(|bound| bound.as_str() == name)
            {
                let _ = hir;
                found = Some(data.type_ref);
            }
        });
        found
    }

    /// The `foreach` that binds `name`, as the element of whatever it iterates.
    ///
    /// The most common way a local gets a type in real code, and the one that needs type arguments:
    /// `foreach ($this->all() as $one)` is the path to `all()` plus one element step. The key target
    /// binds too, and its type is not the element's -- so it binds the name without claiming to know
    /// what it holds.
    ///
    /// The **latest** loop at or before the offset, not the first one found. One function commonly
    /// binds the same name in several loops -- `$handler`, `$item`, `$row` -- and taking the first
    /// gave the whole method the first subject's element type: two loops over
    /// `LeaveHandler[]` and `EnterHandler[]` both read as `LeaveHandler`, so every call in the second
    /// loop's body was an `undefined-method`. Renaming one of the two variables made it go away,
    /// which is what made it look like a scoping rule rather than a bug.
    ///
    /// Position and nothing else, which is the same rule [`local_binding`] applies across the four
    /// kinds of binding, and it gets a nested loop right for free: an inner `foreach` is written
    /// after the outer one.
    fn foreach_binding(
        &self,
        hir: &FileHir,
        offset: TextSize,
        name: &str,
    ) -> Option<(TextSize, ValuePath)> {
        // The path stays optional all the way through: a loop whose subject nothing can read still
        // *binds* the name, and falling back to an earlier loop there would answer with a type the
        // name does not have -- which is worse than answering nothing.
        if !crate::facts::BodyFacts::of(hir).foreach_may_bind(self.body(), name) {
            return None;
        }
        let mut best: Option<(TextSize, Option<ValuePath>)> = None;

        visit_foreach_bindings(
            hir,
            hir.body(self.body()).statements,
            offset,
            &mut |hir, binding| {
                let (subject, key, value, annotated) = (
                    binding.subject,
                    binding.key,
                    binding.value,
                    binding.annotated,
                );
                if let Some(key) = key
                    && bound_by(hir, key).is_some_and(|bound| bound.name().as_str() == name)
                {
                    let at = hir.expr(key).range().start();
                    if best.as_ref().is_none_or(|(previous, _)| *previous <= at) {
                        best = Some((
                            at,
                            path_of(hir, subject).map(|mut path| {
                                path.steps.push(PathStep::IterationKey);
                                path
                            }),
                        ));
                    }
                    return;
                }
                // A destructuring target -- `foreach ($pairs as [$name, $count])` -- reads the
                // slot off the *element*, which is right exactly when the element states its slots:
                // `@return list<array{0: Row, 1: string}>` says what each pass holds, and the shape
                // travels.
                //
                // Not for a subject written as a literal of rows, which is where this was refused
                // before and the refusal was right: such a literal has one element type for all its
                // rows, so a slot of it is the union of every row's -- wider than the truth, and a
                // union carrying an alternative a parameter refuses reports code that passes the one
                // that fits. Measured then at one finding each way and two wrong calls on a test
                // table, which is what the narrower rule keeps out.
                if !matches!(hir.expr(subject), ExprData::ArrayLiteral { .. })
                    && let Some(slot) = destructured_slot(hir, value, name)
                {
                    let at = hir.expr(value).range().start();
                    if best.as_ref().is_some_and(|(previous, _)| *previous > at) {
                        return;
                    }
                    let held = match annotated {
                        Some(annotated) => Some(ValuePath::of(PathRoot::Written(annotated))),
                        None => path_of(hir, subject).map(|mut path| {
                            // Iteration names no slot, and then the destructuring names one per key.
                            path.steps.push(PathStep::Element(None));
                            path.steps.extend(slot.into_iter().map(|key| {
                                PathStep::Element(Some(crate::path::ElementKey::Literal(key)))
                            }));
                            path
                        }),
                    };
                    best = Some((at, held));
                    return;
                }
                if bound_by(hir, value).is_some_and(|bound| bound.name().as_str() == name) {
                    let at = hir.expr(value).range().start();
                    if best.as_ref().is_some_and(|(previous, _)| *previous > at) {
                        return;
                    }
                    // A `@var` above the loop wins over what the subject says its elements are, for
                    // the same reason it wins above an assignment: it is written *because* the
                    // inferred type is too general, and there is nothing else it could mean.
                    let held = match annotated {
                        Some(annotated) => Some(ValuePath::of(PathRoot::Written(annotated))),
                        None => path_of(hir, subject).map(|mut path| {
                            // Iteration names no slot: every pass is a different one.
                            path.steps.push(PathStep::Element(None));
                            path
                        }),
                    };
                    best = Some((at, held));
                }
            },
        );

        best.and_then(|(at, value)| value.map(|value| (at, value)))
    }

    /// Where a `foreach` in this scope binds `name`, whether or not its subject can be read.
    ///
    /// [`Self::foreach_binding`] answers `None` when nothing says what the loop iterates, because
    /// there is no *value* to hand back -- and the question here is a different one: the loop still
    /// rebinds the name on every pass, which is what kills a write carried round from the pass
    /// before. Reading the binding's position off the other function would have made that kill
    /// depend on the subject being typed, and a closure parameter with no type is exactly where it
    /// is not.
    fn foreach_binds(&self, hir: &FileHir, offset: TextSize, name: &str) -> Option<TextSize> {
        if !crate::facts::BodyFacts::of(hir).foreach_may_bind(self.body(), name) {
            return None;
        }
        let mut best: Option<TextSize> = None;
        visit_foreach_bindings(
            hir,
            hir.body(self.body()).statements,
            offset,
            &mut |hir, binding| {
                for bound in [binding.key, Some(binding.value)].into_iter().flatten() {
                    if bound_by(hir, bound).is_some_and(|bound| bound.name().as_str() == name) {
                        let at = hir.expr(bound).range().start();
                        if best.is_none_or(|previous| previous <= at) {
                            best = Some(at);
                        }
                    }
                }
            },
        );
        best
    }

    /// The last assignment to `name` written before `offset` in this scope.
    ///
    /// Textual order, not execution order -- see [`local_value`](crate::local_value). Assignments
    /// inside nested closures belong to those scopes and are not considered here.
    /// The last `/** @var T $name */` written at or before `offset` in this scope.
    ///
    /// A statement walk like the others, so it is found wherever it is written -- inside an `if`, a
    /// loop, a `try` -- and PHP has no block scope for a variable, so position is the whole rule.
    fn annotation_before(
        &self,
        hir: &FileHir,
        offset: TextSize,
        name: &str,
    ) -> Option<(TextSize, ValuePath)> {
        let mut best: Option<(TextSize, ValuePath)> = None;

        // From the body's own list rather than a walk of its statements, in the walk's order.
        for (annotated, start, ty) in crate::facts::BodyFacts::of(hir).annotations(self.body()) {
            if *start > offset || annotated.as_str() != name {
                continue;
            }
            if best.as_ref().is_none_or(|(at, _)| *at <= *start) {
                best = Some((*start, ValuePath::of(PathRoot::Written(*ty))));
            }
        }

        // An annotation written above an assignment to the same name is about what the assignment
        // *produces*. Inside that assignment's own right-hand side the name still holds what it held
        // before: `/** @var UserInterface $user */ $user = $storage->load($user);` passes the
        // parameter's `?int`, and reading it as the annotated class silenced the null PHPStan
        // reports there.
        let (at, _) = best.as_ref()?;
        if Self::inside_an_assignment_to(hir, offset, name, *at) {
            return None;
        }
        best
    }

    /// Whether `offset` is inside the value of an assignment to `name` that starts after `after`.
    fn inside_an_assignment_to(
        hir: &FileHir,
        offset: TextSize,
        name: &str,
        after: TextSize,
    ) -> bool {
        // Only the statement the offset is in, per scope around it. An assignment whose value holds
        // the offset is an expression of the innermost statement holding the offset -- nested
        // statements are disjoint from their parent's own expressions -- so that statement is the
        // only one worth reading. Walking every expression in the file instead ran once per read of
        // an annotated local, and a long procedural file has thousands of them.
        let mut inside = false;
        for scope in crate::scope::Scope::chain_at(hir, offset) {
            let Some(statement) = BodyFacts::of(hir).innermost_statement(hir, scope.body(), offset)
            else {
                continue;
            };
            crate::walk::visit_one_statement(hir, statement, &mut |hir, expr| {
                if inside {
                    return;
                }
                let ExprData::Assign { target, value, .. } = hir.expr(expr) else {
                    return;
                };
                if hir.expr(expr).range().start() < after {
                    return;
                }
                if matches!(hir.expr(*target), ExprData::Variable { name: written, .. } if written.as_str() == name)
                    && hir.expr(*value).range().contains_inclusive(offset)
                {
                    inside = true;
                }
            });
            if inside {
                break;
            }
        }
        inside
    }

    fn assignment_before(
        &self,
        hir: &FileHir,
        offset: TextSize,
        name: &str,
        never: crate::walk::NeverReturns<'_>,
        asserts: crate::walk::AssertsArguments<'_>,
    ) -> Option<(TextSize, ValuePath)> {
        let mut certain: Option<(TextSize, ExprId, Vec<phrost_model::Name>)> = None;
        let mut maybe: Vec<MaybeWrite<'_>> = Vec::new();
        let mut written = None;
        let mut created: Option<TextSize> = None;

        // The body's writes through this name, from the facts derived once for the file, each with
        // how surely it ran before the position -- rather than a walk of the whole body per lookup.
        let facts = BodyFacts::of(hir);

        // A name a closure took by reference is written where this cannot see: the write is in the
        // closure's own body and runs whenever whoever holds it decides. `$found = NULL;` beside a
        // `use (&$found)` therefore says nothing about what `$found` holds afterwards, and reading
        // the visible assignment alone answered *definitely null* -- 69 reports on one library, on
        // the commonest way a test captures what a callback was handed.
        if facts.captured_by_reference_in(self.body(), name) {
            return None;
        }
        for site in facts.writes_that_may_root_at(self.body(), Some(name)) {
            if !site.rooted_at(name) {
                continue;
            }
            // A write the position is inside, *or at the very end of*, has not happened yet: the
            // second is `$foobar = $foobar->meth` with the cursor after `meth` and nothing typed
            // after it, where reading the write as done resolves the name through its own
            // unfinished value and answers nothing.
            //
            // Below it, a loop is the one thing that brings control back round -- see
            // `BodyFacts::carried_from`, which is where that direction is decided.
            let reach = match site.range.end() >= offset {
                true => match facts.carried_from(hir, site, offset, never) {
                    // A write whose value depends on anything the loop also writes is a fixed
                    // point rather than a value arriving from the pass before. `$input =
                    // substr($input, 3)` depends on itself; `$dir = $renamed . SEP` beside
                    // `$renamed = substr_replace($dir, ...)` depends on itself one variable away.
                    // Joining either resolves the name through the loop again until the budget
                    // stops, which widens a step at a time and ends in the union of everything the
                    // name ever holds -- 95 findings on WordPress and Symfony, every one of them a
                    // line that runs.
                    //
                    // What a carried write *is* for is a value the loop settles independently:
                    // `$lowest = $grade` takes the loop's own target, which the header binds and no
                    // write in the body touches.
                    Some(_) if Self::depends_on_the_loop(hir, facts, self.body(), site.expr) => {
                        continue;
                    }
                    Some(from) => Reach::Possible(from),
                    None => continue,
                },
                false => match facts.reach_of(hir, site, offset, never) {
                    Some(reach) => reach,
                    None => continue,
                },
            };
            let (expr, range) = (site.expr, site.range);
            let (ExprData::Assign { target, .. } | ExprData::CompoundAssign { target, .. }) =
                hir.expr(expr)
            else {
                continue;
            };
            // `[$a, $b] = $row` writes through no variable of its own, so `bound_by` answers
            // nothing for it and the slot each name took is what settles the value.
            let Some(bound) = bound_by(hir, *target) else {
                let Some(slot) = destructured_slot(hir, *target, name) else {
                    continue;
                };
                written.get_or_insert_with(|| bound_name(hir, *target, name));
                match reach {
                    Reach::Certain => {
                        if certain
                            .as_ref()
                            .is_none_or(|(start, _, _)| *start < range.start())
                        {
                            certain = Some((range.start(), expr, slot));
                        }
                    }
                    Reach::Possible(since) => maybe.push(MaybeWrite {
                        at: range.start(),
                        expr,
                        site,
                        since,
                        slot,
                        carried: range.start() > offset,
                        repeated: facts.skipped_a_loop(site, offset),
                        ends: facts.branch_end(site, offset),
                        same_pass: facts.same_pass(site, offset),
                    }),
                }
                continue;
            };
            if bound.name().as_str() != name {
                continue;
            }
            written.get_or_insert_with(|| bound.name().clone());
            match bound {
                // The assignment itself, not its right-hand side: a `/** @var T $x */` above it
                // is recorded on the assignment, and reading only the value would step past it.
                Bound::Whole(_) => match reach {
                    Reach::Certain => {
                        if certain
                            .as_ref()
                            .is_none_or(|(start, _, _)| *start < range.start())
                        {
                            certain = Some((range.start(), expr, Vec::new()));
                        }
                    }
                    Reach::Possible(since) => maybe.push(MaybeWrite {
                        at: range.start(),
                        expr,
                        site,
                        since,
                        slot: Vec::new(),
                        carried: range.start() > offset,
                        repeated: facts.skipped_a_loop(site, offset),
                        ends: facts.branch_end(site, offset),
                        same_pass: facts.same_pass(site, offset),
                    }),
                },
                // Earliest, not latest: an element write is the weaker claim, and the one that
                // counts is the write that brought the variable into being -- which a write carried
                // round a loop never is. `foreach ($places as $value) { ... $value['name'] ??= $key;
                // }` says the name holds an array *after* that line, and reading it as what created
                // the variable answered `array` above it, over the `instanceof` guard that had just
                // said otherwise.
                // A write carried round the loop is not first by definition, so it is left out
                // rather than competing for the earliest.
                Bound::Element(_) if range.start() <= offset => {
                    if created.is_none_or(|start| range.start() < start) {
                        created = Some(range.start());
                    }
                }
                Bound::Element(_) => {}
            }
        }

        // A write carried round a loop ran on the pass *before*, so anything that rebinds the name
        // between the loop's start and the read has run since -- and then what it wrote is gone
        // rather than joined. Two things do that and both are ordinary: a certain write inside the
        // loop (`$value = $export[$name];` at the top of a `foreach`, read in a `switch` below it,
        // written again in another arm) and the loop's own target (`foreach ($a as &$v)` rebinds
        // `$v` every pass). Without this the join made every such name a union of every shape it
        // ever holds, which is wider than the truth and reported on code that runs.
        let rebound = certain
            .as_ref()
            .map(|(at, _, _)| *at)
            .into_iter()
            .chain(self.foreach_binds(hir, offset, name))
            .max();
        if let Some(rebound) = rebound {
            maybe.retain(|write| !write.carried || write.since > rebound);
        }

        // A write control had to go through erases anything conditional above it: whichever branch
        // ran, this one ran after.
        if let Some((at, _, _)) = &certain {
            let at = *at;
            maybe.retain(|write| write.at > at);
        }

        // And a write in a condition whose every arm writes again or leaves: nothing it wrote gets
        // past the construct. `elseif (!$attr = $p->getAttributes(X::class)[0] ?? null) {
        // continue; } else { $attr = $attr->newInstance(); }` -- the reflection object is replaced on
        // the one path out, and joining it beside the instance was a union of two values the name
        // never holds together at the read.
        if maybe.len() > 1 {
            let writes = maybe.iter().map(|write| write.expr).collect::<Vec<_>>();
            let superseded = maybe
                .iter()
                .map(|write| {
                    facts.condition_of(write.at).is_some_and(|construct| {
                        facts.writes_fill_every_arm(
                            hir,
                            self.body(),
                            &writes,
                            construct,
                            write.same_pass,
                        )
                    })
                })
                .collect::<Vec<_>>();
            let mut index = 0;
            maybe.retain(|_| {
                index += 1;
                !superseded[index - 1]
            });
        }

        // A conditional write does not replace what the name held, because it may not have run.
        // Returning `None` when one of them cannot be read as a path is deliberate and matches what
        // an unreadable single assignment already did: half a union is not a union, and answering
        // with the operands that happen to be readable would state a claim the code did not make.
        if !maybe.is_empty() {
            let written = written?;
            return self.maybe_written(
                hir,
                &maybe,
                offset,
                ValuePath::of(PathRoot::Local(written.clone())),
                Subject::Local(written.as_str()),
                asserts,
            );
        }
        if let Some((at, expr, slot)) = certain {
            return Some((at, destructured_value(hir, expr, &slot)?));
        }
        // Nothing assigned the variable itself, so an element write is what created it -- PHP's own
        // rule, and the reason this is a fallback rather than a competitor. All it settles is that
        // the variable holds an array: the value went into a slot, not into the variable.
        //
        // Not for a parameter declared as something that is no array, which bound the name before
        // any write did. `$bag['k'] = 1` on a `Bag $bag` is `offsetSet`, and reading it as the
        // array that created the variable made the next `$bag->count()` a method called on an
        // array. A parameter that may be one keeps the rule -- `?array $options = null` is an array
        // after `$options['x'] = 1` -- and so does an untyped one, since a Drupal hook's `&$form`
        // says it is an array in no other way.
        //
        // Nor for one declared as a sealed shape, which says more than the write does and is kept
        // for the layer that folds writes into a record: `$row['bar'] = 2` on an `array{foo: int}`
        // holds both keys, where the array this answers holds neither.
        if let Some((Some(declared), _)) = self.parameter(hir, name)
            && (!Self::admits_an_array(hir, declared)
                || matches!(
                    hir.type_ref(declared),
                    phrost_hir::TypeRefData::Shape { open: false, .. }
                ))
        {
            return None;
        }
        let at = created?;
        Some((at, ValuePath::of(PathRoot::Builtin(BuiltinTy::Array))))
    }

    /// Whether a written type leaves room for an array: `array`, `iterable`, `mixed`, or a union or
    /// nullable holding one of them. A class name does not, whatever it implements.
    fn admits_an_array(hir: &FileHir, ty: phrost_hir::TypeRefId) -> bool {
        use phrost_hir::TypeRefData;
        match hir.type_ref(ty) {
            TypeRefData::Builtin(builtin) => matches!(
                builtin,
                BuiltinTy::Array | BuiltinTy::Iterable | BuiltinTy::Mixed
            ),
            TypeRefData::Nullable(inner) => Self::admits_an_array(hir, *inner),
            TypeRefData::Union(parts) => hir
                .type_ref_list(*parts)
                .iter()
                .any(|&part| Self::admits_an_array(hir, part)),
            TypeRefData::Generic { base, .. } => Self::admits_an_array(hir, *base),
            TypeRefData::Named(_) | TypeRefData::Intersection(_) | TypeRefData::Relative(_) => {
                false
            }
            _ => true,
        }
    }

    /// Whether what an assignment writes depends on a name the same body also assigns.
    ///
    /// The target is not part of the answer -- what is read is the *value*. `$x = $x->next()`
    /// depends on itself, `$dir = $renamed . SEP` depends on `$renamed` where a line beside it
    /// writes that too, and `$lowest = $grade` depends on the loop's own target, which the header
    /// binds and no assignment touches.
    ///
    /// Assignments only, and in this body only. A name the *header* binds is exactly the stable one
    /// a carried write is for, and a name written in a nested closure belongs to that scope.
    fn depends_on_the_loop(
        hir: &FileHir,
        facts: &BodyFacts,
        body: phrost_hir::BodyId,
        assignment: ExprId,
    ) -> bool {
        // A compound assignment reads the very name it writes, so a pass's write is what the
        // next pass reads: `$total += $row->amount` is a fixed point, never a value arriving whole.
        if matches!(hir.expr(assignment), ExprData::CompoundAssign { .. }) {
            return true;
        }
        let ExprData::Assign { value, .. } = hir.expr(assignment) else {
            return false;
        };
        let mut found = false;
        crate::walk::visit_expression(hir, *value, &mut |hir, expr| {
            if found {
                return;
            }
            let ExprData::Variable { name: read, .. } = hir.expr(expr) else {
                return;
            };
            found = facts
                .writes(body)
                .iter()
                .any(|site| site.rooted_at(read.as_str()));
        });
        found
    }

    /// The writes a branch may have made, beside the name to read for what it held if none did.
    ///
    /// `before` is where the outermost branch any of them is in begins, which is the last position
    /// at which none of this had happened. Outside the construct on purpose, not merely before the
    /// first write: inside it the branch's own guard is in effect, and
    /// `if (!isset($x)) { $x = new Formatter(); }` would answer that `$x` was null -- which is true
    /// only on the path that then wrote it, and is the opposite of what the join is asking. It also
    /// keeps the lookup terminating, since none of these writes can be found from there.
    ///
    /// Reported at the *last* write, because that is where a reader is pointed at what happened to
    /// the name -- the same position the single-assignment answer gives.
    ///
    /// `read` is what to ask for the value before the branches, and `subject` is the same thing as
    /// the guard machinery names it -- a bare local, or a slot reached through a path. One join for
    /// both, so a property filled in a guarded branch gets the local's answer rather than none.
    pub(crate) fn maybe_written(
        &self,
        hir: &FileHir,
        maybe: &[MaybeWrite<'_>],
        offset: TextSize,
        read: ValuePath,
        subject: Subject<'_>,
        asserts: crate::walk::AssertsArguments<'_>,
    ) -> Option<(TextSize, ValuePath)> {
        // Only the latest construct's writes are operands, and everything written before it is left
        // to the lookup: two constructs are a *chain*, not one join, and flattening them threw away
        // whatever was written between the two.
        //
        // ```php
        // $found = NULL;
        // foreach ($items as $item) { if ($item instanceof Doc) { $found = $item; break; } }
        // if (!$found) { throw new RuntimeException('missing'); }
        // if ($this->flag) { $found = $this->make(); }
        // ```
        //
        // Read as one join, the value before *everything* is the `NULL`, and the guard that rules it
        // out sits between the two constructs where nothing looked -- so the null came back and the
        // answer was `Doc|Doc|null`. Read as a chain, the second construct joins what it wrote with
        // whatever the name held just before it, which is the first join with the guard applied.
        //
        // Terminating for the reason the position always did: the lookup happens strictly before the
        // writes it is asked about, so each step moves earlier.
        let since = maybe.iter().map(|write| write.since).max()?;
        // And a write a later one in the same branch replaced is no operand at all: whichever way
        // the construct went, what it wrote is gone by the read.
        let facts = BodyFacts::of(hir);
        let maybe = maybe
            .iter()
            .filter(|write| write.since == since)
            .filter(|write| {
                !maybe
                    .iter()
                    .any(|later| facts.overwrites(hir, self.body(), later.site, write.site, offset))
            })
            .cloned()
            .collect::<Vec<_>>();
        let maybe = maybe.as_slice();

        let mut writes = Vec::with_capacity(maybe.len());
        for write in maybe {
            let value = destructured_value(hir, write.expr, &write.slot)?;
            // A guard written between the assignment and the end of its branch describes the value
            // this operand contributes, because control left the branch past it: `$em =
            // $this->find(); if (!$em) { throw ...; }` hands the join what the guard left and not
            // what the call declares. Later than the write, which is what tells it apart from the
            // branch's own entry guard -- that one is about the value the assignment replaced.
            let value = match write
                .ends
                // A "never" of its own, so no memo: see `crate::proved_memo`.
                .and_then(|ends| {
                    self.narrowing_at(
                        hir,
                        ends,
                        subject,
                        &|_, _| false,
                        &crate::proved_memo::WithoutMemo(asserts),
                    )
                }) {
                Some((at, narrowed)) if at > write.at => narrowed,
                _ => value,
            };
            writes.push(crate::path::WriteSite {
                value,
                at: write.at,
            });
        }
        let before = since;
        // Where the join *arrives*, which for a write carried round a loop is the loop's start and
        // not where it is written: what it holds got here through the back edge, so a guard written
        // between there and the read is later than it and is the more specific claim about the very
        // value being joined. Ranked off `write.at` instead, the join outranked every such guard and
        // `if ($best !== NULL) { $best->m(); }` inside the loop kept the null it had just ruled out.
        let at = maybe
            .iter()
            .map(|write| match write.carried {
                true => write.since,
                false => write.at,
            })
            .max()?;
        Some((
            at,
            ValuePath::of(PathRoot::MaybeWritten {
                writes,
                otherwise: self.otherwise(hir, maybe, subject, asserts),
                read: Box::new(read),
                before,
            }),
        ))
    }

    /// What the path that skipped every one of these writes is known to satisfy.
    ///
    /// The skipped path skipped all of them, so it satisfies what skipping *any* of them requires:
    /// a claim true of the wider set is true of the intersection. A write whose branch guard says
    /// nothing about this name is therefore not fatal -- it contributes nothing and the others still
    /// decide -- while two that say *different* things do end it, because this carries one claim and
    /// picking either would state more than the code did.
    ///
    /// The neutral reading is what a nested lazy fill needs:
    ///
    /// ```php
    /// if ($c) { $this->data = new Fmt(); }
    /// else { if (!isset($this->data)) { $this->data = new Fmt(); } }
    /// return $this->data;
    /// ```
    ///
    /// Skipping the first write means taking the other arm, which says nothing; skipping the second
    /// means `isset` was true, which is the whole answer. Read as needing agreement, the first
    /// write's silence made the join keep a null the `isset` had ruled out -- and said it was
    /// *certain*, because what the name held before the construct is what the leaving guard at the
    /// top of the method had proved it to be.
    pub(crate) fn otherwise(
        &self,
        hir: &FileHir,
        maybe: &[MaybeWrite<'_>],
        subject: Subject<'_>,
        asserts: crate::walk::AssertsArguments<'_>,
    ) -> Otherwise {
        // Whether the path exists at all comes first: a construct whose every arm writes has none,
        // and asking what a guard proved about it would be reading a claim about nothing.
        let writes = maybe.iter().map(|write| write.expr).collect::<Vec<_>>();
        if let Some(since) = maybe.first().map(|write| write.since)
            && BodyFacts::of(hir).writes_fill_every_arm(
                hir,
                self.body(),
                &writes,
                since,
                maybe.iter().all(|write| write.same_pass),
            )
        {
            return Otherwise::Impossible;
        }

        // A loop write has a skipped path no guard inside the loop describes, and it arrives in two
        // spellings: `repeated` is the read standing *outside* a loop the write is in, where the
        // body may never have run, and `carried` is the read standing at the top of the body with
        // the write below it, where the first pass reaches it before any guard has been evaluated.
        // Both mean the value from before the loop gets through untouched, so the complement of an
        // inner guard is a claim about a path that is not the only one. Answered before the
        // agreement below rather than as one more disagreeing voice, because what it says is that
        // there is no claim to make and not that two claims differ.
        if maybe.iter().any(|write| write.repeated || write.carried) {
            return Otherwise::Unproved;
        }

        // Every way around the writes, where they are in a tree of `if`s -- see
        // `crate::skipped_paths`. The reading below, one guard per write, is what answers the rest.
        if let Some(otherwise) = self.otherwise_by_paths(hir, maybe, subject, asserts) {
            return otherwise;
        }

        let mut agreed: Option<Otherwise> = None;
        for write in maybe {
            let proved = match self.proved_leaving_the_other_arm(hir, write, subject, asserts) {
                Some(proved) => proved,
                None => self.proved_entering(hir, write, maybe, subject, asserts),
            };
            if proved == Otherwise::Unproved {
                continue;
            }
            agreed = Some(match agreed {
                None => proved,
                Some(had) if had == proved => had,
                // An assertion is a claim this vocabulary cannot compare against the others, and
                // the path that skipped every write satisfies all of them -- so where one of the
                // two *can* be stated it is kept, rather than the pair cancelling out. Symfony's
                // `SlidingWindowLimiter` is the shape: one arm replaces the value when it is not a
                // `SlidingWindow` and the next when `isExpired()` says so, and the class is the
                // half worth carrying.
                Some(had) if matches!(proved, Otherwise::Asserted(_)) => had,
                Some(Otherwise::Asserted(_)) => proved,
                // Two skipped paths that prove different things, which is what an `if`/`elseif`
                // chain normalising a union produces: each arm replaces a different alternative, so
                // the path around all of them is where the value already was one of the others.
                // Symfony's `FixedWindowLimiter` writes it -- `$window` is replaced when it is not a
                // `CalendarAlignedWindow` and, in the other arm, when it is not a `Window`. Neither
                // claim survives the other, and what both say is that the value was there, which is
                // the only part of the meet there is a spelling for.
                Some(had) => match had.rules_out_null() && proved.rules_out_null() {
                    true => Otherwise::NotNull {
                        refused: crate::path::Refused::NullOnly,
                    },
                    false => return Otherwise::Unproved,
                },
            });
        }
        agreed.unwrap_or(Otherwise::Unproved)
    }

    /// What the path that skipped a write proved on its way out, when that path is the other arm of
    /// the same `if`.
    ///
    /// Later than the condition's complement, and so the claim that holds where the two paths meet:
    ///
    /// ```php
    /// if (is_int($level)) { $level = LogLevel::from($level); }
    /// elseif (!$level instanceof LogLevel) { throw new InvalidArgumentException(); }
    /// ```
    ///
    /// Skipping the write means taking the `elseif`, and coming out of it means the value is a
    /// `LogLevel` -- where the complement says only that it is not an int, which of a `mixed` is
    /// nothing. Only a write written directly in an arm: one nested deeper has a skipped path inside
    /// its own arm too, which the other arm's exit says nothing about.
    fn proved_leaving_the_other_arm(
        &self,
        hir: &FileHir,
        write: &MaybeWrite<'_>,
        subject: Subject<'_>,
        asserts: crate::walk::AssertsArguments<'_>,
    ) -> Option<Otherwise> {
        let facts = BodyFacts::of(hir);
        let construct = facts.innermost_statement(hir, self.body(), write.since)?;
        let StmtData::If {
            then_branch,
            else_branch,
            ..
        } = hir.stmt(construct)
        else {
            return None;
        };
        if hir.stmt(construct).range().start() != write.since {
            return None;
        }
        let written_in = facts.innermost_statement(hir, self.body(), write.at)?;
        let other = if hir.stmt_list(*then_branch).contains(&written_in) {
            *else_branch
        } else if hir.stmt_list(*else_branch).contains(&written_in) {
            *then_branch
        } else {
            return None;
        };
        if hir.stmt_list(other).is_empty() {
            return None;
        }
        let proved = self.claim_through(
            hir,
            other,
            subject,
            &|_, _| false,
            &crate::proved_memo::WithoutMemo(asserts),
            crate::narrowing::PROVED_DEPTH,
        )?;
        let from = hir.stmt(construct).range().start();
        match Self::otherwise_of(proved, from) {
            Otherwise::Unproved => None,
            otherwise => Some(otherwise),
        }
    }

    /// What the guard that let control into a write's branch proved about the name there.
    ///
    /// Read as the *complement*: a branch entered because the name was null is skipped on the path
    /// where it was not. Asked of the guard machinery rather than of the condition's shape, because
    /// each of these has four or five spellings -- `!$x`, `$x === NULL`, `empty($x)`, `!isset($x)`,
    /// and each of them again as the left of an `&&` -- and they are already recognised in one
    /// place. Asked at the write, where the branch's own guard holds and nothing after it does.
    ///
    /// Without the `: never` question, for the reason [`crate::reach`] refuses it: answering it
    /// resolves a call at the whole follow limit, and this is on the path every lookup takes. What
    /// that costs is a guard whose branch ends in such a call, which is one spelling of a leaving
    /// guard this will not see.
    fn proved_entering(
        &self,
        hir: &FileHir,
        write: &MaybeWrite<'_>,
        maybe: &[MaybeWrite<'_>],
        subject: Subject<'_>,
        asserts: crate::walk::AssertsArguments<'_>,
    ) -> Otherwise {
        let (at, since) = (write.at, write.since);
        // The branch condition read at the *skipped* polarity, which is not the complement of what
        // entering proved -- see [`Scope::narrowing_skipping`], where the `||` that separates them
        // is written out. With the argument-assertion question, which this used to decline on the
        // grounds that a null, a class and a kind are the only shapes read below: they are not, and
        // `if (!is_wp_error($thing)) { $thing = new WP_Error(); }` is how a whole framework writes
        // the very construct this answers.
        let Some((ends, from, proved)) = self.narrowing_skipping(hir, at, subject, asserts) else {
            return Otherwise::Unproved;
        };
        // A guard that also holds where the value is *read* is not the write's branch guard, and its
        // complement is then a claim about a path that does not exist. `since` is the start of the
        // outermost branch the reader is not in, so a guard written before that encloses both:
        //
        // ```php
        // if (!is_array($terms)) {
        //     if (',' !== $comma) { $terms = str_replace($comma, ',', $terms); }
        //     trim($terms, ' ,');
        // }
        // ```
        //
        // Complementing `!is_array` here says the path that skipped the write held an array, which
        // is the one thing the branch both lines are in rules out. It was invisible while narrowing a
        // value to a kind it could not be left it alone, and became `$string takes string, and this
        // is string|array` the moment that started answering.
        if ends < since {
            return Otherwise::Unproved;
        }
        if self.other_arm_writes(hir, write, maybe, from) {
            return Otherwise::Unproved;
        }
        Self::otherwise_of(proved, from)
    }

    /// Whether the arm of `write`'s guard that was *not* taken is written by another of the writes
    /// on every way through it.
    ///
    /// The complement of a guard describes the path around a write only where that path is not
    /// itself written. `if (null === $n) { $n = 1; } elseif ($n > 9) { $n = 9; }` is the shape: the
    /// `elseif` says nothing about `$n` being null, so the claim for its write comes from the outer
    /// test -- whose complement, `null === $n`, is the first arm, and that arm wrote. Read as the
    /// path around both, it put a null into a value no path leaves null.
    ///
    /// *Every* way through, or the claim stands: the first write's complement is the `else`, and the
    /// `elseif` inside it can be skipped, so part of that arm is the path around both and what the
    /// complement says holds there. Which is the claim that answers -- the value was not null.
    ///
    /// `from` is where the guard's condition starts, which is how [`Scope::narrowing_skipping`]
    /// names the guard it read.
    fn other_arm_writes(
        &self,
        hir: &FileHir,
        write: &MaybeWrite<'_>,
        maybe: &[MaybeWrite<'_>],
        from: TextSize,
    ) -> bool {
        let facts = BodyFacts::of(hir);
        let guarded_by =
            |guard: &crate::facts::Guard| hir.expr(guard.condition).range().start() == from;
        let Some(taken) = facts
            .if_guards_at(self.body(), write.at)
            .filter(guarded_by)
            .map(|guard| guard.positive)
            .last()
        else {
            return false;
        };
        maybe
            .iter()
            .filter(|other| other.at != write.at)
            .any(|other| {
                facts
                    .if_guards_at(self.body(), other.at)
                    .filter(|guard| guarded_by(guard) && guard.positive != taken)
                    .any(|arm| {
                        facts.reach_of(hir, other.site, arm.over.end(), &|_, _| false)
                            == Some(crate::reach::Reach::Certain)
                    })
            })
    }

    /// A claim about the value, in the vocabulary the path around a write is described in.
    pub(crate) fn otherwise_of(proved: crate::narrowing::Proved, from: TextSize) -> Otherwise {
        match proved {
            crate::narrowing::Proved::NotNull { refused, .. } => Otherwise::NotNull { refused },
            // Only the bare claim: what a needle holds is read off its text, which this vocabulary
            // does not carry.
            crate::narrowing::Proved::NotEmptyString { like: None, .. } => {
                Otherwise::NotEmptyString
            }
            crate::narrowing::Proved::NoneOfKinds { refused, kinds, .. } => {
                Otherwise::NoneOfKinds { refused, kinds }
            }
            // Which kind that is, is the container's declaration's to say, and the vocabulary of
            // the path around a write has no word for a claim that needs a second value.
            crate::narrowing::Proved::KeyOf { .. }
            | crate::narrowing::Proved::ElementOf { .. }
            | crate::narrowing::Proved::NotIdenticalTo { .. }
            | crate::narrowing::Proved::SentinelsRefusedBy { .. }
            | crate::narrowing::Proved::MemberKind { .. } => Otherwise::Unproved,
            // `if ($x !== null) { $x = ...; }`, `isset` and `!is_null` alike: the path around the
            // write is the one where the value was null. Stated as the kind, because that is the
            // spelling `rules_out_null` and the reader already know -- a strict null test and
            // `is_null` are one claim. Not a falsy test, whose path around holds `0` and `''` too.
            crate::narrowing::Proved::Null { strictly: true, .. } => Otherwise::OfKind {
                kind: phrost_model::BuiltinTy::Null,
                settled: true,
            },
            crate::narrowing::Proved::Class { class, .. } => Otherwise::OfClass(class),
            crate::narrowing::Proved::NotClass {
                class,
                exactly: false,
                ..
            } => Otherwise::NotOfClass(class),
            // What extends the class is still possible there, and nothing here knows whether
            // anything does.
            crate::narrowing::Proved::NotClass { exactly: true, .. } => Otherwise::Unproved,
            crate::narrowing::Proved::Kind {
                kind,
                holds,
                settled,
                ..
            } => match holds {
                true => Otherwise::OfKind { kind, settled },
                false => Otherwise::NotOfKind(kind),
            },
            // What a docblock tag proves cannot be stated in this vocabulary, so the claim travels
            // as the path a reader standing in the branch that was not taken would have resolved.
            asserted @ (crate::narrowing::Proved::Asserted { .. }
            | crate::narrowing::Proved::AssertedArgument { .. }) => {
                Otherwise::Asserted(Box::new(ValuePath::of(asserted.root(from))))
            }
            _ => Otherwise::Unproved,
        }
    }

    /// Names this scope binds by assignment, by `catch`, or by `use (...)`, before `offset`.
    ///
    /// A `foreach` target counts too: `foreach ($items as $item)` is where `$item` comes from, and
    /// leaving it out would mean the most common loop variable in PHP never being offered.
    fn bound_names(&self, hir: &FileHir, offset: TextSize, names: &mut Vec<Name>) {
        if let ScopeKind::Closure { captures } = self.kind() {
            for &id in hir.expr_list(*captures) {
                // `use (&$found)` binds inside the closure exactly as `use ($found)` does -- what
                // the `&` changes is who else sees the writes.
                if let Some(name) = Scope::captured(hir, id) {
                    push_unique(names, name.clone());
                }
            }
        }

        visit_statements(
            hir,
            hir.body(self.body()).statements,
            &mut |hir, expr| match hir.expr(expr) {
                ExprData::Assign { target, range, .. } if range.end() <= offset => {
                    match bound_by(hir, *target) {
                        Some(bound) => push_unique(names, bound.name().clone()),
                        None => destructured_names(hir, *target, &mut |name| {
                            push_unique(names, name.clone());
                        }),
                    }
                }
                // `preg_match($p, $s, $matches)` -- see `argument_bindings`.
                data if data.range().end() <= offset => {
                    argument_bindings(hir, expr, &mut |name, _| push_unique(names, name.clone()));
                }
                _ => {}
            },
        );

        visit_foreach_targets(
            hir,
            hir.body(self.body()).statements,
            offset,
            &mut |hir, expr| match bound_by(hir, expr) {
                Some(bound) => push_unique(names, bound.name().clone()),
                // `foreach ($rows as [$id, $label])`, the same shape as an assignment's.
                None => destructured_names(hir, expr, &mut |name| {
                    push_unique(names, name.clone());
                }),
            },
        );

        // `static $x;` and `global $x;` put a name in scope without writing to it, which is the
        // one shape none of the searches above can see.
        for &id in hir.stmt_list(hir.body(self.body()).statements) {
            let StmtData::Bind {
                bindings, range, ..
            } = hir.stmt(id)
            else {
                continue;
            };
            if range.end() > offset {
                continue;
            }
            for &binding in hir.expr_list(*bindings) {
                if let Some(bound) = bound_by(hir, binding) {
                    push_unique(names, bound.name().clone());
                }
            }
        }

        // `/** @var T $x */` names a variable without writing to it either. Usually the statement
        // below the annotation writes the name too and one of the searches above already found it;
        // where error recovery dropped that statement the annotation is the only place the name is
        // written at all, and that is exactly the buffer someone is typing into.
        //
        // A walk rather than the top-level list above, matching `annotation_before`: PHP has no
        // block scope, so one written inside an `if` still names the variable after it.
        crate::walk::visit_every_statement(
            hir,
            hir.body(self.body()).statements,
            &mut |hir, id| {
                let StmtData::Annotation { name, range, .. } = hir.stmt(id) else {
                    return;
                };
                if range.start() <= offset {
                    push_unique(names, name.clone());
                }
            },
        );

        visit_catches(hir, hir.body(self.body()).statements, &mut |hir, catch| {
            let data = hir.catch(catch);
            if data.range.contains_inclusive(offset)
                && let Some(bound) = &data.variable
            {
                let _ = hir;
                push_unique(names, bound.clone());
            }
        });
    }
}

/// The expression a destructuring takes for `slot`, where the value is a literal that names it.
///
/// `[$name, $count] = [$row->name(), 0]` gives `$count` the `0` and nothing wider. Reading it as
/// `<literal>[1]` instead would answer with the literal's *element* type, which is the union of
/// everything in it -- wider than the truth, and a union carrying an alternative the parameter
/// refuses is a report on code that passes the one that fits. Two findings on one real project's
/// tests said exactly that.
///
/// `None` for anything this cannot follow exactly: a spread, a computed key, or a literal that mixes
/// a written key with bare elements, where PHP re-numbers what follows and this does not model it.
fn slot_expression(hir: &FileHir, value: ExprId, slot: &[phrost_model::Name]) -> Option<ExprId> {
    let mut current = value;
    for key in slot {
        let ExprData::ArrayLiteral { elements, .. } = hir.expr(current) else {
            return None;
        };
        let mut ordinal = 0usize;
        let mut keyed = false;
        let mut bare = false;
        let mut found = None;
        for &element in hir.expr_list(*elements) {
            match hir.expr(element) {
                ExprData::ArrayElement {
                    kind: phrost_hir::body::ArrayElementKind::LiteralKey { name, .. },
                    value,
                    ..
                } => {
                    keyed = true;
                    if name == key {
                        found = Some(*value);
                    }
                }
                // A spread contributes however many elements another array holds, and a computed key
                // names a slot this layer cannot identify. Either way the positions after it are
                // unknown.
                ExprData::ArrayElement { .. } => return None,
                _ => {
                    if key.as_str().parse::<usize>() == Ok(ordinal) {
                        found = Some(element);
                    }
                    bare = true;
                    ordinal += 1;
                }
            }
        }
        if keyed && bare {
            return None;
        }
        current = found?;
    }
    Some(current)
}

/// The path with a destructuring target's keys applied to it.
///
/// `[$a, $b] = $row` gives `$b` the value `$row[1]`, and the keys are steps on the same path rather
/// than a root of their own -- what is destructured is itself a path, which is the reason
/// [`PathStep::Element`] is a step. Empty keys leave the path exactly as it was, which is every
/// ordinary write.
pub(crate) fn destructured_value(
    hir: &FileHir,
    assignment: ExprId,
    slot: &[phrost_model::Name],
) -> Option<ValuePath> {
    if !slot.is_empty()
        && let ExprData::Assign { value, .. } = hir.expr(assignment)
        && let Some(exact) = slot_expression(hir, *value, slot)
    {
        return path_of(hir, exact);
    }
    Some(into_slot(path_of(hir, assignment)?, slot))
}

fn into_slot(path: ValuePath, slot: &[phrost_model::Name]) -> ValuePath {
    let mut path = path;
    for key in slot {
        path.steps
            .push(PathStep::Element(Some(crate::path::ElementKey::Literal(
                key.clone(),
            ))));
    }
    path
}

/// The name as the target spells it, for a destructuring target, where `bound_by` has no answer.
///
/// The variable the caller asked about, found again inside the target so the *interned* name travels
/// rather than the `&str` the lookup was made with.
fn bound_name(hir: &FileHir, target: ExprId, name: &str) -> phrost_model::Name {
    let mut found = None;
    crate::walk::visit_expression(hir, target, &mut |hir, expr| {
        if let ExprData::Variable { name: written, .. } = hir.expr(expr)
            && written.as_str() == name
            && found.is_none()
        {
            found = Some(written.clone());
        }
    });
    found.unwrap_or_else(|| unreachable!("the slot was found by the same name"))
}
