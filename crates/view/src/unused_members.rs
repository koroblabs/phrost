//! Private members nothing in the file uses.
//!
//! The one kind of dead member a *file* can settle. `private` reaches exactly as far as the class
//! that declares it, and a class is one file, so the question needs no sweep and no index: what
//! reaches a private method is written above or below it. That is what makes this affordable per
//! keystroke where the same question about a public member is a project pass.
//!
//! Precision here means completeness in the other direction from a check about correctness, the
//! way [`locals_never_read`](Self::locals_never_read) already documents: a *missed use* is what
//! produces a false finding, so this stops entirely wherever the file can reach a member by a
//! name it does not write.
//!
//! # Where it stops, and why each one
//!
//! - **A name a template writes.** A Twig template calls a PHP method and no PHP file names it,
//!   and Twig's own shorthand means the name written is often not the name the method carries:
//!   `{{ trade.status.getDescription }}` and `{{ node.title }}` both reach a getter. Twelve of the
//!   reference project's 124 findings were one of these.
//! - **A name written anywhere as a string.** `[$this, 'refresh']`, `'method' => 'refresh'`, an
//!   event map. The call is real and the file never writes `->refresh`.
//! - **A file that reaches a member dynamically at all**: `$this->$name`, `call_user_func`,
//!   `->{...}`. One of those and nothing in the file can be judged.
//! - **A class that uses a trait.** A trait's code runs in the using class's scope, so
//!   `$this->refresh()` inside the trait calls this class's private method -- from another file.
//! - **A magic method.** `__construct`, `__get` and the rest are called by the engine, and a
//!   private `__construct` is how a singleton is written. On one real Drupal checkout every
//!   private method that nothing else in its file mentions is one of these -- 37 of them, and
//!   the honest count of dead private methods there is zero.
//! - **A property**, which is not a stop so much as a kind left out: 88 of Symfony's 113 findings
//!   were one, nearly all an entity's `$id` that an ORM hydrates by reflection. A file cannot see
//!   that and never will.
//!
//! - **A class a closure is bound to.** `Closure::bind($fn, null, CacheItem::class)` gives a
//!   closure another class's scope, so its private methods are callable from a file that does not
//!   declare them -- the one thing a file-local check would otherwise have to assume away. Which
//!   classes those are is a fact the index holds, filled where the body is in hand and asked here
//!   by short name: the `NeverReturns` shape, not a walk. Symfony's Cache component is written
//!   that way and `CacheItem` is the whole of why.

use phrost_model::{FileId, Name, TextRange, Visibility};
use phrost_sema::MemberKind;

use crate::IndexView;

/// A private member the file that declares it never uses.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct UnusedMember {
    pub name: Name,
    pub kind: MemberKind,
    /// The name alone, which is what a report underlines.
    pub range: TextRange,
    /// The class that declares it, for a message that says where to look.
    pub owner: Name,
    /// Whether the name is written somewhere else in the project all the same.
    ///
    /// Only ever set by the exact rule, and it is what makes the finding checkable. A reader's own
    /// test for "reached from nowhere" is a grep, and the exact rule answers a question a grep
    /// cannot: `toArray` is declared by sixty-four classes on the reference project and called
    /// constantly, and *this* class's copy is still dead. Reported without that clause the finding
    /// is true and reads as a lie, which is the same defect a propagated `@throws` had.
    ///
    /// False under the coarse rule by construction: it says nothing is reached precisely when the
    /// name is written nowhere, so the grep and the finding already agree.
    pub the_name_is_written_elsewhere: bool,
    /// Whether what is reported is a contract somebody else is meant to fulfil.
    ///
    /// An interface's method, or an abstract one. It earns a clause of its own because the first
    /// thing a reader asks of `AttachmentTypeInterface::buildJsonSchema is reached from nowhere` is
    /// "but two classes implement it" -- and the answer is that implementing a dead contract does
    /// not make it live, which the message has to say or the finding reads as a rule that forgot
    /// about interfaces.
    ///
    /// What makes the clause *true* is how [`crate::IndexView::reaches`] records: an access is a
    /// claim on every declaration in the receiver's hierarchy that it satisfies, so a call on any
    /// implementation reaches this declaration too. Reported at all means no such call exists --
    /// the implementations are silent about the contract only because
    /// [`IndexView::member_comes_from_above`] exempts them from being reported twice.
    pub the_declaration_is_a_contract: bool,
    /// Whether this *is* the class rather than something in it.
    ///
    /// A flag rather than a kind of its own, because everything else about the two is the same --
    /// a name, a range, and nothing reaching it. What it decides is the word a report uses, and
    /// it used to be decided by which `MemberKind` the class arm happened to pass: harmless while
    /// a class was the only thing that was not a method, and a lie the moment constants were
    /// reported too. [`Self::kind`] means nothing when this is set.
    pub the_class_itself: bool,
}

impl IndexView<'_> {
    /// Every member the current file's classes declare, whatever their visibility.
    ///
    /// For the project pass, which asks a different question with the same vocabulary: what a
    /// *file* can settle is only the private half, and what nothing anywhere calls needs every
    /// file. A magic method is left out here for the reason it is left out there -- the engine
    /// calls it -- and so is a member of a class a framework finds by scanning, whose methods are
    /// called through whatever interface it was found for.
    pub fn declared_members(&self) -> Vec<UnusedMember> {
        let Some(open) = self.current() else {
            return Vec::new();
        };
        let hir = open.hir();
        let mut found = Vec::new();
        for (_, class) in hir.classes() {
            for &id in hir.member_list(class.members) {
                let member = hir.member(id);
                if member.name.as_str().starts_with("__") {
                    continue;
                }
                let kind = match member.kind {
                    phrost_hir::MemberKind::Method => MemberKind::Method,
                    phrost_hir::MemberKind::ClassConstant => MemberKind::ClassConstant,
                    phrost_hir::MemberKind::Property | phrost_hir::MemberKind::EnumCase => continue,
                };
                found.push(UnusedMember {
                    name: member.name.clone(),
                    kind,
                    range: member.name_range,
                    owner: class.name.clone(),
                    // A file-local question, so neither what another file writes nor what
                    // another class implements says anything about it.
                    the_name_is_written_elsewhere: false,
                    the_declaration_is_a_contract: false,
                    the_class_itself: false,
                });
            }
        }
        found
    }

    /// Declarations in the current file that nothing in the *project* appears to reach.
    ///
    /// The editor's half of what `phrost unused` reports, and affordable for the reason that pass
    /// is not: the question "is this name written anywhere else" is a word set away, where "which
    /// declaration does that writing reach" is a sweep. So this is the coarse test -- a name no
    /// other file's set holds is used by nobody -- with the reachability rules that keep it from
    /// being a list of a framework's entry points.
    ///
    /// Not gated on the package being an application, which the report in `phrost unused` is.
    /// That report is run against whatever it is pointed at and has to say what it is looking
    /// at; this is a rule a project switches on, and switching it on is the same statement.
    ///
    /// The rules have to be the *same* rules the report gathers, and for two commits they were
    /// not: a class a framework builds because of what it inherits, and a member the project's
    /// configuration names, were in the report and missing here. Both are live entry points, so
    /// every one of them was reported as dead -- 8 of 76 findings on one real checkout. Nothing
    /// fails when a rule is missing from one of two assemblies of it; what is tested is the pair,
    /// in `lsp/src/tests/flow/unused_declarations.rs`, at the layer where a framework provider
    /// and a configuration file both exist.
    /// Whether this file reaches a member by a name it computes, so none of its members can be
    /// judged. Public for `phrost unused`, which asks the same question of the same files.
    pub fn reaches_a_member_by_a_computed_name(&self) -> bool {
        self.current()
            .is_some_and(|open| reaches_a_member_dynamically(open.text()))
    }

    pub fn declarations_never_reached(&self) -> Vec<UnusedMember> {
        let Some(open) = self.current() else {
            return Vec::new();
        };
        if reaches_a_member_dynamically(open.text()) {
            return Vec::new();
        }
        let file = open.file();
        let hir = open.hir();
        // The words, gathered once for every name the file declares, while they are what decides.
        let words = (!self.skeleton.reached_is_complete()).then(|| {
            let names = hir
                .classes()
                .filter(|(_, class)| !class.is_anonymous)
                .flat_map(|(_, class)| {
                    std::iter::once(class.name.as_str()).chain(
                        hir.member_list(class.members)
                            .iter()
                            .map(|&id| hir.member(id).name.as_str()),
                    )
                })
                .collect();
            crate::word_evidence::WordEvidence::gather(self.skeleton, file, open.text(), names)
        });
        let mut found = Vec::new();
        for (_, class) in hir.classes() {
            if class.is_anonymous {
                continue;
            }
            let fqn = self.qualified_name(class);
            // Asked once for the class rather than per member: it is a climb up the hierarchy,
            // and every member of the class has the same answer.
            let discovered = self.skeleton.is_discovered(&fqn)
                || self.skeleton.inherits_a_discovered_supertype(&fqn);
            // The class itself, unless a framework reaches it without anything naming it or the
            // project's configuration does.
            if !discovered
                && !self.class_is_reached(words.as_ref(), &fqn, class.name.as_str())
                && self.skeleton.config_sites_naming(&fqn).next().is_none()
            {
                found.push(UnusedMember {
                    name: class.name.clone(),
                    kind: MemberKind::ClassConstant,
                    range: class.name_range,
                    owner: class.name.clone(),
                    the_name_is_written_elsewhere: self
                        .also_written_elsewhere(file, class.name.as_str()),
                    // A class is not a contract in the sense the clause is about: what would be
                    // said of an interface nothing names is said of it already.
                    the_declaration_is_a_contract: false,
                    the_class_itself: true,
                });
                // Its members say nothing more than it does.
                continue;
            }
            for &id in hir.member_list(class.members) {
                let member = hir.member(id);
                // A method and a class constant, which are the two a project reaches by name and
                // nothing else. A *property* is left out for the reason the private check leaves
                // it out -- an ORM hydrates one by reflection and no file can see that -- and an
                // enum case for a nearer one: `cases()` and `from()` reach every case of an enum
                // without writing any of their names.
                let kind = match member.kind {
                    phrost_hir::MemberKind::Method => MemberKind::Method,
                    phrost_hir::MemberKind::ClassConstant => MemberKind::ClassConstant,
                    _ => continue,
                };
                // PHP calls a magic method; nobody writes its name.
                if member.name.as_str().starts_with("__")
                    // The framework, the configuration and the templates, in one question -- see
                    // `ProjectSkeleton::member_is_reached_without_a_call` for why it is one.
                    || self
                        .skeleton
                        .member_is_reached_without_a_call(&fqn, member.name.as_str())
                    || self.member_is_reached(words.as_ref(), &fqn, member.name.as_str())
                    || self.member_comes_from_above(&fqn, kind, member.name.as_str())
                {
                    continue;
                }
                found.push(UnusedMember {
                    name: member.name.clone(),
                    kind,
                    range: member.name_range,
                    owner: class.name.clone(),
                    the_name_is_written_elsewhere: self
                        .also_written_elsewhere(file, member.name.as_str()),
                    the_declaration_is_a_contract: class.kind == phrost_hir::ClassKind::Interface
                        || member.modifiers.is_abstract(),
                    the_class_itself: false,
                });
            }
        }
        found
    }

    /// Whether a name the exact rule called unreached is nonetheless written in another file.
    ///
    /// Asked only once a finding exists, which is what keeps a scan of every file's word set off
    /// the budget: it costs what one call of the coarse rule costs, and the coarse rule is asked
    /// per *member* where this is asked per finding. Measured over the reference project the whole
    /// difference is 80 ms on a 1.7 s run -- 0.65 ms a finding, 123 of them -- and the check it
    /// belongs to is off unless a project asks for it.
    ///
    /// Answers no while the coarse rule is the one deciding, because there the two questions are
    /// the same one and a clause saying "the name is written elsewhere" would contradict the
    /// sentence it is attached to.
    fn also_written_elsewhere(&self, file: FileId, name: &str) -> bool {
        self.skeleton.reached_is_complete() && self.skeleton.mentioned_outside(file, name)
    }

    /// Whether anything in the project reaches this member of this class.
    ///
    /// Two rules, and which one answers is a question about the *index* rather than about the
    /// code. Once every file has been analysed the answer is exact -- it is about this class's
    /// member, resolved -- and until then all that is held is which names are written where,
    /// which cannot tell `StockDataManager::importStockData` being called from the `importStockData`
    /// declared next to the call.
    ///
    /// The coarse rule accepts everything the precise one does, because a resolved call writes
    /// the name it calls. So the switch only ever *adds* findings and withdraws none, which is
    /// what lets the check answer from the moment the index is ready instead of going quiet until
    /// the sweep lands.
    ///
    /// Both halves of the coarse rule are here, the file's own text included: a name written
    /// anywhere is the same kind of evidence wherever it is written, and the exact rule subsumes
    /// both -- the declaring file records what *it* reaches like any other.
    ///
    /// `words` is the coarse rule's evidence, present exactly while the coarse rule decides.
    fn member_is_reached(
        &self,
        words: Option<&crate::word_evidence::WordEvidence<'_>>,
        owner: &str,
        name: &str,
    ) -> bool {
        match words {
            None => self.skeleton.member_is_reached(owner, name),
            Some(words) => words.mentioned_outside(name) || words.written_in(name),
        }
    }

    /// Whether anything outside this file names this class.
    ///
    /// The same two rules as [`Self::member_is_reached`] and for the same reasons, one level up.
    /// What the exact one adds here is the namespace: `App\Repo` and `Vendor\Repo` are one word
    /// and two classes, and a word test spares both because somebody uses one.
    fn class_is_reached(
        &self,
        words: Option<&crate::word_evidence::WordEvidence<'_>>,
        fqn: &str,
        short: &str,
    ) -> bool {
        match words {
            None => self.skeleton.class_is_reached(fqn),
            Some(words) => words.mentioned_outside(short),
        }
    }

    /// A class's fully-qualified name, from the namespace the file declares.
    pub(crate) fn qualified_name(&self, class: &phrost_hir::ClassData) -> String {
        match self.declared_namespace() {
            Some(namespace) => format!("{}\\{}", namespace.name, class.name.as_str()),
            None => class.name.as_str().to_owned(),
        }
    }

    /// Whether a class above `class_fqn` also declares this member.
    ///
    /// The test a report of unused *public* members cannot do without. A method that implements
    /// an interface or overrides a parent is called through the base -- `foreach` calls
    /// `getIterator` and never writes the name, a form's `getConfirmText` is called by the class
    /// that defines the form -- so the implementation is reached by something no file writes.
    ///
    /// Asked of the flattened table's own contributors, which is every class that put a member
    /// into it: the declaring class is one of them and the rest are exactly the hierarchy above.
    pub fn member_comes_from_above(&self, class_fqn: &str, kind: MemberKind, name: &str) -> bool {
        let table = self.members(class_fqn);
        table
            .contributors()
            .iter()
            .filter(|above| !above.as_str().eq_ignore_ascii_case(class_fqn))
            .any(|above| self.members(above.as_str()).find(kind, name).is_some())
    }

    /// Every identifier-shaped word the current file writes inside a quoted string.
    ///
    /// A callable passed as `[$this, 'refresh']`, a handler map, a route's `_controller`. None of
    /// them writes `->refresh` anywhere.
    pub fn quoted_member_words(&self) -> Vec<String> {
        self.current()
            .map(|open| quoted_words(open.text()))
            .unwrap_or_default()
    }

    /// Every private member of the current file that nothing in the current file reaches.
    pub fn private_members_never_used(&self) -> Vec<UnusedMember> {
        let Some(open) = self.current() else {
            return Vec::new();
        };
        let (hir, text) = (open.hir(), open.text());
        if reaches_a_member_dynamically(text) {
            return Vec::new();
        }

        // Every member name the file writes, however it writes it. Accesses and strings together,
        // because the second is how a callable is passed and the first cannot see it.
        let mut written: std::collections::HashSet<String> = self
            .member_accesses()
            .iter()
            .map(|access| access.name.as_str().to_ascii_lowercase())
            .collect();
        written.extend(quoted_words(text));

        let mut found = Vec::new();
        for (_, class) in hir.classes() {
            // A trait's private member is reached from every class that uses it, which is every
            // other file -- the opposite of what makes this affordable. Its own declaration says
            // nothing about whether anything calls it.
            if class.kind == phrost_hir::ClassKind::Trait || !class.uses.is_empty() {
                continue;
            }
            for &id in hir.member_list(class.members) {
                let member = hir.member(id);
                if member.visibility != Visibility::Private
                    || member.name.as_str().starts_with("__")
                    || written.contains(&member.name.as_str().to_ascii_lowercase())
                {
                    continue;
                }
                // And the one thing outside the file that can reach in: a closure somewhere in
                // the project given this class's scope. Asked of the index by short name, which
                // is the `NeverReturns` shape -- a fact the index holds that rules the question
                // out, rather than a walk that would have to read every file.
                if self.skeleton.is_bound_scope(class.name.as_str()) {
                    continue;
                }
                // The vocabulary a reader above this boundary has: `phrost-analysis` sees the
                // view and never the HIR, and the two enums are separate types on purpose.
                let kind = match member.kind {
                    phrost_hir::MemberKind::Method => MemberKind::Method,
                    phrost_hir::MemberKind::ClassConstant => MemberKind::ClassConstant,
                    // A private *property* is not file-decidable and the count says so: 88 of
                    // Symfony's 113 findings were one, nearly all of them an entity's `$id` that
                    // an ORM hydrates by reflection. Nothing in the file writes that access and
                    // nothing can, so the answer would be wrong for a whole way of writing PHP.
                    phrost_hir::MemberKind::Property | phrost_hir::MemberKind::EnumCase => continue,
                };
                found.push(UnusedMember {
                    name: member.name.clone(),
                    kind,
                    range: member.name_range,
                    owner: class.name.clone(),
                    // A file-local question, so neither what another file writes nor what
                    // another class implements says anything about it.
                    the_name_is_written_elsewhere: false,
                    the_declaration_is_a_contract: false,
                    the_class_itself: false,
                });
            }
        }
        found
    }
}

/// Words that reach a member the file does not name, as PHP spells them.
///
/// `include` and `require` are here because a template pulled into a method runs in that method's
/// scope and can call anything on `$this` -- Symfony's `HtmlErrorRenderer` is written that way and
/// five of its private methods are reached from nowhere its own text can see.
const REACHES_ANYTHING: [&str; 6] = [
    "include",
    "include_once",
    "require",
    "require_once",
    "call_user_func",
    "call_user_func_array",
];

/// Punctuation that names a member at runtime. Only meaningful in code, like the words above.
const COMPUTES_A_NAME: [&str; 6] = ["->$", "?->$", "::$$", "->{", "::{", "$$"];

/// Whether the file reaches a member by a name it computes.
///
/// One of these and no member in the file can be judged: the name is decided at runtime, so
/// nothing written here says which member is reached.
///
/// **Asked of the code and not of the text.** As a plain substring search this read `required
/// dependencies` in a docblock, a `$required` local and a `getRequired()` call as an `include`, and
/// one of those is in most files somebody wrote prose for: on the reference project it silenced
/// **97 of 196** findings -- half the check, dead, and dead in the direction nothing reports. Same
/// shape as the apostrophe above it and as `core.api.php` teaching the annotation syntax it was
/// scanned for: a text search whose vocabulary is not the language's.
fn reaches_a_member_dynamically(text: &str) -> bool {
    let mut found = false;
    scan(text, |code| {
        found = found
            || COMPUTES_A_NAME
                .iter()
                .any(|spelling| code.contains(spelling))
            || identifiers(code).any(|word| REACHES_ANYTHING.contains(&word.as_str()));
    });
    found
}

/// The identifier-shaped runs of a stretch of code, lowercased.
fn identifiers(code: &str) -> impl Iterator<Item = String> + '_ {
    code.split(|byte: char| !byte.is_ascii_alphanumeric() && byte != '_')
        .filter(|word| !word.is_empty())
        .map(str::to_ascii_lowercase)
}

/// Every identifier-shaped run written inside a quoted string, lowercased.
///
/// A callable is passed as `[$this, 'refresh']` and a handler map is written `'x' => 'refresh'`,
/// and neither writes `->refresh` anywhere. Whole strings are not enough -- `'App\\Foo::refresh'`
/// is one string naming a method -- so the words inside them are what counts.
///
pub(crate) fn quoted_words(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    scan_with_strings(
        text,
        |_| {},
        |inside| {
            out.extend(identifiers(inside));
        },
    );
    out
}

/// The stretches of `text` that are code: neither comment nor quoted string.
///
/// The two questions asked of a file's text here -- which names it writes in a string, and whether
/// it reaches a member by a computed name -- are two readings of the same scan, so they are one
/// scan. Keeping them apart is how a rule ends up written twice and only one copy learns something.
fn scan(text: &str, code: impl FnMut(&str)) {
    scan_with_strings(text, code, |_| {});
}

/// The scan itself: every run of code, and the inside of every quoted string.
///
/// **Comments are skipped, and that is the whole reason this is a state machine rather than a
/// search for quotes.** An apostrophe in English prose -- `Drupal's native authentication stack`
/// in a docblock -- reads as an opening quote, and everything to the next apostrophe becomes one
/// string: the quote state is inverted for the rest of the file, so every name after it lands
/// *outside* a string and is never recorded. On one real subscriber that lost `'onException'` from
/// a `getSubscribedEvents()` map eight lines later, and reported a live event handler as reached
/// from nowhere.
///
/// `#[` opens an attribute and not a comment -- the one place PHP spells a comment the way it
/// spells something else entirely.
fn scan_with_strings(text: &str, mut code: impl FnMut(&str), mut string: impl FnMut(&str)) {
    let bytes = text.as_bytes();
    let (mut at, mut code_from) = (0, 0);
    let hand_over = |code: &mut dyn FnMut(&str), from: usize, to: usize| {
        if let Some(run) = text.get(from..to)
            && !run.is_empty()
        {
            code(run);
        }
    };
    while at < bytes.len() {
        match bytes[at] {
            b'/' if bytes.get(at + 1) == Some(&b'/') => {
                hand_over(&mut code, code_from, at);
                at = line_end(bytes, at);
                code_from = at;
            }
            b'#' if bytes.get(at + 1) != Some(&b'[') => {
                hand_over(&mut code, code_from, at);
                at = line_end(bytes, at);
                code_from = at;
            }
            b'/' if bytes.get(at + 1) == Some(&b'*') => {
                hand_over(&mut code, code_from, at);
                at = match text[at + 2..].find("*/") {
                    Some(end) => at + 2 + end + 2,
                    None => bytes.len(),
                };
                code_from = at;
            }
            quote @ (b'\'' | b'"') => {
                hand_over(&mut code, code_from, at);
                let from = at + 1;
                let mut to = from;
                while to < bytes.len() && bytes[to] != quote {
                    // A backslash escapes the next byte, including a quote of the same kind.
                    to += if bytes[to] == b'\\' { 2 } else { 1 };
                }
                if let Some(inside) = text.get(from..to.min(bytes.len())) {
                    string(inside);
                }
                at = to.saturating_add(1);
                code_from = at.min(bytes.len());
            }
            _ => at += 1,
        }
    }
    hand_over(&mut code, code_from, bytes.len());
}

/// Where the line holding `at` ends, or the end of the text.
fn line_end(bytes: &[u8], at: usize) -> usize {
    bytes[at..]
        .iter()
        .position(|byte| *byte == b'\n')
        .map_or(bytes.len(), |offset| at + offset)
}

#[cfg(test)]
mod tests {
    use super::{quoted_words, reaches_a_member_dynamically};

    /// Prose is not a language construct, and this rule silences a whole file.
    ///
    /// As a plain substring search, `required dependencies` in a docblock, a `$required` local and
    /// a `getRequired()` call each read as an `include` -- and one of those is in most files
    /// somebody wrote prose for. On the reference project it silenced 97 of 196 findings: half the
    /// check, dead, in the direction nothing reports.
    #[test]
    fn a_word_in_prose_is_not_a_construct_that_reaches_anything() {
        let quiet = r#"<?php

/**
 * Constructs a new instance with the required dependencies.
 */
class Service
{
    public function run(array $required): string
    {
        $required = $this->settings->getRequired();
        return implode(',', $required);
    }
}
"#;
        assert!(!reaches_a_member_dynamically(quiet));

        // And the constructs themselves still stop it, in every spelling PHP has for them.
        for reaching in [
            "<?php\ninclude __DIR__ . '/t.php';\n",
            "<?php\nrequire_once __DIR__ . '/t.php';\n",
            "<?php\ncall_user_func([$this, 'refresh']);\n",
            "<?php\n$this->$name();\n",
            "<?php\n$this->{$name}();\n",
            "<?php\nSelf::$$name;\n",
        ] {
            assert!(
                reaches_a_member_dynamically(reaching),
                "{reaching:?} reaches a member this file does not name"
            );
        }
    }

    /// A construct written inside a string reaches nothing, which is the other half of the same
    /// reading: the scan that skips a comment skips a string too.
    #[test]
    fn a_construct_quoted_as_text_reaches_nothing() {
        assert!(!reaches_a_member_dynamically(
            "<?php\n$sql = 'include the required rows';\n"
        ));
    }

    /// An apostrophe in prose is not an opening quote, and the file after it is not one string.
    ///
    /// The shape that cost a real event subscriber its handler: `Drupal's` in a docblock swallowed
    /// everything to the next apostrophe, which inverted the quote state for the rest of the file
    /// -- so `'onException'` eight lines later landed *outside* a string, was never recorded as
    /// reached, and a live handler was reported as dead. A plain list of what the scanner should
    /// see, because what went wrong was what it saw.
    #[test]
    fn prose_between_the_strings_does_not_invert_the_quote_state() {
        let text = r#"<?php

/**
 * Routes that bypass Drupal's native authentication stack.
 */
class Subscriber
{
    // The user's own handler, named here and nowhere else.
    public static function getSubscribedEvents(): array
    {
        # A hash comment's apostrophe counts too.
        return ['kernel.exception' => [['onException', 49]]];
    }
}
"#;
        assert_eq!(
            quoted_words(text),
            ["kernel", "exception", "onexception"],
            "prose apostrophes leaked into the quote state"
        );
    }

    /// `#[` opens an attribute, which is the one place PHP spells a comment the way it spells
    /// something else -- skipping to the end of that line would lose every name an attribute names.
    #[test]
    fn an_attribute_is_not_a_hash_comment() {
        let text = r#"<?php
#[Route('/health', name: 'app_core.health')]
public function check() {}
"#;
        assert_eq!(quoted_words(text), ["health", "app_core", "health"]);
    }

    /// A comment opener written inside a string is text, not a comment.
    #[test]
    fn a_comment_opener_inside_a_string_is_part_of_the_string() {
        let text = r#"<?php
$url = 'https://example.test/path';
$map = ['handler' => 'refresh'];
"#;
        assert_eq!(
            quoted_words(text),
            ["https", "example", "test", "path", "handler", "refresh"]
        );
    }
}
