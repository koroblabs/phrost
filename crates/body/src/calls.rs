//! Where the file calls something, and what it passes.
//!
//! A whole-file projection, for anything that has to line arguments up with parameters. The callee
//! is described as written -- a receiver path, a name reference, a class target -- because deciding
//! what it resolves to needs a catalog, which nothing here has.

use lspkit_arena::IdSlice;
use phrost_hir::{
    ExprId, FileHir, NameRefId, StmtId,
    body::{ClassTarget, ExprData},
};
use phrost_model::{Name, TextRange};

use crate::{
    ValuePath,
    path::{class_path_root, path_of},
    scope::Scope,
};

/// One call written in the file.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CallSite {
    pub range: TextRange,
    /// The callee as written, without the argument list: `find` in `$repo->find(1)`, `Repo` in
    /// `new Repo()`.
    ///
    /// Beside [`Self::range`] rather than derived from it, because the two answer different
    /// questions and a caller that wants one is wrong to use the other. An argument-count report
    /// underlines the whole call; a call hierarchy points at the name a reader would click, and
    /// pointing it at `$repo->find(1)` would select the arguments too.
    pub callee_range: TextRange,
    pub target: CallTarget,
    /// In written order.
    pub arguments: Vec<ArgumentSite>,
}

impl CallSite {
    /// Where the callee's receiver is read: the member name for a method call, and the call's own
    /// start for everything else.
    ///
    /// One position for every reader of a call's receiver, and the one a member-access reader uses
    /// too. The receiver is evaluated whole before the name, so both points see the same value --
    /// and asked from the call's start instead, `$builder->select('*')->from('users')` had every
    /// prefix of the chain resolved twice, once from where the chain begins for the call's readers
    /// and once from the name for the access's, because the memo is keyed by position.
    pub fn receiver_at(&self) -> phrost_model::TextSize {
        match self.target {
            CallTarget::Member { .. } => self.callee_range.start(),
            _ => self.range.start(),
        }
    }
}

/// What is being called.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum CallTarget {
    /// `$repo->find(...)` or `Repo::find(...)`: a member reached through something.
    Member { receiver: ValuePath, name: Name },
    /// `find(...)`, by written name.
    Function(NameRefId),
    /// `new Repo(...)`, whose parameters are the constructor's.
    Constructor(ClassTarget),
    /// `$fn(...)`, `($f)()`, `$handlers[$k](...)`: a call through a value.
    ///
    /// The fourth shape of call, and the one nobody had added -- so every check about what a call
    /// *passes* went silent at a closure, and `$fn(` offered neither signature help nor a parameter
    /// hint. What it names is the value being invoked; whether that value carries a parameter list
    /// is the view's question, and for a closure written in the same file it does.
    Invoked(ValuePath),
}

/// One argument as written.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ArgumentSite {
    pub range: TextRange,
    /// Written as `name: value`.
    pub name: Option<Name>,
    /// Written as `...$rest`.
    pub spread: bool,
    /// The value being passed, for a reader that needs to know what it holds rather than where it is.
    pub value: ExprId,
}

/// Every call in the file, in the order the walk reaches them.
///
/// Closures included, which is not free and is not optional: `array_map(fn ($row) => $repo->find($row), $rows)`
/// holds a call inside a call, and a walk that stopped at the closure would leave signature help and
/// parameter hints dead everywhere a callback is written.
pub fn call_sites(hir: &FileHir) -> Vec<CallSite> {
    let mut sites = Vec::new();

    Scope::visit_every_expression(hir, &mut |hir, expr| {
        if let Some(site) = site_of(hir, expr) {
            sites.push(site);
        }
    });

    sites
}

/// Every call under `statements`, not counting one inside a closure written there.
///
/// The same scope [`throw_sites`](crate::throw_sites) reads, and for the same reason: a rule about
/// what one declaration does must see the same body both halves of it see. A closure has its own
/// scope and its own docblock, and a call written inside one escapes to wherever the closure is
/// invoked rather than to the declaration it was written in.
///
/// Apart from [`call_sites`], which includes closures on purpose -- signature help and parameter
/// hints are about the cursor's position and have no declaration to attribute anything to.
pub fn call_sites_in(hir: &FileHir, statements: IdSlice<StmtId>) -> Vec<CallSite> {
    let mut sites = Vec::new();
    crate::walk::visit_statements(hir, statements, &mut |hir, expr| {
        if let Some(site) = site_of(hir, expr) {
            sites.push(site);
        }
    });
    sites.sort_by_key(|site| site.callee_range.start());
    sites
}

/// Whether a call's *callee name* is written at `offset`.
///
/// Apart from [`call_site_at`] on purpose, and the difference is one line there: that one is the
/// signature-help query and skips any offset at or before the name's end, because a cursor on
/// `find` in `$repo->find(` is not yet writing an argument. Here the name is exactly what is being
/// asked about -- a report on what a call throws is underlined on it.
///
/// A bool, and a whole-file walk that builds nothing: the caller is a prefilter deciding whether a
/// far more expensive question is worth asking at all.
pub fn a_call_is_named_at(hir: &FileHir, offset: phrost_model::TextSize) -> bool {
    let mut found = false;
    Scope::visit_every_expression(hir, &mut |hir, expr| {
        if found {
            return;
        }
        if let Some(callee) = callee_range(hir, hir.expr(expr)) {
            found = callee.contains_inclusive(offset);
        }
    });
    found
}

/// The innermost call `offset` sits inside.
///
/// A position query rather than a filter over [`call_sites`], because the two cost differently: this
/// tests each expression's *range* first and builds exactly one site, where the whole-file form
/// builds one per call in the file -- each with a vector of arguments. On the path a keystroke takes,
/// that is the difference between one allocation and a hundred.
///
/// Innermost, because `outer(inner($x))` is two calls containing the cursor and only the inner one is
/// being written.
///
/// Inside means inside the *arguments*, not inside the expression. A cursor on the callee's own name
/// is not filling any of that call's parameters -- in `outer("a", inn|er("b"))` the argument being
/// written is `outer`'s second, and answering with `inner`'s first highlights a parameter of the
/// wrong function.
pub fn call_site_at(hir: &FileHir, offset: phrost_model::TextSize) -> Option<CallSite> {
    let mut best: Option<(TextRange, ExprId)> = None;

    Scope::visit_every_expression(hir, &mut |hir, expr| {
        let data = hir.expr(expr);
        let Some(callee) = callee_range(hir, data) else {
            return;
        };
        let range = data.range();
        if !range.contains_inclusive(offset) || offset < callee.end() {
            return;
        }
        if best.is_none_or(|(current, _)| range.len() < current.len()) {
            best = Some((range, expr));
        }
    });

    site_of(hir, best?.1)
}

/// Where a call's callee is written, for a call this module models.
///
/// `None` for anything that is not one of those, which is what makes it the filter as well as the
/// measurement: the two questions have the same answer and asking them separately let them drift.
///
/// Everything from its end to the closing parenthesis is the argument list. The opening parenthesis
/// is not recorded anywhere -- the callee's end is the nearest fact the HIR holds, and the only
/// thing between the two is whitespace.
fn callee_range(hir: &FileHir, data: &ExprData) -> Option<TextRange> {
    match data {
        ExprData::MethodCall { name_range, .. } | ExprData::StaticCall { name_range, .. } => {
            Some(*name_range)
        }
        ExprData::FunctionCall { name, .. } => Some(hir.name_ref(*name).range),
        ExprData::New { class, .. } => Some(class_target_range(hir, class)),
        // The value being invoked, which is what a reader points at: there is no name of its own to
        // underline, and `$fn` is where a person looks when the call is wrong.
        ExprData::IndirectCall { callee, .. } => Some(hir.expr(*callee).range()),
        _ => None,
    }
}

/// Where the class a `new` names is written.
fn class_target_range(hir: &FileHir, class: &ClassTarget) -> TextRange {
    match class {
        ClassTarget::Named(reference) => hir.name_ref(*reference).range,
        ClassTarget::Relative { range, .. } => *range,
        ClassTarget::Expr(expr) => hir.expr(*expr).range(),
    }
}

/// The narrowest call whose arguments enclose `inner`.
///
/// What a callback's parameter needs: the closure is an argument of a call, and the call is what
/// says where its elements come from. Strictly containing, so a call written *inside* the closure is
/// not mistaken for the one it was passed to.
pub fn enclosing_call(hir: &FileHir, inner: TextRange) -> Option<CallSite> {
    let calls = call_expressions(hir);
    let holding = |(range, _): &&(TextRange, ExprId)| {
        range.contains_range(inner) && range.len() > inner.len()
    };
    // Through the nesting index: a call holding `inner` holds its start, so the candidates are the
    // calls around that offset -- in the list's order, which is what settles a tie below.
    let best = match calls.nest.holding(inner.start()) {
        Some(around) => around
            .map(|index| &calls.list[index as usize])
            .filter(holding)
            .min_by_key(|(range, _)| range.len())?,
        None => calls
            .list
            .iter()
            .filter(holding)
            .min_by_key(|(range, _)| range.len())?,
    };
    site_of(hir, best.1)
}

/// Every call expression in the file, and their ranges nested -- see [`call_expressions`].
struct CallExpressions {
    list: Vec<(TextRange, ExprId)>,
    nest: crate::range_nest::RangeNest,
}

/// Every call expression in the file, with its range, walked once per tree.
///
/// What [`enclosing_call`] searches. It walked every expression in the file per question, and it is
/// asked once for each read of a closure's parameter -- so a test file of two hundred
/// `->where(function ($q) { ... })` calls walked itself two hundred times over.
fn call_expressions(hir: &FileHir) -> &CallExpressions {
    hir.derived(phrost_hir::Derived::CallExpressions, |hir| {
        let mut calls = Vec::new();
        Scope::visit_every_expression(hir, &mut |hir, expr| {
            let data = hir.expr(expr);
            if matches!(
                data,
                ExprData::MethodCall { .. }
                    | ExprData::StaticCall { .. }
                    | ExprData::FunctionCall { .. }
                    | ExprData::New { .. }
                    | ExprData::IndirectCall { .. }
            ) {
                calls.push((data.range(), expr));
            }
        });
        CallExpressions {
            nest: crate::range_nest::RangeNest::of(calls.iter().map(|(range, _)| Some(*range))),
            list: calls,
        }
    })
}

pub(crate) fn site_of(hir: &FileHir, expr: ExprId) -> Option<CallSite> {
    let data = hir.expr(expr);
    // Asked of the same expression the arms below read, so a site can never carry a callee range
    // belonging to a different shape of call than the target it reports.
    let callee_range = callee_range(hir, data)?;

    let (target, args, range) = match data {
        ExprData::MethodCall {
            receiver,
            name: Some(name),
            args,
            range,
            ..
        } => (
            CallTarget::Member {
                receiver: path_of(hir, *receiver)?,
                name: name.clone(),
            },
            *args,
            *range,
        ),
        ExprData::StaticCall {
            class,
            name: Some(name),
            args,
            range,
            ..
        } => (
            CallTarget::Member {
                receiver: ValuePath::of(class_path_root(*class)),
                name: name.clone(),
            },
            *args,
            *range,
        ),
        ExprData::FunctionCall {
            name, args, range, ..
        } => (CallTarget::Function(*name), *args, *range),
        ExprData::New { class, args, range } => (CallTarget::Constructor(*class), *args, *range),
        ExprData::IndirectCall {
            callee,
            args,
            range,
        } => (CallTarget::Invoked(path_of(hir, *callee)?), *args, *range),
        _ => return None,
    };

    let arguments = hir
        .expr_list(args)
        .iter()
        .map(|&id| match hir.expr(id) {
            ExprData::Argument {
                name,
                spread,
                value,
                range,
            } => ArgumentSite {
                range: *range,
                name: name.clone(),
                spread: *spread,
                value: *value,
            },
            // A bare expression is a plain positional argument -- see `ExprData::Argument`.
            other => ArgumentSite {
                range: other.range(),
                name: None,
                spread: false,
                value: id,
            },
        })
        .collect();

    Some(CallSite {
        target,
        arguments,
        range,
        callee_range,
    })
}
