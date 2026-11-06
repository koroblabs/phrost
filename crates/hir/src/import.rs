use phrost_model::{Name, TextRange};

/// One `namespace` declaration in a file.
///
/// A file can hold several: brace-delimited blocks, or a single implicit one covering the rest of
/// the file. Declarations record which namespace they were in, so the def-map can resolve the same
/// short name differently in two halves of one file.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct NamespaceData {
    /// `None` is the global namespace, either declared as `namespace { }` or simply absent.
    pub name: Option<Name>,
    /// Range of the name, or of the `namespace` keyword when there is no name.
    pub name_range: TextRange,
    /// Everything this namespace covers, which is what decides membership for a position.
    pub full_range: TextRange,
}

/// One `use` import, after group imports are flattened.
///
/// `use App\{A, B as C};` becomes two of these. Flattening at lowering time means every consumer
/// sees one shape, instead of each one re-deriving what a group import means.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ImportData {
    /// The imported path as written, without a leading separator. Always absolute in PHP: a `use`
    /// statement is never relative to the current namespace.
    pub path: Name,
    /// The name this is bound to locally — the alias if given, otherwise the last path segment.
    /// Pre-computed because every lookup wants it and re-splitting the path is pure waste.
    pub local: Name,
    /// `Some` only when the source wrote `as`. Distinguishing an explicit alias from an implicit
    /// one matters for rename, which must not rewrite a name the author never typed.
    pub alias: Option<Name>,
    pub kind: ImportKind,
    pub namespace: Option<crate::ids::NamespaceId>,
    /// Range of the imported path, which is what goto on a `use` line targets.
    pub path_range: TextRange,
    /// Range of the alias token, when the source wrote one.
    ///
    /// Its own range because `full_range` cannot stand in: a grouped import shares one of those
    /// across every item in the braces, so it says nothing about where any single alias is. Without
    /// this, a cursor on the `Y` of `use X as Y;` matched nothing and goto answered nothing --
    /// while the same cursor one word to the left worked.
    pub alias_range: Option<TextRange>,
    pub full_range: TextRange,
}

/// Which symbol table an import binds into.
///
/// PHP keeps three separate ones, so `use App\render;` and `use function App\render;` do not
/// collide and cannot substitute for each other.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum ImportKind {
    /// `use App\Repo;` — classes, interfaces, traits, and enums share one table.
    ClassLike,
    /// `use function App\render;`
    Function,
    /// `use const App\MAX;`
    Constant,
}

impl ImportKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ClassLike => "class",
            Self::Function => "function",
            Self::Constant => "const",
        }
    }
}
