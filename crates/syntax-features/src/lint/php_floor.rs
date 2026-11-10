//! Which PHP a rule gated on a version is held to.
//!
//! mago builds only the rules its configured PHP version satisfies, and what it is configured
//! with when nothing says is 8.0. This crate said nothing, so every rule that needs 8.1 or later
//! was never built: six of the seven deprecation rules -- an implicitly nullable parameter (8.4),
//! `(integer)` and the other long casts, a `case` ended by a semicolon, the backtick operator
//! (all 8.5), a class named `_` (8.4), a reference returned from a `void` function (8.2) -- and
//! five more elsewhere. Nothing failed. A category whose whole point is what a newer PHP stops
//! accepting was one rule, and read as a project with nothing deprecated in it.
//!
//! One version cannot be right for all of them, because a version gate means one of two things:
//!
//! - **A deprecation is about the newest PHP**, which is where the code is going. A project that
//!   requires `^8.1` says it runs on 8.5 as well, so what 8.4 deprecates is its business now, and
//!   every fix these rules ask for (`?T` for an implicit null, `(int)` for `(integer)`, a colon
//!   after a `case`) runs on every 8.x. So the deprecation category is never held to the floor.
//! - **Anything else gated on a version is advice to write something that version added** -- a
//!   first-class callable (8.1), `0o17` (8.1), `DateTimeImmutable::createFromTimestamp` (8.4) --
//!   and that is only advice on a project whose *oldest* PHP has it. So those are held to the
//!   floor composer's `require.php` states, the same one the formatter writes for.
//!
//! And a deprecation is shown as a warning whatever its rule says: three of them report an error
//! for a line that runs on every PHP there is.
//!
//! Where a project states no floor nothing is held back: the other reading is that silence means
//! the oldest PHP there is, which is the reading that left these rules dead.
//!
//! So the linter is built for the newest PHP, where every rule exists to be asked, and the floor
//! decides afterwards which of the rest speak. Measured on four frameworks' own source with the
//! four factual categories: 18 implicitly nullable parameters appear in WordPress's bundled
//! sodium library, each a deprecation notice on PHP 8.4, and nothing else moves -- the other
//! three have already been cleaned for 8.4 and 8.5.

use mago_linter::requirements::RuleRequirements;
use mago_php_version::PHPVersion;
use phrost_model::PhpVersion;

use super::LintCategory;

/// The oldest PHP a project says it runs on, as the linter's rules are held to it.
#[derive(Clone, Copy, Debug)]
pub(super) struct PhpFloor(Option<PHPVersion>);

impl PhpFloor {
    /// The version the linter is built for: the newest there is, so that every rule gated on a
    /// version is there to be asked.
    pub(super) const NEWEST: PHPVersion = PHPVersion::LATEST;

    pub(super) fn of(php: Option<PhpVersion>) -> Self {
        Self(php.map(|php| PHPVersion::new(u32::from(php.major()), u32::from(php.minor()), 0)))
    }

    /// Whether a rule with these requirements has anything to say to this project.
    ///
    /// Every version range a rule names has to hold the floor, wherever in the requirement it is
    /// written: a rule that could run on either of two ranges is not a shape any rule has, and
    /// reading it strictly errs towards saying less.
    pub(super) fn admits(self, category: LintCategory, requirements: RuleRequirements) -> bool {
        if category == LintCategory::Deprecation {
            return true;
        }
        let Some(floor) = self.0 else {
            return true;
        };
        requirements
            .php_version_ranges()
            .iter()
            .all(|range| range.includes(floor))
    }
}
