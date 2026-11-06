//! Where a variable is written.
//!
//! The lexical half of answering about a local: which name a cursor is on, and whether it is a use
//! of the variable or the parameter that introduced it. What it *holds* is a separate question, and
//! a much harder one -- see [`local_value`](crate::local_value).

use phrost_hir::{
    ExprId, FileHir, TypeRefId,
    body::{ExprData, StmtData},
};
use phrost_model::{Name, TextRange, TextSize};

use crate::ValuePath;
use crate::walk::visit_calls_before;
use crate::{
    BindingKind,
    binding::{Bound, bound_by},
    path::path_of,
    scope::Scope,
    walk::{visit_catches, visit_foreach_bindings, visit_statements},
};

/// A variable named at one place in the source.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum VariableSite {
    /// A use inside a body: `$repo->find()`, `echo $count`.
    Use { name: Name, range: TextRange },
    /// The target of an assignment: the `$local` in `$local = new Repo()`.
    ///
    /// Apart from a plain use because the binding being asked about is the assignment the cursor is
    /// standing on, and the search backwards deliberately excludes that one -- for a use like
    /// `$x = $x->next()` the *previous* assignment is the right answer, and taking the enclosing one
    /// would be circular.
    Assigned {
        name: Name,
        range: TextRange,
        value: ValuePath,
    },
    /// The parameter's own name in a signature, which is where it is introduced.
    ///
    /// Separate because nothing in the body binds it, so the type comes from the declaration rather
    /// than from a search backwards through the scope.
    Parameter {
        name: Name,
        range: TextRange,
        /// The declared type, native or from `@param`.
        declared: Option<TypeRefId>,
    },
}

impl VariableSite {
    pub fn name(&self) -> &Name {
        match self {
            Self::Use { name, .. } | Self::Assigned { name, .. } | Self::Parameter { name, .. } => {
                name
            }
        }
    }

    pub fn range(&self) -> TextRange {
        match self {
            Self::Use { range, .. }
            | Self::Assigned { range, .. }
            | Self::Parameter { range, .. } => *range,
        }
    }
}

/// The variable the cursor is on, if it is on one.
///
/// Parameters first, because a signature is not inside the body and a use-site search would miss
/// them entirely -- and hovering the name in `function run(Repo $repo)` is the one place a reader is
/// most likely to ask.
pub fn variable_at(hir: &FileHir, offset: TextSize) -> Option<VariableSite> {
    if let Some(found) = parameter_at(hir, offset) {
        return Some(found);
    }
    if let Some(found) = caught_at(hir, offset) {
        return Some(found);
    }

    let mut best: Option<VariableSite> = None;
    // Closures included: the cursor can be on a variable written inside one, and it used to find
    // nothing there while the member call beside it resolved perfectly.
    Scope::visit_every_expression(hir, &mut |hir, expr| {
        let found = match hir.expr(expr) {
            ExprData::Variable { name, range } if range.contains_inclusive(offset) => {
                VariableSite::Use {
                    name: name.clone(),
                    range: *range,
                }
            }
            // `[$a, $b] = $pair`: the cursor is on a binding here too, of the slot its key names.
            // Without it the name was a plain use searched backwards from its own write, which
            // finds nothing -- so the card said "no declared type" on the line that gives it one,
            // while every later use of `$a` named the class.
            ExprData::Assign { target, .. }
                if matches!(hir.expr(*target), ExprData::ArrayLiteral { .. }) =>
            {
                let Some((name, range)) = destructured_variable_at(hir, *target, offset) else {
                    return;
                };
                let Some(slot) = crate::binding::destructured_slot(hir, *target, name.as_str())
                else {
                    return;
                };
                let Some(value) = crate::locals::destructured_value(hir, expr, &slot) else {
                    return;
                };
                VariableSite::Assigned { name, range, value }
            }
            ExprData::Assign { target, .. } => {
                let ExprData::Variable { name, range } = hir.expr(*target) else {
                    return;
                };
                if !range.contains_inclusive(offset) {
                    return;
                }
                let Some(value) = path_of(hir, expr) else {
                    return;
                };
                VariableSite::Assigned {
                    name: name.clone(),
                    range: *range,
                    value,
                }
            }
            // `compact('title')` names `$title`, so the string is a use of it -- and the one place
            // a person stands to ask where the variable it collects came from.
            ExprData::FunctionCall { .. } => {
                let Some((name, range)) = crate::occurrences::compacted_names(hir, expr)
                    .find(|(_, range)| range.contains_inclusive(offset))
                else {
                    return;
                };
                VariableSite::Use {
                    name: name.clone(),
                    range,
                }
            }
            _ => return,
        };

        // Narrowest wins, the same rule every other position lookup uses. An assignment target
        // and the variable expression under it share a range exactly, and the assignment is the
        // more useful of the two -- so it replaces an equal-width match rather than losing to it.
        let better = match best.as_ref() {
            None => true,
            Some(current) => match (current, &found) {
                (VariableSite::Use { .. }, VariableSite::Assigned { .. }) => {
                    found.range().len() <= current.range().len()
                }
                _ => found.range().len() < current.range().len(),
            },
        };
        if better {
            best = Some(found);
        }
    });

    best
}

/// The variable written inside a destructuring target that the cursor is on.
fn destructured_variable_at(
    hir: &FileHir,
    target: phrost_hir::ExprId,
    offset: TextSize,
) -> Option<(Name, TextRange)> {
    let mut found = None;
    crate::walk::visit_expression(hir, target, &mut |hir, expr| {
        if let ExprData::Variable { name, range } = hir.expr(expr)
            && range.contains_inclusive(offset)
        {
            found = Some((name.clone(), *range));
        }
    });
    found
}

/// The variable a `catch` clause binds, when the cursor is on its name.
///
/// Its own search for [`parameter_at`]'s reason: the name is written in the clause and not in the
/// body, so it is no expression the walk below would meet. `catch (RuntimeException $exception)`
/// hovered and went nowhere while every *use* of `$exception` answered -- the tell for this class of
/// bug, which §8 records, is a feature that works at a use and not at the declaration of the same
/// thing.
///
/// Reported as an ordinary [`VariableSite::Use`] rather than a binding of its own, because the
/// binding site is where the variable is introduced and `Scope::catch_binding` already resolves it
/// from the whole clause -- so what a reader is shown here is what they are shown at a use, which is
/// the answer they wanted.
fn caught_at(hir: &FileHir, offset: TextSize) -> Option<VariableSite> {
    let mut found = None;
    for scope in Scope::every_scope(hir) {
        visit_catches(hir, hir.body(scope.body()).statements, &mut |hir, catch| {
            let data = hir.catch(catch);
            let (Some(name), Some(range)) = (data.variable.as_ref(), data.variable_range) else {
                return;
            };
            if range.contains_inclusive(offset) {
                let _ = hir;
                found = Some(VariableSite::Use {
                    name: name.clone(),
                    range,
                });
            }
        });
        if found.is_some() {
            return found;
        }
    }
    found
}

/// Visits every variable a `static` or `global` in this body names.
///
/// Its own walk because a `Bind` is the one statement whose whole content is a declaration: there
/// is no "inside" of it that comes after the cursor, so the search that only looks at preceding
/// statements skips the declaration a cursor is sitting on.
fn visit_bind_statements(
    hir: &FileHir,
    body: phrost_hir::BodyId,
    visit: &mut impl FnMut(&FileHir, ExprId),
) {
    for &id in hir.stmt_list(hir.body(body).statements) {
        if let StmtData::Bind { bindings, .. } = hir.stmt(id) {
            for &binding in hir.expr_list(*bindings) {
                visit(hir, binding);
            }
        }
    }
}

/// The range of the variable a binding names, whichever spelling it was written in.
fn declared_range(hir: &FileHir, binding: ExprId) -> TextRange {
    match hir.expr(binding) {
        ExprData::Assign { target, .. } => hir.expr(*target).range(),
        other => other.range(),
    }
}

/// The parameter whose name the offset sits on, in whichever declaration contains it.
fn parameter_at(hir: &FileHir, offset: TextSize) -> Option<VariableSite> {
    for (_, class) in hir.classes() {
        for &id in hir.member_list(class.members) {
            let member = hir.member(id);
            if let Some(found) = in_params(hir, member.params, offset) {
                return Some(found);
            }
            // A `set(string $value)` declares its parameter where no member's list holds it.
            for &id in hir.hook_list(member.hooks) {
                if let Some(found) = in_params(hir, hir.hook(id).params, offset) {
                    return Some(found);
                }
            }
        }
    }
    for (_, function) in hir.functions() {
        if let Some(found) = in_params(hir, function.params, offset) {
            return Some(found);
        }
    }

    // Closures declare parameters too, and theirs are reachable only through the body that wrote
    // them -- which for a closure inside a closure is a body no declaration lists, so the walk has to
    // be the one that enters closures. `->withExceptions(function ($e) { $e->when(fn (Request $r)
    // => ...); })` answered at every use of `$r` and at nothing where it is declared.
    let mut found = None;
    Scope::visit_every_expression(hir, &mut |hir, expr| {
        if found.is_none()
            && let ExprData::Closure { params, .. } = hir.expr(expr)
        {
            found = in_params(hir, *params, offset);
        }
    });
    found
}

fn in_params(
    hir: &FileHir,
    params: lspkit_arena::IdSlice<phrost_hir::ParamId>,
    offset: TextSize,
) -> Option<VariableSite> {
    hir.param_list(params)
        .iter()
        .map(|&id| hir.param(id))
        .find(|param| param.name_range.contains_inclusive(offset))
        .map(|param| VariableSite::Parameter {
            name: param.name.clone(),
            range: param.name_range,
            declared: phrost_hir::refined_type_ref(hir, param.type_ref, param.doc_type_ref),
        })
}

/// Where the name is bound, for the scope containing `offset`.
///
/// A different question from what the name *holds*, and deliberately its own search. The two can
/// disagree and both be right: after `if ($x instanceof Repo)` the value comes from the guard while
/// the binding is still the parameter, and goto belongs on the parameter.
///
/// Within a scope the *first* binding wins. PHP has no block scope, so a later write to the same
/// name is a reassignment and not a second declaration -- and "where does this variable come from"
/// is answered by where it came into being. Both other servers this is compared against agree, and
/// they disagreed with us on twenty positions before this.
///
/// Correctness rests on the candidates already being filtered: a `catch` clause's variable is only
/// offered inside that clause, and a `foreach` target only after the loop that binds it, so the
/// earliest candidate is the earliest one that can actually reach the cursor. A `$e` in a second
/// `catch` does not resolve to the first one's.
/// Where a `foreach` target binds `name`: the target itself, or a variable inside one written as a
/// destructuring.
///
/// Through `bound_by` first, so `foreach ($rows as &$row)` reaches `$row`: the reference wraps the
/// loop variable rather than replacing it. Then into the target, because `foreach ($cases as
/// [$handler, $matching])` binds through no variable of its own -- hover and completion knew both
/// names and goto went nowhere, the same miss an assignment's destructuring had before
/// [`destructured_binding`] existed.
fn foreach_target_binding(hir: &FileHir, target: ExprId, name: &str) -> Option<TextRange> {
    if bound_by(hir, target).is_some_and(|bound| bound.name().as_str() == name) {
        return Some(hir.expr(target).range());
    }
    let ExprData::ArrayLiteral { elements, .. } = hir.expr(target) else {
        return None;
    };
    let mut found = None;
    destructured_binding(hir, *elements, name, &mut |range, _| {
        found.get_or_insert(range);
    });
    found
}

/// Where a destructuring target binds `name`, if it does.
///
/// Each element, unwrapped: a target's elements are wrapped so they can carry the slot they take
/// -- a key, or the index a hole still counts towards -- so the variable is one level in. Nested,
/// because `[[$a, $b], $c] = $rows` is a target inside a target and PHP binds all three.
fn destructured_binding(
    hir: &FileHir,
    elements: lspkit_arena::IdSlice<ExprId>,
    name: &str,
    consider: &mut impl FnMut(TextRange, BindingKind),
) {
    for &element in hir.expr_list(elements) {
        let value = match hir.expr(element) {
            ExprData::ArrayElement { value, .. } => *value,
            _ => element,
        };
        match hir.expr(value) {
            ExprData::ArrayLiteral { elements, .. } => {
                destructured_binding(hir, *elements, name, consider);
            }
            _ => {
                if bound_by(hir, value).is_some_and(|bound| bound.name().as_str() == name) {
                    consider(hir.expr(value).range(), BindingKind::Assignment);
                }
            }
        }
    }
}

pub fn local_declaration(
    hir: &FileHir,
    offset: TextSize,
    name: &str,
) -> Option<(TextRange, BindingKind)> {
    for (index, scope) in Scope::chain_at(hir, offset).iter().enumerate().rev() {
        let mut best: Option<(TextRange, BindingKind)> = None;
        let mut consider = |range: TextRange, kind: BindingKind| {
            if range.start() <= offset
                && best.is_none_or(|(current, _)| range.start() < current.start())
            {
                best = Some((range, kind));
            }
        };

        if let Some(param) = hir
            .param_list(scope.params())
            .iter()
            .map(|&id| hir.param(id))
            .find(|param| param.name.as_str() == name)
        {
            consider(param.name_range, BindingKind::Parameter);
        }

        visit_catches(hir, hir.body(scope.body()).statements, &mut |hir, catch| {
            let data = hir.catch(catch);
            if !data.range.contains_inclusive(offset) {
                return;
            }
            if data
                .variable
                .as_ref()
                .is_some_and(|bound| bound.as_str() == name)
                && let Some(range) = data.variable_range
            {
                consider(range, BindingKind::Catch);
            }
        });

        // A `foreach` binding is ranked apart from every other kind, because the rule `consider`
        // follows is wrong for it. Earliest-wins is right for an assignment -- `$x = 1; ...; $x = 2`
        // has one declaration and it is the first -- and a loop variable is not that: two sibling
        // `foreach`es over two different lists are two declarations that happen to share a name, and
        // a reader inside the second means the second. Measured against another server on a real
        // file, every one of three loops in one method answered with the first.
        //
        // So: the loop whose body the cursor is *inside* wins, innermost first; and failing that,
        // the last one written above it, which is the binding whose value leaked past its closing
        // brace. Only that one candidate is offered to `consider`, so the general rule still
        // decides between kinds.
        let mut looped: Option<(TextRange, TextRange)> = None;
        visit_foreach_bindings(
            hir,
            hir.body(scope.body()).statements,
            offset,
            &mut |hir, binding| {
                for target in [binding.key, Some(binding.value)].into_iter().flatten() {
                    let Some(range) = foreach_target_binding(hir, target, name) else {
                        continue;
                    };
                    let inside = binding.loop_range.contains_inclusive(offset);
                    let better = match &looped {
                        None => true,
                        Some((_, held)) => match (inside, held.contains_inclusive(offset)) {
                            // Inside beats outside; between two the cursor is inside, the tighter
                            // loop is the inner one; between two it is outside, the later.
                            (true, false) => true,
                            (false, true) => false,
                            (true, true) => binding.loop_range.len() < held.len(),
                            (false, false) => range.start() >= looped_start(&looped),
                        },
                    };
                    if better {
                        looped = Some((range, binding.loop_range));
                    }
                }
            },
        );
        if let Some((range, _)) = looped {
            consider(range, BindingKind::Foreach);
        }

        // `static $x;` / `global $x;`: the statement introduces the name, and it is the only
        // binding that is not a write -- so every search above is blind to it, and a read written
        // above the first write (the `empty($x)` guard that is the whole idiom) had nothing at all.
        // Over the whole body rather than only the statements before the cursor, the same way the
        // assignment search below works: `consider` is what decides position, and the declaration
        // itself has to count, so goto on `static $x;` lands on the name it is already on.
        visit_bind_statements(hir, scope.body(), &mut |hir, binding| {
            if bound_by(hir, binding).is_some_and(|b| b.name().as_str() == name) {
                consider(declared_range(hir, binding), BindingKind::Declared);
            }
        });

        // A write through an element is the weakest binding there is, so it is collected apart:
        // `$rows[] = x` written after `$rows = [...]` is a mutation, and goto belongs on the
        // declaration above it.
        let mut created: Option<TextRange> = None;
        visit_statements(hir, hir.body(scope.body()).statements, &mut |hir, expr| {
            let ExprData::Assign { target, .. } = hir.expr(expr) else {
                return;
            };
            // A destructuring target writes through no variable of its own, so `bound_by` has
            // nothing to say about it -- which left every name `[$a, $b] = $row` binds with no
            // declaration at all: goto answered nothing, on the use *and* on the binding. The
            // type half of this was built when destructuring was modelled; this is the other
            // place a local lives, and §8 has the general form.
            if let ExprData::ArrayLiteral { elements, .. } = hir.expr(*target) {
                destructured_binding(hir, *elements, name, &mut consider);
                return;
            }
            let Some(bound) = bound_by(hir, *target) else {
                return;
            };
            if bound.name().as_str() != name {
                return;
            }
            let range = hir.expr(*target).range();
            match bound {
                Bound::Whole(_) => consider(range, BindingKind::Assignment),
                // Earliest: when nothing else declares it, the write that created the variable is
                // the first one -- which is also PHP's rule for when a write creates it at all.
                Bound::Element(_) => {
                    if range.start() <= offset
                        && created.is_none_or(|current| range.start() < current.start())
                    {
                        created = Some(range);
                    }
                }
            }
        });

        // The binding that is not a write: `preg_match($p, $s, $matches)` creates `$matches` at
        // the call, and nothing in this body assigns it. Collected apart from `consider` because it
        // ranks below everything there -- which argument is by reference is the callee's business,
        // and no layer below the index can say.
        let mut from_argument: Option<TextRange> = None;
        visit_calls_before(
            hir,
            hir.body(scope.body()).statements,
            offset,
            &mut |hir, expr, _| {
                for range in bare_arguments_named(hir, expr, name) {
                    if range.start() <= offset
                        && from_argument.is_none_or(|current| range.start() < current.start())
                    {
                        from_argument = Some(range);
                    }
                }
            },
        );

        if best.is_none() {
            best = created
                .map(|range| (range, BindingKind::Assignment))
                // Last of all, and right for the same reason it is last: a call taking the argument
                // by *value* leaves a name nothing set, and the call is still where a reader looks.
                .or_else(|| from_argument.map(|range| (range, BindingKind::Argument)));
        }
        if best.is_some() {
            return best;
        }
        // Nothing here binds it, so whether it can be the enclosing scope's is the capture rule.
        if index == 0 || !scope.reaches_outward(hir, name) {
            return None;
        }
    }

    None
}

/// The constructor parameter that declares a property of this name, when one is in scope.
///
/// Constructor property promotion writes one declaration that PHP reads as two: `__construct(public
/// int $across)` binds a parameter *and* declares `$this->across`. Everything that renames or lists
/// occurrences has to treat them as the one thing they are, and each half on its own is a search
/// that finds the other half's sites and stops -- a rename from the parameter leaves every
/// `$this->across` in the project spelled the old way, and one from the property leaves the
/// constructor body reading a variable nothing binds.
///
/// Only a constructor may promote, so the flag alone identifies it and the enclosing function's
/// name is never asked. Answered from the scope chain, so a `$across` bound by a different method
/// of the same class is not this.
pub fn promoting_parameter(hir: &FileHir, offset: TextSize, name: &str) -> Option<TextRange> {
    Scope::chain_at(hir, offset).iter().rev().find_map(|scope| {
        hir.param_list(scope.params())
            .iter()
            .map(|&id| hir.param(id))
            .find(|param| param.promotion.is_some() && param.name.as_str() == name)
            .map(|param| param.name_range)
    })
}

/// Every assignment to a plain variable in the file, with the value it took.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Assigned {
    pub name: Name,
    /// The target, `$name`.
    pub range: TextRange,
    /// The right-hand side, for reporting what could not be typed.
    pub value_range: TextRange,
    /// The path to the value, or `None` when the expression is not one this layer models as a path.
    pub value: Option<ValuePath>,
}

/// Every assignment in the file, in the order the walk reaches them.
///
/// For measuring how much of a real project's inference works, which is the only way to tell a gap
/// that matters from one nobody writes. An assignment is where a local gets its type, so an
/// assignment whose value has no path is a variable with no type -- and the right-hand side's range
/// says which expression to go and look at.
pub fn assignments(hir: &FileHir) -> Vec<Assigned> {
    let mut found = Vec::new();

    // Closures included, or the measurement is blind to every assignment written inside one --
    // which would make the coverage it reports flatter than the truth.
    Scope::visit_every_expression(hir, &mut |hir, expr| {
        let ExprData::Assign { target, value, .. } = hir.expr(expr) else {
            return;
        };
        let ExprData::Variable { name, range } = hir.expr(*target) else {
            return;
        };
        found.push(Assigned {
            name: name.clone(),
            range: *range,
            value_range: hir.expr(*value).range(),
            // From the assignment rather than from its right-hand side: an inline `@var` is
            // recorded on the assignment, and reading only the value would step past it.
            value: path_of(hir, expr),
        });
    });

    found
}

/// Every argument of a call written as exactly this variable.
///
/// The match is on the *bare* name and nothing reached through it: `f($rows)` may write `$rows`
/// back and `f($rows['k'])` never writes `$rows`, whatever the parameter says.
fn bare_arguments_named(hir: &FileHir, call: ExprId, name: &str) -> Vec<TextRange> {
    let args = match hir.expr(call) {
        ExprData::MethodCall { args, .. }
        | ExprData::StaticCall { args, .. }
        | ExprData::FunctionCall { args, .. }
        | ExprData::New { args, .. } => *args,
        _ => return Vec::new(),
    };

    hir.expr_list(args)
        .iter()
        .filter_map(|&passed| match hir.expr(passed) {
            ExprData::Variable {
                name: written,
                range,
            } if written.as_str() == name => Some(*range),
            _ => None,
        })
        .collect()
}

/// The start of the binding a `foreach` search is currently holding.
fn looped_start(held: &Option<(TextRange, TextRange)>) -> TextSize {
    held.map_or(TextSize::new(0), |(range, _)| range.start())
}
