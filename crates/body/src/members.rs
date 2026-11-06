//! Every member-access site in the file, and whether an offset sits inside a string literal.
//!
//! The string check rides along because it answers the same kind of question: what a cursor is on
//! before completion or hover reasons about it, and a half-typed string is the one shape the lowered
//! tree cannot show at all.

use phrost_hir::{
    ExprId, FileHir,
    body::{CallableReceiver, ClassTarget, ExprData},
};
use phrost_model::TextSize;

use crate::{
    path::{MemberSite, SiteKind, ValuePath, path_of},
    scope::Scope,
};

/// The member named at `offset`, if the cursor is on one.
///
/// Narrowest wins, the same rule position lookups use elsewhere: in `$this->a->b` the cursor is on
/// exactly one of the two names, and the ranges say which.
pub fn member_site_at(hir: &FileHir, offset: TextSize) -> Option<MemberSite> {
    let mut best: Option<&MemberSite> = None;

    for site in walked_member_sites(hir) {
        if !site.name_range.contains_inclusive(offset) {
            continue;
        }
        let narrower = best.is_none_or(|current| site.name_range.len() < current.name_range.len());
        if narrower {
            best = Some(site);
        }
    }

    best.cloned()
}

/// Every member named anywhere in the file, in source order.
///
/// The whole-file counterpart of [`member_site_at`]. A cursor query wants one site and allocates
/// nothing; anything that has to visit all of them -- checking each access, listing where a member
/// is used -- wants the list, and both read the same walk so neither can drift from the other.
pub fn member_sites(hir: &FileHir) -> Vec<MemberSite> {
    let mut sites = walked_member_sites(hir).to_vec();
    // Source order, because the walk order follows body discovery rather than the text, and a
    // caller reporting positions to a person reads them top to bottom.
    sites.sort_by_key(|site| site.name_range.start());
    sites
}

/// Every member-access site in the file, in walk order, walked once per tree.
///
/// Kept because [`member_site_at`] is asked per candidate by the checks -- whether the author
/// guarded a member with `method_exists` is asked of every access about to be reported -- and a test
/// file of a few hundred mocked calls walked itself once for each.
fn walked_member_sites(hir: &FileHir) -> &[MemberSite] {
    hir.derived(phrost_hir::Derived::MemberSites, |hir| {
        let mut sites = Vec::new();
        Scope::visit_every_expression(hir, &mut |hir, expr| {
            if let Some(site) = site_of(hir, expr) {
                sites.push(site);
            }
            sites.extend(cloned_property_sites(hir, expr));
        });
        sites
    })
}

/// The properties a `clone($object, [...])` writes on the copy, each a member named on the object.
fn cloned_property_sites(hir: &FileHir, expr: ExprId) -> Vec<MemberSite> {
    let Some(clone) = crate::clone_with::clone_with(hir, expr) else {
        return Vec::new();
    };
    let Some(receiver) = path_of(hir, clone.object) else {
        return Vec::new();
    };
    let receiver_range = hir.expr(clone.object).range();
    clone
        .entries
        .into_iter()
        .map(|entry| MemberSite {
            receiver: receiver.clone(),
            name: entry.name,
            kind: SiteKind::Property,
            name_range: entry.name_range,
            receiver_range,
            nullsafe: false,
            presumed: false,
            named: false,
        })
        .collect()
}

/// Whether `offset` sits inside a string literal.
///
/// Asked before offering names, because a class name is not what someone typing inside
/// `service('...')` is reaching for -- and a list of 188 classes over a service id is noise that
/// hides the editor's own history.
///
/// From the token stream recorded at lowering, not from the tree. A string being typed is not closed
/// yet, so it does not parse and the recovered tree has no literal for it -- which is precisely the
/// moment a completion list must not appear. The lexer has a token for the unfinished form, so one
/// mechanism covers both.
pub fn string_literal_at(hir: &FileHir, offset: TextSize) -> bool {
    hir.inside_string(offset)
}

/// Where PHP reads a value without complaining about a null part way along it.
///
/// Two constructs and one operator: `isset($a->b)`, `empty($a->b)`, and the left operand of
/// `$a->b ?? $default`. All three exist to be asked about something that may not be there, so a
/// property read through a null inside one is the author handling the null rather than missing it.
///
/// Ranges rather than a flag on each site, because the suppression belongs to the expression around
/// the access and a site is read without its surroundings. Only whole-file callers pay for the walk;
/// a cursor query never asks.
///
/// A method *call* inside one of these is not covered, and PHP agrees: the call is evaluated, so
/// `$a->b() ?? $c` on a null `$a` is a fatal error like anywhere else. The caller applies that rule,
/// because only it knows which kind of member the site named.
pub fn null_suppressed_ranges(hir: &FileHir) -> Vec<phrost_model::TextRange> {
    let mut ranges = Vec::new();

    Scope::visit_every_expression(hir, &mut |hir, expr| match hir.expr(expr) {
        ExprData::NullTest { value, kind, .. } if kind.reads_through_null() => {
            ranges.push(hir.expr(*value).range());
        }
        ExprData::Coalesce { left, .. } => ranges.push(hir.expr(*left).range()),
        _ => {}
    });

    ranges
}

/// Reads one expression as a member site, if it names a member.
fn site_of(hir: &FileHir, expr: ExprId) -> Option<MemberSite> {
    let presumed = matches!(
        hir.expr(expr),
        ExprData::CallableMember { presumed: true, .. }
    );
    let (receiver, name, kind, name_range, receiver_range, nullsafe) = match hir.expr(expr) {
        ExprData::PropertyAccess {
            receiver,
            name: Some(name),
            name_range,
            nullsafe,
            ..
        } => (
            path_of(hir, *receiver)?,
            name.clone(),
            SiteKind::Property,
            *name_range,
            hir.expr(*receiver).range(),
            *nullsafe,
        ),
        ExprData::MethodCall {
            receiver,
            name: Some(name),
            name_range,
            nullsafe,
            ..
        } => (
            path_of(hir, *receiver)?,
            name.clone(),
            SiteKind::Method,
            *name_range,
            hir.expr(*receiver).range(),
            *nullsafe,
        ),
        ExprData::StaticPropertyAccess {
            class,
            name: Some(name),
            name_range,
            ..
        } => (
            ValuePath::of(crate::path::class_path_root(*class)),
            name.clone(),
            SiteKind::Property,
            *name_range,
            class_range(hir, *class),
            false,
        ),
        ExprData::StaticCall {
            class,
            name: Some(name),
            name_range,
            ..
        } => (
            ValuePath::of(crate::path::class_path_root(*class)),
            name.clone(),
            SiteKind::Method,
            *name_range,
            class_range(hir, *class),
            false,
        ),
        ExprData::ClassConstAccess {
            class,
            name: Some(name),
            name_range,
            ..
        } => (
            ValuePath::of(crate::path::class_path_root(*class)),
            name.clone(),
            SiteKind::ClassConstant,
            *name_range,
            class_range(hir, *class),
            false,
        ),
        // `$repo->find(...)`, `Repo::find(...)`. A site and not a call: the name is written and a
        // cursor lands on it, but nothing is invoked -- so this is here and deliberately absent from
        // `call_sites`, where it would be a call with no arguments and every argument check would
        // fire on it. Never nullsafe: PHP rejects `?->` combined with `(...)`.
        ExprData::CallableMember {
            receiver,
            name: Some(name),
            name_range,
            ..
        } => (
            match receiver {
                CallableReceiver::Value(value) => path_of(hir, *value)?,
                CallableReceiver::Class(class) => {
                    ValuePath::of(crate::path::class_path_root(*class))
                }
            },
            name.clone(),
            SiteKind::Method,
            *name_range,
            match receiver {
                CallableReceiver::Value(value) => hir.expr(*value).range(),
                CallableReceiver::Class(class) => class_range(hir, *class),
            },
            false,
        ),
        _ => return None,
    };

    Some(MemberSite {
        // A receiver is a *prefix* of the expression it is part of, and PHP short-circuits the rest
        // of a chain rather than reaching through a null -- `$a?->b()->c()` never calls `c()`. So
        // what the whole expression may be says nothing about this step, and carrying it would
        // report every chain written with a `?->` in front of it.
        receiver: ValuePath {
            may_be_absent: false,
            ..receiver
        },
        name,
        kind,
        name_range,
        receiver_range,
        nullsafe,
        presumed,
        named: false,
    })
}

/// Where a static receiver was written.
fn class_range(hir: &FileHir, class: ClassTarget) -> phrost_model::TextRange {
    match class {
        ClassTarget::Named(name) => hir.name_ref(name).range,
        ClassTarget::Relative { range, .. } => range,
        ClassTarget::Expr(expr) => hir.expr(expr).range(),
    }
}

/// Where the author has written a runtime check that a member exists, covering `site`.
///
/// `if (method_exists($this, 'getWarranty')) { $this->getWarranty(); }` is how PHP expresses "this
/// may be on a subclass" -- there is no other way to say it -- and reporting the call inside the
/// guard reports the guard itself as the mistake. Five of thirty-six undefined-member findings on one
/// real project were this, and a checker that argues with a written guard is a checker people switch
/// off.
///
/// Returns the ranges of the *name literals* rather than the names, because the name is the check's
/// second argument and only the first is carried in the tree. Whoever has the file's text compares
/// them; a range is the honest thing for this layer to hand over.
///
/// Three guarding shapes, because real code uses all of them: the branch of an `if`, an operand of
/// the same `&&` -- `method_exists($e, 'getResponse') && $e->getResponse()` -- and an `if` above
/// that leaves when the check fails.
pub fn existence_checked(hir: &FileHir, site: &MemberSite) -> Vec<phrost_model::TextRange> {
    let offset = site.name_range.start();
    let mut ranges = Vec::new();
    // `$this->type::tryFrom()` is a class named by a value, and the check names that value:
    // `method_exists($this->type, 'tryFrom')`. The site's receiver is the class target, so it is
    // compared as the value it was written as.
    let named_by_a_value = match (&site.receiver.root, site.receiver.steps.as_slice()) {
        (crate::PathRoot::Class(phrost_hir::body::ClassTarget::Expr(value)), []) => {
            path_of(hir, *value)
        }
        _ => None,
    };
    let receiver = named_by_a_value.as_ref().unwrap_or(&site.receiver);

    let facts = crate::facts::BodyFacts::of(hir);
    for scope in Scope::chain_at(hir, offset).iter().rev() {
        let body = scope.body();
        for guard in facts
            .if_guards_at(body, offset)
            .chain(facts.operand_guards_at(body, offset))
        {
            checked_names(hir, guard.condition, receiver, guard.positive, &mut ranges);
        }
        // And a check that left when it failed: `if (!method_exists($t, 'tryFrom')) { return; }`
        // vouches for everything after it in the block. Only a branch that leaves by its own
        // syntax -- a call declared `never` would need the callee looked up, and this layer
        // cannot, so it is taken as falling through, which only ever costs an excuse.
        crate::walk::visit_preceding_statements(
            hir,
            hir.body(body).statements,
            offset,
            &|_, _| false,
            &mut |hir, statement| {
                let phrost_hir::body::StmtData::If {
                    condition,
                    then_branch,
                    ..
                } = hir.stmt(statement)
                else {
                    return;
                };
                if crate::walk::always_leaves(hir, *then_branch, &|_, _| false) {
                    checked_names(hir, *condition, receiver, false, &mut ranges);
                }
            },
        );
    }

    ranges
}

/// The value a check is about, looking through an assignment to the name it wrote:
/// `method_exists($guard = $auth->guard(), 'setDispatcher')` tests what `$guard` holds after it, and
/// `$guard` is what the branch reads -- Laravel's `AuthServiceProvider` writes exactly that.
fn tested_path(hir: &FileHir, expr: ExprId) -> Option<ValuePath> {
    match hir.expr(expr) {
        ExprData::Assign { target, .. } => path_of(hir, *target),
        _ => path_of(hir, expr),
    }
}

/// Every member name a condition checks the existence of, on `receiver`.
///
/// Walks through `&&` because a check is usually one term of several:
/// `is_a($f, X::class) && method_exists($d, 'id') && !in_array($d->id(), $skip)`.
///
/// With the guard's polarity, because the commonest way to write one is to leave when it fails:
/// `if (!enum_exists($t) || !method_exists($t, 'tryFrom')) { return false; }` holds, below it, that
/// both negations are false -- De Morgan, which is what `||` on the negative side of a guard means.
fn checked_names(
    hir: &FileHir,
    condition: ExprId,
    receiver: &ValuePath,
    holds: bool,
    into: &mut Vec<phrost_model::TextRange>,
) {
    match hir.expr(condition) {
        // `&&` where the guard held and `||` where it did not: each operand holds on its own.
        // The other two pairings vouch for nothing, since either operand may be the one that
        // decided.
        ExprData::Logical {
            operator,
            left,
            right,
            ..
        } if matches!(
            (operator, holds),
            (phrost_hir::body::LogicalOp::And, true) | (phrost_hir::body::LogicalOp::Or, false)
        ) =>
        {
            checked_names(hir, *left, receiver, holds, into);
            checked_names(hir, *right, receiver, holds, into);
        }
        // `||` where the guard held: the branch runs when any one operand does, so the operands
        // vouch only together. A `method_exists` vouches for its name, and an `instanceof` on the
        // same value for whatever its class declares -- which this layer cannot read, and which the
        // author named because it does: `$t instanceof Translator || method_exists($t,
        // 'getFallbackLocales')` is how Symfony writes "the class that has it, or anything else
        // that does". Any other operand vouches for nothing, and then neither does the whole.
        ExprData::Logical {
            operator: phrost_hir::body::LogicalOp::Or,
            left,
            right,
            ..
        } if holds => {
            let mut vouched = Vec::new();
            for operand in [*left, *right] {
                if matches!(
                    hir.expr(operand),
                    ExprData::Instanceof { value, .. }
                        if tested_path(hir, *value).as_ref() == Some(receiver)
                ) {
                    continue;
                }
                let before = vouched.len();
                checked_names(hir, operand, receiver, true, &mut vouched);
                if vouched.len() == before {
                    return;
                }
            }
            into.extend(vouched);
        }
        ExprData::Not { value, .. } => checked_names(hir, *value, receiver, !holds, into),
        ExprData::FunctionCall { name, args, .. }
            if holds
                && matches!(
                    hir.name_ref(*name)
                        .path
                        .as_str()
                        .to_ascii_lowercase()
                        .as_str(),
                    "method_exists" | "property_exists"
                ) =>
        {
            let [subject, member] = hir.expr_list(*args) else {
                return;
            };
            // The check has to be about *this* value. `method_exists($other, 'foo')` says nothing
            // about `$this->foo()`, and treating it as a licence would silence a real finding
            // wherever the two happen to share a member name.
            if tested_path(hir, *subject).as_ref() != Some(receiver) {
                return;
            }
            if let ExprData::Literal {
                kind: phrost_hir::body::LiteralKind::String,
                range,
            } = hir.expr(*member)
            {
                into.push(*range);
            }
        }
        // `isset($decoded->rjti) ? (string) $decoded->rjti : NULL`: a property PHP was asked about
        // and answered "set" is one that exists, and `!empty()` says as much. Affirmative "is null"
        // in the tree, so the guard holds where the test does not. The range handed back is the
        // name's own, which reads as the name the way a literal's does once its quotes are trimmed.
        ExprData::NullTest { value, kind, .. } if !holds && kind.reads_through_null() => {
            if let ExprData::PropertyAccess {
                receiver: subject,
                name: Some(_),
                name_range,
                ..
            } = hir.expr(*value)
                && path_of(hir, *subject).as_ref() == Some(receiver)
            {
                into.push(*name_range);
            }
        }
        _ => {}
    }
}
