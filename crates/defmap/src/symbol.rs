use phrost_hir::{ClassId, ConstantId, FunctionId};
use phrost_model::{Name, SymbolKind, TextRange};

/// One symbol a file declares, with the fully-qualified name it declares it under.
///
/// The FQN is computed once here rather than re-derived from namespace plus name at every lookup:
/// it is the key everything cross-file is organized by, and joining strings on a hot path to
/// rediscover a fact that cannot change is pure waste.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DeclaredSymbol {
    /// Fully qualified, with no leading separator: `App\Repo`, or `Repo` in the global namespace.
    pub fqn: Name,
    pub kind: SymbolKind,
    pub source: DeclarationSource,
    /// What the declaration says about itself, one bit each.
    ///
    /// Packed rather than a field per word, for the reason `phrost_hir::MemberModifiers` gives
    /// about member rows: a fourth bool grew this struct by a whole word, which on one real
    /// checkout is 0.3 MB of symbols to record one bit. The bits are read through methods.
    pub words: SymbolWords,
    /// Range of the name alone — the goto target, and the range that has to survive being cached
    /// and reloaded, because there is no text scan to fall back on.
    pub name_range: TextRange,
    pub full_range: TextRange,
    /// Where the declaration's docblock is, if there is one. The range, not the text — see
    /// [`phrost_sema::MemberRow::docblock_range`] for why.
    pub docblock_range: Option<TextRange>,
}

/// The four facts a declaration states about itself, packed into one byte.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct SymbolWords(u8);

impl SymbolWords {
    /// Whether the name can be written in PHP source.
    ///
    /// False for an anonymous class, whose name is synthesised from where its `class` keyword
    /// sits. It is a declaration like any other -- `$this`, `self`, `static` and `parent` inside
    /// its body all name it, and every member reached through them needs the catalog to hold it --
    /// but nobody can type that name, so a list of names to offer must leave it out.
    const NAMEABLE: u8 = 1 << 0;
    /// Whether PHP reaches the declaration only when the code around it runs.
    ///
    /// Asked of a *declarer* -- a file and a symbol index, which is what the catalog hands out and
    /// what ranking sorts -- because a `class_exists`-guarded class is the same fact as a
    /// `function_exists`-guarded function. See [`phrost_hir::ClassData::is_conditional`] for what
    /// sets it, and `ProjectSkeleton::rank` for what it decides.
    const CONDITIONAL: u8 = 1 << 1;
    /// The declaration is marked `@ignore`. Loses a tie between two declarers of one name and does
    /// nothing where there is only one -- see [`phrost_hir::ClassData::is_ignored`].
    const IGNORED: u8 = 1 << 2;
    /// The declaration says it is deprecated, by `@deprecated` or `#[\Deprecated]`.
    ///
    /// Here rather than looked up per candidate: every completion row is drawn from this table and
    /// an editor strikes a deprecated row through, so the answer has to arrive with the row rather
    /// than when one is highlighted.
    const DEPRECATED: u8 = 1 << 3;

    pub fn new(nameable: bool, conditional: bool, ignored: bool, deprecated: bool) -> Self {
        let mut words = Self::default();
        words.set(Self::NAMEABLE, nameable);
        words.set(Self::CONDITIONAL, conditional);
        words.set(Self::IGNORED, ignored);
        words.set(Self::DEPRECATED, deprecated);
        words
    }

    /// The bits as one number, for a codec. Paired with [`Self::from_bits`], which is the only way
    /// back in -- so an unused bit cannot arrive from a cache as a word nothing declared.
    pub fn bits(self) -> u8 {
        self.0
    }

    pub fn from_bits(words: u8) -> Self {
        Self(words & (Self::NAMEABLE | Self::CONDITIONAL | Self::IGNORED | Self::DEPRECATED))
    }

    fn set(&mut self, bit: u8, on: bool) {
        if on {
            self.0 |= bit;
        } else {
            self.0 &= !bit;
        }
    }
}

impl DeclaredSymbol {
    pub fn is_nameable(&self) -> bool {
        self.words.0 & SymbolWords::NAMEABLE != 0
    }

    pub fn is_conditional(&self) -> bool {
        self.words.0 & SymbolWords::CONDITIONAL != 0
    }

    pub fn is_ignored(&self) -> bool {
        self.words.0 & SymbolWords::IGNORED != 0
    }

    pub fn is_deprecated(&self) -> bool {
        self.words.0 & SymbolWords::DEPRECATED != 0
    }
}

/// Which HIR entity a symbol came from.
///
/// Carried so a later layer can go from "the catalog says `App\Repo` lives here" to that class's
/// members without searching the file again.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DeclarationSource {
    Class(ClassId),
    Function(FunctionId),
    Constant(ConstantId),
}
