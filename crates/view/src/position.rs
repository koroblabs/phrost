//! What is under the cursor, and what it refers to.
//!
//! One question with two answers: the target, which is lexical, and the declaration it resolves to,
//! which is not. Several features start from the first and only some need the second, so they are
//! separate — and both live here so a new one cannot answer them differently.

use phrost_body::SiteKind;
use phrost_defmap::DeclarationSource;
use phrost_hir::ImportKind;
use phrost_model::{SymbolKind, TextRange, TextSize};
use phrost_sema::MemberKind;

use crate::{
    cursor::CursorTarget,
    location::SymbolLocation,
    view::{FOLLOW_LIMIT, IndexView, hir_member_kind},
};

impl<'a> IndexView<'a> {
    /// What the cursor is on.
    ///
    /// When several things contain the offset, the narrowest wins. That is both the right answer —
    /// the innermost thing is what the user clicked — and a deterministic one, which matters because
    /// several features start from this.
    pub fn cursor_target(&self, offset: TextSize) -> Option<CursorTarget> {
        let open = self.current()?;
        let hir = open.hir();
        let mut best: Option<CursorTarget> = None;

        let mut consider = |candidate: CursorTarget| {
            let narrower = best
                .as_ref()
                .is_none_or(|current| candidate.range().len() < current.range().len());
            if narrower {
                best = Some(candidate);
            }
        };

        // Before the name walk, because a string holds no names and this is the only reader that
        // looks inside one. It is a whole-literal question rather than a per-range one, so it cannot
        // be folded into the loops below.
        if let Some(key) = self.catalog_key_at(offset) {
            consider(key);
        }
        // A key the enclosing class's builder wrote: a form element read back from the form state.
        if let Some(key) = self.built_key_at(offset) {
            consider(key);
        }
        // An input a command's signature declares, read back by its name.
        if let Some(input) = self.declared_input_at(offset) {
            consider(input);
        }

        // The same place asked a different question: a string that names a class the index holds.
        // After the catalog, because a catalog key is the more specific reading of the same text --
        // a service id resolves to the class that implements it, and answering with the *name* of
        // a class that happens to be spelled like one would be the wider guess.
        if let Some(named) = self.class_named_by_a_string_at(offset) {
            consider(named);
        }

        // And the third question a string can answer: a method of the class it is written in,
        // where a framework says that spelling means one. See `own_method_prefixes`.
        if let Some(named) = self.own_method_named_by_a_string_at(offset) {
            consider(named);
        }

        // And a callable written as data: `[$formatter, 'format']`, `'App\Repo::find'`,
        // `'App\render'` -- each only where the index holds what it names.
        if let Some(named) = self.callable_named_by_a_string_at(offset) {
            consider(named);
        }

        // A name written in a *comment*, which no name reference covers and nothing else here would
        // look at. It is an ordinary written name once found -- a function the index holds -- so it
        // arrives as one, and goto, hover and references answer about it without knowing where it was
        // written.
        if let Some((path, range)) = self.documented_name_at(offset) {
            consider(CursorTarget::WrittenName {
                path,
                anchor: phrost_hir::NameAnchor::Relative,
                kind: ImportKind::Function,
                range,
            });
        // The same name, when nothing declares it and a call in the project dispatches it. Two
        // arms rather than one target that carries either, because the two are answered from
        // different tables and only one of them is a symbol -- and disjoint, because a name the
        // index holds a declaration for is not one this asks about.
        } else if let Some((name, range)) = self.dispatched_name_at(offset) {
            consider(CursorTarget::DispatchedName { name, range });
        }

        for (_, name_ref) in hir.name_refs() {
            if name_ref.range.contains_inclusive(offset) {
                consider(CursorTarget::WrittenName {
                    path: name_ref.path.clone(),
                    anchor: name_ref.anchor,
                    kind: name_ref.kind,
                    range: name_ref.range,
                });
            }
        }

        for (_, import) in hir.imports() {
            // The alias answers the same question as the path -- `use X as Y;` puts the cursor on
            // the local name for what `X` names -- so it is the same target with its own range,
            // which is what keeps narrowest-wins meaning something.
            for range in [Some(import.path_range), import.alias_range]
                .into_iter()
                .flatten()
            {
                if range.contains_inclusive(offset) {
                    consider(CursorTarget::ImportPath {
                        path: import.path.clone(),
                        kind: import.kind,
                        range,
                    });
                }
            }
        }

        for symbol in open.scope().symbols() {
            if symbol.name_range.contains_inclusive(offset) {
                consider(CursorTarget::Declaration {
                    fqn: symbol.fqn.clone(),
                    kind: symbol.kind,
                    range: symbol.name_range,
                });
            }
        }

        // A member named at a `->` or `::`. Resolved here, because the answer needs the catalog.
        // Or one a test double's configuring call names in a string -- see `stand_ins`.
        if let Some(site) =
            phrost_body::member_site_at(hir, offset).or_else(|| self.stand_in_site_at(hir, offset))
        {
            let kind = match site.kind {
                SiteKind::Property => MemberKind::Property,
                SiteKind::Method => MemberKind::Method,
                SiteKind::ClassConstant => MemberKind::ClassConstant,
            };
            if let Some(owner) = self
                .resolve_path(offset, hir, &site.receiver, FOLLOW_LIMIT)
                .and_then(|held| {
                    self.owner_at_site(offset, hir, &site.receiver, &held, kind, site.name.as_str())
                })
            {
                consider(CursorTarget::MemberAccess {
                    owner,
                    name: site.name,
                    kind,
                    range: site.name_range,
                });
            }
        }

        // `self`, `static`, `parent` written where a class goes. Resolved here for the same reason
        // a member access is: what `static` means depends on where it was written.
        //
        // Guarded by the word under the cursor first. Finding one needs a walk of the whole file,
        // and on every other position in the file that walk would be pure cost -- three string
        // comparisons buy it back.
        if spells_a_relative_class(open.text(), offset)
            && let Some((relative, range)) = phrost_body::relative_class_at(hir, offset)
        {
            let class = match relative {
                phrost_model::RelativeTy::SelfTy | phrost_model::RelativeTy::Static => {
                    self.enclosing_class(offset)
                }
                phrost_model::RelativeTy::Parent => self.parent_of_enclosing_class(offset),
            };
            if let Some(class) = class {
                consider(CursorTarget::RelativeClass {
                    relative,
                    class,
                    range,
                });
            }
        }

        for (class_id, class) in hir.classes() {
            let owner = open
                .scope()
                .symbol_of(DeclarationSource::Class(class_id))
                .map_or_else(|| class.name.clone(), |symbol| symbol.fqn.clone());

            for &member_id in hir.member_list(class.members) {
                let member = hir.member(member_id);
                if member.name_range.contains_inclusive(offset) {
                    consider(CursorTarget::MemberDeclaration {
                        owner: owner.clone(),
                        name: member.name.clone(),
                        kind: hir_member_kind(member.kind),
                        range: member.name_range,
                    });
                }
            }
        }

        best
    }

    /// The declaration the thing at `offset` refers to.
    ///
    /// A projection rather than a feature: goto, hover, and references all need exactly this, and
    /// each deriving it from [`Self::cursor_target`] separately is how they end up disagreeing
    /// about what the user clicked.
    ///
    /// Landing on a declaration resolves to itself, which is what an editor expects when goto is
    /// invoked on the thing already being looked at.
    pub fn declaration_at(&self, offset: TextSize) -> Option<SymbolLocation> {
        self.declaration_of(offset, &self.cursor_target(offset)?)
    }

    /// The declaration a [`CursorTarget`] already read off the buffer refers to.
    ///
    /// Split out of [`Self::declaration_at`] for the caller that wants the *origin* as well: the
    /// span the cursor was on is what an editor underlines for a ctrl-click, and it is already in
    /// the target. Asking for both separately would read the position twice.
    pub fn declaration_of(
        &self,
        offset: TextSize,
        target: &CursorTarget,
    ) -> Option<SymbolLocation> {
        match target {
            CursorTarget::WrittenName {
                path, anchor, kind, ..
            } => self.resolve_name(offset, path.as_str(), *anchor, *kind),
            CursorTarget::ImportPath { path, kind, .. } => match kind {
                ImportKind::ClassLike => self.class_location(path.as_str()),
                ImportKind::Function => self.function_location(path.as_str()),
                ImportKind::Constant => self.constant_location(path.as_str()),
            },
            CursorTarget::Declaration { fqn, kind, .. } => match kind {
                SymbolKind::Function => self.function_location(fqn.as_str()),
                // PHP's third table. Without this arm a cursor on `const MAX = 10;` asked the
                // *class* catalog, which has no such name -- so goto, hover, find-references and
                // rename were all dead at a constant's own declaration while every use of it
                // answered.
                SymbolKind::Constant => self.constant_location(fqn.as_str()),
                _ => self.class_location(fqn.as_str()),
            },
            // The keyword means a class, so it goes where that class is declared -- which for
            // `self` inside the class is the declaration a reader is already looking at, the same
            // as landing on any other declaration.
            CursorTarget::RelativeClass { class, .. } => self.class_location(class.as_str()),
            CursorTarget::MemberAccess {
                owner, name, kind, ..
            }
            | CursorTarget::MemberDeclaration {
                owner, name, kind, ..
            } => self.member_location(owner.as_str(), *kind, name.as_str()),
            // The id names a class, and where that class is declared is where a reader wants to land
            // -- and what a hover has to describe. Nothing else at that position says anything.
            CursorTarget::CatalogKey { class, .. } => self.class_location(class.as_str()),
            // Declared in a file that is not PHP, so there is no symbol to return. Where to go is
            // asked for separately -- see [`IndexView::place_at`].
            CursorTarget::CatalogPlace { .. } => None,
            // No declaration anywhere: the call that dispatches the name is what says it exists, so
            // that call is where a reader goes and what a hover describes. Ranked already, and the
            // first is the one a jump lands on -- see [`IndexView::dispatch_sites`].
            CursorTarget::DispatchedName { name, .. } => self
                .dispatched_declaration(name)
                .first()
                .map(crate::DispatchSite::location),
        }
    }
}

impl IndexView<'_> {
    /// Where the catalog key at `offset` is declared, for a key that names a place.
    ///
    /// Separate from [`Self::declaration_at`] because the answer is a different kind of thing: a
    /// route name is declared in a `*.routing.yml`, which holds no symbol and is not in the index at
    /// all. A jump there is a path and a position, not a location in a file this project owns.
    ///
    /// `None` for a key the framework generates rather than reads, which has nowhere to go: sending
    /// a reader to the entity class an `entity.*` route was derived from would be sending them to
    /// something that is not the route.
    ///
    /// With the key's own span in this file, which is what a client is told was clicked: a key is
    /// full of dots, and a client left to find the word itself marks one segment of it.
    pub fn place_at(&self, offset: TextSize) -> Option<(Box<str>, phrost_index::Place, TextRange)> {
        match self
            .catalog_key_at(offset)
            .or_else(|| self.built_key_at(offset))
            .or_else(|| self.declared_input_at(offset))?
        {
            CursorTarget::CatalogPlace {
                key, place, range, ..
            } => Some((key, place?, range)),
            _ => None,
        }
    }
}

/// Whether the word `offset` sits on is `self`, `static` or `parent`.
///
/// Case-insensitively: PHP's keywords are, and `Self::make()` is the same call as `self::make()`.
fn spells_a_relative_class(text: &str, offset: TextSize) -> bool {
    let at = usize::from(offset).min(text.len());
    let is_word = |byte: &u8| byte.is_ascii_alphanumeric() || *byte == b'_';

    let bytes = text.as_bytes();
    let start = bytes[..at]
        .iter()
        .rposition(|b| !is_word(b))
        .map_or(0, |i| i + 1);
    let end = at
        + bytes[at..]
            .iter()
            .position(|b| !is_word(b))
            .unwrap_or(bytes.len() - at);

    matches!(
        text.get(start..end).map(str::to_ascii_lowercase).as_deref(),
        Some("self" | "static" | "parent")
    )
}
