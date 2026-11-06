//! A method a test double is configured with, named by a string.
//!
//! `$repo->method('find')` and `$repo->expects($this->once())->method('find')` name `find` on
//! whatever `$repo` stands in for, and the double's type says what that is -- `createMock(Repo::class)`
//! is a `MockObject&Repo`. Read as nothing, the string was where a rename of `Repo::find` stopped:
//! the method moved and every test configuring it went on naming the old one, which PHPUnit only
//! reports when the test runs. 556 such strings in one real project's own tests.
//!
//! So each one is a member access on the double, presumed in the way `[Foo::class, 'bar']` is: kept
//! where the double's subject has the method and dropped where it does not, and from there goto,
//! hover, references and rename treat it as the `->find()` it configures.

use phrost_body::{MemberSite, PathStep, SiteKind};
use phrost_hir::FileHir;
use phrost_hir::body::{ExprData, LiteralKind};
use phrost_model::Name;

use crate::view::{FOLLOW_LIMIT, IndexView};

impl IndexView<'_> {
    /// The site a double's configuring call names under the cursor, when the cursor is in one.
    ///
    /// Only inside a string, which is the one place such a site can be: everywhere else this costs
    /// nothing.
    pub(crate) fn stand_in_site_at(
        &self,
        hir: &FileHir,
        offset: phrost_model::TextSize,
    ) -> Option<MemberSite> {
        hir.string_at(offset)?;
        self.stand_in_sites(hir, &phrost_body::member_sites(hir))
            .into_iter()
            .find(|site| site.name_range.contains_inclusive(offset))
    }

    /// The classes a double stands in for, when the cursor is in the string one of its configuring
    /// calls is passed -- what completion there offers the methods of.
    ///
    /// Its own parts, without the double's own type: `expects()` and `method()` are what a mock
    /// answers itself, not names it can be configured with.
    pub fn stand_in_subjects_at(&self, offset: phrost_model::TextSize) -> Option<Vec<Name>> {
        let open = self.current()?;
        let hir = open.hir();
        let quoted = hir.string_at(offset)?;
        // What stands before the opening quote, cheaply first: a completion inside any string of any
        // file asks, and only one written as `method(` -- the first argument of a configuring call --
        // can be this. The walk below is over every member site in the file.
        let before = open.text().get(..usize::from(quoted.start()))?.trim_end();
        let called = before.strip_suffix('(')?.trim_end();
        let word = &called[called
            .rfind(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
            .map_or(0, |at| at + 1)..];
        if !self
            .skeleton()
            .stand_in_methods()
            .iter()
            .any(|rule| rule.at == 0 && word.eq_ignore_ascii_case(&rule.method))
        {
            return None;
        }
        let configured = self
            .configured(hir, &phrost_body::member_sites(hir))
            .into_iter()
            .find(|configured| configured.inside.contains_inclusive(offset))?;
        let subjects = configured
            .parts
            .into_iter()
            .filter(|part| !self.is_a_stand_in(part, &configured.stand_in))
            .collect::<Vec<_>>();
        (!subjects.is_empty()).then_some(subjects)
    }

    pub(crate) fn stand_in_sites(&self, hir: &FileHir, sites: &[MemberSite]) -> Vec<MemberSite> {
        self.configured(hir, sites)
            .into_iter()
            .filter_map(|configured| {
                Some(MemberSite {
                    // Read rather than interned: a request writes nothing, and a method any class
                    // declares is a name the index already holds. One it does not hold is one no
                    // subject has, which is the site a presumed access drops anyway.
                    name: self
                        .skeleton()
                        .interner()
                        .lookup_name(&configured.written)?,
                    receiver: configured.receiver,
                    kind: SiteKind::Method,
                    name_range: configured.inside,
                    receiver_range: configured.receiver_range,
                    nullsafe: false,
                    presumed: true,
                    named: true,
                })
            })
            .collect()
    }

    /// Every string a double's configuring call is passed, with the double it was passed to.
    ///
    /// Whatever the string says: what it names is the reader's question, and completion asks it of a
    /// string that is still being typed and names nothing yet.
    fn configured(&self, hir: &FileHir, sites: &[MemberSite]) -> Vec<Configured> {
        let rules = self.skeleton().stand_in_methods();
        if rules.is_empty() {
            return Vec::new();
        }
        let configuring = sites
            .iter()
            .filter(|site| matches!(site.kind, SiteKind::Method))
            .filter_map(|site| {
                let rule = rules
                    .iter()
                    .find(|rule| site.name.as_str().eq_ignore_ascii_case(&rule.method))?;
                Some((site, rule))
            })
            .collect::<Vec<_>>();
        // Nearly every file: the name is the prefilter, and only a test file writes `method(`.
        if configuring.is_empty() {
            return Vec::new();
        }

        let calls = self.file_calls();
        configuring
            .into_iter()
            .filter_map(|(site, rule)| {
                let call = calls
                    .iter()
                    .find(|call| call.callee_range == site.name_range)?;
                let argument = call
                    .arguments
                    .get(usize::from(rule.at))
                    .filter(|argument| argument.name.is_none() && !argument.spread)?;
                let ExprData::Literal {
                    kind: LiteralKind::String,
                    range,
                } = hir.expr(argument.value)
                else {
                    return None;
                };
                let inside = self.inside_the_quotes(*range)?;
                let written = self
                    .current()?
                    .text()
                    .get(usize::from(inside.start())..usize::from(inside.end()))?
                    .to_owned();

                // The double itself: the receiver, or the receiver of a builder call made on it.
                let mut receiver = site.receiver.clone();
                if let Some(PathStep::MethodReturn { name, .. }) = receiver.steps.last()
                    && rule
                        .through
                        .iter()
                        .any(|through| name.as_str().eq_ignore_ascii_case(through))
                {
                    receiver.steps.pop();
                }
                let at = site.name_range.start();
                let held = self.resolve_path(at, hir, &receiver, FOLLOW_LIMIT)?;
                let parts = match self.intersection_of_held(at, hir, &held) {
                    parts if !parts.is_empty() => parts,
                    _ => vec![self.class_of_held(at, hir, &held)?],
                };
                if !parts
                    .iter()
                    .any(|part| self.is_a_stand_in(part, &rule.stand_in))
                {
                    return None;
                }

                Some(Configured {
                    receiver,
                    receiver_range: site.receiver_range,
                    inside,
                    written,
                    parts,
                    stand_in: rule.stand_in.clone(),
                })
            })
            .collect()
    }

    fn is_a_stand_in(&self, class: &Name, stand_in: &str) -> bool {
        class.as_str().eq_ignore_ascii_case(stand_in)
            || self.descends_from(class.as_str(), stand_in, 8)
    }
}

/// One string a double's configuring call is passed.
struct Configured {
    /// The double.
    receiver: phrost_body::ValuePath,
    receiver_range: phrost_model::TextRange,
    /// The string's contents, without its quotes.
    inside: phrost_model::TextRange,
    written: String,
    /// The double's type, operand by operand.
    parts: Vec<Name>,
    /// What made it a double.
    stand_in: Box<str>,
}
