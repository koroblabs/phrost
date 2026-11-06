use phrost_model::{FileId, Name, SymbolKind, TextRange, Visibility};
use phrost_sema::{ClassKind, MemberKind};

/// Where a symbol is declared.
///
/// The range is the **name** alone, because that is what navigation should land on and what rename
/// has to rewrite. It comes from the skeleton, which keeps it for every declaration in every file
/// including vendor — which is what lets this be the one answer rather than the first of several
/// guesses.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SymbolLocation {
    pub file: FileId,
    /// Fully qualified, with no leading separator.
    pub fqn: Name,
    pub kind: SymbolKind,
    pub name_range: TextRange,
    /// The whole declaration, for an editor that wants to preview it.
    pub full_range: TextRange,
    /// The declaring class, for a member. `None` for a top-level symbol.
    pub container: Option<Name>,
    /// A member's declaration as text, rendered at ingest. `None` for a top-level symbol, whose
    /// header is described by [`ClassHeader`] instead — composing text is the feature layer's job,
    /// and the only reason a member's is pre-rendered is that the index stores it that way.
    pub signature: Option<Box<str>>,
    /// Visibility and modifiers, for a member.
    pub member_modifiers: Option<MemberModifiers>,
    /// Declared by an `@method` or `@property` tag rather than by PHP.
    ///
    /// Surfaced because it explains something otherwise baffling: goto on this member lands inside a
    /// comment. The member is real -- it answers at runtime -- but its declaration is a claim about
    /// generated code rather than code.
    pub from_docblock: bool,
    /// How a member reached the class that was asked about: inherited, from a trait, or its own.
    ///
    /// `None` for a top-level symbol, which has nowhere to have come from. Worth carrying because a
    /// method that looks like it belongs to a class may have arrived from three classes up, and
    /// that changes where a reader has to go to change it.
    pub origin: Option<Box<str>>,
    /// Where the declaration's docblock is, in [`Self::file`].
    ///
    /// The range and not the text: keeping docblock text for every member of every vendor class is
    /// the weight this design avoids, so the text is read on demand.
    pub docblock_range: Option<TextRange>,
    /// Whether the declaration says it is deprecated, by `@deprecated` or `#[\Deprecated]`.
    ///
    /// On every location rather than read from the docblock where one is rendered: an outline and a
    /// workspace-symbol list draw it struck through, and neither reads a docblock.
    pub deprecated: bool,
}

/// Visibility and modifiers of a member, for whoever renders it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct MemberModifiers {
    pub visibility: Visibility,
    pub set_visibility: Option<Visibility>,
    pub is_static: bool,
    pub is_abstract: bool,
    pub is_final: bool,
    pub is_readonly: bool,
}

impl SymbolLocation {
    /// The member kind, for a location that describes one.
    ///
    /// `SymbolKind` covers both top-level symbols and members; this narrows it, so a caller that
    /// only makes sense for a member does not have to match on kinds that cannot be one.
    pub fn member_kind(&self) -> Option<MemberKind> {
        match self.kind {
            SymbolKind::Method => Some(MemberKind::Method),
            SymbolKind::Property => Some(MemberKind::Property),
            SymbolKind::ClassConstant => Some(MemberKind::ClassConstant),
            SymbolKind::EnumCase => Some(MemberKind::EnumCase),
            _ => None,
        }
    }
}

/// What a class-like declaration says about itself.
///
/// Facts, not text. Composing a header a person reads is presentation, and presentation belongs to
/// the feature layer — a rendered string here would mean every consumer got one crate's idea of
/// formatting whether it suited them or not.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ClassHeader {
    pub fqn: Name,
    pub kind: ClassKind,
    /// Whether nothing can extend it.
    ///
    /// A bool beside the words below, and not read out of them: this one is asked as a *question* --
    /// can a subclass exist that satisfies something this class does not -- and answering it by
    /// searching a list meant for display would tie a judgement to a rendering.
    pub is_final: bool,
    /// PHP 8.2's `readonly class`, which makes every property readonly without saying so on any of
    /// them. A bool beside the words for the reason `is_final` is one: it decides a judgement about
    /// writes, and reading it out of a list meant for display would tie the two together.
    pub is_readonly: bool,
    /// Whether the declaration carries `#[Attribute]`, which is what makes a class usable as one.
    ///
    /// A bool beside the words for the reason `is_final` is one: it decides a *ranking* rather than
    /// a rendering, and reading it out of a list meant for display would tie the two together.
    pub is_attribute: bool,
    /// `abstract`, `final`, `readonly`, in the order PHP writes them.
    pub modifiers: Vec<&'static str>,
    pub extends: Vec<Name>,
    pub implements: Vec<Name>,
    pub uses: Vec<Name>,
    /// The interface the language gives the class without it being written: `UnitEnum` for an
    /// enum, `BackedEnum` for a backed one.
    ///
    /// Apart from `implements`, which is what the source says and what a card renders. The member
    /// table has merged these members since enums existed; the hierarchy did not know them, so
    /// `takes(\UnitEnum $case)` refused every enum there is -- Laravel's cache and queue APIs take
    /// them by the dozen.
    pub implied: Option<Name>,
}
