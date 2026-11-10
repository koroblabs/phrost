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

/// The text typed before the client's word: everything up to and including the last character no
/// word holds. Empty when the whole of what was typed is one word.
pub(crate) struct TypedHead<'a>(&'a str);

impl<'a> TypedHead<'a> {
    pub(crate) fn of(typed: &'a str) -> Self {
        Self(
            typed
                .char_indices()
                .rev()
                .find(|(_, ch)| !Self::in_word(*ch))
                .map_or("", |(at, ch)| &typed[..at + ch.len_utf8()]),
        )
    }

    /// Whether some client counts `ch` as part of the word it filters by.
    fn in_word(ch: char) -> bool {
        ch.is_alphanumeric() || ch == '_' || ch == '$'
    }

    /// The `filterText` for an item labelled `label`: its rest past the head, when it starts with it.
    pub(crate) fn filter_text(&self, label: &str) -> Option<String> {
        let head = self.0;
        (!head.is_empty()
            && label.len() > head.len()
            && label
                .get(..head.len())
                .is_some_and(|start| start.eq_ignore_ascii_case(head)))
        .then(|| label[head.len()..].to_owned())
    }
}
