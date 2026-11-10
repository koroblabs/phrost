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
//! or label. The two are one question wherever the range is the word, and they part where the range
//! starts before it: `use App\Http\Resp|` replaced the whole path and was filtered by `Response`,
//! `service('application.ma|')` replaced the whole id and was filtered by `manager`, and a client
//! matching `App\Http\Resp` against `Response` finds no `\` there and drops an item the server
//! offered. All forty-four `use` lines sampled in Symfony's source, and every id with a dot in it.
//!
//! One string cannot hold the last word for one client and the whole path for the other -- but the
//! *range* can move instead. An item that begins with the head exactly as it was typed would write
//! those characters back unchanged, so its edit leaves them standing and replaces only the word:
//! `Resp` with `Response`, `ma` with `manager`. The buffer afterwards is the same byte for byte,
//! and now the replaced text and the client's word are one thing, so both kinds of client ask the
//! same question of the same `filterText`. It is also the edit a client that ignores the range and
//! replaces its own word would have needed.
//!
//! What is left is the item that does *not* go on from what was typed: one spelled in another case,
//! or `new \RuntimeE|` offered as an import. Its edit still covers everything typed, and where its
//! label does not hold that text it is handed the plain text it writes, which does.

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

    /// How many bytes of what an item writes are left standing rather than written again: the
    /// whole head, when `written` begins with it exactly as typed, and nothing otherwise.
    ///
    /// `written` is the item's text before any snippet syntax is added, and `cursor` the place in
    /// it the cursor is sent to, which has to be past what is taken off.
    pub(crate) fn kept(&self, written: &str, cursor: Option<usize>) -> usize {
        let head = self.head;
        let goes_on = !head.is_empty() && written.len() > head.len() && written.starts_with(head);
        match goes_on && cursor.is_none_or(|at| at >= head.len()) {
            true => head.len(),
            false => 0,
        }
    }

    /// The `filterText` for an item labelled `label`, whose edit leaves the first `kept` bytes of
    /// what was typed standing -- see [`Self::kept`].
    ///
    /// Its rest past the head, when it starts with it. Failing that, `written` -- the plain text
    /// the item's edit writes, where there is one -- when the label does not hold what the edit
    /// replaces and that text does: see the module on the client that filters by what is replaced.
    pub(crate) fn filter_text(
        &self,
        label: &str,
        written: Option<&str>,
        kept: usize,
    ) -> Option<String> {
        let head = self.head;
        if !head.is_empty()
            && label.len() > head.len()
            && label
                .get(..head.len())
                .is_some_and(|start| start.eq_ignore_ascii_case(head))
        {
            return Some(label[head.len()..].to_owned());
        }
        let replaced = self.typed.get(kept..).unwrap_or(self.typed);
        let written = written?;
        (!Self::holds(label, replaced) && Self::holds(written, replaced))
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
