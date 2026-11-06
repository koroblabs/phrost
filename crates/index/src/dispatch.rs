//! The extension points a project's own calls declare.
//!
//! Every other catalog in this crate is filled from something that *declares*: an attribute on a
//! class, a key in a YAML file, a service id in a container. This one is filled from a call, and
//! that is not a variation on the same idea -- it is the case where **there is no declaration at
//! all**. `$this->moduleHandler->invokeAll('notifications_alter')` is the entire statement that
//! `hook_notifications_alter` exists; the implementations of it scattered across the project
//! reference a name nothing in the checkout writes down.
//!
//! # Why this covers a project's own code and nothing else
//!
//! Bodies are lowered for [`SourceOrigin::User`](phrost_project::SourceOrigin) and dropped, and for
//! nothing else -- that is the memory rule, not an accident of where this happens to run. So a
//! dependency's dispatches are invisible here, and that is the right boundary: a framework's own
//! extension points are documented in files the index already reads in full, and what is left over
//! is exactly the set nobody documents.
//!
//! # Why the receiver travels unresolved
//!
//! What makes a call a dispatch is its receiver -- `alter` is a method a project declares for
//! things of its own, and one real checkout calls it three times on a breadcrumb builder and a
//! form-alter service. Resolving the receiver *here* is not available: ingest happens once per file
//! in discovery order, so asking the index what a class holds would give a file ingested early a
//! different answer from the same file ingested late, and `incremental == from-scratch` exists to
//! refuse exactly that.
//!
//! So the two halves are split at the only line that keeps both honest. This records what the file
//! itself settles -- which class the receiver was read off, and which member -- and whoever can
//! walk a hierarchy decides whether that member holds what the rule requires. Both halves are
//! derived from the dispatching file alone, so the answer does not depend on when it was read.

use phrost_defmap::FileScope;
use phrost_hir::{
    FileHir, ImportKind,
    body::{ClassTarget, ExprData, LiteralKind},
};
use phrost_model::{FileId, Interner, SymbolKind, TextRange};

use crate::{
    ProjectSkeleton,
    returns::{Dispatch, DispatchReceiver, ReturnContracts},
};

/// One extension point one call declares.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DispatchRow {
    pub catalog: Box<str>,
    pub call: Dispatch,
}

impl lspkit_memsize::MemSize for DispatchRow {
    /// The strings this row owns. The name is a pointer into the interner and is counted there.
    fn heap_size(&self) -> usize {
        self.catalog.len()
            + self.call.method.len()
            + self.call.required.len()
            + self
                .call
                .receiver
                .as_ref()
                .map_or(0, |receiver| receiver.owner.len() + receiver.member.len())
    }
}

impl ProjectSkeleton {
    /// Every extension point this file's own calls declare.
    ///
    /// A linear pass over the file's expression arena rather than a walk, because the question is
    /// about a call wherever it is written -- inside a method, a closure, or a parameter default --
    /// and the arena already holds every one of them with no recursion and no allocation to reach
    /// them. The first thing each expression is asked is whether any framework named its method,
    /// which for every project is nearly always no and costs a scan of a handful of short strings.
    pub(crate) fn collect_dispatches(
        returns: &ReturnContracts,
        interner: &mut Interner,
        file: FileId,
        scope: &FileScope,
        text: &str,
        hir: &FileHir,
    ) -> Box<[DispatchRow]> {
        if !returns.names_any_dispatcher() {
            return Box::default();
        }

        let mut found = Vec::new();
        for (_, expr) in hir.expressions() {
            // A method call and not a static one, because a dispatching method is called *on*
            // something and a static call has no receiver to check. A framework that dispatched
            // from a static would need a different gate, not a wider version of this one.
            let ExprData::MethodCall {
                receiver,
                name: Some(method),
                args: arguments,
                range,
                ..
            } = expr
            else {
                continue;
            };
            let range = *range;
            if returns.dispatch_rules(method.as_str()).is_empty() {
                continue;
            }
            let receiver = Self::member_read(*receiver, scope, hir);

            for rule in returns.dispatch_rules(method.as_str()) {
                let written = Self::names_written(
                    hir,
                    text,
                    hir.expr_list(*arguments),
                    rule.argument,
                    rule.several,
                );
                for (name, name_range) in written {
                    let declared = interner.name(&format!("{}{name}{}", rule.names, rule.suffix));
                    found.push(DispatchRow {
                        catalog: rule.catalog.clone(),
                        call: Dispatch {
                            file,
                            name: declared,
                            range,
                            name_range,
                            method: method.as_str().into(),
                            required: rule.class.clone(),
                            receiver: receiver.clone(),
                        },
                    });
                }
            }
        }
        found.into_boxed_slice()
    }

    /// The names an argument list writes at `position`, each with where it is written.
    ///
    /// One for the ordinary case, several for a rule whose method takes a list. Anything that is not
    /// a plain string literal contributes nothing: a name built by concatenation is not a name this
    /// can read, and guessing at half of one would put a key in the catalog that no call dispatches.
    fn names_written<'t>(
        hir: &FileHir,
        text: &'t str,
        arguments: &[phrost_hir::ExprId],
        position: u16,
        several: bool,
    ) -> Vec<(&'t str, TextRange)> {
        let Some(&argument) = arguments.get(usize::from(position)) else {
            return Vec::new();
        };
        // A named or spread argument is wrapped, and a plain positional one is not -- so a wrapper
        // at this position means the slot is filled by something other than the argument the rule
        // named, and reading it would answer about a different parameter.
        match hir.expr(argument) {
            ExprData::Literal {
                kind: LiteralKind::String,
                range,
            } => unquoted(text, *range)
                .filter(|name| is_a_name(name))
                .map(|name| vec![(name, *range)])
                .unwrap_or_default(),
            ExprData::ArrayLiteral { elements, .. } if several => hir
                .expr_list(*elements)
                .iter()
                .filter_map(|&element| match hir.expr(element) {
                    // A bare element is its own expression; a keyed one wraps the value it holds.
                    ExprData::Literal {
                        kind: LiteralKind::String,
                        range,
                    } => Some(*range),
                    ExprData::ArrayElement { value, .. } => match hir.expr(*value) {
                        ExprData::Literal {
                            kind: LiteralKind::String,
                            range,
                        } => Some(*range),
                        _ => None,
                    },
                    _ => None,
                })
                .filter_map(|range| Some((unquoted(text, range).filter(|n| is_a_name(n))?, range)))
                .collect(),
            _ => Vec::new(),
        }
    }
}

impl ProjectSkeleton {
    /// The class a dispatching call's receiver was read off, and the member read on it.
    ///
    /// Exactly one member on a class the file can name, which is every spelling the frameworks
    /// known here actually use: a service held in a property, or one handed back by a static
    /// accessor. A receiver that is a bare local, or a chain of two, answers nothing -- and nothing
    /// is the right answer, because a claim made about a receiver this cannot identify is a claim
    /// gated on the method name alone.
    fn member_read(
        receiver: phrost_hir::ExprId,
        scope: &FileScope,
        hir: &FileHir,
    ) -> Option<DispatchReceiver> {
        let (holder, member) = match hir.expr(receiver) {
            ExprData::PropertyAccess {
                receiver,
                name: Some(name),
                ..
            } => (Holder::Value(*receiver), name.as_str()),
            ExprData::MethodCall {
                receiver,
                name: Some(name),
                ..
            } => (Holder::Value(*receiver), name.as_str()),
            ExprData::StaticCall {
                class,
                name: Some(name),
                ..
            } => (Holder::Class(class), name.as_str()),
            ExprData::StaticPropertyAccess {
                class,
                name: Some(name),
                ..
            } => (Holder::Class(class), name.as_str()),
            _ => return None,
        };
        Some(DispatchReceiver {
            owner: holder.class(scope, hir)?,
            member: member.into(),
        })
    }
}

/// What a member was read on.
enum Holder<'a> {
    Value(phrost_hir::ExprId),
    Class(&'a ClassTarget),
}

impl Holder<'_> {
    /// The class, as this file names it.
    fn class(&self, scope: &FileScope, hir: &FileHir) -> Option<Box<str>> {
        match self {
            // `$this->handler` and `self::$handler`: the class the call is written inside, which the
            // file settles by itself. `parent` is not one of them -- it needs a hierarchy.
            Self::Value(expr) => match hir.expr(*expr) {
                ExprData::Variable { name, range } if name.as_str() == "this" => {
                    enclosing_class(scope, *range)
                }
                _ => None,
            },
            Self::Class(ClassTarget::Named(reference)) => {
                let written = hir.name_ref(*reference);
                let resolved = scope.scope_at(written.range.start()).resolve(
                    written.path.as_str(),
                    written.anchor,
                    ImportKind::ClassLike,
                );
                Some(resolved.primary().fqn().into())
            }
            Self::Class(ClassTarget::Relative { relative, range }) => match relative {
                phrost_model::RelativeTy::Parent => None,
                _ => enclosing_class(scope, *range),
            },
            Self::Class(ClassTarget::Expr(_)) => None,
        }
    }
}

/// The class-like declaration `at` is written inside, innermost first.
///
/// By range rather than by walking the HIR downwards, because the file's declared symbols already
/// carry a full range each and are the same list every other claim is derived from. Innermost, so a
/// class declared inside another's method body answers for its own calls.
fn enclosing_class(scope: &FileScope, at: TextRange) -> Option<Box<str>> {
    scope
        .symbols()
        .iter()
        .filter(|symbol| {
            matches!(
                symbol.kind,
                SymbolKind::Class | SymbolKind::Interface | SymbolKind::Trait | SymbolKind::Enum
            )
        })
        .filter(|symbol| symbol.full_range.contains_range(at))
        .min_by_key(|symbol| symbol.full_range.len())
        .map(|symbol| symbol.fqn.as_str().into())
}

/// What a string literal says, without the quotes.
///
/// An escape means the name is not what is written between them, and working out what it would be
/// is the lexer's job rather than this one's.
pub(crate) fn unquoted(text: &str, range: TextRange) -> Option<&str> {
    let written = text.get(usize::from(range.start())..usize::from(range.end()))?;
    let quote = written.chars().next()?;
    if quote != '\'' && quote != '"' {
        return None;
    }
    let inner = written.strip_prefix(quote)?.strip_suffix(quote)?;
    (!inner.contains('\\')).then_some(inner)
}

/// Whether a string could be part of a declaration's name.
///
/// PHP's own rule. A dispatched name is pasted into a declaration name, so a string that could not
/// appear in one names nothing that could ever be declared -- and putting it in the catalog would
/// mean holding a key no reference can reach.
fn is_a_name(written: &str) -> bool {
    !written.is_empty()
        && written
            .chars()
            .all(|c| c == '_' || c.is_ascii_alphanumeric() || !c.is_ascii())
}
