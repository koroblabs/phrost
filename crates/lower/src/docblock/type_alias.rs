//! `@phpstan-type` / `@psalm-type`: a name a class's own docblocks may use in place of a type.
//!
//! Declared once on the class -- `@phpstan-type UserList array<int, User>` -- and used wherever a
//! member's own `@return` / `@param` / `@var` would otherwise write `UserList` out again. mago
//! already folds both vendor spellings (and the bare `@type`) into one [`TagValue::TypeAlias`], the
//! same way it folds `@phpstan-return` into [`TagValue::Return`] -- so there is no prefix to strip
//! here, unlike the hand-rolled hover renderer in `phrost-analysis`, which reads raw tag text mago
//! never sees.
//!
//! # `@phpstan-import-type` is read, and read as a refusal
//!
//! `@phpstan-import-type Name from OtherClass` names an alias declared on a *different* file's
//! class. Resolving it needs that class's own docblock, which lowering cannot reach -- lowering is
//! one file at a time, and the class it would have to read may not even have been parsed yet. That
//! is a cross-file index question, not a substitution one, and it stays unbuilt.
//!
//! What is built is the *refusal*, because "unimplemented" was not what happened. An imported name
//! that nothing resolves is not nothing: it is an ordinary class reference, so `@return Row` above a
//! method that hands back an array read as a class the project does not declare, and the return was
//! reported. The same trap a `@template` parameter sets, and the same answer -- see
//! [`Lowerer::doc_names_a_template`](crate::Lowerer). The name is recorded and every type written
//! with it settles nothing, which leaves the native hint standing and says nothing false.
//!
//! # The termination bound
//!
//! An alias's right-hand side is lowered against **the aliases already resolved above it**, and a
//! name is entered only once its own side is done. So a definition may name a sibling declared
//! earlier and nothing can loop: `@phpstan-type A A` looks itself up in a table it is not in yet,
//! and any cycle has to close through a name that is still resolving, which is the same miss.
//! Declaration order is the bound, and it needs no depth counter to hold.
//!
//! It used to be `TypeAliasTable::EMPTY` for every alias, on the ground that "every use of
//! `@phpstan-type` this shipped against writes a self-contained shape". Symfony's container
//! configurator is the counter-example the corpora did not cover: `ConfigType` is built out of
//! `ImportsConfig`, `ParametersConfig` and `ServicesConfig`, each its own `@psalm-type` above it.
//! What that cost is worse than a missing answer -- an unresolved alias name is not *nothing*, it
//! is an ordinary class reference, so `services?: ServicesConfig` read as a class nobody declares
//! and every array passed for it was reported. Six findings on one file, all wrong.

use mago_allocator::{Arena as _, LocalArena};
use mago_phpdoc_syntax::{
    PHPDocParser,
    cst::{Element, TagValue},
};
use phrost_hir::TypeRefId;
use phrost_model::{Name, TextSize};

use crate::lowerer::Lowerer;

/// The `@phpstan-type` / `@psalm-type` aliases one class-like declaration's own docblock names,
/// each already lowered to the type it stands for.
///
/// Resolved once per class rather than once per member: a class with a handful of documented
/// methods would otherwise re-parse and re-lower the same tag for each of them. Interned names and
/// [`TypeRefId`]s only -- nothing here holds a `String`.
#[derive(Default)]
pub(crate) struct TypeAliasTable {
    entries: Vec<(Name, TypeRefId)>,
    /// The names this class's docblock *imports* and cannot expand. See the module doc.
    imported: Vec<Name>,
}

impl TypeAliasTable {
    /// No aliases at all -- the overwhelming majority of classes, and the whole reason this is a
    /// `const` rather than always built: a call site with no class context (a free function, a
    /// `@throws`, a body-level `@var`) hands this in and pays nothing further.
    pub(crate) const EMPTY: Self = Self {
        entries: Vec::new(),
        imported: Vec::new(),
    };

    /// Names standing for types that are not aliases: a declaration's templates, each for its
    /// declared bound, where nothing reads the template off an argument and the bound is all that is
    /// known. See `Lowerer::bounded_branch`.
    pub(crate) fn standing_for(entries: Vec<(Name, TypeRefId)>) -> Self {
        Self {
            entries,
            imported: Vec::new(),
        }
    }

    /// This table with more names standing for types, which a lookup finds after the aliases.
    pub(crate) fn and_standing_for(&self, more: Vec<(Name, TypeRefId)>) -> Self {
        Self {
            entries: self.entries.iter().cloned().chain(more).collect(),
            imported: self.imported.clone(),
        }
    }

    /// The type `name` stands for, if it names one of this table's aliases.
    fn get(&self, name: &Name) -> Option<TypeRefId> {
        self.entries
            .iter()
            .find(|(candidate, _)| candidate == name)
            .map(|(_, ty)| *ty)
    }

    /// The type a written reference names, once vendor-prefixed spellings and shape have already
    /// been ruled out -- the one thing every caller in [`super::type_lowering`] needs, which is why
    /// this stays the only public entry point rather than exposing [`Self::get`] itself.
    ///
    /// Read-only: a name nothing has interned names no alias either, by construction -- every
    /// entry in this table was itself produced by interning the class's own `@phpstan-type` tag,
    /// so a member's reference to a real alias is always already in the table by the time this is
    /// asked.
    pub(crate) fn resolve(
        &self,
        written: &str,
        interner: &phrost_model::Interner,
    ) -> Option<TypeRefId> {
        let name = interner.lookup_name(written)?;
        self.get(&name)
    }

    /// Whether this name is one the class imported from elsewhere and nothing here can expand.
    ///
    /// A written name is compared, not an interned one: an imported alias that no member ever uses
    /// interns nothing, and asking the interner first would answer no for exactly the name that
    /// needs the answer.
    pub(crate) fn is_imported(&self, written: &str) -> bool {
        self.imported
            .iter()
            .any(|name| name.as_str().eq_ignore_ascii_case(written))
    }
}

impl Lowerer<'_, '_> {
    /// The aliases a class-like declaration's own docblock declares.
    ///
    /// Called once, from [`crate::members::Lowerer::class_members`] with the same
    /// `declaration_start` the class itself was lowered from -- so this always reads the class's
    /// docblock, never a member's, no matter which member is being lowered when the result is
    /// consulted.
    pub(crate) fn docblock_type_aliases(&mut self, declaration_start: TextSize) -> TypeAliasTable {
        let Some(docblock) = self.raw_docblock_before(declaration_start) else {
            return TypeAliasTable::EMPTY;
        };
        // Cheap prefilter, as every sibling here has: most classes declare no alias, and parsing is
        // the expensive part. Matches `@phpstan-type`, `@psalm-type`, the bare `@type`, and the
        // `-import-type` siblings this does not read (see the module doc) -- a prefilter that missed
        // one of those would make it invisible with nothing to notice.
        if !docblock.text.contains("type") {
            return TypeAliasTable::EMPTY;
        }

        let base = docblock.range.start();
        let text = docblock.text.to_owned();
        let arena = LocalArena::new();
        let content = arena.alloc_slice_copy(text.as_bytes());
        let document = PHPDocParser::parse(
            &arena,
            mago_database::file::FileId::new(b"docblock"),
            content,
        );

        // The imports first, wherever they are written: an alias may be built out of one --
        // `@phpstan-type Input string|MessagePartArrayShape` above or below the
        // `@phpstan-import-type MessagePartArrayShape from MessagePart` it names.
        let mut imported: Vec<Name> = Vec::new();
        for element in document.elements.iter() {
            let Element::Tag(tag) = element else {
                continue;
            };
            // Recorded, not expanded: the class it names is in another file. See the module doc for
            // what recording it buys, which is that a member writing the name settles nothing
            // instead of naming a class nobody declared.
            if let TagValue::TypeAliasImport(value) = &tag.value {
                // The local spelling: `@phpstan-import-type Row from Source as Line` is used as
                // `Line` in this file, and `Row` never appears in it at all.
                let local = value
                    .imported_as
                    .as_ref()
                    .map_or(value.imported_alias.value, |alias| alias.local.value);
                let written = std::str::from_utf8(local).unwrap_or_default();
                imported.push(self.interner.name(written));
            }
        }

        let mut entries: Vec<(Name, TypeRefId)> = Vec::new();
        for element in document.elements.iter() {
            let Element::Tag(tag) = element else {
                continue;
            };
            let TagValue::TypeAlias(value) = &tag.value else {
                continue;
            };

            let written = std::str::from_utf8(value.alias.value).unwrap_or_default();
            let name = self.interner.name(written);
            // First declaration of a name wins: nothing above this line has parsed yet, so it is
            // where a reader's eye would meet the name too.
            if entries.iter().any(|(existing, _)| existing == &name) {
                continue;
            }

            // Against what is resolved *so far*, and the name is entered only after -- which is
            // the whole of the termination bound, see the module doc.
            let resolved = TypeAliasTable {
                entries: std::mem::take(&mut entries),
                imported: std::mem::take(&mut imported),
            };
            let lowered = self.lower_doc_type(value.r#type, base, &text, &resolved);
            entries = resolved.entries;
            imported = resolved.imported;
            // One that lowers to nothing -- built out of an import, which nothing here expands --
            // is held the way the import is: a name that settles nothing, rather than a class.
            match lowered {
                Some(ty) => entries.push((name, ty)),
                None => imported.push(name),
            }
        }

        TypeAliasTable { entries, imported }
    }
}
