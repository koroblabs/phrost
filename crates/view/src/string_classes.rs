//! A class named by a string literal, which is how PHP named one before `::class`.
//!
//! `class_exists('App\Repo')`, `is_a($x, 'App\Repo')`, a container definition keyed by a class name,
//! an annotation's `'entity_class'`: each holds a name the index knows, and none of them was a
//! place a cursor could do anything at. Goto answered nothing, hover answered nothing, and
//! find-references listed every other spelling of the class and not these.
//!
//! # Only a string that already resolves
//!
//! What keeps this from claiming every string in a project is that the name has to *be* a class the
//! index holds. A string is data -- a message, a key, a path -- and nearly none of them resolve; the
//! ones that do were written to name a class, because nothing else is spelled `App\Repo`.
//!
//! Read as fully qualified, which is what PHP does with one: a string is not affected by the file's
//! `use` statements, so `'Repo'` inside `namespace App;` names the *global* `Repo` and not
//! `App\Repo`. Getting that backwards would send goto to the wrong class in exactly the files where
//! both exist.

use phrost_model::TextSize;

use crate::{CursorTarget, view::IndexView};

impl IndexView<'_> {
    /// The class a string literal under the cursor names, when the index holds one.
    pub fn class_named_by_a_string_at(&self, offset: TextSize) -> Option<CursorTarget> {
        let open = self.current()?;
        let quoted = open.hir().string_at(offset)?;
        let inside = self.inside_the_quotes(quoted)?;
        let located = self.class_spelled(inside)?;

        Some(CursorTarget::WrittenName {
            // The name the index holds rather than the one the string spelled, so two castings of
            // one class -- `app\repo` and `App\Repo` -- are one answer.
            path: located.fqn,
            anchor: phrost_hir::NameAnchor::FullyQualified,
            kind: phrost_view_import_kind(),
            range: inside,
        })
    }
}

impl IndexView<'_> {
    /// The class a string literal names, for a value that is one: `$class = '\\Countable'; new
    /// $class()`.
    ///
    /// The cursor's rule read off an expression rather than a position, so the two cannot disagree
    /// about which strings name a class. Only a quoted literal: a heredoc's text is not its value.
    pub(crate) fn class_a_literal_names(
        &self,
        literal: phrost_model::TextRange,
    ) -> Option<phrost_model::Name> {
        let text = self.current()?.text();
        if !matches!(
            text.as_bytes().get(usize::from(literal.start())),
            Some(b'\'' | b'"')
        ) {
            return None;
        }
        self.class_spelled(self.inside_the_quotes(literal)?)
            .map(|located| located.fqn)
    }

    /// The class the text between a string's quotes names, when the index holds one.
    fn class_spelled(&self, inside: phrost_model::TextRange) -> Option<crate::SymbolLocation> {
        let written = self
            .current()?
            .text()
            .get(usize::from(inside.start())..usize::from(inside.end()))?;

        // A double-quoted string may interpolate, and one that does is not a name. The cheap test
        // is the one that matters: a `$` anywhere in it means the text at rest is not the text at
        // run time.
        if written.is_empty() || written.contains('$') {
            return None;
        }
        let written = unescaped(written);
        let path = written.trim_start_matches('\\');
        let located = self.class_location(path)?;
        // The index folds case, as PHP does for a class -- so without this `'name' => ...` was a
        // reference to whichever global `Name` the project or its dependencies' test data
        // declares, and goto and hover on an array key landed in PHP_CodeSniffer's fixtures.
        if !names_the_class(&written, located.fqn.as_str())
            || self.reads_as_a_word(&written, located.file)
        {
            return None;
        }
        Some(located)
    }

    /// The method a string literal under the cursor names, when a framework's spelling says it is
    /// one of the enclosing class's.
    ///
    /// Drupal's form API writes `'::districtCallback'` and resolves the leading `::` against the form
    /// object, which is the class the array was written in -- the same target `[$this, 'save']`
    /// already reaches. Two spellings of one callable, and only one was taught.
    ///
    /// The prefix comes from the framework rather than from here: nothing in PHP itself makes a
    /// string name a method, and a rule that assumed one would claim ordinary strings in projects
    /// that use no framework at all.
    pub fn own_method_named_by_a_string_at(&self, offset: TextSize) -> Option<CursorTarget> {
        let open = self.current()?;
        let quoted = open.hir().string_at(offset)?;
        let inside = self.inside_the_quotes(quoted)?;
        let written = open
            .text()
            .get(usize::from(inside.start())..usize::from(inside.end()))?;

        let name = self.skeleton().own_method_named(written)?;
        let class = self.enclosing_class(inside.start())?;
        // The class has to actually declare it: a string starting with the prefix and naming
        // nothing is a string, and answering for it would send goto somewhere invented.
        self.member_location(class.as_str(), phrost_sema::MemberKind::Method, name)?;
        // The name as the member table spells it, so a report and a rename both use the
        // declaration's own casing rather than what the string happened to write.
        let table = self.members(class.as_str());
        let name = table
            .find(phrost_sema::MemberKind::Method, name)?
            .name
            .clone();

        // The method's own half of the string, so a cursor on the prefix is on the same target and
        // a rename replaces the name rather than the whole literal.
        let range = phrost_model::TextRange::new(
            inside.end() - phrost_model::TextSize::try_from(name.as_str().len()).ok()?,
            inside.end(),
        );

        Some(CursorTarget::MemberAccess {
            owner: class,
            name,
            kind: phrost_sema::MemberKind::Method,
            range,
        })
    }

    /// Every string in the current file that names a method of the class it is written in through
    /// a framework's spelling, with the range of the name and that class.
    ///
    /// The whole-file counterpart of [`Self::own_method_named_by_a_string_at`], for the reader that
    /// lists uses rather than following one: `'#submit' => ['::saveNote']` is a use of `saveNote`,
    /// and a rename that did not edit it left the form calling a method that no longer exists. Only
    /// the name is matched here; which declaration the class reaches is the caller's to decide,
    /// since the method may be inherited.
    pub fn own_method_strings(
        &self,
        name: &str,
    ) -> Vec<(phrost_model::TextRange, phrost_model::Name)> {
        let Some(open) = self.current() else {
            return Vec::new();
        };
        let text = open.text();
        open.hir()
            .string_literals()
            .filter_map(|quoted| self.inside_the_quotes(quoted))
            .filter_map(|inside| {
                let written = text.get(usize::from(inside.start())..usize::from(inside.end()))?;
                let named = self.skeleton().own_method_named(written)?;
                if !named.eq_ignore_ascii_case(name) {
                    return None;
                }
                let class = self.enclosing_class(inside.start())?;
                let range = phrost_model::TextRange::new(
                    inside.end() - phrost_model::TextSize::try_from(named.len()).ok()?,
                    inside.end(),
                );
                Some((range, class))
            })
            .collect()
    }

    /// Every string literal in the current file that names this class.
    ///
    /// For rename and find-references, which walk written *names* -- and a string holds none, so a
    /// class named by one was renamed everywhere except where it was spelled out. A Drupal
    /// annotation, a container definition and a `class_exists` guard are all this shape.
    ///
    /// Which strings count is [`names_the_class`]'s to say, the same rule a cursor on one follows.
    pub fn string_class_references(&self, fqn: &str) -> Vec<phrost_model::TextRange> {
        let Some(open) = self.current() else {
            return Vec::new();
        };
        let text = open.text();
        let wanted = fqn.trim_start_matches('\\');
        let declared_in = self.class_location(wanted).map(|located| located.file);

        open.hir()
            .string_literals()
            .filter_map(|quoted| self.inside_the_quotes(quoted))
            .filter_map(|inside| {
                let written = text.get(usize::from(inside.start())..usize::from(inside.end()))?;
                if !names_the_class(written, wanted)
                    || declared_in.is_some_and(|file| self.reads_as_a_word(written, file))
                {
                    return None;
                }
                // The whole name, as a `use` line's reference is: a cursor on its namespace is on
                // the class as much as one on its last segment, and a search from there has to
                // list where it stands. What a rename replaces is narrower, and `rename_range`
                // narrows every reference the same way.
                Some(inside)
            })
            .collect()
    }
}

impl IndexView<'_> {
    /// Whether a string spelling a dependency's global class is the word rather than the class.
    ///
    /// A dependency names its classes in a namespace, so a global one it declares is legacy code
    /// or a fixture -- on the reference project `enum Name` in PHP_CodeSniffer's test data, where
    /// every `'Name'` the project wrote went: a key, a label, `$this->t('Name')`, `$fields['Name']`.
    /// Only a name of one word, because that is the shape a coincidence takes: `'WP_Query'` and
    /// `'DateTime'` are not written by accident. And only a dependency's: the language's own
    /// `'Exception'` and a global class the project declares itself are what a string is written
    /// to name.
    fn reads_as_a_word(&self, written: &str, declared_in: phrost_model::FileId) -> bool {
        let bare = written.trim_start_matches('\\');
        let one_word = !bare.contains(['\\', '_'])
            && !bare
                .chars()
                .skip(1)
                .any(|letter| letter.is_ascii_uppercase());
        one_word
            && self
                .skeleton
                .file(declared_in)
                .is_some_and(|file| file.origin() == phrost_model::SourceOrigin::Vendor)
    }
}

/// Whether a string's text names the class the index spells `fqn`.
///
/// Compared with a leading separator taken off both: `'\App\Repo'` and `'App\Repo'` are one class
/// written two ways.
///
/// **Case-insensitively only where the name is qualified**, which is where a coincidence cannot
/// happen: nothing writes `'App\Repo'` by accident. A name in the global namespace is a bare word,
/// and matching one loosely reads data as a reference -- renaming a global `function fail()` turned
/// `new \LogicException('Fail')` and `'body' => 'FAIL'` into `'fail'` across four of Symfony's test
/// files, and goto on the key of `'name' => 'Received'` landed on an `enum Name` in
/// PHP_CodeSniffer's test data. Exact case keeps what this exists for -- `class_exists('WP_Query')`,
/// a container's `class:`, an autoload map's `'Psl\Iter\random'` -- and drops the word that
/// merely reads like the name.
///
/// One function for the cursor and for the list of references, because they had one rule each:
/// the list had learned about case and the cursor had not, so a rename refused a string that goto
/// and hover still claimed.
fn names_the_class(written: &str, fqn: &str) -> bool {
    let written = unescaped(written);
    let written = written.trim_start_matches('\\');
    let fqn = fqn.trim_start_matches('\\');
    match fqn.contains('\\') {
        true => written.eq_ignore_ascii_case(fqn),
        false => written == fqn,
    }
}

/// A string's text as PHP reads it, as far as a class name goes.
///
/// `'App\\Repo'` is `App\Repo` at run time, in either kind of quote: a doubled separator is the
/// escape, and the spelling a careful author writes. Read here once, so the cursor and the list of
/// references cannot disagree about it.
fn unescaped(written: &str) -> std::borrow::Cow<'_, str> {
    match written.contains("\\\\") {
        true => std::borrow::Cow::Owned(written.replace("\\\\", "\\")),
        false => std::borrow::Cow::Borrowed(written),
    }
}

/// The one kind a string can name here.
///
/// Its own function rather than the constant inline, because the two spellings of `ImportKind` in
/// this crate -- the re-export and the `phrost_hir` original -- are the same type and reading which
/// one a caller means takes a moment.
fn phrost_view_import_kind() -> phrost_hir::ImportKind {
    phrost_hir::ImportKind::ClassLike
}
