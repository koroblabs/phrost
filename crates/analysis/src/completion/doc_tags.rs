//! The tags a docblock can hold, for an `@` typed where a tag starts.
//!
//! Which ones depends on what the docblock documents: `@param` is about a function's parameters and
//! means nothing over a property, `@method` is about a class. A tag a docblock may hold once and
//! already holds is not offered again -- a second `@return` is a mistake, and offering it is offering
//! the mistake.

use phrost_model::{SymbolKind, TextRange, TextSize};
use phrost_view::{DocumentedItem, IndexView, MemberKind};

use super::{CompletionItem, CompletionList, Insertion};

/// Tags any declaration may carry.
const ANY: &[&str] = &[
    "@deprecated",
    "@see",
    "@internal",
    "@since",
    "@todo",
    "@link",
];

const FUNCTION: &[&str] = &[
    "@param",
    "@return",
    "@throws",
    "@template",
    "@global",
    "@phpstan-param",
    "@phpstan-return",
    "@phpstan-assert",
    "@phpstan-assert-if-true",
    "@phpstan-assert-if-false",
    "@phpstan-impure",
    "@phpstan-pure",
    "@psalm-param",
    "@psalm-return",
];

const CLASS_LIKE: &[&str] = &[
    "@template",
    "@template-covariant",
    "@template-contravariant",
    "@extends",
    "@implements",
    "@use",
    "@method",
    "@property",
    "@property-read",
    "@property-write",
    "@mixin",
    "@final",
    "@phpstan-type",
    "@phpstan-import-type",
    "@psalm-immutable",
];

const PROPERTY: &[&str] = &["@var", "@readonly", "@phpstan-var", "@psalm-var"];

const CONSTANT: &[&str] = &["@var"];

const STATEMENT: &[&str] = &["@var", "@phpstan-var", "@psalm-var"];

/// Tags a docblock holds at most once.
const ONCE: &[&str] = &[
    "@return",
    "@var",
    "@deprecated",
    "@internal",
    "@final",
    "@readonly",
    "@phpstan-return",
    "@psalm-return",
    "@phpstan-var",
    "@psalm-var",
    "@phpstan-impure",
    "@phpstan-pure",
    "@psalm-immutable",
    INHERIT_DOC,
];

/// The inline tag that borrows the documentation of the declaration above, spelled the way the
/// navigation that follows it reads it.
const INHERIT_DOC: &str = "{@inheritdoc}";

/// The tags worth offering at `offset`, when it is where a tag is being typed in a docblock.
///
/// `None` anywhere else, so the request goes on to whatever it would otherwise have been: a type
/// after `@param ` is not a tag.
pub(super) fn doc_tags(view: &IndexView<'_>, offset: TextSize) -> Option<CompletionList> {
    let text = view.current_text()?;
    let at = usize::from(offset);
    let before = text.get(..at)?;
    let typed = before.len()
        - before
            .trim_end_matches(|ch: char| ch.is_ascii_alphanumeric() || ch == '-')
            .len();
    let sign = at.checked_sub(typed + 1)?;
    if before.as_bytes().get(sign) != Some(&b'@') {
        return None;
    }

    // `{@` anywhere in the text is an inline tag; a bare `@` has to start the line's text, because
    // an `@` inside a sentence is an address or a decorator in an example, not a tag.
    let inline = sign > 0 && before.as_bytes()[sign - 1] == b'{';
    let line = &before[before[..sign].rfind('\n').map_or(0, |at| at + 1)..sign];
    let at_tag_start = line
        .trim_start()
        .trim_start_matches("/**")
        .trim_start_matches('*')
        .trim()
        .is_empty();
    if !inline && !at_tag_start {
        return None;
    }

    let (item, docblock) = view.documented_item_at(offset)?;
    // The rest of the docblock, without the tag being typed: a fully typed `@return` is still the
    // one being written, not one already there.
    let written = format!(
        "{}{}",
        text.get(usize::from(docblock.start())..sign)?,
        text.get(at..usize::from(docblock.end()))?
    )
    .to_ascii_lowercase();
    let typed_tag = before[sign..].to_ascii_lowercase();

    let mut offered: Vec<&str> = match inline {
        true => Vec::new(),
        false => {
            let own: &[&str] = match &item {
                DocumentedItem::Function
                | DocumentedItem::Member {
                    kind: MemberKind::Method,
                    ..
                } => FUNCTION,
                DocumentedItem::ClassLike => CLASS_LIKE,
                DocumentedItem::Member {
                    kind: MemberKind::Property,
                    ..
                } => PROPERTY,
                DocumentedItem::Constant | DocumentedItem::Member { .. } => CONSTANT,
                DocumentedItem::Statement => STATEMENT,
            };
            let any: &[&str] = match item {
                DocumentedItem::Statement => &[],
                _ => ANY,
            };
            own.iter().chain(any).copied().collect()
        }
    };
    // Only where the index knows something above to borrow from -- the rule goto on the tag follows,
    // so the tag is offered exactly where following it goes somewhere. A property and a constant
    // override as a method does: Drupal writes `{@inheritdoc}` over every `protected $key`.
    if let DocumentedItem::Member { owner, kind, name } = &item
        && view
            .overridden_member(owner, *kind, name.as_str())
            .is_some()
    {
        offered.insert(0, INHERIT_DOC);
    }

    let replace = TextRange::new(TextSize::try_from(sign - usize::from(inline)).ok()?, offset);
    let closes = text.as_bytes().get(at) == Some(&b'}');
    let items = offered
        .into_iter()
        .filter(|tag| !(ONCE.contains(tag) && written.contains(*tag)))
        .filter(|tag| tag.trim_start_matches('{').starts_with(typed_tag.as_str()))
        .map(|tag| CompletionItem {
            demoted: false,
            deprecated: false,
            label: tag.to_owned(),
            kind: SymbolKind::Keyword,
            detail: None,
            describe: None,
            // A brace the editor already closed stays the one that closes the tag.
            insert: Some(Insertion::plain(match closes && tag.ends_with('}') {
                true => tag.trim_end_matches('}').to_owned(),
                false => tag.to_owned(),
            })),
            call: None,
            import: None,
        })
        .collect();

    Some(CompletionList {
        items,
        replace,
        is_complete: false,
    })
}
