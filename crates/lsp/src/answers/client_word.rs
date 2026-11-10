//! What a client filters a completion list by, and what to hand it so that it ranks the right way.
//!
//! A client filters with the *word* at the cursor by its own idea of a word -- `manage` in
//! `service('application.manage|')` -- and ranks every item holding that word by how well it matches,
//! ahead of any order the server sent: Zed's `sort_string_matches` puts its fuzzy score before
//! `sortText`. So in a list of ids full of dots the ones starting with what was typed sank among every
//! id that merely contains the last word. An item starting with everything typed up to that word is
//! handed over with the rest of itself as `filterText` -- `manager`, `memo.manager` -- which is what
//! the client's word is then compared against.
//!
//! A word holds `$`. Zed's PHP language says so (`word_characters`, `completion_query_characters`)
//! and so does VS Code's PHP word pattern, so at `$ap|` the client filters by `$ap` -- and `$app`
//! handed over as `filterText: "app"` has no `$` to match it with, which hid every variable and
//! every static property from the list. Counting `$` in the word when a client does not is only a
//! shorter head: the item keeps more of its label, which still holds everything that was typed.
//!
//! # The other kind of client
//!
//! Not every client filters by a word. VS Code, and the completion that follows it, filters by the
//! text the item's edit *replaces* -- the range this server sends -- against the same `filterText`
//! or label. For most items the two are one question, because the range is the word. They part
//! where the range starts before it: `new \RuntimeE|` replaces `\RuntimeE` and is labelled
//! `RuntimeException`, so a client matching the replaced text finds no `\` in the label and drops
//! the item, while one matching the word `RuntimeE` keeps it. Seven of fifty `new` sites sampled in
//! Laravel's and Drupal's own source are written that way.
//!
//! Such an item is handed the text it writes, `\RuntimeException`, which holds the replaced text
//! for the one client and the word for the other. Only where the label does not already hold what
//! was typed, so every other item is filtered by its label as before.
//!
//! What this does not reach is the rule above it: a rest past the head -- `manager` for
//! `application.ma|` -- is the word model's, and a client matching `application.ma` against it
//! finds nothing. One string cannot rank an id by its last word for one client and hold all of it
//! for the other, and the client the ranking was measured in is the one it is kept for.

/// What was typed where an item goes, and the part of it before the client's word: everything up
/// to and including the last character no word holds. The head is empty when the whole of what was
/// typed is one word.
pub(crate) struct TypedHead<'a> {
    typed: &'a str,
    head: &'a str,
}

impl<'a> TypedHead<'a> {
    pub(crate) fn of(typed: &'a str) -> Self {
        Self {
            typed,
            head: typed
                .char_indices()
                .rev()
                .find(|(_, ch)| !Self::in_word(*ch))
                .map_or("", |(at, ch)| &typed[..at + ch.len_utf8()]),
        }
    }

    /// Whether some client counts `ch` as part of the word it filters by.
    fn in_word(ch: char) -> bool {
        ch.is_alphanumeric() || ch == '_' || ch == '$'
    }

    /// The `filterText` for an item labelled `label`.
    ///
    /// Its rest past the head, when it starts with it. Failing that, `written` -- the plain text
    /// the item's edit writes, where there is one -- when the label does not hold what was typed
    /// and that text does: see the module on the client that filters by what is replaced.
    pub(crate) fn filter_text(&self, label: &str, written: Option<&str>) -> Option<String> {
        let head = self.head;
        if !head.is_empty()
            && label.len() > head.len()
            && label
                .get(..head.len())
                .is_some_and(|start| start.eq_ignore_ascii_case(head))
        {
            return Some(label[head.len()..].to_owned());
        }
        let written = written?;
        (!Self::holds(label, self.typed) && Self::holds(written, self.typed))
            .then(|| written.to_owned())
    }

    /// Whether `text` holds every character of `typed` in order, case aside -- the least any
    /// client's matcher asks, so a pair failing it is dropped by all of them.
    fn holds(text: &str, typed: &str) -> bool {
        let mut rest = text.chars();
        typed
            .chars()
            .all(|wanted| rest.any(|ch| ch.eq_ignore_ascii_case(&wanted)))
    }
}
