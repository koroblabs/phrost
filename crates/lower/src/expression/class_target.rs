//! The class in a `new`, a `::`, or an `instanceof`.

use mago_syntax::cst::Expression;
use phrost_hir::body::ClassTarget;
use phrost_model::RelativeTy;

use crate::lowerer::Lowerer;

impl<'p> Lowerer<'_, 'p> {
    pub(crate) fn class_target(&mut self, expression: &Expression<'p>) -> ClassTarget {
        match expression {
            Expression::Identifier(identifier) => ClassTarget::Named(self.name_ref(identifier)),
            // The keyword's own span, so a cursor on the `self` of `new self()` has something to
            // land on: the class it means is worked out from where it is written, and until now
            // where it was written was thrown away.
            Expression::Self_(keyword) => ClassTarget::Relative {
                relative: RelativeTy::SelfTy,
                range: self.within_the_text(keyword.span),
            },
            Expression::Static(keyword) => ClassTarget::Relative {
                relative: RelativeTy::Static,
                range: self.within_the_text(keyword.span),
            },
            Expression::Parent(keyword) => ClassTarget::Relative {
                relative: RelativeTy::Parent,
                range: self.within_the_text(keyword.span),
            },
            other => ClassTarget::Expr(self.expression(other)),
        }
    }
}
