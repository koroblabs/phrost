//! A value used in a way no value of its kind can be.
//!
//! `Value of type int is not callable` and `clone(): Argument #1 ($object) must be of type object`.
//! Both stop the line, so like the non-object receiver this is proof rather than an absence of
//! knowledge -- see [`impossible_uses`](phrost_view::IndexView::impossible_uses) for which kinds,
//! which was run rather than reasoned: a string and an array are both callables PHP accepts.

use phrost_view::{IndexView, UsedAs};

use super::{Check, Diagnostic, Severity};

pub(crate) const IMPOSSIBLE_USE: &str = "impossible-use";

pub(super) struct ImpossibleUse;

impl Check for ImpossibleUse {
    fn run(&self, view: &IndexView<'_>, out: &mut Vec<Diagnostic>) {
        for found in view.impossible_uses() {
            out.push(Diagnostic {
                range: found.range,
                severity: Severity::Error,
                code: IMPOSSIBLE_USE,
                message: match (found.used, &found.class) {
                    (UsedAs::Invoked, Some(class)) => {
                        format!("an object of {class} is not callable: it declares no __invoke")
                    }
                    (UsedAs::Subscripted, Some(class)) => {
                        format!("an object of {class} cannot be used as an array: it is not an ArrayAccess")
                    }
                    (UsedAs::AsAString, Some(class)) => format!(
                        "an object of {class} cannot be converted to a string: it declares no __toString"
                    ),
                    (UsedAs::Subscripted | UsedAs::AsAString, None) => continue,
                    (UsedAs::Invoked, None) => {
                        format!("a value of type {} is not callable", found.kind.as_str())
                    }
                    (UsedAs::Cloned, _) => {
                        format!("clone needs an object, and this is {}", found.kind.as_str())
                    }
                    (UsedAs::AsAMethodName, Some(class)) => format!(
                        "a method name has to be a string, and this is an object of {class}"
                    ),
                    (UsedAs::AsAMethodName, None) => format!(
                        "a method name has to be a string, and this is {}",
                        found.kind.as_str()
                    ),
                    (UsedAs::AsAClassName, _) => format!(
                        "a class name has to be an object or a string, and this is {}",
                        found.kind.as_str()
                    ),
                },
                unnecessary: false,
                deprecated: false,
            });
        }
    }
}
