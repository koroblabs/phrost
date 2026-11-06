//! A write the property's own declaration refuses.
//!
//! `readonly` and PHP 8.4's asymmetric visibility both say where a property may be written from,
//! and both are an `Error` at runtime rather than a coercion -- so a line this reports is one that
//! cannot run. Neither was checked at all: `$point->x = 9;` on a readonly property, a second write
//! to one the constructor promoted, and `$user->name = 'x';` on a `private(set)` were all silent.
//!
//! What is proven and what is not is settled one layer down; see
//! [`refused_writes`](phrost_view::IndexView::refused_writes). The short of it is that this reports
//! only what the declaration decides on its own, and never the question of whether a write already
//! happened on some path.

use phrost_view::{IndexView, RefusedBecause};

use super::{Check, Diagnostic, Severity};

pub(crate) const READONLY_WRITE: &str = "readonly-write";

pub(super) struct ReadonlyWrite;

impl Check for ReadonlyWrite {
    fn run(&self, view: &IndexView<'_>, out: &mut Vec<Diagnostic>) {
        for write in view.refused_writes() {
            out.push(Diagnostic {
                range: write.range,
                severity: Severity::Error,
                code: READONLY_WRITE,
                message: match write.reason {
                    RefusedBecause::ReadonlyFromOutside => format!(
                        "Cannot write readonly {}::${} from outside the class that declares it",
                        write.class, write.property
                    ),
                    RefusedBecause::ReadonlyAlreadyPromoted => format!(
                        "Cannot write readonly {}::${} again -- the constructor promoted it",
                        write.class, write.property
                    ),
                    RefusedBecause::SetScope(set) => format!(
                        "Cannot write {}::${}, which declares {}(set)",
                        write.class,
                        write.property,
                        set.as_str()
                    ),
                    RefusedBecause::ReadOnlyTag => format!(
                        "Cannot write {}::${}, which is documented `@property-read`",
                        write.class, write.property
                    ),
                    RefusedBecause::OnlyAGetHook => format!(
                        "Cannot write {}::${}, which is a virtual property with no set hook",
                        write.class, write.property
                    ),
                },
                unnecessary: false,
                deprecated: false,
            });
        }
    }
}
