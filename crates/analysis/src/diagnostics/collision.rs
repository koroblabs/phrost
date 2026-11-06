//! Two traits that give one class the same member.
//!
//! `Trait method T2::m has not been applied as C::m, because of collision with T1::m` is a fatal at
//! class load, and the shape it catches is a trait *gaining* a method: two traits composed happily
//! for a year, one grows a `log()`, and every class using both stops loading.
//!
//! The flattening already decided it -- see `MemberTable::conflicts`, which exists because picking
//! a winner would make the server disagree with the runtime. This says it out loud.

use phrost_view::IndexView;

use super::{Check, Diagnostic, Severity};

pub(crate) const TRAIT_COLLISION: &str = "trait-collision";

pub(super) struct TraitCollision;

impl Check for TraitCollision {
    fn run(&self, view: &IndexView<'_>, out: &mut Vec<Diagnostic>) {
        for found in view.trait_collisions() {
            let (first, second) = &found.traits;
            out.push(Diagnostic {
                range: found.range,
                severity: Severity::Error,
                code: TRAIT_COLLISION,
                message: match found.kind {
                    phrost_view::MemberKind::Property => format!(
                        "{first} and {second} declare {}::${} differently, and PHP refuses to compose them",
                        found.class, found.member
                    ),
                    _ => format!(
                        "{first} and {second} both give {} {}, and nothing says which",
                        found.class, found.member
                    ),
                },
                unnecessary: false,
                deprecated: false,
            });
        }
    }
}
