//! What each offer acts on, and where those things are in the range a request is about.
//!
//! An offer is asked for at a position, and almost nothing in a file is at that position. Without
//! this, every offer found that out for itself: six offers, six whole-file walks, on a request where
//! at most one of them had anything to say. The kind filter a client sends does not help, because an
//! editor asking on cursor-idle asks for every kind.
//!
//! So an offer declares the *sort of thing* it acts on -- [`ActionSubject`] -- and the generic layer
//! works out once, for the whole request, which of those sorts the range actually holds. An offer
//! whose subject is absent never runs.
//!
//! # Shared, and worked out at most once
//!
//! The sharing is the point. If each offer ran its own check it would still be six checks, and two
//! offers about the same sort of thing would each pay for it. Here the answer for a sort is worked
//! out when the first offer that declares it asks, kept, and handed to the rest -- and a sort no
//! surviving offer declares is never worked out at all.
//!
//! What is kept is the projection whole, and what an offer reads is the part of it the range
//! concerns. Narrowing on the way through rather than into a second list is deliberate: a
//! declaration carries the text of its docblock and an access carries the class it resolved to, so
//! copying the ones in range costs more than the walk that found them on a request that selected the
//! whole file.
//!
//! # Conservative, deliberately
//!
//! A gate that wrongly says "nothing of that sort here" removes an action from the menu, and nobody
//! notices a menu entry that is not there. So what each offer is handed is a *superset* of what it
//! can act on: the same projection it used to read for itself, narrowed by the same [`concerns`]
//! test it used to apply, and nothing else. Whatever it refuses for its own reasons -- an import
//! already written, a receiver that can never be null -- it still gets the chance to refuse.
//!
//! # What this costs today, and what it should cost
//!
//! Every sort here is answered by walking the whole file, because that is the only shape the view
//! offers: [`IndexView::member_accesses_in`] resolves only the receivers the range touches, where
//! there is one under the cursor. On a 12 KB file that one projection is 0.60 ms of a 0.64 ms
//! request -- nine tenths of a code-action request, paid because a cursor *might* be on an access.
//!
//! Sharing a walk is not the same as not doing it, so the gate is right today but not yet cheap.
//! Making it cheap needs the ranged form of each projection -- the sites *within* a range, resolved
//! only for the sites in it -- and when those exist it is this file that changes, not the offers.

#[cfg(test)]
mod tests;

use std::cell::OnceCell;

use phrost_model::TextRange;
use phrost_view::{
    DeclaredClass, Documentable, DocumentedDeclaration, IndexView, MemberAccess, NameReference,
    UnusedImport,
};

use super::{ActionRequest, concerns};

/// The sort of thing an offer has something to say about.
///
/// One per offer, and required rather than defaulted: an offer that did not have to declare a
/// subject would default to running always, which is the cost this exists to remove.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum ActionSubject {
    /// A function-like declaration -- its own name, or one of the parameters in its signature.
    Declaration,
    /// A class-like declaration: its name, or its docblock.
    ClassDeclaration,
    /// An expression, or a place inside one.
    ///
    /// Answered by the offer itself rather than gated here: what counts is the innermost expression
    /// *containing* the range, and finding that is the whole of what the offer does -- a gate would
    /// be the same walk run twice.
    Expression,
    /// A class-like name written in code or in a docblock.
    WrittenName,
    /// A member reached through `->`, `?->` or `::`, or the value it is reached through.
    MemberAccess,
    /// The path of a `use` statement.
    Import,
    /// The file, and no place in it.
    ///
    /// What a `Source` action is about: "organize the imports" is asked of the buffer and answered
    /// the same wherever the caret sits, so a range gate has nothing to say about it. Its cost is
    /// held down by the client's own `only` filter instead -- an editor asks for these when a person
    /// picks them from a menu or on save, not on every cursor move.
    WholeFile,
}

/// Where each sort of subject is written in the range one request is about.
///
/// Built once per request by [`code_actions`](super::code_actions) and handed to every offer, which
/// reads the sites for its own subject instead of walking the file for them.
#[derive(Debug)]
pub(super) struct SubjectSites<'a, 'v> {
    request: &'a ActionRequest<'v>,
    classes: OnceCell<Vec<DeclaredClass>>,
    declarations: OnceCell<Vec<DocumentedDeclaration>>,
    documentable: OnceCell<Option<Documentable>>,
    names: OnceCell<Vec<NameReference>>,
    accesses: OnceCell<Vec<MemberAccess>>,
    imports: OnceCell<Vec<UnusedImport>>,
}

impl<'a, 'v> SubjectSites<'a, 'v> {
    pub(super) fn of(request: &'a ActionRequest<'v>) -> Self {
        Self {
            request,
            classes: OnceCell::new(),
            declarations: OnceCell::new(),
            documentable: OnceCell::new(),
            names: OnceCell::new(),
            accesses: OnceCell::new(),
            imports: OnceCell::new(),
        }
    }

    pub(super) fn view(&self) -> &'v IndexView<'v> {
        self.request.view
    }

    /// What the client selected, empty when there is only a cursor.
    pub(super) fn at(&self) -> TextRange {
        self.request.at
    }

    /// The checks the project switched on -- see [`ActionRequest::asked`].
    pub(super) fn asked(&self) -> crate::AskedFor<'v> {
        self.request.asked
    }

    /// Whether the range holds anything of this sort, and so whether the offer has to run.
    pub(super) fn holds(&self, subject: ActionSubject) -> bool {
        match subject {
            // Two questions, because the offers about a declaration are not all asking the same
            // one: the tag rules read every declaration the range covers, while writing a whole
            // docblock is about the one under the cursor. The list is asked first and the position
            // query only where it found nothing -- which is what keeps a member declared by an
            // `@method` tag, and so absent from the list, from silently losing its offers.
            // The *wider* list, because this decides whether an offer runs at all and one of them
            // reads it: a throw a call raises is a position inside a body, and gating on the narrow
            // list meant the tag offer never got to look. The other four iterate the narrow list
            // themselves, so a body position still produces nothing from them.
            ActionSubject::Declaration => {
                self.declarations_or_their_bodies().next().is_some()
                    || self.documentable().is_some()
            }
            ActionSubject::ClassDeclaration => self.declared_classes().next().is_some(),
            ActionSubject::Expression => true,
            ActionSubject::WrittenName => self.names().next().is_some(),
            ActionSubject::MemberAccess => self.accesses().next().is_some(),
            ActionSubject::Import => self.imports().next().is_some(),
            ActionSubject::WholeFile => true,
        }
    }

    /// Every function-like whose name, one of whose parameter names, or one of whose `throw`s the
    /// range covers.
    ///
    /// The throws count because a report about an undocumented one is underlined *there*, and the
    /// quick fix a person reaches for from a squiggle is asked about the squiggle's range.
    pub(super) fn declarations(&self) -> impl Iterator<Item = &DocumentedDeclaration> {
        let at = self.at();
        self.declarations
            .get_or_init(|| self.view().documented_declarations())
            .iter()
            .filter(move |declaration| {
                concerns(declaration.name_range, at)
                    || declaration
                        .parameters
                        .iter()
                        .any(|parameter| concerns(parameter.name_range, at))
                    || declaration
                        .throws
                        .iter()
                        .any(|thrown| concerns(thrown.range, at))
            })
    }

    /// Every declaration the range falls in, by name, by parameter, by written throw -- or by
    /// sitting anywhere in its body.
    ///
    /// Apart from [`Self::declarations`], which the four other declaration offers share. That one
    /// admits a declaration at the positions its offers *act on*, and widening it would put "Write
    /// a docblock" on the menu from the middle of a method body, which is not where anybody asks
    /// for one.
    ///
    /// The sixth position is a throw a *call* raises: it is inside the body and on none of the
    /// three. Only the tag offer reads this, and its own per-position guards decide what may be
    /// offered from here -- a body position is neither a name nor a parameter, so the only arm that
    /// can fire is the one about a throw.
    pub(super) fn declarations_or_their_bodies(
        &self,
    ) -> impl Iterator<Item = &DocumentedDeclaration> {
        let at = self.at();
        self.declarations
            .get_or_init(|| self.view().documented_declarations())
            .iter()
            .filter(move |declaration| {
                concerns(declaration.name_range, at)
                    || declaration
                        .parameters
                        .iter()
                        .any(|parameter| concerns(parameter.name_range, at))
                    || declaration
                        .throws
                        .iter()
                        .any(|thrown| concerns(thrown.range, at))
                    || (declaration
                        .body
                        .is_some_and(|body| concerns(body.span(), at))
                        // Only where a call is written. Being inside a body is what makes the
                        // *question* askable; being on a call is what makes the answer possibly
                        // non-empty, and settling it here keeps a cursor resting on a blank line
                        // from resolving every call in the method around it.
                        && self.view().a_call_is_written_at(at.start()))
            })
    }

    /// Every class-like the range covers the name or the docblock of.
    ///
    /// The docblock counts as well as the name, because one offer here acts on what is written *in*
    /// the comment: a cursor sitting on an annotation is on the thing, not near it.
    pub(super) fn declared_classes(&self) -> impl Iterator<Item = &DeclaredClass> {
        let at = self.at();
        self.classes
            .get_or_init(|| self.view().declared_classes())
            .iter()
            .filter(move |class| {
                concerns(class.name_range, at)
                    || class
                        .docblock
                        .as_ref()
                        .is_some_and(|docblock| concerns(docblock.range, at))
            })
    }

    /// The declaration a docblock could be written for, at the start of the range.
    ///
    /// Shared because two offers stand at the same place with two different docblocks to write, and
    /// each working it out separately was one whole-file position query run twice.
    pub(super) fn documentable(&self) -> Option<&Documentable> {
        self.documentable
            .get_or_init(|| self.view().documentable_at(self.at().start()))
            .as_ref()
    }

    /// Every class-like name the range covers.
    pub(super) fn names(&self) -> impl Iterator<Item = &NameReference> {
        let at = self.at();
        self.names
            .get_or_init(|| self.view().name_references())
            .iter()
            .filter(move |reference| concerns(reference.range, at))
    }

    /// Every member access the range covers either end of -- the name, or the receiver it is
    /// reached through.
    ///
    /// Both ends, because a cursor is on the name somebody was typing or on the receiver a
    /// diagnostic underlined, and an offer about the access has to be found for either.
    pub(super) fn accesses(&self) -> impl Iterator<Item = &MemberAccess> {
        // Ranged, and that is the difference between this gate paying for itself and not. Resolving
        // a receiver is a cross-file question; asking it of the whole file to find out there is no
        // access at the cursor was 94 % of the request's cost. The filter below stays, because the
        // projection admits an access the range only touches at one end.
        self.accesses
            .get_or_init(|| self.view().member_accesses_in(self.at()))
            .iter()
    }

    /// Every unused import in the file, ignoring the range.
    ///
    /// For a `WholeFile` offer, which is about the buffer rather than about a place in it. Shares
    /// the same cell as [`Self::imports`], so asking both costs one projection.
    pub(super) fn all_imports(&self) -> &[UnusedImport] {
        self.imports.get_or_init(|| self.view().unused_imports())
    }

    /// Every import the range covers the path of, that nothing in the file names.
    pub(super) fn imports(&self) -> impl Iterator<Item = &UnusedImport> {
        let at = self.at();
        self.imports
            .get_or_init(|| self.view().unused_imports())
            .iter()
            .filter(move |unused| concerns(unused.range, at))
    }
}
