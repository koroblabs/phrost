//! A declaration PHP refuses on its own.
//!
//! Rules that need nothing but the file in front of you, each a fatal at compile time -- see
//! [`refused_declarations`](phrost_view::IndexView::refused_declarations), where every one of them
//! is written down with the message PHP gives.

use phrost_view::{IndexView, Refused};

use super::{Check, Diagnostic, Severity};

pub(crate) const REFUSED_DECLARATION: &str = "refused-declaration";

pub(super) struct RefusedDeclaration;

impl Check for RefusedDeclaration {
    fn run(&self, view: &IndexView<'_>, out: &mut Vec<Diagnostic>) {
        for found in view.refused_declarations() {
            let name = &found.member;
            out.push(Diagnostic {
                range: found.range,
                severity: Severity::Error,
                code: REFUSED_DECLARATION,
                message: match found.because {
                    Refused::ReadonlyWithoutType => {
                        format!("readonly ${name} has no type, and PHP requires one")
                    }
                    Refused::ReadonlyWithDefault => {
                        format!("readonly ${name} cannot have a default value")
                    }
                    Refused::ReadonlyAndStatic => {
                        format!("static ${name} cannot be readonly")
                    }
                    Refused::AbstractWithBody => {
                        format!("abstract {name}() cannot have a body")
                    }
                    Refused::AbstractAndPrivate => {
                        format!("abstract {name}() cannot be private -- nothing could implement it")
                    }
                    Refused::PropertyOnEnum => {
                        format!("an enum cannot hold a property, and ${name} is one")
                    }
                    Refused::AbstractAndFinal => {
                        format!("{name}() is abstract, so it cannot also be final")
                    }
                    Refused::CaseWithoutValue => {
                        format!("{name} is a case of a backed enum, so it must have a value")
                    }
                    Refused::CaseWithValue => {
                        format!("{name} is a case of an enum with no backing type, so it cannot have a value")
                    }
                    Refused::DuplicateParameter => {
                        format!("${name} is declared twice in this parameter list")
                    }
                    Refused::ThisAsParameter => {
                        "$this cannot be a parameter -- it is the receiver".to_owned()
                    }
                    Refused::VariadicNotLast => {
                        format!("...${name} takes every argument after it, so nothing may follow it")
                    }
                    Refused::VariadicWithDefault => {
                        format!("...${name} takes every argument after it, so a default says nothing")
                    }
                    Refused::VariadicPromotion => {
                        format!("...${name} is variadic, and a variadic cannot promote a property")
                    }
                    Refused::PromotedOutsideConstructor => {
                        format!("${name} promotes a property, which only a constructor may do")
                    }
                    Refused::StandaloneTypeOnly(builtin) => format!(
                        "`{}` may only be written on its own, and here it is one operand of a type",
                        builtin.as_str()
                    ),
                    Refused::ReturnTypeOnly(builtin) => format!(
                        "`{}` may only be a return type, and this is not one",
                        builtin.as_str()
                    ),
                    Refused::NotAClassInIntersection(builtin) => format!(
                        "an intersection holds classes, and `{}` is not one",
                        builtin.as_str()
                    ),
                    Refused::ConstantCalledClass => {
                        format!("`{name}` is the name-fetching syntax, so no constant may be called it")
                    }
                    Refused::DuplicateInUnion => {
                        format!("{name} writes one type twice in a union")
                    }
                    Refused::BothTrueAndFalse => {
                        format!("{name} writes both `true` and `false`, which is what `bool` is")
                    }
                    Refused::PromotedWithoutABody => {
                        format!("${name} promotes a property, and this constructor has no body to assign it in")
                    }
                },
                unnecessary: false,
                deprecated: false,
            });
        }
    }
}
