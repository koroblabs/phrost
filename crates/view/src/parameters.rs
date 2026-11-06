//! What a call's callee takes, whether or not the callee is a declaration.
//!
//! Three of PHP's four shapes of call name something the index has stored -- a method, a function,
//! a constructor -- and their parameters arrive as [`ParamRow`]s whose types are interned. The
//! fourth, `$fn(...)`, names a *value*; when that value is a closure written in the same file its
//! parameters are in that file's own arena and can never be interned from a reading view. So the
//! two spellings of "a parameter" have to be one thing here, or every check about what a call
//! passes has to learn both -- which is the shape §8 calls two constructors of one rule, and is why
//! `sentinel_arguments`, `incompatible_arguments` and `written_back_arguments` all went silent at
//! the first closure rather than growing a second path.
//!
//! [`crate::compat::Expectation`] is the seam that already exists for exactly this: it is what the
//! four checks holding a written expression use, and its whole reason for being is that a `@var` in
//! this file is an index into its arena where a property's type is an id. A closure's parameter is
//! the fifth site with the same problem and the same answer.

use phrost_hir::{FileHir, ParamData};
use phrost_model::Name;
use phrost_sema::ParamRow;
use phrost_ty::TypeTable;

use crate::compat::Expectation;

/// One parameter of whatever a call is calling.
#[derive(Clone, PartialEq, Eq, Debug)]
pub(crate) enum Parameter {
    /// A method's, a function's or a constructor's: stored, and interned.
    Declared(ParamRow),
    /// A closure literal's, whose types are references into the arena of the file it was written
    /// in -- so this arm is only ever built for a closure the current file holds.
    Closure(ParamData),
}

impl Parameter {
    pub(crate) fn name(&self) -> &Name {
        match self {
            Self::Declared(row) => &row.name,
            Self::Closure(data) => &data.name,
        }
    }

    pub(crate) fn has_default(&self) -> bool {
        match self {
            Self::Declared(row) => row.has_default,
            Self::Closure(data) => data.has_default,
        }
    }

    /// The default as written, which is read for one thing: PHP makes a parameter defaulting to
    /// `null` implicitly nullable whatever its hint says.
    pub(crate) fn default(&self) -> Option<&str> {
        match self {
            Self::Declared(row) => row.default.as_deref(),
            Self::Closure(data) => data.default.as_deref(),
        }
    }

    pub(crate) fn is_variadic(&self) -> bool {
        match self {
            Self::Declared(row) => row.is_variadic,
            Self::Closure(data) => data.is_variadic,
        }
    }

    pub(crate) fn by_reference(&self) -> bool {
        match self {
            Self::Declared(row) => row.by_reference,
            Self::Closure(data) => data.by_reference,
        }
    }

    /// Whether the callee fills this parameter in through the reference -- a `&` beside a
    /// `@param-out` -- so that what was passed is overwritten rather than read.
    pub(crate) fn filled_by_the_callee(&self) -> bool {
        match self {
            Self::Declared(row) => row.by_reference && row.out_ty.is_some(),
            Self::Closure(_) => false,
        }
    }

    /// What the parameter takes, in the spelling its declaration had.
    ///
    /// A declared one merges its hint with its `@param` the way it always has. A closure's cannot
    /// be merged -- `refined` judges two interned types and one of these is not one -- so the
    /// native hint wins and the docblock answers only where there is no hint, which is the same
    /// order [`ParamRow::effective_ty`] applies and the only part of it that is expressible here.
    pub(crate) fn expected(&self, types: &TypeTable) -> Option<Expectation> {
        match self {
            Self::Declared(row) => row.effective_ty(types).map(Expectation::Stored),
            Self::Closure(data) => data
                .type_ref
                .or(data.doc_type_ref)
                .map(Expectation::Written),
        }
    }

    /// The hint alone, which is what PHP enforces whatever the docblock beside it says.
    pub(crate) fn native(&self) -> Option<Expectation> {
        match self {
            Self::Declared(row) => row.ty.map(Expectation::Stored),
            Self::Closure(data) => data.type_ref.map(Expectation::Written),
        }
    }

    /// Whether the parameter refuses every type it accepts.
    ///
    /// The declared arm delegates to [`ParamRow::rejects`], which owns the rule that a `@param`
    /// wider than the hint is the callee promising to handle the extra shapes. A closure's two
    /// slots cannot be merged and cannot be widened by a docblock the same way, so the rule there
    /// is the honest half of it: whichever was written is the expectation.
    pub(crate) fn rejects(
        &self,
        types: &TypeTable,
        mut fit: impl FnMut(Expectation) -> phrost_ty::compat::Fit,
    ) -> bool {
        match self {
            Self::Declared(row) => row.rejects(types, |ty| fit(Expectation::Stored(ty))),
            Self::Closure(data) => match data.type_ref.or(data.doc_type_ref) {
                Some(id) => fit(Expectation::Written(id)) == phrost_ty::compat::Fit::Rejected,
                // An untyped parameter takes anything, so there is nothing to reject it against.
                None => false,
            },
        }
    }

    /// What it takes, rendered for a message.
    pub(crate) fn render(&self, types: &TypeTable, hir: &FileHir) -> Option<String> {
        Some(match self.expected(types)? {
            Expectation::Stored(ty) => types.render_qualified(ty),
            Expectation::Written(id) => phrost_hir::render::render_type_ref(hir, id),
        })
    }

    /// The declaration as PHP, for a signature-help label.
    pub(crate) fn write(&self, out: &mut String, types: &TypeTable, hir: &FileHir) {
        match self {
            Self::Declared(row) => row.write(out, types),
            Self::Closure(data) => {
                if let Some(id) = data.type_ref.or(data.doc_type_ref) {
                    out.push_str(&phrost_hir::render::render_type_ref(hir, id));
                    out.push(' ');
                }
                if data.by_reference {
                    out.push('&');
                }
                if data.is_variadic {
                    out.push_str("...");
                }
                out.push('$');
                out.push_str(data.name.as_str());
                if let Some(written) = &data.default {
                    out.push_str(" = ");
                    out.push_str(written);
                }
            }
        }
    }
}
