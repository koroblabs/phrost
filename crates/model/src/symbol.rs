/// What kind of PHP entity a symbol is.
///
/// Kept flat rather than nested (`ClassLike { Class, Interface, .. }`) because every consumer —
/// completion kinds, symbol listings, index shards — wants to switch on the leaf. `is_class_like`
/// covers the one grouping that matters for resolution.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum SymbolKind {
    Class,
    Interface,
    Trait,
    Enum,
    Function,
    Method,
    Property,
    ClassConstant,
    EnumCase,
    Constant,
    Namespace,
    Variable,
    Parameter,
    /// A key of a project catalog: a service id, an entity type -- a name configuration gave a class.
    ///
    /// Here for the same reason [`Self::Keyword`] is: this enum is what a language server *names*,
    /// and nothing declares one of these in PHP. It exists to be offered inside the string that
    /// accepts it, and to be drawn with an icon that is not a class's.
    CatalogKey,
    /// A file, which declares nothing and is still where some code is written.
    ///
    /// Here for one reader: "who calls this" has to name the thing a call was written *in*, and PHP
    /// lets a call be written at a file's top level or inside a closure the file returns -- a script,
    /// a bootstrap, Symfony's PHP-DSL configuration. There is no declaration around those, and
    /// dropping them made a caller list read as complete while it was not.
    File,
    /// A word of the language rather than a name in it.
    ///
    /// Here because this enum is what a language server *names* rather than what an index holds --
    /// `Variable` and `Parameter` are not indexed either. Nothing declares a keyword, so nothing
    /// looks one up; it exists to be offered, and to be drawn with the right icon when it is.
    Keyword,
}

impl SymbolKind {
    /// Whether this kind can appear in a type position and take part in inheritance.
    ///
    /// PHP resolves all four of these through the same case-insensitive class table, so name
    /// resolution treats them uniformly even though their members differ.
    pub fn is_class_like(self) -> bool {
        matches!(
            self,
            Self::Class | Self::Interface | Self::Trait | Self::Enum
        )
    }

    /// Whether PHP compares this kind's name case-insensitively.
    ///
    /// See the `phrost-intern` crate docs for why this split exists.
    pub fn is_case_insensitive(self) -> bool {
        match self {
            Self::Class
            | Self::Interface
            | Self::Trait
            | Self::Enum
            | Self::Function
            | Self::Method
            | Self::Namespace
            // PHP's own keywords are case-insensitive: `RETURN` is a return.
            | Self::Keyword => true,
            Self::Property
            | Self::ClassConstant
            | Self::EnumCase
            | Self::Constant
            | Self::Variable
            | Self::Parameter
            // A container answers for the id it was given and no other spelling of it.
            | Self::CatalogKey
            // A path, compared however the filesystem compares one, which is not this question.
            | Self::File => false,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Class => "class",
            Self::Interface => "interface",
            Self::Trait => "trait",
            Self::Enum => "enum",
            Self::Function => "function",
            Self::Method => "method",
            Self::Property => "property",
            Self::ClassConstant => "const",
            Self::EnumCase => "case",
            Self::Constant => "constant",
            Self::Namespace => "namespace",
            Self::Variable => "variable",
            Self::Parameter => "parameter",
            Self::Keyword => "keyword",
            Self::CatalogKey => "catalog key",
            Self::File => "file",
        }
    }
}
