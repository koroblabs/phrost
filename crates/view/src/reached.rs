//! What analysing one file found it reaches, for the project's record of it.
//!
//! The producer side of [`Reached`](phrost_index::Reached). Every file the project owns is
//! analysed anyway — the project-wide pass does the closed ones, the buffer check the open ones —
//! and until this existed the one thing that analysis settles about *other* files was thrown
//! away: which member of which class each access turned out to name.
//!
//! Nothing here resolves anything. `member_accesses` has already done it, and the one extra
//! question asked per access is which class *declares* what it resolved to, out of the flattened
//! table a request has already built.

use phrost_index::{Reaches, WordSet};

use crate::{IndexView, MemberAccessOutcome};

impl IndexView<'_> {
    /// What the current file reaches, as the pair and name sets the index records.
    ///
    /// Two sets and not one, because an access that resolves to nothing says *something is
    /// called* and nothing about whose it is — see the note on [`Reaches`]. Reading it as "nobody
    /// calls this member" is how a report about absence earns a false positive, so it goes in as
    /// a bare name and keeps every declaration of that name alive.
    pub fn reaches(&self) -> Reaches {
        let accesses = self.member_accesses();
        let mut pairs = Vec::with_capacity(accesses.len());
        let mut loose = Vec::new();
        // The classes using each trait an access resolved on, asked once per trait: a pass over
        // the class table, and a trait's file is the only place this is asked from.
        let mut users_of: std::collections::HashMap<String, Vec<String>> =
            std::collections::HashMap::new();
        // A double's configured name is skipped: the double answers in the member's place, so the
        // member itself never runs because a test named it.
        for access in accesses.iter().filter(|access| !access.named) {
            let name = access.name.as_str();
            // The class the access resolved through is the *receiver's*, and a declaration is
            // asked about itself -- so what is recorded is every declaration in that receiver's
            // hierarchy that this call satisfies, not the one class it was written on.
            //
            // Both directions matter and each was a false positive on a real project. Downward:
            // an inherited method is called on the subclass and declared on the parent, so
            // recording the receiver leaves the parent's declaration looking dead. Upward: an
            // interface method is *the* contract, and a project whose callers happen to be typed
            // as the implementation would otherwise report every interface it defines as dead.
            let mut declared_anywhere = false;
            if let MemberAccessOutcome::Resolved { class } = &access.outcome {
                for above in self.members(class.as_str()).contributors() {
                    if self.declares_member(above.as_str(), access.kind, name) {
                        pairs.push(WordSet::fold_pair(above.as_str(), name));
                        declared_anywhere = true;
                    }
                }
                // A trait's code runs in the class that uses it, so `$this->productTypes()` inside
                // one -- resolved to the trait's own `abstract` declaration -- calls the using
                // class's method and satisfies the interface that class implements. Recorded on
                // the trait alone, both read as dead on the reference project.
                if self.is_trait(class.as_str()) {
                    let users = users_of
                        .entry(class.as_str().to_ascii_lowercase())
                        .or_insert_with(|| {
                            self.skeleton
                                .implementers_of(class.as_str())
                                .into_iter()
                                .map(|(_, user)| user.fqn().as_str().to_owned())
                                .collect()
                        });
                    for user in users.iter() {
                        for above in self.members(user).contributors() {
                            if self.declares_member(above.as_str(), access.kind, name) {
                                pairs.push(WordSet::fold_pair(above.as_str(), name));
                            }
                        }
                    }
                }
            }
            if !declared_anywhere {
                loose.push(WordSet::fold(name));
            }
        }
        // A name written inside a string is a call whose receiver is not written at all:
        // `[$this, 'refresh']`, a handler map, a route's `_controller`. Loose for the same reason
        // an unresolved receiver is.
        loose.extend(
            self.quoted_member_words()
                .iter()
                .map(|word| WordSet::fold(word)),
        );

        // And the classes, which is the same question one level up and the same two answers. A
        // reference that resolved is a claim on *that* class, fully qualified -- which is the
        // half a word cannot say, since `App\Repo` and `Vendor\Repo` are one word and two
        // classes. One that resolved to nothing is a claim on the word, docblock prose included:
        // `@see Monica` names no class and must not be read as proof that none is needed.
        //
        // A file's references to what it declares itself are left out, for the same reason
        // `mentioned_outside` leaves out the whole declaring file: a class that only names itself
        // is not used by anybody, and counting it would make this rule *withdraw* a finding the
        // coarse one gives -- which is the one thing the switch between them must never do.
        let namespace = self.declared_namespace().map(|declared| declared.name);
        let own: std::collections::HashSet<String> = self
            .current()
            .map(|open| {
                open.hir()
                    .classes()
                    .map(|(_, class)| match &namespace {
                        Some(namespace) => format!("{namespace}\\{}", class.name.as_str()),
                        None => class.name.as_str().to_owned(),
                    })
                    .map(|fqn| fqn.to_ascii_lowercase())
                    .collect()
            })
            .unwrap_or_default();
        let mut classes = Vec::new();
        for reference in self.name_references() {
            match &reference.resolved {
                Some(resolved) if !own.contains(&resolved.as_str().to_ascii_lowercase()) => {
                    classes.push(WordSet::fold(resolved.as_str().trim_start_matches('\\')));
                }
                Some(_) => {}
                None => loose.push(WordSet::fold(
                    reference.path.as_str().rsplit('\\').next().unwrap_or(""),
                )),
            }
        }
        Reaches::of(pairs, classes, loose)
    }
}
