//! Which declaration a call is written inside, and which declaration it reaches.
//!
//! The two halves of walking a call graph, and both are about *one* file: what this file calls, and
//! where in this file something is called from. Joining them across a project is a sweep, which is a
//! workspace's job for the same reason finding references is -- only a workspace can open a file
//! that is not the one being looked at.
//!
//! Nothing here is stored. A call is written in a body, bodies exist only for the buffer a view is
//! over, and the declaration a call lands in comes from the skeleton -- which is what keeps every
//! range an answer carries out of the buffer's reach and safe across compaction and a cache restore.

use phrost_body::{CallSite, CallTarget};
use phrost_hir::{FileHir, ImportKind};
use phrost_model::{TextRange, TextSize};
use phrost_sema::MemberKind;

use crate::{
    location::SymbolLocation,
    view::{FOLLOW_LIMIT, IndexView},
};

/// One call written in the current file, and the declaration it reaches.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ResolvedCall {
    /// Where the callee is declared.
    pub callee: SymbolLocation,
    /// The callee's name as written here -- `find` in `$repo->find(1)`, `Repo` in `new Repo()`.
    ///
    /// The name and not the whole call, because this is what an editor reveals when a reader picks
    /// the row: selecting `$repo->find(1, $that)` would highlight the arguments as well as the thing
    /// being navigated to.
    pub at: TextRange,
}

impl IndexView<'_> {
    /// The declaration in the current file that encloses `offset`.
    ///
    /// What "who calls this" needs to group by: a call is a position, and the thing a reader wants
    /// listed is the method or function it was written in. The narrowest wins, so a call in a method
    /// is attributed to the method rather than to its class.
    ///
    /// `None` at a position no declaration covers -- a call written at a file's top level, an
    /// `include`d script, a constant expression between two classes. That is an honest nothing: the
    /// protocol's unit here is a declaration, and there is none.
    ///
    /// Read from the skeleton, so the ranges survive compaction and a cache restore, and so the
    /// answer is the same whether the file happens to be open.
    pub fn enclosing_declaration(&self, offset: TextSize) -> Option<SymbolLocation> {
        let file = self.current_file()?;
        let skeleton = self.skeleton.file(file)?;

        // A member first and on its own terms rather than after locating the class: an anonymous
        // class declared inside a method has a synthesised name the catalog may not answer for, and
        // the method inside it is still the narrowest declaration covering the call.
        let member = skeleton
            .classes()
            .iter()
            .flat_map(|class| {
                class
                    .members()
                    .iter()
                    .map(move |member| (class.fqn().as_str(), member))
            })
            .filter(|(_, member)| member.full_range.contains_inclusive(offset))
            .min_by_key(|(_, member)| member.full_range.len());

        if let Some((class, member)) = member {
            return self.member_location(class, member.kind, member.name.as_str());
        }

        let function = skeleton
            .scope()
            .symbols()
            .iter()
            .filter(|symbol| symbol.kind == phrost_model::SymbolKind::Function)
            .filter(|symbol| symbol.full_range.contains_inclusive(offset))
            .min_by_key(|symbol| symbol.full_range.len())?;

        self.function_location(function.fqn.as_str())
    }

    /// Every call written inside `within`, resolved to what it reaches.
    ///
    /// Cut down to the range before anything is resolved, the same way
    /// [`member_accesses_in`](Self::member_accesses_in) is and for the same reason: resolving a
    /// receiver is a cross-file question, and one declaration's body is a small part of a file.
    ///
    /// Silent about a call whose callee it cannot name -- a receiver of unknown type, a name that
    /// resolves to nothing, `$callable()`. A row pointing nowhere is worse than an absent row: the
    /// tree would offer it and the jump would land on the file the reader is already in.
    pub fn calls_within(&self, within: TextRange) -> Vec<ResolvedCall> {
        let Some(open) = self.current() else {
            return Vec::new();
        };
        let hir = open.hir();

        let mut inside = phrost_body::call_sites(hir)
            .into_iter()
            .filter(|call| within.contains_range(call.callee_range))
            .collect::<Vec<_>>();
        // Source order, because the walk follows body discovery rather than the text and a reader
        // shown a list of call sites reads them top to bottom.
        inside.sort_by_key(|call| call.callee_range.start());

        inside
            .into_iter()
            .filter_map(|call| {
                Some(ResolvedCall {
                    at: call.callee_range,
                    callee: self.callee_of(hir, &call)?,
                })
            })
            .collect()
    }

    /// Where every call in the current file to the free function `fqn` is written.
    ///
    /// Matched by what the written name *resolves to*, never by the text: `Drupal\node\format()`
    /// and a global `format()` are two functions, and a search that returned both would be worse
    /// than useless for deciding whether a change is safe. This is the same rule
    /// [`member_accesses`](Self::member_accesses) applies to a receiver, asked of PHP's other
    /// symbol table.
    pub fn function_calls_to(&self, fqn: &str) -> Vec<TextRange> {
        let Some(open) = self.current() else {
            return Vec::new();
        };
        let hir = open.hir();

        let mut found = phrost_body::call_sites(hir)
            .into_iter()
            .filter_map(|call| {
                let CallTarget::Function(reference) = &call.target else {
                    return None;
                };
                let found = self.resolve_function(hir, &call, *reference)?;
                (found.fqn.as_str() == fqn).then_some(call.callee_range)
            })
            .collect::<Vec<_>>();
        found.sort_by_key(|at| at.start());
        found
    }

    /// Where every `new` in the current file that runs `fqn`'s constructor is written.
    ///
    /// How PHP spells a call to a constructor, and the only way most of them are ever reached: a
    /// project that never writes `->__construct()` still calls one on every line that says `new`.
    /// Listing the explicit spelling alone would answer "nothing calls this" about a constructor
    /// used a hundred times.
    ///
    /// By the constructor a `new` *reaches* and not by the class it names, which is the same rule
    /// [`member_uses`](crate::IndexView::member_accesses_named) applies to a method call: `new
    /// PremiumTier()` runs `BaseTier::__construct` when the subclass declares none of its
    /// own, and a list keyed on the written name leaves out every such site. Two spellings of one
    /// call resolved by two different rules is how a caller list comes back empty for a constructor
    /// a project instantiates on forty lines.
    pub fn constructions_of(&self, fqn: &str) -> Vec<TextRange> {
        let Some(open) = self.current() else {
            return Vec::new();
        };
        let hir = open.hir();

        let mut found = phrost_body::call_sites(hir)
            .into_iter()
            .filter_map(|call| {
                let CallTarget::Constructor(target) = &call.target else {
                    return None;
                };
                let class = self.class_of_target(call.range.start(), hir, *target)?;
                self.construction_reaches(class.as_str(), fqn)
                    .then_some(call.callee_range)
            })
            .collect::<Vec<_>>();
        found.sort_by_key(|at| at.start());
        found
    }

    /// Whether `new <class>()` runs the constructor `fqn` owns.
    ///
    /// The class itself where nothing in the hierarchy declares one -- which is the same stand-in
    /// [`callee_of`](Self::callee_of) resolves such a `new` to, so the two halves of the call graph
    /// answer about the same declaration rather than about the same spelling.
    fn construction_reaches(&self, class: &str, fqn: &str) -> bool {
        match self.member_location(class, MemberKind::Method, "__construct") {
            Some(declared) => declared
                .container
                .is_some_and(|owner| owner.as_str() == fqn),
            None => class == fqn,
        }
    }

    /// Where one call's callee is declared, whichever of the three kinds of call it is.
    ///
    /// The same three-way split [`parameters_of`](Self::parameters_of) makes, asked for the
    /// declaration instead of the signature. Deliberately not written as a lookup on top of that
    /// one: a parameter list and a location are different facts, and threading one through the
    /// other would tie a navigation answer to whether the callee happens to declare parameters.
    pub(crate) fn callee_of(&self, hir: &FileHir, call: &CallSite) -> Option<SymbolLocation> {
        match &call.target {
            CallTarget::Member { receiver, name } => {
                let at = call.receiver_at();
                let class = self
                    .resolve_path(at, hir, receiver, FOLLOW_LIMIT)
                    .and_then(|held| {
                        self.declaring_class(at, hir, &held, MemberKind::Method, name.as_str())
                    })?;
                self.member_location(class.as_str(), MemberKind::Method, name.as_str())
            }
            CallTarget::Function(reference) => self.resolve_function(hir, call, *reference),
            CallTarget::Constructor(target) => {
                let class = self.class_of_target(call.range.start(), hir, *target)?;
                // A class that declares no constructor still has one, and the place a reader has to
                // go to read what `new` does is the class itself. This is not a second attempt at a
                // missed lookup: the two cases are disjoint, and which applies is decided by the
                // declaration rather than by whether the first answer came back empty.
                match self.member_location(class.as_str(), MemberKind::Method, "__construct") {
                    Some(declared) => Some(declared),
                    None => self.class_location(class.as_str()),
                }
            }
            // A closure has a declaration and it is the literal itself, which is where the
            // cursor already is: `goto` on `$fn(` would land on the line it was invoked from as
            // often as on the `fn ()` that made it, and answering nothing is the honest half of
            // that until the value is followed to its literal.
            CallTarget::Invoked(_) => None,
        }
    }

    /// The function a written call name stands for, resolved against the file's imports.
    fn resolve_function(
        &self,
        hir: &FileHir,
        call: &CallSite,
        reference: phrost_hir::NameRefId,
    ) -> Option<SymbolLocation> {
        let written = hir.name_ref(reference);
        self.resolve_name(
            call.range.start(),
            written.path.as_str(),
            written.anchor,
            ImportKind::Function,
        )
    }
}
