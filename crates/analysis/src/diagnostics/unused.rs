//! A local nothing ever reads.
//!
//! The one finding in this file that is not about something being wrong. `$unused = $this->work();`
//! runs correctly; it is code somebody left behind, and the protocol has a tag for exactly that --
//! editors fade it instead of underlining it, which asks for the right reaction.
//!
//! Precision here means completeness in the other direction from the rest of these checks. A
//! *missed read* is what produces a false finding, so the layer below has to see every way PHP can
//! read a variable -- inside a string, as a closure capture, through an arrow function, through a
//! member name computed at runtime -- and to stop entirely where it cannot: an `extract()`, a
//! `compact()`, an `include`, a `$$name`, a file's own top level.

use phrost_view::IndexView;

use super::{Check, Diagnostic, Severity};

pub(crate) const UNUSED_VARIABLE: &str = "unused-variable";
pub(crate) const UNUSED_MEMBER: &str = "unused-member";
pub(crate) const UNUSED_DECLARATION: &str = "unused-declaration";

pub(super) struct UnusedLocal;

impl Check for UnusedLocal {
    /// The only check here that reads the whole file: a variable is unused because nothing anywhere
    /// reads it, so text the parser skipped would read as nothing reading it.
    fn needs_a_whole_file(&self) -> bool {
        true
    }

    fn run(&self, view: &IndexView<'_>, out: &mut Vec<Diagnostic>) {
        for local in view.locals_never_read() {
            out.push(Diagnostic {
                range: local.range,
                // A warning rather than an error, and faded rather than underlined: nothing about
                // the program is wrong.
                severity: Severity::Warning,
                code: UNUSED_VARIABLE,
                message: format!("${} is assigned and never read", local.name),
                unnecessary: true,
                deprecated: false,
            });
        }
    }
}

/// A private member the class that declares it never uses.
///
/// The one kind of dead member decidable without a sweep, and the reason is `private` itself: it
/// reaches exactly as far as the declaring class, and a class is one file. The same question
/// about a public member is a project pass -- `phrost unused` -- because it needs every file.
///
/// Faded rather than underlined, like the local above it: the program is not wrong.
pub(super) struct UnusedPrivateMember;

impl Check for UnusedPrivateMember {
    /// Off unless a project asks for it, and the measurement is why rather than caution.
    ///
    /// Run over five real projects it finds **nothing** on four of them -- the Drupal reference,
    /// WordPress, Laravel and Slim -- and nineteen on Symfony, of which seventeen are test
    /// fixtures declaring a private member so that a component can prove reflection reaches it,
    /// and two are `CacheItem`'s, which a bound closure calls from another file. A rule that
    /// fires on nothing real does not belong in a default set that is deliberately two
    /// categories wide; a rule that is correct where it fires and costs nothing is worth having
    /// for the project whose code grows one.
    fn asked_for(&self) -> Option<&'static str> {
        Some(UNUSED_MEMBER)
    }

    /// Whole-file for the reason the local's check is: a member is unused because nothing anywhere
    /// in the file reaches it, so text the parser skipped would read as nothing reaching it.
    fn needs_a_whole_file(&self) -> bool {
        true
    }

    fn run(&self, view: &IndexView<'_>, out: &mut Vec<Diagnostic>) {
        for member in view.private_members_never_used() {
            let what = match member.kind {
                phrost_view::MemberKind::Method => "method",
                phrost_view::MemberKind::Property => "property",
                phrost_view::MemberKind::ClassConstant => "constant",
                phrost_view::MemberKind::EnumCase => continue,
            };
            out.push(Diagnostic {
                range: member.range,
                severity: Severity::Warning,
                code: UNUSED_MEMBER,
                message: format!(
                    "private {what} {}::{} is never used",
                    member.owner, member.name
                ),
                unnecessary: true,
                deprecated: false,
            });
        }
    }
}

/// A class or public method the project appears to reach from nowhere.
///
/// The editor's half of `phrost unused`, and it is here rather than only in that pass because the
/// question got cheap: whether a name is written in any other file is a word set away, where
/// which declaration that writing *reaches* is a sweep. The coarse test is the one that fits a
/// diagnostics budget, and coarse is the direction a report about absence has to be wrong in.
///
/// Off unless a project asks for it, like the private member above and for a plainer reason: a
/// framework reaches code in ways no file writes, and while the rules for that cover a plugin
/// manager, a test runner, a service container and a route, they cover them *for the frameworks
/// phrost knows*. A project on a framework it does not is owed the choice.
///
/// Measured on the reference project: 76 findings and **+20 ms over 1 726 files**, which is 12 us
/// a file against a thirty-millisecond budget. The one category worth knowing about before
/// turning it on is a getter a serializer calls by convention -- `getCreated` on an entity is
/// reached by a name derived from a property rather than written anywhere, which is the same
/// mechanism that keeps private *properties* out of the check above.
pub(super) struct UnusedDeclaration;

impl Check for UnusedDeclaration {
    fn asked_for(&self) -> Option<&'static str> {
        Some(UNUSED_DECLARATION)
    }

    /// Whole-file, because a declaration is unused when nothing anywhere writes its name and text
    /// the parser skipped would read as nothing writing it.
    fn needs_a_whole_file(&self) -> bool {
        true
    }

    fn run(&self, view: &IndexView<'_>, out: &mut Vec<Diagnostic>) {
        for found in view.declarations_never_reached() {
            let (what, name) = match (found.the_class_itself, found.kind) {
                (true, _) => ("class", found.name.as_str().to_owned()),
                (false, phrost_view::MemberKind::ClassConstant) => {
                    ("constant", format!("{}::{}", found.owner, found.name))
                }
                (false, _) => ("method", format!("{}::{}", found.owner, found.name)),
            };
            // The clause is the difference between a finding a reader can check and one they
            // cannot. Their test for "reached from nowhere" is a grep, and on a name like
            // `toArray` -- sixty-four classes declare one on the reference project -- the grep
            // comes back full of calls and the finding reads as a lie. Saying so is what puts the
            // claim back on the line it sits on: it is about this declaration, not about the word.
            // One clause or the other, never both. A contract's implementations *declare* the
            // name, so the word test is true of every contract by construction and says nothing
            // -- and read beside a finding it implies a rival call that usually is not there.
            // What the reader needs is which grep hits were already accounted for, and on a
            // contract that is the implementations.
            let checked = match (
                found.the_declaration_is_a_contract,
                found.the_name_is_written_elsewhere,
            ) {
                (true, _) => ", nor is any implementation of it",
                (false, true) => {
                    ", though the name is written elsewhere and nothing resolves to this one"
                }
                (false, false) => "",
            };
            out.push(Diagnostic {
                range: found.range,
                severity: Severity::Warning,
                code: UNUSED_DECLARATION,
                message: format!("{what} {name} is reached from nowhere in this project{checked}"),
                unnecessary: true,
                deprecated: false,
            });
        }
    }
}
