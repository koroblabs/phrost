//! The older spelling of an attribute: a Doctrine-style annotation in the docblock.
//!
//! ```text
//! /**
//!  * Defines the Behavior settings entity.
//!  *
//!  * @ConfigEntityType(
//!  *   id = "behavior_settings",
//!  *   label = @Translation("Rabbit hole settings"),
//!  *   entity_keys = { "id" = "id" },
//!  * )
//!  */
//! ```
//!
//! The same fact as `#[ConfigEntityType(id: 'behavior_settings')]` and the same answer, so it lowers
//! to the same [`AttributeLiteralData`] and nothing above this file learns that a declaration has two
//! spellings. Measured on one real thirty-thousand-file checkout: 107 entity type ids are written as
//! an attribute and 11 as an annotation, overlapping in 7 -- so four ids exist in code that runs and
//! in no index, and a report on that catalog would call each of them undeclared.
//!
//! # Why here and not at the framework seam
//!
//! The value is only half the fact. What the id *names* is the class the annotation is written on,
//! and deriving that from a file means knowing its namespace and its declarations -- parsing PHP, in
//! a crate that must not. Reading it here also makes it a per-file claim, withdrawn and re-made when
//! the file changes, which a bootstrap scan is not.
//!
//! # What mago decides and what this file decides
//!
//! The docblock parser decides *what a tag is*, which is not a detail: Drupal's `core.api.php`
//! documents the annotation syntax with a worked `@ContentEntityType(id = "comment")` inside an
//! `@code` block, and a plain text search would read that example as a second declaration of
//! `comment` -- enough to make the catalog answer nothing for the id, since two classes claiming one
//! key is a conflict rather than an answer. mago folds the example into the `@code` tag's value, so
//! it is never a tag of its own.
//!
//! What is left is the argument list, which is Doctrine's syntax and not PHPDoc's -- `key = "value"`
//! rather than `key: "value"`, `{ ... }` maps, and nested annotations as values. mago models none of
//! it, so [`AnnotationArguments`] walks it: the top level of the parentheses and nothing deeper.

use mago_allocator::{Arena as _, LocalArena};
use mago_phpdoc_syntax::{
    PHPDocParser,
    cst::{Element, TagValue},
};
use mago_span::HasSpan as _;
use phrost_hir::{AttributeArgument, AttributeLiteralData};
use phrost_model::TextSize;

use crate::lowerer::Lowerer;

impl Lowerer<'_, '_> {
    /// The string literals an annotation writes in the docblock above a declaration.
    ///
    /// Only for the attribute names handed down, which is what keeps this off the parse path for a
    /// project that has no use for it: the prefilter is a substring scan of text already in hand,
    /// and a project with no framework detected does not even do that.
    pub(crate) fn annotation_literals(
        &mut self,
        declaration_start: TextSize,
    ) -> Vec<AttributeLiteralData> {
        let Some(docblock) = self.raw_docblock_before(declaration_start) else {
            return Vec::new();
        };
        if !self.keyed_attributes.mentioned_in(docblock.text) {
            return Vec::new();
        }

        let arena = LocalArena::new();
        let content = arena.alloc_slice_copy(docblock.text.as_bytes());
        // A synthetic file id, as everywhere a docblock is parsed here: the parser only stamps
        // spans with it, and the spans are read back against this same text.
        let document = PHPDocParser::parse(
            &arena,
            mago_database::file::FileId::new(b"docblock"),
            content,
        );

        let written: Vec<(&str, &str)> = document
            .elements
            .iter()
            .filter_map(|element| match element {
                // `Generic` is what an unknown tag parses to, and an annotation is exactly that: a
                // name PHPDoc does not know, followed by text it does not interpret. Matching the
                // arm rather than only the name is what keeps a tag PHPDoc *does* know -- one whose
                // value it has already taken apart -- from being read as an argument list.
                Element::Tag(tag) if matches!(tag.value, TagValue::Generic(_)) => {
                    let name = std::str::from_utf8(tag.name.value).ok()?;
                    if !self.keyed_attributes.wants(name) {
                        return None;
                    }
                    let span = tag.span();
                    let raw = docblock
                        .text
                        .get(span.start.offset as usize..span.end.offset as usize)?;
                    Some((name, raw))
                }
                _ => None,
            })
            .collect();

        let mut literals = Vec::new();
        for (name, raw) in written {
            let attribute = self.interner.name(name);
            for written in AnnotationArguments::of(raw, self.keyed_attributes.entered()) {
                let argument = match written.slot {
                    AnnotationSlot::Named(name) => {
                        AttributeArgument::Named(self.interner.name(name))
                    }
                    AnnotationSlot::At(position) => AttributeArgument::At(position),
                };
                literals.push(AttributeLiteralData {
                    attribute: attribute.clone(),
                    argument,
                    value: written.value.into(),
                    key: written.key.map(Into::into),
                });
            }
        }
        literals
    }
}

/// Which argument of an annotation a literal was written at, before anything is interned.
///
/// The same two cases [`AttributeArgument`] has. Its own type because the walk below has no
/// interner and must not need one: it is a scan over text, and the names it finds are mostly
/// arguments nothing reads.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum AnnotationSlot<'a> {
    Named(&'a str),
    At(u16),
}

/// The string literals written at the top level of an annotation's argument list.
///
/// Doctrine's own grammar, restricted to the part that can carry an id:
///
/// - `id = "node"` is a named argument, matching what `#[X(id: 'node')]` lowers to;
/// - `@ViewsArea("entity")` is the shorthand every single-key annotation is written with, and
///   matches what `#[ViewsArea('entity')]` lowers to;
/// - a value that is not a string literal carries no id -- `label = @Translation("Content")` names
///   none, and `handlers = { ... }` is a map of class names;
/// - anything inside a nested `(` or `{` is stepped over, which is what keeps `entity_keys = { "id"
///   = "id" }` from claiming the id `id` for every entity type in the project -- *unless* the name in
///   front of it is one the caller asked to enter, in which case the group is walked by a second copy
///   of this and its named arguments come back as keyed entries of the outer one.
///
/// The position counts over the whole comma-separated list, named and positional alike, so it means
/// what a reader of the source would count -- the same rule the attribute path follows.
struct AnnotationArguments<'a> {
    /// Everything between the annotation's outermost parentheses. Empty for one written without
    /// them, which is a bare marker and declares nothing.
    body: &'a str,
    at: usize,
    position: u16,
    /// The word last seen at the top level, which becomes the argument's name when an `=` follows.
    name: Option<&'a str>,
    /// Argument names whose `{ }` value is walked rather than stepped over.
    entered: &'a [Box<str>],
    /// Entries read out of one such group, waiting to be handed back one at a time.
    pending: std::collections::VecDeque<Written<'a>>,
}

/// One literal the walk found, and what it was written under.
struct Written<'a> {
    slot: AnnotationSlot<'a>,
    /// The key of an entry inside an entered group, and `None` for the argument itself.
    key: Option<&'a str>,
    value: &'a str,
}

impl<'a> AnnotationArguments<'a> {
    fn of(tag: &'a str, entered: &'a [Box<str>]) -> Self {
        Self::inside(Self::inside_parentheses(tag).unwrap_or(""), entered)
    }

    /// The same walk over a body already delimited, which is what an entered group hands back.
    fn inside(body: &'a str, entered: &'a [Box<str>]) -> Self {
        Self {
            body,
            at: 0,
            position: 0,
            name: None,
            entered,
            pending: std::collections::VecDeque::new(),
        }
    }

    /// The text between the outermost parentheses of an annotation.
    ///
    /// Matched rather than taken to the end, because a tag's span can run past the annotation into
    /// the prose after it, and a quoted word there would otherwise read as an argument.
    fn inside_parentheses(tag: &'a str) -> Option<&'a str> {
        let open = tag.find('(')?;
        let mut depth = 0usize;
        let mut quote = None;
        for (at, ch) in tag[open..].char_indices() {
            match quote {
                Some(held) => {
                    if ch == held {
                        quote = None;
                    }
                }
                None => match ch {
                    '"' | '\'' => quote = Some(ch),
                    '(' => depth += 1,
                    ')' => {
                        depth = depth.saturating_sub(1);
                        if depth == 0 {
                            return Some(&tag[open + 1..open + at]);
                        }
                    }
                    _ => {}
                },
            }
        }
        // Unclosed, which a docblock cut short by an edit is: what was written is still worth
        // reading, and the walk simply runs out.
        Some(&tag[open + 1..])
    }

    /// The rest of the body from the cursor.
    fn rest(&self) -> &'a str {
        &self.body[self.at..]
    }

    /// Steps over a `(`- or `{`-delimited group, and over the strings inside it.
    ///
    /// The cursor is on the opening delimiter. Nothing inside is read: a nested annotation's
    /// arguments belong to that annotation, and a map's entries are values rather than arguments.
    fn skip_group(&mut self, open: char, close: char) {
        let mut depth = 0usize;
        while let Some(ch) = self.rest().chars().next() {
            match ch {
                '"' | '\'' => {
                    self.skip_string(ch);
                    continue;
                }
                _ if ch == open => depth += 1,
                _ if ch == close => depth = depth.saturating_sub(1),
                _ => {}
            }
            self.at += ch.len_utf8();
            if depth == 0 {
                return;
            }
        }
    }

    /// Steps over a quoted string, leaving the cursor after the closing quote.
    ///
    /// No escape handling, because Doctrine has none to handle: a backslash inside an annotation
    /// string is a literal one -- `"Drupal\node\NodeStorage"` is how every handler in every
    /// annotation is written -- and a quote is escaped by doubling it rather than by preceding it.
    fn skip_string(&mut self, quote: char) -> &'a str {
        let from = self.at + quote.len_utf8();
        let len = self.body[from..]
            .find(quote)
            .unwrap_or(self.body.len() - from);
        self.at = (from + len + quote.len_utf8()).min(self.body.len());
        &self.body[from..from + len]
    }
}

impl<'a> Iterator for AnnotationArguments<'a> {
    type Item = Written<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(entry) = self.pending.pop_front() {
            return Some(entry);
        }
        while let Some(ch) = self.rest().chars().next() {
            match ch {
                '(' => self.skip_group('(', ')'),
                '{' => {
                    let from = self.at;
                    self.skip_group('{', '}');
                    let entered = self
                        .name
                        .filter(|name| self.entered.iter().any(|held| &**held == *name));
                    if let Some(name) = entered {
                        // The group without its braces. `skip_group` left the cursor after the
                        // closing one, and it skipped the strings inside, so a `}` written in a URL
                        // did not end it early.
                        let body = self.body[from..self.at]
                            .trim_start_matches('{')
                            .trim_end_matches('}');
                        self.pending
                            .extend(Self::inside(body, self.entered).filter_map(|entry| {
                                Some(Written {
                                    slot: AnnotationSlot::Named(name),
                                    key: Some(match entry.slot {
                                        AnnotationSlot::Named(key) => key,
                                        AnnotationSlot::At(_) => return None,
                                    }),
                                    value: entry.value,
                                })
                            }));
                    }
                    self.name = None;
                    if let Some(entry) = self.pending.pop_front() {
                        return Some(entry);
                    }
                }
                '"' | '\'' => {
                    let position = self.position;
                    let name = self.name.take();
                    let value = self.skip_string(ch);
                    // A quoted *name* is not an argument. Doctrine allows one, and reading it as a
                    // value would report two arguments where the source wrote one.
                    if self.rest().trim_start_matches(is_filler).starts_with('=') {
                        self.name = Some(value);
                        continue;
                    }
                    let slot = match name {
                        Some(name) => AnnotationSlot::Named(name),
                        None => AnnotationSlot::At(position),
                    };
                    return Some(Written {
                        slot,
                        key: None,
                        value,
                    });
                }
                ',' => {
                    self.position = self.position.saturating_add(1);
                    self.name = None;
                    self.at += ch.len_utf8();
                }
                '=' => self.at += ch.len_utf8(),
                _ if is_filler(ch) => self.at += ch.len_utf8(),
                // A bare word: the argument's name when an `=` follows it, and nothing otherwise.
                _ => {
                    let len = self.rest().find(is_break).unwrap_or(self.rest().len());
                    match len {
                        // A closing delimiter with nothing open, which only a malformed annotation
                        // writes. Stepping over it is what keeps the walk finite.
                        0 => self.at += ch.len_utf8(),
                        _ => {
                            self.name = Some(&self.rest()[..len]);
                            self.at += len;
                        }
                    }
                }
            }
        }
        None
    }
}

/// Whitespace, plus the `*` every line of a docblock begins with.
///
/// The tag's text is the docblock's own bytes, line prefixes included, because that is what a mago
/// span points at. A `*` cannot mean anything else inside an annotation's argument list.
fn is_filler(ch: char) -> bool {
    ch.is_whitespace() || ch == '*'
}

/// Where a bare word ends.
fn is_break(ch: char) -> bool {
    is_filler(ch) || matches!(ch, '=' | ',' | '(' | ')' | '{' | '}' | '"' | '\'')
}

#[cfg(test)]
mod tests {
    use super::{AnnotationArguments, AnnotationSlot};

    /// Every argument the walk reads, rendered whole: an extra one has to be as visible as a
    /// missing one, since an extra is what claims a key nobody declared.
    fn read(tag: &str) -> Vec<(String, String)> {
        read_entering(tag, &[])
    }

    /// The same, entering the array arguments named.
    fn read_entering(tag: &str, entered: &[Box<str>]) -> Vec<(String, String)> {
        AnnotationArguments::of(tag, entered)
            .map(|written| {
                let slot = match written.slot {
                    AnnotationSlot::Named(name) => name.to_owned(),
                    AnnotationSlot::At(position) => format!("#{position}"),
                };
                let slot = match written.key {
                    Some(key) => format!("{slot}[{key}]"),
                    None => slot,
                };
                (slot, written.value.to_owned())
            })
            .collect()
    }

    #[test]
    fn a_named_argument_is_read_and_a_nested_annotation_is_not() {
        assert_eq!(
            read(
                "@ContentEntityType(\n *   id = \"node\",\n *   label = @Translation(\"Content\"),\n *   base_table = \"node\"\n * )"
            ),
            vec![
                ("id".to_owned(), "node".to_owned()),
                ("base_table".to_owned(), "node".to_owned()),
            ]
        );
    }

    #[test]
    fn a_map_entry_is_a_value_and_never_an_argument() {
        // The one that matters: `entity_keys = { "id" = "id" }` is in nearly every entity
        // annotation there is, and reading it would have every entity type claim the key `id`.
        assert_eq!(
            read(
                "@ContentEntityType(\n * id = \"node\",\n * entity_keys = {\n *   \"id\" = \"nid\",\n *   \"uuid\" = \"uuid\"\n * }\n * )"
            ),
            vec![("id".to_owned(), "node".to_owned())]
        );
    }

    #[test]
    fn the_shorthand_form_writes_its_id_positionally() {
        assert_eq!(
            read("@RenderElement(\"horizontal_tabs\")"),
            vec![("#0".to_owned(), "horizontal_tabs".to_owned())]
        );
    }

    #[test]
    fn an_annotation_with_no_argument_list_writes_nothing() {
        assert_eq!(read("@ContentEntityType"), Vec::<(String, String)>::new());
    }

    #[test]
    fn a_word_after_the_closing_parenthesis_is_prose() {
        // A tag's span can run past the annotation, and a quoted word in the sentence after it
        // would otherwise be read as an argument the source never wrote.
        assert_eq!(
            read("@X(id = \"real\") and see \"not an argument\""),
            vec![("id".to_owned(), "real".to_owned())]
        );
    }

    #[test]
    fn a_backslash_in_a_value_is_a_backslash() {
        // Doctrine has no escape: this is how every handler in every annotation is written, and
        // treating `\"` as an escaped quote would run the string past its end and lose the rest of
        // the argument list.
        assert_eq!(
            read("@X(a = \"Drupal\\node\\NodeStorage\", b = \"after\")"),
            vec![
                ("a".to_owned(), "Drupal\\node\\NodeStorage".to_owned()),
                ("b".to_owned(), "after".to_owned()),
            ]
        );
    }

    #[test]
    fn a_position_counts_over_the_whole_list() {
        // The same rule the attribute path follows, so a contract naming argument 1 reaches the
        // same argument whichever spelling the declaration used.
        assert_eq!(
            read("@X(\"first\", id = \"second\", \"third\")"),
            vec![
                ("#0".to_owned(), "first".to_owned()),
                ("id".to_owned(), "second".to_owned()),
                ("#2".to_owned(), "third".to_owned()),
            ]
        );
    }
}
