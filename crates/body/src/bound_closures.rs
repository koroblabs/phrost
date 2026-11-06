//! The class a closure runs as, when something handed it one.
//!
//! `Closure::bind($fn, $newThis, $scope)` and `$fn->bindTo($newThis, $scope)` are how PHP reaches
//! into another object's internals on purpose: the closure's *scope* becomes `$scope`, and inside it
//! that class's private and protected members are as reachable as they are in its own methods.
//! Symfony's Cache component is written around it -- every adapter builds its `CacheItem`s through a
//! closure bound to `CacheItem::class`, because the item's fields are protected and the adapter is
//! not the item.
//!
//! Read from the closure outward rather than from the call inward: what a reader has is an offset,
//! and the question is which class the code at that offset counts as. A closure written inside a
//! bound one inherits the binding, so every closure the offset is in is asked, innermost first.
//!
//! Asked only where a check is about to speak: `enclosing_call` walks every expression in the file,
//! which is affordable once for an access that has already resolved to a member and been found
//! unreachable, and would not be per access. A caller that wants it on a hot path should derive it
//! per revision the way `facts.rs` does.
//!
//! Only where the scope is *written* as a class. `Closure::bind($fn, $object)` takes the scope from
//! `$object`'s class, which is a resolution question and lives above this crate; leaving it out here
//! costs the two-argument spelling and keeps this a walk of the tree.

use phrost_hir::{
    FileHir,
    body::{ClassTarget, ClosureKind, ExprData},
};
use phrost_model::TextSize;

use crate::{calls::enclosing_call, scope::Scope};

/// The class the closure containing `offset` was bound to, if one was.
pub fn bound_closure_scope(hir: &FileHir, offset: TextSize) -> Option<ClassTarget> {
    Scope::chain_at(hir, offset)
        .iter()
        .rev()
        .filter(|scope| scope.is_closure())
        .find_map(|scope| {
            let closure = hir.body(scope.body()).range;
            let call = enclosing_call(hir, closure)?;
            let scope_argument = match &call.target {
                // `Closure::bind($fn, $newThis, $scope)`, and `$closure->bindTo($newThis, $scope)`.
                // Both are the same method under two spellings, and the scope sits one place later
                // in the static one because the closure itself is its first argument.
                crate::calls::CallTarget::Member { name, .. } => {
                    match name.as_str().to_ascii_lowercase().as_str() {
                        "bind" => 2,
                        "bindto" => 1,
                        _ => return None,
                    }
                }
                _ => return None,
            };
            class_written_at(hir, call.arguments.get(scope_argument)?.value)
        })
}

/// Where `$this` inside one closure around an offset comes from -- see [`closure_this_at`].
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ClosureThis {
    /// Handed to a callee as an argument, whose parameter may say what `$this` will be:
    /// `@param-closure-this`, which is how Laravel's `macro()` and every manager's `extend()` say it.
    Passed(crate::PassedTo),
    /// Bound where it is written -- `Closure::bind(function () {...}, $kernel)`, and the closure as
    /// the receiver of `->bindTo($kernel)` or `->call($kernel)` -- to the value written there.
    ///
    /// `at` is where that value is read, which is the call: it is evaluated before the closure
    /// runs. `None` for a value that is no path, which says `$this` is something nobody can name.
    Bound {
        at: TextSize,
        value: Option<crate::ValuePath>,
    },
}

/// What says what `$this` is in each closure around `offset`, innermost first.
///
/// Every level and not only the innermost, because a closure written inside a bound one captures
/// its `$this`: `macro('x', function () { return array_map(fn ($v) => $this->wrap($v), ...); })`.
/// A level whose callee says nothing is the reader's to step past, and a static closure has no
/// `$this` to be told about, so the list stops at one.
///
/// Only a closure written *in place* -- as the argument, or as the receiver of the bind. One
/// stored in a variable first is bound by whoever calls with the variable, which is a flow question
/// this does not ask.
pub fn closure_this_at(hir: &FileHir, offset: TextSize) -> Vec<ClosureThis> {
    let mut found = Vec::new();
    for scope in Scope::chain_at(hir, offset).iter().rev() {
        if !scope.is_closure() {
            break;
        }
        let inside = hir.body(scope.body()).range;
        let Some(call) = enclosing_call(hir, inside) else {
            continue;
        };
        let written = call
            .arguments
            .iter()
            .enumerate()
            .find_map(|(position, argument)| {
                let value = unwrapped(hir, argument.value);
                match hir.expr(value) {
                    ExprData::Closure { kind, body, .. } if *body == scope.body() => {
                        Some((position, argument, *kind))
                    }
                    _ => None,
                }
            });
        let Some((position, argument, kind)) = written else {
            // Not an argument: the receiver, for the two binds written on the closure itself.
            if let Some(bound) = bound_as_receiver(hir, &call, scope.body()) {
                found.push(bound);
            }
            continue;
        };
        if matches!(
            kind,
            ClosureKind::StaticClosure | ClosureKind::StaticArrowFunction
        ) {
            break;
        }
        if position == 0 && is_closure_bind(hir, &call) {
            found.push(ClosureThis::Bound {
                at: call.range.start(),
                value: call
                    .arguments
                    .get(1)
                    .and_then(|bound| crate::path::path_of(hir, unwrapped(hir, bound.value))),
            });
            continue;
        }
        let name = argument.name.clone();
        let Some(callee) = crate::locals::callee_of_target(call.target) else {
            continue;
        };
        found.push(ClosureThis::Passed(crate::PassedTo {
            at: call.range.start(),
            callee,
            passed_as: crate::PassedAs::Argument(position),
            name,
            class_arguments: Vec::new(),
        }));
    }
    found
}

/// `Closure::bind(...)`, matched on its receiver as [`bound_scope_names`] does: `bind` is what a
/// container is told to do, and Laravel's `$router->bind('post', function () {...})` hands its
/// closure over as the second argument of a call with the same name.
fn is_closure_bind(hir: &FileHir, call: &crate::CallSite) -> bool {
    let crate::CallTarget::Member { receiver, name } = &call.target else {
        return false;
    };
    name.as_str().eq_ignore_ascii_case("bind")
        && receiver.steps.is_empty()
        && matches!(
            receiver.root,
            crate::PathRoot::Class(ClassTarget::Named(class))
                if hir.name_ref(class).path.as_str().trim_start_matches('\\')
                    .eq_ignore_ascii_case("Closure")
        )
}

/// `(function () {...})->bindTo($kernel)` and `->call($kernel)`: the closure bound to the first
/// argument, when the closure is what the call is made on.
fn bound_as_receiver(
    hir: &FileHir,
    call: &crate::CallSite,
    body: phrost_hir::BodyId,
) -> Option<ClosureThis> {
    let crate::CallTarget::Member { receiver, name } = &call.target else {
        return None;
    };
    if !(name.as_str().eq_ignore_ascii_case("bindTo") || name.as_str().eq_ignore_ascii_case("call"))
        || !receiver.steps.is_empty()
    {
        return None;
    }
    let crate::PathRoot::Closure(closure) = receiver.root else {
        return None;
    };
    match hir.expr(closure) {
        ExprData::Closure {
            kind, body: own, ..
        } if *own == body
            && !matches!(
                kind,
                ClosureKind::StaticClosure | ClosureKind::StaticArrowFunction
            ) =>
        {
            Some(ClosureThis::Bound {
                at: call.range.start(),
                value: call
                    .arguments
                    .first()
                    .and_then(|bound| crate::path::path_of(hir, unwrapped(hir, bound.value))),
            })
        }
        _ => None,
    }
}

/// An argument's value, past the `name:` wrapper a named argument lowers to.
fn unwrapped(hir: &FileHir, expr: phrost_hir::ExprId) -> phrost_hir::ExprId {
    match hir.expr(expr) {
        ExprData::Argument { value, .. } => *value,
        _ => expr,
    }
}

/// Every class this file names as a bind scope, as written and by its short name.
///
/// The other direction of the question above, and the one a *different file* has to ask. A check
/// that says "nothing in this file reaches this private member" is right only where nothing
/// outside the file can, and `Closure::bind($fn, null, CacheItem::class)` written anywhere in the
/// project is exactly that: a closure with another class's scope, calling its privates from a
/// file that does not declare them.
///
/// The short name as *written*, not resolved. Resolving needs the file's imports and belongs
/// above this crate, and the difference only ever makes this suppress more -- two classes of one
/// short name are one entry, and a reader is told nothing about either rather than told something
/// wrong about one. The set is a handful of names on a project that uses the pattern at all.
pub fn bound_scope_names(hir: &FileHir) -> Vec<String> {
    let mut found = Vec::new();
    for (_, expr) in hir.expressions() {
        // `Closure::bind($fn, $newThis, $scope)` and `$closure->bindTo($newThis, $scope)`, the
        // two spellings `bound_closure_scope` reads from the other end.
        //
        // The static one is matched on its *receiver* as well, which reading from the other end
        // does not have to: a call enclosing a closure is already narrow, and a walk of every
        // expression is not. `bind` is what a container is told to do -- Symfony's own
        // `->bind()` is written all over its dependency injection -- and taking those measured
        // every finding on that corpus suppressed rather than the two that deserved it.
        let (name, args) = match expr {
            ExprData::MethodCall { name, args, .. } => (name, args),
            ExprData::StaticCall {
                class: ClassTarget::Named(class),
                name,
                args,
                ..
            } if hir
                .name_ref(*class)
                .path
                .as_str()
                .eq_ignore_ascii_case("Closure") =>
            {
                (name, args)
            }
            _ => continue,
        };
        let Some(name) = name else { continue };
        let at = match (expr, name.as_str().to_ascii_lowercase().as_str()) {
            (ExprData::StaticCall { .. }, "bind") => 2,
            // `bindTo` is `Closure`'s alone, so the receiver need not be checked -- and cannot be
            // without resolving it, which is a question for a layer above this crate.
            (ExprData::MethodCall { .. }, "bindto") => 1,
            _ => continue,
        };
        let Some(&argument) = hir.expr_list(*args).get(at) else {
            continue;
        };
        if let Some(ClassTarget::Named(named)) = class_written_at(hir, argument) {
            let path = hir.name_ref(named).path.as_str();
            found.push(phrost_model::short_name(path).to_owned());
        }
    }
    found
}

/// The class an argument names, for the one shape a scope is written in: `Foo::class`.
fn class_written_at(hir: &FileHir, expr: phrost_hir::ExprId) -> Option<ClassTarget> {
    let expr = match hir.expr(expr) {
        ExprData::Argument { value, .. } => *value,
        _ => expr,
    };
    match hir.expr(expr) {
        ExprData::ClassConstAccess {
            class,
            name: Some(name),
            ..
        } if name.as_str().eq_ignore_ascii_case("class") => Some(*class),
        _ => None,
    }
}
