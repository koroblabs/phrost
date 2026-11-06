//! Property, static-property and class-constant access, and the `->name` / `::name` selector they
//! share.

use mago_span::HasSpan as _;
use mago_syntax::cst::Variable;
use phrost_hir::{ExprId, body::ExprData};
use phrost_model::{Name, TextRange};
use phrost_syntax::identifier_text;

use crate::lowerer::Lowerer;

impl<'p> Lowerer<'_, 'p> {
    pub(crate) fn access(
        &mut self,
        access: &mago_syntax::cst::Access<'p>,
        range: TextRange,
    ) -> ExprData {
        use mago_syntax::cst::Access;

        match access {
            Access::Property(access) => {
                let receiver = self.expression(access.object);
                let (name, selector) = self.member_selector(&access.property);
                ExprData::PropertyAccess {
                    receiver,
                    name,
                    selector,
                    name_range: self.within_the_text(access.property.span()),
                    nullsafe: false,
                    range,
                }
            }
            Access::NullSafeProperty(access) => {
                let receiver = self.expression(access.object);
                let (name, selector) = self.member_selector(&access.property);
                ExprData::PropertyAccess {
                    receiver,
                    name,
                    selector,
                    name_range: self.within_the_text(access.property.span()),
                    nullsafe: true,
                    range,
                }
            }
            Access::StaticProperty(access) => {
                let class = self.class_target(access.class);
                let property_range = self.within_the_text(access.property.span());
                let (name, selector) = match &access.property {
                    Variable::Direct(direct) => (Some(self.variable_name(direct.name)), None),
                    indirect => {
                        let data = self.variable(indirect, property_range);
                        (None, Some(self.builder.add_expr(data)))
                    }
                };
                ExprData::StaticPropertyAccess {
                    class,
                    name,
                    selector,
                    name_range: property_range,
                    range,
                }
            }
            Access::ClassConstant(access) => {
                let class = self.class_target(access.class);
                let (name, selector) = match &access.constant {
                    mago_syntax::cst::ClassLikeConstantSelector::Identifier(identifier) => (
                        Some(self.interner.name(&identifier_text(identifier.value))),
                        None,
                    ),
                    mago_syntax::cst::ClassLikeConstantSelector::Expression(expression) => {
                        (None, Some(self.expression(expression.expression)))
                    }
                    mago_syntax::cst::ClassLikeConstantSelector::Missing(_) => (None, None),
                };
                ExprData::ClassConstAccess {
                    class,
                    name,
                    selector,
                    name_range: self.within_the_text(access.constant.span()),
                    range,
                }
            }
        }
    }

    /// A `->name` or `::name` selector, or `None` when the name is computed at runtime.
    pub(crate) fn member_selector(
        &mut self,
        selector: &mago_syntax::cst::ClassLikeMemberSelector<'p>,
    ) -> (Option<Name>, Option<ExprId>) {
        match selector {
            mago_syntax::cst::ClassLikeMemberSelector::Identifier(identifier) => (
                Some(self.interner.name(&identifier_text(identifier.value))),
                None,
            ),
            // A name computed at runtime. No name, but the expression that computes it is lowered
            // and kept: it holds a variable, and one nothing can reach is one a rename breaks.
            mago_syntax::cst::ClassLikeMemberSelector::Variable(variable) => {
                let range = self.within_the_text(variable.span());
                let data = self.variable(variable, range);
                (None, Some(self.builder.add_expr(data)))
            }
            mago_syntax::cst::ClassLikeMemberSelector::Expression(expression) => {
                (None, Some(self.expression(expression.expression)))
            }
            mago_syntax::cst::ClassLikeMemberSelector::Missing(_) => (None, None),
        }
    }
}
