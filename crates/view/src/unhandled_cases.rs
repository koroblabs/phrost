//! A `match` over an enum with no arm for some of its cases.
//!
//! PHP throws `UnhandledMatchError` there -- the one place it turns a missing case into a crash
//! rather than a default -- and over an enum it is decidable: the cases are declared, the arms name
//! them, and what is left over is what the value will fall through on. It is also the shape a
//! *new* case creates, which is when it matters: adding a case to an enum breaks every `match` over
//! it that has no `default`, and nothing else in a PHP toolchain says where those are.

use phrost_model::{Name, TextRange};
use phrost_sema::MemberKind;

use crate::{view::FOLLOW_LIMIT, view::IndexView};

/// One `match` that does not cover its subject's enum.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct UnhandledCases {
    /// The subject, which is what a report underlines.
    pub range: TextRange,
    /// The enum, fully qualified.
    pub enumeration: Name,
    /// The cases no arm names, in declaration order.
    pub missing: Vec<Name>,
}

impl IndexView<'_> {
    /// Every `match` in the current file that leaves a case of its subject's enum unhandled.
    pub fn unhandled_cases(&self) -> Vec<UnhandledCases> {
        let Some(open) = self.current() else {
            return Vec::new();
        };
        let hir = open.hir();

        phrost_body::match_sites(hir)
            .into_iter()
            .filter_map(|site| self.unhandled(hir, &site))
            .collect()
    }

    fn unhandled(
        &self,
        hir: &phrost_hir::FileHir,
        site: &phrost_body::MatchSite,
    ) -> Option<UnhandledCases> {
        let at = site.range.start();
        let held = self.resolve_path(at, hir, &site.subject, FOLLOW_LIMIT)?;
        // A value that may be null falls through the match for a reason of its own, which is the
        // null check's finding and not a missing case.
        if self.null_reach_of(hir, &held) != crate::references::SentinelReach::Impossible {
            return None;
        }
        let class = self.class_of_held(at, hir, &held)?;
        let header = self.class_header(class.as_str())?;
        if header.kind != phrost_sema::ClassKind::Enum {
            return None;
        }

        let table = self.members(class.as_str());
        let missing = table
            .members()
            .iter()
            .filter(|member| member.row.kind == MemberKind::EnumCase)
            .filter(|case| {
                !site.named.iter().any(|named| named == &case.name)
                    && !site.named_elsewhere.iter().any(|named| named == &case.name)
            })
            .map(|case| case.name.clone())
            .filter(|case| !self.backing_value_compared(class.as_str(), case, site))
            .collect::<Vec<_>>();

        (!missing.is_empty()).then_some(UnhandledCases {
            range: site.range,
            enumeration: header.fqn,
            missing,
        })
    }
}

impl IndexView<'_> {
    /// Whether the body compares a `->value` with the backing value of `case` -- the mention a case
    /// gets when it is ruled out by what it is backed by: `if ($suit->value === 'H') { return; }`.
    ///
    /// Read off the two texts, the literal's in this file and the case's initialiser in the enum's,
    /// with the quotes taken off both: `'H'` and `"H"` back the same case. Asked only for a case a
    /// `match` leaves out and only where the body compares a `->value` at all, so the declaration is
    /// read for the handful of sites that could be this.
    fn backing_value_compared(
        &self,
        enumeration: &str,
        case: &Name,
        site: &phrost_body::MatchSite,
    ) -> bool {
        if site.values_elsewhere.is_empty() {
            return false;
        }
        let Some(location) = self.member_location(enumeration, MemberKind::EnumCase, case.as_str())
        else {
            return false;
        };
        let range = TextRange::new(location.name_range.end(), location.full_range.end());
        let Some(declared) = self.source_between(location.file, range) else {
            return false;
        };
        let Some(backed) = declared
            .trim_start()
            .strip_prefix('=')
            .map(|rest| rest.trim().trim_end_matches(';').trim_end())
        else {
            return false;
        };
        let Some(text) = self.current().map(|open| open.text()) else {
            return false;
        };
        site.values_elsewhere.iter().any(|literal| {
            text.get(usize::from(literal.start())..usize::from(literal.end()))
                .is_some_and(|written| unquoted(written) == unquoted(backed))
        })
    }
}

/// A literal's text without the quotes around it, when it has a pair.
fn unquoted(text: &str) -> &str {
    let text = text.trim();
    for quote in ['\'', '"'] {
        if let Some(inner) = text
            .strip_prefix(quote)
            .and_then(|rest| rest.strip_suffix(quote))
        {
            return inner;
        }
    }
    text
}
