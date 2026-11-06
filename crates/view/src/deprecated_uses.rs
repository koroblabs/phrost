//! Reaching for something its own declaration says not to.
//!
//! `@deprecated` and `#[\Deprecated]` are a claim about a declaration; this is the question a
//! *reader* has, which is whether the file in front of them uses one. The bit is on the row already
//! -- see the skeleton -- so what is left is finding the uses, and every walk that needs doing is
//! one some other feature already does: member accesses for a method, a property, a constant or a
//! case; class-like name references for a `new`, a hint, an `extends`; call sites for a free
//! function.
//!
//! A deprecated declaration may use deprecated things. That is what a shim *is* -- a wrapper kept
//! around for one release, calling the thing it replaced -- so a use written inside a deprecated
//! class or member is not reported. Computed as ranges once per file rather than asked per site,
//! which is the split §8.1 names: the tree settles which spans are sheltered and the position
//! settles which span it is in.

use phrost_model::{Name, SymbolKind, TextRange};
use phrost_sema::{MemberKind, MemberProvenance};

use crate::{IndexView, references::MemberAccessOutcome};

/// One use of a deprecated declaration.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DeprecatedUse {
    /// The name as written, which is what a report underlines.
    pub range: TextRange,
    /// What was reached, spelled as a reader would write it: `App\Repo::find`, `App\Repo`,
    /// `App\helper`.
    pub named: String,
    /// What kind of declaration it is, for a message that reads as PHP rather than as a category.
    pub kind: DeprecatedKind,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DeprecatedKind {
    Class,
    Method,
    Property,
    Constant,
    Case,
    Function,
}

impl DeprecatedKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Class => "class",
            Self::Method => "method",
            Self::Property => "property",
            Self::Constant => "constant",
            Self::Case => "case",
            Self::Function => "function",
        }
    }

    fn of_member(kind: MemberKind) -> Self {
        match kind {
            MemberKind::Method => Self::Method,
            MemberKind::Property => Self::Property,
            MemberKind::ClassConstant => Self::Constant,
            MemberKind::EnumCase => Self::Case,
        }
    }
}

impl IndexView<'_> {
    /// Every use in the current file of a declaration marked deprecated.
    ///
    /// In source order, because a reader goes down the file. Empty when nothing is open, which is
    /// the shape of the two tiers rather than a fallback: a use lives in a body.
    pub fn deprecated_uses(&self) -> Vec<DeprecatedUse> {
        let Some(open) = self.current() else {
            return Vec::new();
        };
        let hir = open.hir();
        let sheltered = self.sheltered_spans();
        let classes = self.class_spans();
        let mut out = Vec::new();

        for access in self.member_accesses().iter() {
            let MemberAccessOutcome::Resolved { class } = &access.outcome else {
                continue;
            };
            if Self::is_inside(&sheltered, access.range) {
                continue;
            }
            let table = self.members(class.as_str());
            let Some(found) = table.find(access.kind, access.name.as_str()) else {
                continue;
            };
            if !found.row.modifiers.is_deprecated() {
                continue;
            }
            // The class that *declares* it, which is whose claim this is -- the same rule the
            // discarded-result check follows for the attribute it reads.
            let owner = match &found.provenance {
                MemberProvenance::Own | MemberProvenance::Trait(_) => class.clone(),
                MemberProvenance::Inherited(fqn) | MemberProvenance::Interface(fqn) => fqn.clone(),
            };
            // A class keeping its own deprecated member working is the shim, not a caller of one:
            // Laravel's `OutputStyle::write()` assigns `$this->newLineWritten` on every line it
            // prints, which is how the property is still true for whoever reads it. Reporting that
            // tells the maintainer what they wrote and buries the uses that are somebody else's.
            if Self::innermost_class(&classes, access.range).is_some_and(|here| here == &owner) {
                continue;
            }
            out.push(DeprecatedUse {
                range: access.range,
                named: format!("{owner}::{}", access.name),
                kind: DeprecatedKind::of_member(access.kind),
            });
        }

        let declared_here = self.declaration_spans();
        for reference in self.name_references() {
            let Some(resolved) = reference.resolved.as_ref() else {
                continue;
            };
            // A docblock is prose about the code and not code. `{@see ClientConfiguration}` in the
            // class that replaced it is the sentence explaining the deprecation, and reporting it
            // reports the fix as the problem -- three of psl's fifty findings were that.
            if reference.in_docblock || Self::is_inside(&sheltered, reference.range) {
                continue;
            }
            // A name written at the file's own top level, where what declares it is also here: the
            // shape is a file announcing its own deprecation, which Symfony writes as
            // `trigger_deprecation('symfony/...', '8.2', '...', Middleware::class, ...)` above the
            // class. Only the top level, so a *use* by a neighbouring declaration in the same file
            // is still a use.
            if !Self::is_inside(&declared_here, reference.range)
                && self
                    .class_location(resolved.as_str())
                    .is_some_and(|found| Some(found.file) == self.current_file())
            {
                continue;
            }
            if !self.class_is_deprecated(resolved.as_str()) {
                continue;
            }
            out.push(DeprecatedUse {
                range: reference.range,
                named: resolved.as_str().to_owned(),
                kind: DeprecatedKind::Class,
            });
        }

        for call in phrost_body::call_sites(hir) {
            let phrost_body::CallTarget::Function(reference) = &call.target else {
                continue;
            };
            if Self::is_inside(&sheltered, call.callee_range) {
                continue;
            }
            let written = hir.name_ref(*reference);
            let Some(location) = self.resolve_name(
                call.callee_range.start(),
                written.path.as_str(),
                written.anchor,
                phrost_hir::ImportKind::Function,
            ) else {
                continue;
            };
            if !self.function_is_deprecated(location.fqn.as_str()) {
                continue;
            }
            out.push(DeprecatedUse {
                range: call.callee_range,
                named: location.fqn.as_str().to_owned(),
                kind: DeprecatedKind::Function,
            });
        }

        out.sort_by_key(|use_of| use_of.range.start());
        out
    }

    /// The spans of this file's own deprecated declarations, inside which a use says nothing.
    ///
    /// Both levels, because they shelter different things: a deprecated class shelters its whole
    /// body, and a deprecated method inside a current class shelters only itself.
    fn sheltered_spans(&self) -> Vec<TextRange> {
        let Some(file) = self.current_file().and_then(|id| self.skeleton.file(id)) else {
            return Vec::new();
        };
        let mut out = file
            .scope()
            .symbols()
            .iter()
            .filter(|symbol| symbol.is_deprecated())
            .map(|symbol| symbol.full_range)
            .collect::<Vec<_>>();

        out.extend(file.classes().iter().flat_map(|class| {
            class
                .members()
                .iter()
                .filter(|member| member.modifiers.is_deprecated())
                .map(|member| member.full_range)
        }));
        out
    }

    /// Every class-like this file declares, with the span it covers.
    ///
    /// For the neighbourhood rule: a use written inside the class that declares what it names is
    /// the declaration's own business. Read once per file rather than asked per site, which is the
    /// split §8.1 names -- the tree settles the spans and the position settles which one it is in.
    fn class_spans(&self) -> Vec<(TextRange, Name)> {
        self.current_file()
            .and_then(|id| self.skeleton.file(id))
            .map(|file| {
                file.scope()
                    .symbols()
                    .iter()
                    .filter(|symbol| {
                        matches!(
                            symbol.kind,
                            SymbolKind::Class
                                | SymbolKind::Interface
                                | SymbolKind::Trait
                                | SymbolKind::Enum
                        )
                    })
                    .map(|symbol| (symbol.full_range, symbol.fqn.clone()))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// The narrowest class-like span holding `range`, for the neighbourhood rule.
    fn innermost_class(classes: &[(TextRange, Name)], range: TextRange) -> Option<&Name> {
        classes
            .iter()
            .filter(|(span, _)| span.contains_range(range))
            .min_by_key(|(span, _)| span.len())
            .map(|(_, fqn)| fqn)
    }

    /// Every declaration this file holds, so a name written between them can be told apart from
    /// one written inside one.
    fn declaration_spans(&self) -> Vec<TextRange> {
        self.current_file()
            .and_then(|id| self.skeleton.file(id))
            .map(|file| {
                file.scope()
                    .symbols()
                    .iter()
                    .map(|symbol| symbol.full_range)
                    .collect()
            })
            .unwrap_or_default()
    }

    fn is_inside(spans: &[TextRange], range: TextRange) -> bool {
        spans.iter().any(|span| span.contains_range(range))
    }

    /// Whether a class-like declaration says it is deprecated.
    pub(crate) fn class_is_deprecated(&self, fqn: &str) -> bool {
        let Some(location) = self.class_location(fqn) else {
            return false;
        };
        self.skeleton
            .file(location.file)
            .and_then(|file| {
                file.classes()
                    .iter()
                    .find(|class| class.fqn() == &location.fqn)
            })
            .is_some_and(|class| class.is_deprecated())
    }

    /// Whether a free function says it is deprecated.
    pub(crate) fn function_is_deprecated(&self, fqn: &str) -> bool {
        let Some(location) = self.function_location(fqn) else {
            return false;
        };
        self.skeleton
            .file(location.file)
            .and_then(|file| {
                file.scope()
                    .symbols()
                    .iter()
                    .position(|symbol| symbol.fqn == location.fqn)
                    .and_then(|index| file.function_of(u32::try_from(index).ok()?))
            })
            .is_some_and(|function| function.is_deprecated())
    }
}
