//! Extension points a project declares by dispatching them, rather than by writing them down.
//!
//! The one projection whose subject has no declaration. Everywhere else "where is this declared" is
//! a question with a symbol behind it; here the answer is a *call*, because the call is all there
//! is -- `$this->moduleHandler->invokeAll('notifications_alter')` is the entire statement that the
//! name exists, and the implementations of it elsewhere in the project reference something nothing
//! writes down.
//!
//! Nothing here names a framework. Which calls dispatch, and what the declaration a dispatched name
//! implies is called, both arrive from the seam; a project with no framework pays one empty lookup.
//!
//! # The gate
//!
//! The index records a candidate per call and leaves the receiver unresolved, because resolving one
//! at ingest would make the answer depend on the order files were read. So the gate is here, where
//! the hierarchy is: a call counts only when the member its receiver was read off holds the class
//! the rule names. `alter` is a method any project declares for something of its own -- one real
//! checkout calls it three times on unrelated services -- and without the gate every string those
//! were handed would be an extension point.
//!
//! The gate costs one member lookup per candidate, and candidates are per *name*: asking about one
//! hook reaches the one or two calls that write it, never the project.
//!
//! # Two questions, and why an unidentifiable receiver answers them differently
//!
//! "Where does this name come from" and "does this name exist" are not the same question about the
//! same evidence. A call whose receiver the index could not identify -- a bare local -- is no
//! evidence that the call dispatches, and no evidence that it does not. So the first question
//! declines it, because sending a reader somewhere on that basis is a claim made from the method
//! name alone; and the second accepts it, because *reporting a name as nonexistent* on that basis is
//! reporting on absent evidence, which is how a checker loses its reader. Both are the cautious
//! reading of one missing fact.

use phrost_index::DispatchReceiver;
use phrost_model::{FileId, Name, SymbolKind, TextRange};
use phrost_sema::MemberKind;

use crate::{location::SymbolLocation, resolve::HIERARCHY_DEPTH, view::IndexView};

/// One call that dispatches an extension point.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DispatchSite {
    pub file: FileId,
    /// The name it dispatches, spelled the way a declaration of it would be.
    pub name: Name,
    /// Where the name is written inside the call: what a jump selects.
    pub name_range: TextRange,
    /// The whole call: what a preview shows, and what says which arguments it passes.
    pub range: TextRange,
    /// The dispatching method, as the call wrote it.
    pub method: Box<str>,
}

impl DispatchSite {
    /// The site as somewhere to go.
    ///
    /// A [`SymbolLocation`] because that is what the call *is* to everything above: the place the
    /// name comes from. Its kind says what it really is -- a key, not a declaration -- so nothing
    /// downstream mistakes it for a function it could read a signature off.
    pub fn location(&self) -> SymbolLocation {
        SymbolLocation {
            file: self.file,
            fqn: self.name.clone(),
            kind: SymbolKind::CatalogKey,
            name_range: self.name_range,
            full_range: self.range,
            container: None,
            signature: None,
            member_modifiers: None,
            from_docblock: false,
            origin: None,
            docblock_range: None,
            // A key is not a declaration and nothing can deprecate one.
            deprecated: false,
        }
    }
}

impl IndexView<'_> {
    /// Every call in the project that dispatches `name`, best first.
    ///
    /// Ordered by path and then by position, never by the order files were ingested: two runs over
    /// one project have to agree about which call a jump lands on, and a file id reflects discovery
    /// order. The same rule every other ranked list here follows.
    pub fn dispatch_sites(&self, catalog: &str, name: &str) -> Vec<DispatchSite> {
        let mut found = self
            .skeleton
            .returns()
            .dispatches(catalog, name)
            .iter()
            .filter(|call| self.receiver_is(call.receiver.as_ref(), &call.required))
            .map(|call| DispatchSite {
                file: call.file,
                name: call.name.clone(),
                name_range: call.name_range,
                range: call.range,
                method: call.method.clone(),
            })
            .collect::<Vec<_>>();
        found.sort_by(|left, right| {
            self.file_path(left.file)
                .cmp(&self.file_path(right.file))
                .then(left.name_range.start().cmp(&right.name_range.start()))
        });
        found
    }

    /// Whether anything in the project may dispatch `name`.
    ///
    /// The reading a check about the name's *absence* has to take: a call this can prove is a
    /// dispatch, or one whose receiver it could not identify at all. See the module doc for why the
    /// second belongs here and not in [`Self::dispatch_sites`].
    pub fn maybe_dispatched(&self, catalog: &str, name: &str) -> bool {
        self.skeleton
            .returns()
            .dispatches(catalog, name)
            .iter()
            .any(|call| match &call.receiver {
                Some(receiver) => self.receiver_is(Some(receiver), &call.required),
                None => true,
            })
    }

    /// Whether the receiver of a candidate call really is what the rule requires.
    ///
    /// One member lookup: what reading -- or calling -- the member gives back. A property and a
    /// method are the same question here, because a service is reached both ways and what matters
    /// is the class that comes out either way.
    ///
    /// `false` for a member that resolves to nothing, which is the safe direction for the question
    /// this half answers: a receiver the index cannot type is a receiver nothing can be *claimed*
    /// about, and claiming anyway is the method-name-only rule the module doc refuses.
    fn receiver_is(&self, receiver: Option<&DispatchReceiver>, required: &str) -> bool {
        let Some(receiver) = receiver else {
            return false;
        };
        let Some(held) = self
            .member_class(&receiver.owner, MemberKind::Property, &receiver.member)
            .or_else(|| self.member_class(&receiver.owner, MemberKind::Method, &receiver.member))
        else {
            return false;
        };
        held.as_str().eq_ignore_ascii_case(required)
            || self.descends_from(held.as_str(), required, HIERARCHY_DEPTH)
    }
}
