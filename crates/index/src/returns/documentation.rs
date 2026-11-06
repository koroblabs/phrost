//! What a framework's own documentation conventions say, and how to read one.
//!
//! Two conventions, and they answer opposite questions about the same comment. One says the
//! documentation for this declaration is on *another* one -- `Implements hook_cron().` -- and the
//! other says this class's docblock describes the keys of an array somebody writes elsewhere.
//!
//! Both arrive from the seam as data and are *read* here, which is the rule the seam exists for: a
//! framework contributes the words its standard uses and no framework gets to run code below the
//! boundary. So the parsing lives in this crate, next to the tables it fills, and `ReturnContracts`
//! still owns them -- a child module reaches its private fields.

use std::ops::Range;

use super::ReturnContracts;

/// How a framework writes "what documents this is written on another declaration".
///
/// Strings, like everything else here: nothing in this crate names a framework, and the words a
/// coding standard uses are the framework's to choose. What the parts mean is written where they
/// come from; what they are *for* is the two questions below, which are asked in different places
/// and must not be allowed to disagree about what a claim looks like.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DocumentedByRule {
    /// The words the prose opens with when it is making the claim.
    pub lead: Box<str>,
    /// What the declaration it names is called before the part that varies.
    pub names: Box<str>,
    /// The catalog holding the names a declaration making the claim may begin with, when the
    /// framework dispatches to it by name. `None` leaves nothing to check.
    pub implementers: Option<Box<str>>,
    /// The catalog holding the names the project dispatches from its own calls. `None` leaves the
    /// claim answerable only by a declaration.
    pub dispatched: Option<Box<str>>,
}

impl DocumentedByRule {
    /// Where in `line` the declaration this claims to implement is named.
    ///
    /// The lead only at the start, and only as the whole opening word of the line: a sentence that
    /// mentions it in passing is prose, and reading that as a claim would attribute another
    /// declaration's documentation to this one.
    ///
    /// The **name** may sit further along, though, and this used to demand it immediately after the
    /// lead. `Implements custom hook_report_types_info().` is how a project writes a hook of its
    /// own, three times on one real checkout, and phpcs reads it as a claim too -- coder's
    /// `HookComment` sniff anchors the *end* of the line (` hook_x().`) and does not care what sits
    /// between. Demanding the next word meant phrost read it as ordinary prose and offered to write
    /// the `@param` and `@return` the sniff then reports as duplicated: the two tools disagreeing
    /// about one sentence, with our answer generating the other's error.
    ///
    /// So every word start after the lead is tried. What that risks is a sentence which opens with
    /// the lead and mentions a name in passing -- `Implements the alter half of hook_form_alter().`
    /// -- and there is none in 30 000 files, nor would the sniff read one differently.
    ///
    /// Compared as bytes, and that is not a shortcut: prose is UTF-8, and slicing it by a prefix's
    /// *length* can land inside a character -- an em dash in a docblock panicked the checker once.
    /// A framework's own words are ASCII, so byte equality is the same answer without the boundary.
    pub fn claim_in(&self, line: &str) -> Option<Range<usize>> {
        let indent = line.len() - line.trim_start().len();
        let after_lead = indent + self.lead.len();
        if !line
            .as_bytes()
            .get(indent..after_lead)?
            .eq_ignore_ascii_case(self.lead.as_bytes())
        {
            return None;
        }

        let rest = line.get(after_lead..)?;
        let gap = rest.len() - rest.trim_start().len();
        if gap == 0 {
            // `Implementsomething` is one word, not a claim followed by a name.
            return None;
        }

        let from = after_lead + gap;
        let mut before = ' ';
        for (at, character) in line.char_indices() {
            // A word start rather than any offset, so the prefix is matched where a name could
            // begin: `hook_` inside `pre_hook_x` is not a name that starts there.
            if at >= from
                && !is_name_char(before)
                && let Some(found) = self.name_at(line, at)
            {
                return Some(found);
            }
            before = character;
        }
        None
    }

    /// Every declaration this convention names in `text`, wherever each is written.
    ///
    /// The claim is one of them; a `@see hook_theme()` and a mention in the prose are the others, and
    /// they are the same kind of thing -- a name of a declaration the index holds, written where
    /// nothing else would look for one. Found by the prefix at a word boundary, so `$hook_info` and
    /// `mymodule_hook_x` are words that merely contain it rather than references.
    pub fn references_in(&self, text: &str) -> Vec<Range<usize>> {
        let mut found = Vec::new();
        let mut at = 0;
        while let Some(hit) = text.get(at..).and_then(|rest| rest.find(&*self.names)) {
            let start = at + hit;
            at = start + self.names.len();
            let opens = text[..start]
                .chars()
                .next_back()
                .is_none_or(|before| !is_name_char(before) && before != '$');
            if opens && let Some(range) = self.name_at(text, start) {
                at = range.end;
                found.push(range);
            }
        }
        found
    }

    /// The whole name beginning at `start`, when one written there belongs to this convention.
    ///
    /// Longer than the prefix, because the prefix alone names nothing: `Implements hook_` with the
    /// hook left out is an unfinished sentence, and reporting it as a reference to a declaration
    /// called `hook_` is worse than reading it as prose.
    fn name_at(&self, text: &str, start: usize) -> Option<Range<usize>> {
        let after_prefix = start + self.names.len();
        if !text
            .as_bytes()
            .get(start..after_prefix)?
            .eq_ignore_ascii_case(self.names.as_bytes())
        {
            return None;
        }

        let tail = text.get(after_prefix..)?;
        let length = tail
            .char_indices()
            .find(|(_, character)| !is_name_char(*character))
            .map_or(tail.len(), |(at, _)| at);
        (length > 0).then(|| start..after_prefix + length)
    }
}

/// A declaration whose docblock a framework shows to a person, by the shape of its name.
///
/// The index-side half of the framework seam's `UserFacingDocblock`, which says what the shape is;
/// the reading of it is here, so that the seam carries data and no framework gets to run code below
/// the boundary.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct UserFacingDocblockRule {
    /// What the name carries before the part that varies.
    pub names: Box<str>,
    /// Whether what follows has to be a number.
    pub numbered: bool,
}

impl UserFacingDocblockRule {
    /// Whether a declaration called this is one of them.
    ///
    /// The **last** occurrence of the infix, with something before it and something after: the
    /// framework's own pattern is greedy on what comes before, so `a_update_b_update_8001` is one
    /// update numbered `8001` rather than one named `b_update_8001`. `numbered` then asks the tail
    /// to be digits, which is the whole of what tells `mymodule_update_8001` from
    /// `mymodule_update_status` -- and a rule that skipped it would take a documentation check off
    /// every ordinary function whose name happens to carry the word.
    pub fn covers(&self, name: &str) -> bool {
        let Some((before, tail)) = name.rsplit_once(&*self.names) else {
            return false;
        };
        if before.is_empty() || tail.is_empty() {
            return false;
        }
        !self.numbered || tail.bytes().all(|byte| byte.is_ascii_digit())
    }
}

/// Whether a character can be part of a declaration's name.
///
/// PHP's own rule, which is what decides where a name written in prose ends.
fn is_name_char(character: char) -> bool {
    character == '_' || character.is_ascii_alphanumeric() || !character.is_ascii()
}

/// Where a class writes down the keys of the array its catalog entry configures.
///
/// The index-side half of the framework seam's `DocumentedKeys`, which says what the
/// convention looks like; the reading of it is here, so that the seam carries data and no framework
/// gets to run code below it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DocumentedKeysRule {
    /// The key whose value chooses the class: `#type`.
    pub discriminator: Box<str>,
    /// The catalog that value is a key of.
    pub catalog: Box<str>,
    /// What every documented key begins with: `#`.
    pub prefix: Box<str>,
}

/// One key a class's docblock says the array it configures accepts.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DocumentedKey {
    /// The key as written, sigil included: `#maxlength`.
    pub name: Box<str>,
    /// The parenthesised note the convention puts at the head of the description -- `(bool)`,
    /// `(string[], read-only)` -- without its parentheses, when there is one.
    ///
    /// Worth keeping apart from the prose because it is the only statement anywhere of what the key
    /// holds, and a completion list has a column for exactly that.
    pub kind: Option<Box<str>>,
    /// The first sentence of the description, with continuation lines joined.
    pub summary: Box<str>,
}

impl DocumentedKeysRule {
    /// Every key a docblock documents, in the order it wrote them.
    ///
    /// A bullet and nothing above it. `- #maxlength: Maximum number of characters of input
    /// allowed.` is the shape, with the description running on over as many more-indented lines as
    /// it needs -- so a line is a new key exactly when it opens a bullet whose name carries the
    /// prefix and ends in a colon, and everything until the next such line belongs to the one
    /// before it.
    ///
    /// Deliberately not anchored on a heading: the rule's own `prefix` field records the two
    /// spellings one framework's base classes use, which between them rule a heading out.
    pub fn keys_in(&self, docblock: &str) -> Vec<DocumentedKey> {
        let mut found: Vec<DocumentedKey> = Vec::new();
        let mut running = String::new();
        let mut open = false;

        for line in docblock.lines() {
            let line = line.trim_start().trim_start_matches('*').trim();
            match self.opens_a_key(line) {
                Some((name, rest)) => {
                    Self::settle(&mut found, &mut running);
                    found.push(DocumentedKey {
                        name: name.into(),
                        kind: None,
                        summary: Box::from(""),
                    });
                    running.push_str(rest);
                    open = true;
                }
                // Only while a bullet is open. A blank line, a tag and a bullet this cannot spell
                // all close one -- which is what keeps the prose *around* the list, and the `@see`
                // under it, from being attached to whichever key happened to come last.
                None if open
                    && !line.is_empty()
                    && !line.starts_with('-')
                    && !line.starts_with('@') =>
                {
                    running.push(' ');
                    running.push_str(line);
                }
                None => {
                    Self::settle(&mut found, &mut running);
                    open = false;
                }
            }
        }
        Self::settle(&mut found, &mut running);

        found
    }

    /// The key a line opens, when it opens one.
    fn opens_a_key<'a>(&self, line: &'a str) -> Option<(&'a str, &'a str)> {
        let bullet = line.strip_prefix("- ")?;
        if !bullet.starts_with(&*self.prefix) {
            return None;
        }
        let (name, rest) = bullet.split_once(':')?;
        // A name is one word. `- #ajax: Array of elements` opens a key and `- #prefix and #suffix:`
        // is prose about two of them, which nothing can offer as a single key.
        (!name.contains(char::is_whitespace)).then(|| (name, rest.trim_start()))
    }

    /// Splits the description that has accumulated into the kind and the first sentence.
    fn settle(found: &mut [DocumentedKey], running: &mut String) {
        let text = running.trim();
        // Nothing accumulated is a bullet already settled, not one whose description is empty:
        // overwriting it here is how the `@see` at the foot of a docblock became the last key's
        // summary.
        if text.is_empty() {
            running.clear();
            return;
        }
        let Some(last) = found.last_mut() else {
            running.clear();
            return;
        };
        let (kind, rest) = match text.strip_prefix('(').and_then(|rest| rest.split_once(')')) {
            Some((kind, rest)) => (Some(Box::from(kind)), rest.trim_start()),
            None => (None, text),
        };
        last.kind = kind;
        last.summary = Box::from(Self::first_sentence(rest));
        running.clear();
    }

    /// Up to the first sentence end, which is what a one-line detail has room for.
    ///
    /// `. ` and not `.`, because the descriptions are full of `\Drupal\Core\Render\Element` and
    /// `#prefix, #suffix, etc.` and cutting at either leaves half a word.
    fn first_sentence(text: &str) -> &str {
        match text.find(". ") {
            Some(at) => &text[..=at],
            None => text,
        }
    }
}

impl ReturnContracts {
    /// Whether some framework says a docblock reading like this documents itself elsewhere.
    ///
    /// Case-insensitively on the opening words, because a coding standard is prose and prose is
    /// typed by hand. Empty by default, so a project with no framework pays one length check.
    ///
    /// Compared as bytes, and that is not a shortcut: prose is UTF-8, and slicing it by a prefix's
    /// *length* can land inside a character -- an em dash in a docblock panicked the checker before
    /// this did. The prefixes are ASCII, so byte equality is the same answer without the boundary.
    pub fn documents_elsewhere(&self, prose: &str) -> bool {
        self.documented_elsewhere
            .iter()
            .any(|rule| rule.claim_in(prose).is_some())
    }

    /// Every convention a framework contributed, for a reader that needs the name and not the fact.
    ///
    /// The two questions are different -- "may this docblock leave its parameters undocumented" and
    /// "which declaration is the documentation on" -- and only the second needs the parts. Answering
    /// both from one list is what keeps them from disagreeing about what a claim looks like.
    pub fn documentation_conventions(&self) -> &[DocumentedByRule] {
        &self.documented_elsewhere
    }

    /// Records that the class named under `discriminator` documents the array's other keys.
    pub fn document_keys_under(&mut self, discriminator: &str, catalog: &str, prefix: &str) {
        let rule = DocumentedKeysRule {
            discriminator: discriminator.into(),
            catalog: catalog.into(),
            prefix: prefix.into(),
        };
        if !self.documented_keys.contains(&rule) {
            self.documented_keys.push(rule);
        }
    }

    /// The conventions a framework contributed for reading an array's keys off a class.
    ///
    /// Empty by default, which is the gate: a project with no framework never walks an array
    /// literal looking for a discriminator.
    pub fn key_documentation(&self) -> &[DocumentedKeysRule] {
        &self.documented_keys
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drupal's real shapes, kept verbatim: a heading on one class and a sentence on another, a
    /// parenthesised kind on some bullets and none on others, and descriptions that run over three
    /// lines. The prose above the list is the half a heading-based reader would have needed and
    /// this one has to step over.
    #[test]
    fn a_docblocks_bullets_are_the_keys_and_the_prose_around_them_is_not() {
        let rule = DocumentedKeysRule {
            discriminator: "#type".into(),
            catalog: "element".into(),
            prefix: "#".into(),
        };

        let found = rule.keys_in(
            r#"
 * Provides a one-line text field form element.
 *
 * Here is a list of properties that are used during rendering, besides those
 * documented in \Drupal\Core\Render\Element\RenderElementBase (for example:
 * #prefix, #suffix):
 * - #maxlength: Maximum number of characters of input allowed.
 * - #disabled: (bool) If TRUE, the element is shown but does not accept
 *   user input. See also #access.
 * - #array_parents: (string[], read-only) Array of names of all the element's
 *   parents.
 * - #prefix and #suffix: prose about two of them, not a key.
 *
 * @see \Drupal\Core\Render\Element
"#,
        );

        let rendered = found
            .iter()
            .map(|key| {
                format!(
                    "{} [{}] {}",
                    key.name,
                    key.kind.as_deref().unwrap_or(""),
                    key.summary
                )
            })
            .collect::<Vec<_>>();

        assert_eq!(
            rendered,
            [
                "#maxlength [] Maximum number of characters of input allowed.",
                "#disabled [bool] If TRUE, the element is shown but does not accept user input.",
                "#array_parents [string[], read-only] Array of names of all the element's parents.",
            ]
        );
    }
}

#[cfg(test)]
mod user_facing_docblocks {
    use super::UserFacingDocblockRule;

    fn numbered() -> UserFacingDocblockRule {
        UserFacingDocblockRule {
            names: "_update_".into(),
            numbered: true,
        }
    }

    fn named() -> UserFacingDocblockRule {
        UserFacingDocblockRule {
            names: "_post_update_".into(),
            numbered: false,
        }
    }

    /// The shape Drupal's own pattern matches, and the neighbour its own comment warns about.
    #[test]
    fn a_number_after_the_name_is_what_tells_an_update_from_a_function_that_updates() {
        assert!(numbered().covers("wh_item_snapshot_update_8001"));
        assert!(!numbered().covers("wh_item_snapshot_update_status"));
        assert!(!numbered().covers("wh_item_snapshot_update_8001_helper"));
    }

    /// `.+` is greedy on both sides of the framework's pattern, so the last occurrence is the one.
    #[test]
    fn the_last_occurrence_wins_and_something_has_to_come_before_it() {
        assert!(numbered().covers("a_update_b_update_8001"));
        assert!(!numbered().covers("_update_8001"));
        assert!(!numbered().covers("wh_item_snapshot_update_"));
    }

    /// A post update is named rather than numbered, which is why it needs a rule of its own even
    /// though every name carrying `_post_update_` carries `_update_` too.
    #[test]
    fn a_post_update_is_named_and_the_numbered_rule_would_exempt_none_of_them() {
        assert!(named().covers("wh_item_snapshot_post_update_backfill_flags"));
        assert!(!numbered().covers("wh_item_snapshot_post_update_backfill_flags"));
    }

    /// The whole of what a project with no framework pays.
    #[test]
    fn nothing_is_covered_where_no_framework_contributed_a_rule() {
        assert!(
            !super::super::ReturnContracts::default().user_facing_docblock("mymodule_update_8001")
        );
    }
}
