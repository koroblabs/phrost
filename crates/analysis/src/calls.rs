//! Who calls this, and what does this call.
//!
//! Three requests for one feature, the same shape the class hierarchy next door has: the client asks
//! which callable the cursor is on, then asks that item for its callers or its callees, as many times
//! as a person keeps expanding the tree. So an item has to carry enough to be resolved later, and
//! what it carries is the declaration's identity -- a class and a member name, or a function's
//! fully-qualified name.
//!
//! # The two directions are not the same kind of question
//!
//! **Outgoing** is one declaration's body, read once. It needs the body of whatever the tree is
//! expanded on, which after the first step is a file nobody has open -- so the caller has to arrange
//! for that file to be analysed, exactly the way a reference sweep arranges it, and hand a view over
//! it here.
//!
//! **Incoming** is a project-wide search. It is [`crate::references_in`] narrowed to the calls and
//! grouped by the declaration each is written in, and it reuses that module's matching rather than
//! inventing a second one: a method call resolves through its receiver, so "who calls `Repo::find`"
//! is not a text search for `find`, and there must be exactly one answer in this codebase to what
//! that means.
//!
//! Both answer about the file a view is over. Joining them across a project is a workspace's job,
//! for the reason `references` gives: only a workspace can open a file that is not the one being
//! looked at.

use phrost_model::{SymbolKind, TextRange, TextSize};
use phrost_view::{IndexView, MemberKind};

use crate::navigation::{NavigationTarget, target_of};
use crate::references::SweepQuery;

/// What PHP calls a constructor, and the one member reached by a syntax that never writes its name.
const CONSTRUCTOR: &str = "__construct";

/// The declaration a call-hierarchy question is about.
///
/// Identified by what declares it rather than by the name at the cursor, the same rule
/// [`crate::ReferenceTarget`] follows: `$repo->find()` and `$other->find()` are the same item only
/// when they reach the same declaration, and an inherited method reached through a subclass is the
/// declaration in the parent.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CallSubject {
    /// The declaring class, for a method. `None` for a free function.
    pub container: Option<Box<str>>,
    /// The member's name, or the function's fully-qualified name.
    pub name: Box<str>,
}

impl CallSubject {
    /// The identity an item carries from the prepare step to the follow-ups.
    ///
    /// A single string, as the class hierarchy's items carry one, because a client hands back
    /// whatever it was given and the fewer parts that has the fewer ways the two ends can disagree.
    /// `::` is safe as the separator: no PHP name may contain it.
    pub fn key(&self) -> String {
        match &self.container {
            Some(class) => format!("{class}::{}", self.name),
            None => self.name.to_string(),
        }
    }

    /// The subject a key names.
    ///
    /// Split at the last `::`, so a class in a namespace stays whole. Nothing is validated here --
    /// whether the declaration still exists is settled by looking it up, which is what every step
    /// after this does anyway.
    pub fn of_key(key: &str) -> Self {
        match key.rsplit_once("::") {
            Some((class, member)) => Self {
                container: Some(class.into()),
                name: member.into(),
            },
            None => Self {
                container: None,
                name: key.into(),
            },
        }
    }

    /// What a project-wide sweep must look for in candidate files' text.
    ///
    /// The name, for almost everything. A constructor is reached in more ways than it is written:
    /// by `parent::__construct()`, by `new Repo()`, and -- for every subclass that declares no
    /// constructor of its own -- by `new Child()`, which names neither the member nor the class
    /// that declares it. A sweep for the first two alone makes a file that only writes the third
    /// no candidate at all, so the caller list comes back missing every subclass instantiation
    /// while reading as complete.
    ///
    /// The view, because that last group is a question about the class table and the answer cannot
    /// be spelled from the subject alone.
    pub fn sweep_query(&self, view: &IndexView<'_>) -> SweepQuery {
        match (&*self.name == CONSTRUCTOR, &self.container) {
            (true, Some(class)) => SweepQuery::construction(view, [&**class]),
            _ => SweepQuery::word(phrost_model::short_name(&self.name).to_owned()),
        }
    }
}

/// One declaration that calls the subject, and where inside it the calls are.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct IncomingCall {
    pub caller: NavigationTarget,
    /// The call sites, in source order, in `caller.file`.
    ///
    /// The individual sites and not the caller's own range: a tree built from the wrong one looks
    /// right and jumps to the top of a two-hundred-line method instead of to the line that calls.
    pub at: Vec<TextRange>,
}

/// One declaration the subject calls, and where inside the subject the calls are.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct OutgoingCall {
    pub callee: NavigationTarget,
    /// The call sites, in source order, in the *subject's* file -- not in the callee's.
    pub at: Vec<TextRange>,
}

/// The callable a call hierarchy should start from, for a cursor at `offset`.
///
/// Two questions in precedence order, not a fallback: a cursor is either on a name or it is not, and
/// when it is on one the reader picked that name. A cursor resting anywhere else in a body is asking
/// about the declaration it is sitting in, which is how the request arrives from an editor whose
/// shortcut does not require aiming at anything.
///
/// A class-like name stands for its constructor, because `new Repo()` is how PHP writes a call to
/// one and a reader clicking `Repo` is asking about that edge.
///
/// `None` where neither question has an answer: a cursor in a docblock at the top of a file, on an
/// import, on a class with no body around it.
pub fn call_hierarchy_head(view: &IndexView<'_>, offset: TextSize) -> Option<NavigationTarget> {
    if let Some(named) = view.declaration_at(offset)
        && let Some(callable) = as_callable(view, named)
    {
        return Some(callable);
    }

    view.enclosing_declaration(offset)
        .filter(|location| is_callable(location.kind))
        .map(target_of)
}

/// Where `subject` is declared.
///
/// `None` when nothing declares it any more, which is what a client asking about an item from before
/// an edit gets. A class named as a subject with no constructor of its own resolves to the class:
/// `new Repo()` still calls something, and the place a reader has to go to read what it does is the
/// declaration itself.
pub fn callable_declaration(
    view: &IndexView<'_>,
    subject: &CallSubject,
) -> Option<NavigationTarget> {
    let Some(class) = &subject.container else {
        return view.function_location(&subject.name).map(target_of);
    };

    match view.member_location(class, MemberKind::Method, &subject.name) {
        Some(declared) => Some(target_of(declared)),
        None if &*subject.name == CONSTRUCTOR => view.class_location(class).map(target_of),
        None => None,
    }
}

/// Every call in the current file that reaches `subject`, grouped by the declaration it sits in.
///
/// A call written where no declaration covers it -- at a script's top level -- is left out rather
/// than attributed to the file: the protocol's unit here is a declaration, and there is none.
pub fn incoming_calls(view: &IndexView<'_>, subject: &CallSubject) -> Vec<IncomingCall> {
    let mut sites = match &subject.container {
        Some(class) => {
            crate::references::member_uses(view, class, MemberKind::Method, &subject.name)
        }
        None => view.function_calls_to(&subject.name),
    };

    // The other spelling of the same call. A class that declares a constructor is reached by `new`
    // far more often than by naming the member, and a list without those is a constructor nothing
    // appears to use.
    if &*subject.name == CONSTRUCTOR
        && let Some(class) = &subject.container
    {
        sites.extend(view.constructions_of(class));
    }
    sites.sort_by_key(|site| site.start());
    sites.dedup();

    let mut found: Vec<IncomingCall> = Vec::new();
    for at in sites {
        let Some(caller) = view
            .enclosing_declaration(at.start())
            .map(target_of)
            .or_else(|| file_as_caller(view))
        else {
            continue;
        };
        // Linear, because a single file holds a handful of callers and building a map of them would
        // cost more than the scan it replaced.
        match found.iter_mut().find(|found| found.caller == caller) {
            Some(existing) => existing.at.push(at),
            None => found.push(IncomingCall {
                caller,
                at: vec![at],
            }),
        }
    }
    found
}

/// The file itself, as the caller of a call no declaration encloses.
///
/// PHP lets a call be written at a file's top level, or inside a closure the file returns -- a
/// script, a bootstrap, Symfony's PHP-DSL configuration, a WordPress plugin's body. There is no
/// declaration around any of those, and dropping them made "who calls this" read as complete while
/// it was not: 12 of 250 sampled call sites on one real project and 30 of 240 on Symfony, all of
/// them silent.
///
/// The row points at the top of the file, which is where a jump lands. A file has no declaration to
/// preview and no name to expand, and both of those are honest: nothing calls a file, so a client
/// that asks this item for *its* callers is answered with nothing.
fn file_as_caller(view: &IndexView<'_>) -> Option<NavigationTarget> {
    let file = view.current_file_id()?;
    let path = view.file_path(file)?;
    Some(NavigationTarget {
        file,
        name: phrost_model::display_name(path),
        kind: SymbolKind::File,
        container: None,
        selection_range: TextRange::empty(TextSize::new(0)),
        full_range: TextRange::empty(TextSize::new(0)),
        origin: None,
    })
}

/// Every call written inside `subject`'s own declaration, grouped by what it reaches.
///
/// The view has to be over the file `subject` is declared in, and that file has to be analysed --
/// which for a closed one means a caller that read it into a scratch buffer set first. There is no
/// way around that and no attempt to pretend otherwise: a body exists only where a buffer does, and
/// a declaration with no body -- an interface method, an abstract one, a class standing in for an
/// implicit constructor -- honestly calls nothing.
pub fn outgoing_calls(view: &IndexView<'_>, subject: &CallSubject) -> Vec<OutgoingCall> {
    let Some(declared) = callable_declaration(view, subject) else {
        return Vec::new();
    };
    // A class reached as a stand-in for an implicit constructor covers every method it declares.
    // Walking its whole range would report the whole class's calls as the constructor's.
    if !is_callable(declared.kind) {
        return Vec::new();
    }

    let mut found: Vec<OutgoingCall> = Vec::new();
    for call in view.calls_within(declared.full_range) {
        let callee = target_of(call.callee);
        match found.iter_mut().find(|found| found.callee == callee) {
            Some(existing) => existing.at.push(call.at),
            None => found.push(OutgoingCall {
                callee,
                at: vec![call.at],
            }),
        }
    }
    found
}

/// The callable a declaration stands for, when it stands for one.
fn as_callable(
    view: &IndexView<'_>,
    location: phrost_view::SymbolLocation,
) -> Option<NavigationTarget> {
    if is_callable(location.kind) {
        return Some(target_of(location));
    }
    // A class-like name is the one other thing a call can be written against, through `new`. Its own
    // declaration stands in when no constructor is declared anywhere in the hierarchy: that is what
    // `new` names, it is a real range in a real file, and [`subject_of`] reads it as the constructor
    // it stands for.
    if !location.kind.is_class_like() {
        return None;
    }
    Some(target_of(
        view.member_location(location.fqn.as_str(), MemberKind::Method, CONSTRUCTOR)
            .unwrap_or(location),
    ))
}

/// Whether a declaration is one with a body that could call anything.
fn is_callable(kind: SymbolKind) -> bool {
    matches!(kind, SymbolKind::Method | SymbolKind::Function)
}

/// The subject an item stands for.
///
/// Beside [`CallSubject::key`] so the two cannot drift: an item is built from a target here and
/// resolved back from its key in the follow-up, and a mismatch would show as a tree that expands to
/// nothing.
///
/// A class-like target means its constructor, the same reading [`call_hierarchy_head`] takes of a
/// class name under the cursor. Written once, here, so a `new Repo()` shown as an outgoing call and
/// a `Repo` clicked in the editor expand to the same thing.
pub fn subject_of(target: &NavigationTarget) -> CallSubject {
    if target.kind.is_class_like() {
        return CallSubject {
            container: Some(target.name.as_str().into()),
            name: CONSTRUCTOR.into(),
        };
    }

    CallSubject {
        container: target.container.as_ref().map(|class| class.as_str().into()),
        name: target.name.as_str().into(),
    }
}
