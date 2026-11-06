//! The keys of something indexed by a string: a shape's fields, a project catalog's ids.
//!
//! Both are the answer inside a subscript or a quote, where a class name is not what anybody is
//! reaching for.

use phrost_model::{SymbolKind, TextRange, TextSize, matches_loosely};
use phrost_view::IndexView;

use super::{
    CompletionItem, CompletionList, Describe, Insertion, NAME_LIMIT,
    chain::{chain_root, chain_steps},
};
use crate::context::Receiver;

/// The keys of the shape a subscript is being written on.
///
/// A shape is the only PHP type that names its own keys, so this is the only place a person typing
/// `$row['` can be told what is legal. Everything else offers nothing, which is the same answer a
/// receiver with no type gets: a list of guesses inside a subscript is worse than an empty one.
pub(super) fn shape_keys(
    view: &IndexView<'_>,
    offset: TextSize,
    receiver: &Receiver,
    quoted: bool,
    replace: TextRange,
    typed: &str,
) -> CompletionList {
    let empty = CompletionList {
        items: Vec::new(),
        is_complete: true,
        replace,
    };

    let steps = chain_steps(receiver);
    let Some(root) = chain_root(&receiver.root) else {
        return empty;
    };

    CompletionList {
        items: view
            .shape_keys_at(offset, root, &steps)
            .into_iter()
            .filter(|key| matches_loosely(key.name.as_str(), typed))
            .map(|key| CompletionItem {
                demoted: false,
                deprecated: false,
                // The quotes are part of what has to end up in the file, and whether they are
                // already there is the client's question to answer, not the person's: accepting a
                // key at `$row[` has to write `'id'` and at `$row['` only `id`.
                insert: (!quoted).then(|| Insertion::plain(format!("'{}'", key.name))),
                label: key.name.to_string(),
                // A field of a structured value, which is what a shape's key is: something a
                // declaration named, drawn with the icon a property gets rather than a class's.
                kind: SymbolKind::Property,
                // What the key holds, and whether it is one the shape says may be absent.
                detail: key.ty.map(|ty| match key.optional {
                    true => format!("{ty} (optional)"),
                    false => ty.into_string(),
                }),
                describe: None,
                call: None,
                import: None,
            })
            .collect(),
        is_complete: true,
        replace,
    }
}

/// The keys a call accepts, for a string that has been opened.
///
/// The whole literal is replaced and not just the prefix, because a person editing an existing id puts
/// the cursor in the middle of one and expects to end up with a valid id, not a spliced one.
///
/// The prefix is read by the view, from the literal itself: the lexical context reader stops at the
/// first character that cannot be in an identifier, and a service id is full of dots.
///
/// One limit, and it is the tree's: a literal whose quote is still open does not parse, so the call
/// around it cannot be found and nothing is offered. Editors close the quote as it is typed, which is
/// what makes that liveable -- `service('|')` is what a buffer actually holds.
/// The keys a form element can be read back by: what the enclosing class's builder wrote.
pub(super) fn built_keys(view: &IndexView<'_>, offset: TextSize) -> CompletionList {
    let Some(entries) = view.built_keys_at(offset) else {
        return CompletionList::default();
    };
    CompletionList {
        items: entries
            .into_iter()
            .map(|entry| CompletionItem {
                demoted: false,
                deprecated: false,
                label: entry.key,
                kind: SymbolKind::CatalogKey,
                // The element's `#type`, which is what tells `email` the field from `email` the
                // fieldset.
                detail: (!entry.detail.is_empty()).then_some(entry.detail),
                describe: None,
                insert: None,
                call: None,
                import: None,
            })
            .collect(),
        is_complete: true,
        ..CompletionList::default()
    }
}

pub(super) fn catalog_keys(view: &IndexView<'_>, offset: TextSize) -> CompletionList {
    let empty = CompletionList {
        items: Vec::new(),
        is_complete: true,
        replace: TextRange::empty(offset),
    };

    let Some((written, candidates)) = view.catalog_keys_at(offset, NAME_LIMIT) else {
        return empty;
    };
    // The same rule a bare name follows, and for the same measured reason: an empty prefix matches
    // every service a project has, and building all of them costs more than the whole budget. What is
    // cut is announced, so the client re-asks as soon as a letter narrows it.
    let is_complete = candidates.len() < NAME_LIMIT;

    CompletionList {
        items: candidates
            .into_iter()
            .map(|candidate| CompletionItem {
                demoted: false,
                deprecated: false,
                label: candidate.key.into_string(),
                kind: SymbolKind::CatalogKey,
                // What it names: two ids differing by a suffix are told apart by what they build,
                // and nothing else in the list distinguishes them.
                // Absent rather than blank when nothing declares what the key names: a service
                // built by a factory has no class to show, and an empty column is worse than none.
                detail: (!candidate.detail.is_empty()).then(|| candidate.detail.into_string()),
                describe: None,
                insert: None,
                call: None,
                import: None,
            })
            .collect(),
        is_complete,
        replace: written,
    }
}

/// The inputs a command declares, for the name a read of one is given: `$this->option('|')`.
///
/// What each holds beside it, which is the half of the signature a person reading `option('to')`
/// cannot see from there -- whether `to` is a flag, a value, or a list of them.
pub(super) fn declared_inputs(view: &IndexView<'_>, offset: TextSize) -> CompletionList {
    let Some((replace, entries)) = view.declared_inputs_at(offset) else {
        return CompletionList::default();
    };
    CompletionList {
        items: entries
            .into_iter()
            .map(|entry| CompletionItem {
                demoted: false,
                deprecated: false,
                label: entry.name,
                kind: SymbolKind::CatalogKey,
                detail: Some(entry.holds),
                describe: None,
                insert: None,
                call: None,
                import: None,
            })
            .collect(),
        is_complete: true,
        replace,
    }
}

/// The keys a framework documents for the array the cursor is writing inside.
///
/// `['#type' => 'textfield', '#ma|']`. What separates this from [`catalog_keys`] is which half of
/// the pair is being written: an id goes in a *value* and is a key of a catalog, and this is the
/// *key* beside it -- a name that appears in no declaration anywhere, only in the prose of the class
/// the id named.
///
/// The keys already written in this array are left out. Offering `#type` back to somebody looking
/// at `'#type' => 'textfield'` two lines up is offering the one item that cannot be the answer, and
/// a duplicate key is the one mistake this list could actively cause.
pub(super) fn documented_keys(view: &IndexView<'_>, offset: TextSize) -> CompletionList {
    let Some(found) = view.documented_keys_at(offset) else {
        return CompletionList {
            items: Vec::new(),
            is_complete: true,
            replace: TextRange::empty(offset),
        };
    };

    CompletionList {
        items: found
            .keys
            .into_iter()
            .filter(|key| !found.written.iter().any(|held| held.as_str() == &*key.name))
            .filter(|key| matches_loosely(&key.name, &found.prefix))
            .map(|key| CompletionItem {
                demoted: false,
                deprecated: false,
                // What it holds, when the documentation said -- `bool`, `string[], read-only`. The
                // only statement of a type these keys have anywhere.
                detail: key.kind.map(String::from),
                // The sentence beside it, fetched only for the row somebody highlights: sixty keys
                // with a sentence each is a list several times the size of the one that stopped the
                // server, for prose nobody reads sixty of.
                describe: Some(Describe::DocumentedKey {
                    class: found.class.to_string(),
                    name: key.name.to_string(),
                }),
                label: String::from(key.name),
                // A slot of a structured value, which is what one of these is: something the
                // framework named, drawn the way a shape's key is rather than as a class.
                kind: SymbolKind::Property,
                insert: None,
                call: None,
                import: None,
            })
            .collect(),
        is_complete: true,
        replace: found.replace,
    }
}
