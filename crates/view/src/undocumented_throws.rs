//! What a declaration can throw and its docblock does not admit to.
//!
//! **One answer for the two things that ask it.** The check reports each of these and the quick fix
//! offers a tag for each, and until this existed they each worked it out: the action read only what
//! the body *writes*, so the 840 findings a call raised had no fix at all, and it offered a tag for
//! a class the interface had already documented -- a fix for a finding that was no longer made.
//! §8.1's oldest form, and the repair it prescribes is the seam rather than the catch.
//!
//! The parsed docblock is handed in rather than read here: parsing a tag is the analysis layer's
//! and both callers have already done it. What lives here is the *rule*.
//!
//! Reading what a call declares is the expensive half. Reading what a body
//! writes costs a walk; reading what its calls can throw costs a resolution per call, and on one
//! real project that is 34 000 of them. `doc-missing-throws` is opt-in, so nothing that is not
//! asked for pays: `DocumentedDeclaration::throws` stays what the body writes, and this is asked
//! beside it by the one rule that wants it.
//!
//! **What a callee declares, not what its body does -- unless the callee is in this file.** A
//! `@throws` is a promise a caller may rely on, and a callee elsewhere has its body in a file this
//! view cannot see. So across files the chain is one level deep by construction: a method that
//! documents nothing propagates nothing, which is also how PhpStorm and PHPStan read the tag.
//!
//! A callee in the open file is read for what it throws, which is what its docblock says once the
//! fix for this very finding is applied. The one-level reading made `source.fixAll` a ratchet
//! rather than a fix: a save documented `load()`, the next one `find()` that calls it, the one after
//! that the callers of `find()` -- 32 of the reference project's 117 fixable files changed again on
//! their second save. A save has to be a fixpoint, and inside one file the bodies are here to read.
//!
//! The same scope as `throw_sites`, and the same handling of a `catch`. Both halves of "what does
//! this declaration throw" have to read the same body or they are §8.1's oldest form waiting to
//! disagree: closures are out of it, and a call a surrounding `catch` covers is not thrown.

use phrost_model::{Name, TextRange};

use crate::{
    documentation::{BodyRef, Thrown},
    view::IndexView,
};

impl IndexView<'_> {
    /// Whether a framework runs this class and catches whatever it throws.
    ///
    /// A `@throws` documents a contract for callers, and a class the framework runs has no caller
    /// in the project. See `FrameworkProvider::runs_and_catches`.
    pub fn is_run_by_a_framework(&self, class_fqn: &str) -> bool {
        self.skeleton().is_run_by_a_framework(class_fqn)
    }

    /// What the declarations this one overrides say it throws.
    ///
    /// A contract is written where it is declared -- on the interface, or on the abstract method --
    /// and an implementation inherits it. PhpStorm, PHPStan and psalm all read a parent's `@throws`
    /// that way, and a rule that does not demands the tag be repeated on every implementation of
    /// every interface in the project. `{@inheritdoc}` was handled and this, the case where nobody
    /// wrote anything at all, was not.
    ///
    /// Ancestors only: what the declaration itself wrote is read from its own docblock, where the
    /// text is, and adding it twice would say nothing new.
    pub fn inherited_throws(&self, owner: &str, member: &str) -> Vec<Name> {
        let mut found: Vec<Name> = Vec::new();
        for above in self.members(owner).contributors() {
            if above.as_str() == owner {
                continue;
            }
            let Some(declared) =
                self.member_location(above.as_str(), phrost_sema::MemberKind::Method, member)
            else {
                continue;
            };
            for class in
                self.skeleton()
                    .declared_throws(declared.file, Some(above.as_str()), member)
            {
                if !found.contains(class) {
                    found.push(class.clone());
                }
            }
        }
        found
    }

    /// Whether a call's name is written at `offset`, without resolving it.
    ///
    /// The prefilter §8.1 calls the `NeverReturns` shape. A throw a call raises is reported at the
    /// call, so a quick fix has to be reachable from every position inside a body -- and working out
    /// what those throws are means resolving every call in it, which is far too much to do because
    /// somebody's cursor is resting on a blank line. Nearly every position is settled here by one
    /// position query instead.
    pub fn a_call_is_written_at(&self, offset: phrost_model::TextSize) -> bool {
        self.current()
            .is_some_and(|open| phrost_body::a_call_is_named_at(open.hir(), offset))
    }

    /// What the calls written in `body` declare themselves as throwing, and it does not catch.
    ///
    /// In source order, one entry per class: a body that calls the same throwing method four times
    /// has one thing to document, and four identical findings on four lines is a list nobody reads
    /// to the end.
    ///
    /// Each carries **which call** declared it, because that is the half a reader cannot see:
    /// `return new self(...$fields);` holds no `throw`, and a report naming only the exception is an
    /// accusation with the evidence left out.
    pub fn throws_from_calls(&self, body: BodyRef) -> Vec<ThrownByCall> {
        let Some(open) = self.current() else {
            return Vec::new();
        };
        let hir = open.hir();
        let statements = hir.body(body.0).statements;
        let scopes = phrost_body::catch_scopes(hir, statements);

        let mut found: Vec<ThrownByCall> = Vec::new();
        for call in phrost_body::call_sites_in(hir, statements) {
            let Some(callee) = self.callee_of(hir, &call) else {
                continue;
            };
            // What the table is keyed by, and what resolving a call already answers with: a
            // member's `fqn` is its own name and its `container` the class that declares it -- the
            // declaring one, so a tag written on a parent is found through a call on the child. A
            // free function has no container and its `fqn` is fully qualified.
            let owner = callee.container.as_ref().map(Name::as_str);
            let declared = self
                .skeleton()
                .declared_throws(callee.file, owner, callee.fqn.as_str());
            // A callee in this file throws what its docblock will say once it is fixed -- see the
            // module documentation for why that is read here and nowhere else.
            let undeclared = match Some(callee.file) == self.current_file_id() {
                true => self.undocumented_by(owner, callee.fqn.as_str()),
                false => Vec::new(),
            };
            let thrown = declared
                .iter()
                .map(|class| (class, true))
                .chain(undeclared.iter().map(|class| (class, false)));
            for (class, declares) in thrown {
                if self.caught_by(hir, &scopes, class, call.callee_range) {
                    continue;
                }
                if found.iter().any(|seen| seen.thrown.class == *class) {
                    continue;
                }
                found.push(ThrownByCall {
                    thrown: Thrown {
                        class: class.clone(),
                        range: call.callee_range,
                    },
                    // As a reader would write it: the declaring class and the member, or a free
                    // function's own name.
                    through: match &callee.container {
                        Some(owner) => {
                            format!("{}::{}()", owner.as_str(), callee.fqn.as_str())
                        }
                        None => format!("{}()", callee.fqn.as_str()),
                    },
                    declares,
                });
            }
        }
        found
    }

    /// What a declaration of the open file throws beyond what its own docblock says -- what the fix
    /// for its finding would write above it.
    ///
    /// Memoized for the life of the view: every call to a sibling asks, and a file of two hundred
    /// methods holds thousands of calls. The entry is written empty before it is worked out, so a
    /// method reaching itself, directly or round a ring, reads as throwing what it throws.
    fn undocumented_by(&self, owner: Option<&str>, name: &str) -> Vec<Name> {
        let key: Box<str> =
            format!("{}::{}", owner.unwrap_or(""), name.to_ascii_lowercase()).into();
        if let Some(known) = self.undocumented_in_file.borrow().get(&key) {
            return known.clone();
        }
        self.undocumented_in_file
            .borrow_mut()
            .insert(key.clone(), Vec::new());

        let declarations = self
            .documented
            .get_or_init(|| self.documented_declarations());
        let found: Vec<Name> = declarations
            .iter()
            .find(|declaration| {
                declaration.name.as_str().eq_ignore_ascii_case(name)
                    && match (declaration.owner.as_deref(), owner) {
                        (Some(declared), Some(asked)) => {
                            declared.trim_start_matches('\\') == asked.trim_start_matches('\\')
                        }
                        (None, None) => true,
                        _ => false,
                    }
            })
            .map(|declaration| {
                let documented: Vec<String> = self
                    .current_file_id()
                    .map(|file| {
                        self.skeleton()
                            .declared_throws(file, owner, name)
                            .iter()
                            .map(ToString::to_string)
                            .collect()
                    })
                    .unwrap_or_default();
                self.undocumented_throws(declaration, &documented)
                    .into_iter()
                    .map(|undeclared| undeclared.class)
                    .collect()
            })
            .unwrap_or_default();

        self.undocumented_in_file
            .borrow_mut()
            .insert(key, found.clone());
        found
    }
}

/// One throw a call can raise, with the call that raises it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ThrownByCall {
    pub thrown: Thrown,
    /// The callee as a reader would write it -- `App\Repo::find()`, or `helper()`.
    pub through: String,
    /// Whether the callee's docblock says so, rather than its body in this file throwing it.
    pub declares: bool,
}

/// One class a declaration can throw and its docblock does not admit to.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct UndeclaredThrow {
    pub class: Name,
    /// Every place it is raised -- a `throw new`, or a call that declares it -- first occurrence
    /// first. More than one, because the fix is offered from each of them as well as from the name.
    pub sites: Vec<TextRange>,
    /// The call that declares it, when the throw is not written here.
    ///
    /// `None` for a `throw new` in this body, where the class is on the line the report underlines
    /// and nothing needs explaining. For a propagated one it is everything: a reader looking at
    /// `return new self(...$fields);` sees no `throw`, and a message that only names the exception
    /// reads as an accusation with no evidence -- which is how a true finding gets called spam.
    pub through: Option<String>,
    /// Whether that call's docblock says it throws this, rather than its body in this file doing so
    /// undocumented -- which is what decides whether a report says "declares" or "throws".
    pub through_declares: bool,
}

impl IndexView<'_> {
    /// What `declaration` can throw that `documented` does not cover, one entry per class.
    ///
    /// `documented` is what its own docblock's `@throws` tags named, as written. What this adds is
    /// everything the declaration cannot say for itself: the classes its *calls* declare, and the
    /// tags written on whatever it overrides.
    ///
    /// Empty for a declaration a framework runs -- see [`Self::is_run_by_a_framework`] -- because a
    /// contract with no caller is not missing anything.
    /// Whether a class is one PHP itself calls a defect rather than a condition.
    ///
    /// PHP splits `Throwable` in two and means something by it. An `Exception` is a condition the
    /// caller may reasonably handle -- a file that is not there, an id nothing matches -- and
    /// documenting it is a promise about the contract. An `Error` is a *defect*: `ValueError`,
    /// `DivisionByZeroError`, `TypeError`, `AssertionError`, `ArgumentCountError`. Nobody writes
    /// `@throws \DivisionByZeroError` above a method that divides, because the answer to it is to
    /// fix the code rather than to catch it -- which is why PhpStorm and PHPStan do not ask for one
    /// either.
    ///
    /// Counted before changing it, on the reference project: 81 of 919 findings were
    /// `\ValueError`, `\AssertionError` and `\DivisionByZeroError`, and not one of them named a
    /// contract anybody could act on.
    ///
    /// By the hierarchy rather than by a list of names, because a project declares its own: an
    /// exception extending `\Error` is making the same statement about itself, and one extending
    /// `\Exception` is asking to be documented however it is spelled.
    ///
    /// `\LogicException` is the second root, and it is the same rule rather than a second one.
    /// PHP's own manual says what it means: "Exception that represents error in the program logic.
    /// This kind of exception should lead directly to a fix in your code." That is the sentence
    /// above, written by the language about the other half of its split -- and it is where
    /// `InvalidArgumentException`, `DomainException`, `BadMethodCallException`, `LengthException`
    /// and `OutOfRangeException` live. PhpStorm's unhandled-exception inspection ignores this root
    /// by default for the same reason.
    ///
    /// `\RuntimeException` is deliberately *not* here, and that is what keeps the check worth
    /// having: the manual calls it "an error which can only be found on runtime", which is exactly
    /// a condition a caller can act on.
    ///
    /// Counted before changing it, on the reference project: **356 of 828** findings were
    /// `\InvalidArgumentException` and 173 of those came from three methods --
    /// `FieldableEntityInterface::get()`, `EntityHelperTrait::_value()` and `::_field()`, each of
    /// which throws it when a field name does not exist. Nobody documents that on every method
    /// that reads a field, because the answer to it is to write the right name.
    fn is_a_programming_error(&self, class: &str) -> bool {
        let fqn = class.trim_start_matches('\\');
        ["Error", "LogicException"].into_iter().any(|root| {
            fqn.eq_ignore_ascii_case(root)
                || self.descends_from(fqn, root, crate::view::FOLLOW_LIMIT)
        })
    }

    pub fn undocumented_throws(
        &self,
        declaration: &crate::DocumentedDeclaration,
        documented: &[String],
    ) -> Vec<UndeclaredThrow> {
        if declaration
            .owner
            .as_deref()
            .is_some_and(|owner| self.is_run_by_a_framework(owner))
        {
            return Vec::new();
        }

        let mut covered: Vec<String> = documented.to_vec();
        if let Some(owner) = &declaration.owner {
            covered.extend(
                self.inherited_throws(owner, declaration.name.as_str())
                    .iter()
                    .map(ToString::to_string),
            );
        }

        // What the body writes first, then what its calls promise, so a class raised both ways is
        // reported where a reader can see the class written.
        let mut raised: Vec<(Name, TextRange, Option<String>, bool)> = declaration
            .throws
            .iter()
            .map(|site| (site.class.clone(), site.range, None, false))
            .collect();
        if let Some(body) = declaration.body {
            raised.extend(self.throws_from_calls(body).into_iter().map(|site| {
                (
                    site.thrown.class,
                    site.thrown.range,
                    Some(site.through),
                    site.declares,
                )
            }));
        }

        let mut found: Vec<UndeclaredThrow> = Vec::new();
        for (class, at, through, through_declares) in raised {
            if self.throw_documented(&class, &covered)
                || self.is_a_programming_error(class.as_str())
            {
                continue;
            }
            match found.iter_mut().find(|entry| entry.class == class) {
                Some(entry) => entry.sites.push(at),
                None => found.push(UndeclaredThrow {
                    class,
                    sites: vec![at],
                    through,
                    through_declares,
                }),
            }
        }
        found
    }
}
