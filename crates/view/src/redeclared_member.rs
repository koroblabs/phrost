//! A class that declares one name twice.
//!
//! `Cannot redeclare C::m()` is a fatal at *class load*, so a file with one in it does not run at
//! all. Unlike its neighbours here nothing but the class's own body is needed to see it -- which is
//! why it is worth having anyway: the shape it catches is a thousand-line class, a merge that kept
//! both sides, or a promoted constructor parameter beside a property somebody forgot to delete.
//!
//! # Which names collide, which was asked of PHP rather than assumed
//!
//! Three namespaces, not one. A method and a constant of the same name live together, and so do a
//! method and a property -- `class C { const m = 1; public function m() {} }` loads. Within a
//! namespace the comparison differs: **methods are folded**, so `m()` beside `M()` is the fatal,
//! and **properties are exact**, so `$m` beside `$M` is two properties. A class constant and an
//! **enum case** share one namespace, which is why `enum E { case A; const A = 1; }` reports
//! `Cannot redefine class constant E::A`.
//!
//! A promoted constructor parameter is a property declaration like any other, and PHP says so:
//! `public int $x` beside `__construct(public int $x)` is `Cannot redeclare C::$x`.
//!
//! A `@property` or `@method` tag is not a declaration at all and is skipped: PHP never sees one,
//! and documenting `@property string $scheme` above a real `$scheme` is how a class says what its
//! magic accessor answers. That was twenty of this check's first twenty-one findings on real code.

use phrost_model::{Name, TextRange};
use phrost_sema::MemberKind;

use crate::view::IndexView;

/// One name a class declares more than once.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct RedeclaredMember {
    /// The *second* declaration, which is the one a reader would delete.
    pub range: TextRange,
    pub class: Name,
    pub member: Name,
    pub kind: MemberKind,
}

impl IndexView<'_> {
    /// Every name declared twice by one class in the current file.
    pub fn redeclared_members(&self) -> Vec<RedeclaredMember> {
        let Some(file) = self.current_file() else {
            return Vec::new();
        };
        let Some(skeleton) = self.skeleton.file(file) else {
            return Vec::new();
        };
        let mut out = Vec::new();

        for class in skeleton.classes() {
            let mut seen: Vec<(u8, String)> = Vec::new();
            for member in class.members() {
                // A `@property` or `@method` tag is not a declaration -- PHP never sees it, and a
                // class that documents `@property string $scheme` above a real `$scheme` is the
                // ordinary way to say what a magic accessor returns. WordPress's `Iri` does it
                // eleven times, which is how this was found.
                if member.from_docblock {
                    continue;
                }
                let key = match member.kind {
                    // Folded, because PHP compares a method name that way and `m()` beside `M()`
                    // is the fatal.
                    MemberKind::Method => (0, member.name.as_str().to_ascii_lowercase()),
                    MemberKind::Property => (1, member.name.as_str().to_owned()),
                    // One namespace for both: a case *is* a class constant.
                    MemberKind::ClassConstant | MemberKind::EnumCase => {
                        (2, member.name.as_str().to_owned())
                    }
                };
                if seen.contains(&key) {
                    out.push(RedeclaredMember {
                        range: member.name_range,
                        class: class.fqn().clone(),
                        member: member.name.clone(),
                        kind: member.kind,
                    });
                    continue;
                }
                seen.push(key);
            }
        }

        out
    }
}
