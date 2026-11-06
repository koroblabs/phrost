//! Imports the file never names.
//!
//! A whole-file projection, and a precision problem rather than a coverage one: a wrong "unused
//! import" on real code teaches people to stop reading the list, so a *missed* usage is the failure
//! that matters and a missed *finding* is not. Every rule here is therefore written to fail towards
//! silence.
//!
//! # Why it is not built from name references
//!
//! [`name_references`](IndexView::name_references) merges written names and imports into one flat
//! list with no field saying which is which and no local name — an aliased `use App\Repo as Store;`
//! appears there as `App\Repo`, so the name a check has to underline is not in it. This reads the
//! two apart instead: the file's imports on one side, every written name on the other, and the
//! def-map's own resolution deciding which import each name reached.
//!
//! # Why the def-map decides
//!
//! Which `use` a written name is bound through is one of PHP's asymmetric rules — three separate
//! symbol tables, case-insensitive for two of them, and a *qualified* name whose first segment goes
//! through the class-like table whatever it is a name for. That rule already exists, in
//! [`NamespaceScope::import_of`](phrost_defmap::NamespaceScope::import_of), and is what every goto
//! and hover in the server resolves through. Asking it rather than re-deriving it is what keeps this
//! check and the rest of the server from disagreeing about what `Models\User` means.
//!
//! # What it refuses to answer about
//!
//! Names live in bodies — a `new`, an `instanceof`, a static call — so a file whose bodies were not
//! lowered knows almost nothing about its own imports, and a file that did not parse has lost every
//! statement after the break. Reporting from either would mark most of a file's imports unused. Both
//! return nothing instead.
//!
//! # Where it is still wrong
//!
//! Four constructs name an import in a place lowering does not record, so an import used only from
//! one of them reads as unused. Each is a false positive on real code, and
//! `a_used_import_is_still_invisible_from_these` in the tests holds the measured list:
//!
//! - the **first-class callable** syntax, `Repo::find(...)` and `render(...)` — mago models it as a
//!   closure creation rather than a call, and nothing lowers that node, so neither the class nor the
//!   function name exists in the HIR;
//! - an **anonymous class**'s `extends` and `implements`, `new class extends Base {}` — the class in
//!   a `new` is lowered as a target, and an anonymous one is an expression this layer has no shape
//!   for;
//! - **`@see`**, which names a class and is not one of the tags docblock lowering reads;
//! - a **vendor-prefixed docblock tag** — `@phpstan-param`, `@psalm-var` and the rest. The
//!   prefilters match on the bare tag, and `@phpstan-param` does not contain `@param`.
//!
//! Nothing here works around them. The recovery available at this layer would be a text scan of the
//! buffer for the local name, which is exactly the fuzzy fallback the index is built to refuse: it
//! would also match the name in a comment, in a string, and in an unrelated identifier that ends
//! with it. The fix belongs in lowering, where the names are.

use phrost_hir::{ImportId, ImportKind};
use phrost_model::{Name, TextRange};
use rustc_hash::FxHashSet;

use crate::view::IndexView;

/// One `use` statement nothing in the file names.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct UnusedImport {
    /// Range of the imported path, which is what a diagnostic underlines.
    ///
    /// The path and not the statement: `use App\{Repo, Model};` is two independent imports sharing
    /// one statement, so underlining the statement would mark the used half of it too. For a grouped
    /// entry this is the tail alone — the `Repo` between the braces.
    pub range: TextRange,
    /// The imported path as written, without a leading separator.
    pub path: Name,
    /// The name the import bound locally: the alias when the source wrote one, the last path segment
    /// otherwise.
    ///
    /// What the file would have had to write to use it, which is what a message should name. For
    /// `use App\Repo as Store;` that is `Store`, and saying `App\Repo` there tells a reader to look
    /// for a spelling the file does not contain.
    pub local: Name,
    /// Which of PHP's three symbol tables the import bound into.
    ///
    /// Carried because the three are independent: `use function App\render;` is unused in a file
    /// that writes `render` as a class name, and a message that did not say which table it means
    /// reads as simply wrong.
    pub kind: ImportKind,
}

impl IndexView<'_> {
    /// Imports in the current file that no name written in it is bound through.
    ///
    /// All three of PHP's symbol tables, because they neither collide nor substitute for one
    /// another: `use App\b;`, `use function App\b;` and `use const App\b;` are three imports, and a
    /// call names only the second. The rule that decides which one a written name reached is the
    /// def-map's, not this crate's.
    ///
    /// Empty when the file is not open, when its bodies were not lowered, or when it did not parse.
    /// That is the shape of the two tiers and of the precision rule rather than a fallback — see the
    /// module docs.
    pub fn unused_imports(&self) -> Vec<UnusedImport> {
        let Some(open) = self.current() else {
            return Vec::new();
        };
        let hir = open.hir();
        if hir.imports().len() == 0 {
            return Vec::new();
        }
        // Almost everything that names an import is written in a body, and a statement after a parse
        // break is dropped entirely. Either way most of the file's names are missing, and every
        // answer would be a guess in the one direction that costs a reader trust.
        if !hir.has_bodies() || !open.errors().is_empty() {
            return Vec::new();
        }

        let scope = open.scope();
        let mut used: FxHashSet<ImportId> = FxHashSet::default();

        for (_, reference) in hir.name_refs() {
            // By position, because a file with brace-delimited namespaces has one import table per
            // namespace and a name reaches only its own.
            let binding = scope.scope_at(reference.range.start()).import_of(
                reference.path.as_str(),
                reference.anchor,
                reference.kind,
            );
            if let Some(binding) = binding {
                used.insert(binding.import());
            }
        }

        let mut found = hir
            .imports()
            .filter(|(id, _)| !used.contains(id))
            .map(|(_, import)| UnusedImport {
                range: import.path_range,
                path: import.path.clone(),
                local: import.local.clone(),
                kind: import.kind,
            })
            .collect::<Vec<_>>();

        found.sort_by_key(|unused| unused.range.start());
        found
    }
}
