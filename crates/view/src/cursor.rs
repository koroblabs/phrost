use phrost_hir::{ImportKind, NameAnchor};
use phrost_model::{Name, RelativeTy, SymbolKind, TextRange};
use phrost_sema::MemberKind;

/// What the cursor is sitting on.
///
/// One enum rather than a method per feature: goto, hover, references, and rename all start by
/// asking the same question, and each deriving its own answer from raw ranges is how they end up
/// disagreeing about what the user clicked.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum CursorTarget {
    /// A written name: a type hint, an `extends`, a function call's callee, a bare constant.
    ///
    /// Unresolved on purpose — the anchor is carried so resolution can apply the file's imports, and
    /// the kind says which of PHP's three symbol tables to look in.
    WrittenName {
        path: Name,
        anchor: NameAnchor,
        kind: ImportKind,
        range: TextRange,
    },
    /// The path of a `use` import.
    ImportPath {
        path: Name,
        kind: ImportKind,
        range: TextRange,
    },
    /// The name of a top-level declaration in this file.
    Declaration {
        fqn: Name,
        kind: SymbolKind,
        range: TextRange,
    },
    /// A member named at a `->` or `::`, with the class the receiver turned out to hold.
    ///
    /// Resolved here rather than handed over unresolved: working out what `$this->repo` holds needs
    /// the catalog and member tables, which is exactly what a feature must not reach for itself.
    MemberAccess {
        owner: Name,
        name: Name,
        kind: MemberKind,
        range: TextRange,
    },
    /// A `self`, `static` or `parent` keyword written where a class goes, and the class it means.
    ///
    /// Its own variant rather than a [`Self::WrittenName`], because `self` is not a name the
    /// def-map can resolve: handing it to `resolve_name` would look for a class called "self".
    /// Resolved here for the same reason a member access is -- working out what `static` means
    /// needs to know where it was written, which is this layer's job.
    RelativeClass {
        relative: RelativeTy,
        class: Name,
        range: TextRange,
    },
    /// A string that a call reads as a key into a project catalog, and the class it names.
    ///
    /// `\Drupal::service('application.manager')` -- the string is not a name in PHP, so nothing else
    /// here would look at it, and it is the only thing at that position that says anything. Resolved,
    /// like a member access, because turning an id into a class needs the catalog.
    CatalogKey {
        /// The id as written. Not interned: it is a name configuration gave, not one PHP declares.
        key: Box<str>,
        class: Name,
        range: TextRange,
    },
    /// A string that a call reads as a key into a catalog whose keys name no class.
    ///
    /// Its own variant rather than a class-less [`Self::CatalogKey`], because there is nothing to
    /// resolve: a route name is declared in a `*.routing.yml`, which is not PHP and holds no symbol.
    /// The only answers are something to say and, usually, somewhere to go.
    CatalogPlace {
        key: Box<str>,
        /// A line describing it: a route's path, a permission's title. Empty when nothing does.
        detail: Box<str>,
        /// Where it is declared. `None` for a key the framework generates rather than reads:
        /// `entity.node.canonical` comes from the `canonical` link template of the `node` entity
        /// type, and the class carrying that template is not the route, so there is a description
        /// and nowhere honest to send a reader.
        place: Option<phrost_index::Place>,
        range: TextRange,
    },
    /// A name written in a docblock that names an extension point the project dispatches.
    ///
    /// Its own variant rather than a [`Self::WrittenName`], because there is nothing to resolve: no
    /// file declares the name, and handing it to the function table would look for a function that
    /// does not exist. What answers instead is the call that dispatches it -- see
    /// [`DispatchSite`](crate::DispatchSite), which is what the call *is* to a reader.
    DispatchedName {
        /// The name as the docblock wrote it. Not interned: nothing declares it, so it is not in
        /// any symbol table.
        name: Box<str>,
        range: TextRange,
    },
    /// The name of a member in its own declaration.
    MemberDeclaration {
        owner: Name,
        name: Name,
        kind: MemberKind,
        range: TextRange,
    },
}

impl CursorTarget {
    pub fn range(&self) -> TextRange {
        match self {
            Self::WrittenName { range, .. }
            | Self::ImportPath { range, .. }
            | Self::Declaration { range, .. }
            | Self::MemberAccess { range, .. }
            | Self::RelativeClass { range, .. }
            | Self::CatalogKey { range, .. }
            | Self::CatalogPlace { range, .. }
            | Self::DispatchedName { range, .. }
            | Self::MemberDeclaration { range, .. } => *range,
        }
    }

    pub fn label(&self) -> String {
        match self {
            Self::WrittenName {
                path, anchor, kind, ..
            } => format!("written name {path} ({anchor:?}) [{}]", kind.as_str()),
            Self::ImportPath { path, kind, .. } => format!("import {path} [{}]", kind.as_str()),
            Self::Declaration { fqn, kind, .. } => format!("declaration {} {fqn}", kind.as_str()),
            Self::MemberAccess {
                owner, name, kind, ..
            } => format!("member access {}::{name} [{}]", owner, kind.as_str()),
            Self::RelativeClass {
                relative, class, ..
            } => format!("{} is {class}", relative.as_str()),
            Self::MemberDeclaration {
                owner, name, kind, ..
            } => format!("member declaration {}::{name} [{}]", owner, kind.as_str()),
            Self::CatalogKey { key, class, .. } => format!("catalog key {key} is {class}"),
            Self::DispatchedName { name, .. } => format!("dispatched name {name}"),
            Self::CatalogPlace {
                key, detail, place, ..
            } => match place {
                Some(place) => format!(
                    "catalog key {key} is declared at {}:{} ({detail})",
                    place.line + 1,
                    place.character + 1,
                ),
                None => format!("catalog key {key} is registered ({detail})"),
            },
        }
    }
}
