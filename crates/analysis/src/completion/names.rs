//! Top-level names from the catalog: classes, functions, constants — and the paths a `use` line
//! takes, which are the same names spelled the way an import has to spell them.

use phrost_model::{SymbolKind, TextSize, short_name};
use phrost_view::{ImportEdit, IndexView, NameAnchor, NameKinds};

use super::{CompletionItem, CompletionList, Describe, Insertion, NAME_LIMIT};
use crate::context::NamePlace;

/// Top-level names matching a prefix, filtered to the ones the place can take.
pub(super) fn names(
    view: &IndexView<'_>,
    offset: TextSize,
    prefix: &str,
    spelling: Spelling,
    place: NamePlace,
) -> CompletionList {
    // The whole prefix, separators included: whether it narrows by short name or by fully-qualified
    // name is the projection's decision, and the two mean different things. The place's kinds go
    // in with it, so the cap counts only candidates that could actually be offered here -- `new `
    // no longer spends its budget on functions and constants it would only discard below.
    //
    // A qualified prefix asks for the names under it and the namespaces past it at once, because
    // both come out of one walk over every declared name and asking twice walked it twice.
    let (candidates, namespace_paths) = match prefix.contains('\\') {
        true => {
            let found = view.path_continuations(offset, prefix, place.kinds(), NAME_LIMIT);
            (found.names, found.namespaces)
        }
        false => (
            view.candidates_with_prefix(offset, prefix, place.kinds(), NAME_LIMIT),
            Vec::new(),
        ),
    };
    // What the catalog cut, before the stub-internal filter has its say. A marker interface
    // excluded there is one a narrower prefix would not bring back either, so counting it as a
    // reason to re-ask would have the client ask the same question again for the same answer. The
    // place's own filter no longer needs this caveat -- it now happens inside the projection,
    // before the cap, so every candidate counted here is one the place could actually accept.
    let is_complete = candidates.len() < NAME_LIMIT;

    let mut namespaces = namespaces_written_at(prefix, namespace_paths);
    // A bare word is relative to the file's namespace, so the namespaces it can begin are the ones
    // under that -- `Types` in `Types\ArrayType::class` written inside `Illuminate\JsonSchema`.
    if !prefix.contains('\\') {
        namespaces.extend(
            view.namespaces_beginning(offset, prefix, NAME_LIMIT)
                .into_iter()
                .map(|path| {
                    let segment = path.rsplit('\\').next().unwrap_or(&path).to_owned();
                    namespace_item(segment.clone(), format!("{segment}\\"), Some(path))
                }),
        );
    }
    let aliases = aliases_offered(view, prefix, place);
    let mut classes: Vec<(bool, CompletionItem)> = candidates
        .into_iter()
        // `__INTERNAL_DO_NOT_USE__StringBackedEnum` and its `Int` sibling are interfaces a stub
        // declares on purpose and that PHP never asks anyone to write. See
        // [`crate::render::is_stub_internal`].
        .filter(|candidate| !crate::render::is_stub_internal(candidate.fqn.as_str()))
        .map(|candidate| {
            let fqn = candidate.fqn.as_str();
            // A path already being written keeps what was written: `\sprintf` stays anchored and
            // `\GuzzleHttp\Ev` becomes `\GuzzleHttp\Events`. Rewriting it to a short name and an
            // import second-guesses a spelling somebody chose -- Symfony writes every global function
            // from the root, and its own fixer puts the separator back.
            let (insert, import) = match written_head(prefix) {
                Some(head) => (format!("{head}{}", candidate.short()), None),
                None => written_form(view, offset, fqn, candidate.kind, spelling),
            };
            // Whether the bare name already means this declaration here: it is written short and
            // needs no `use` line.
            let meant = import.is_none() && insert == candidate.short();
            let item = CompletionItem {
                demoted: false,
                deprecated: candidate.deprecated,
                label: candidate.short().to_owned(),
                kind: candidate.kind,
                detail: Some(fqn.to_owned()),
                // The header, on request: `final class Repo extends Base implements Contract` is
                // what a person is choosing between when two names look alike, and it comes out of
                // the skeleton with no disk read. Not the docblock -- that still needs one.
                describe: Some(Describe::TopLevel {
                    fqn: fqn.to_owned(),
                    kind: candidate.kind,
                }),
                insert: Some(Insertion::plain(insert)),
                call: None,
                import,
            };
            (meant, item)
        })
        .collect();
    // Of several declarations sharing one short name, the one the name already means here stands
    // first among them. The catalog orders namesakes by origin and then by where each is declared,
    // which says nothing about which is meant -- and the file has said, by an import or by the
    // namespace it is in. `new RuntimeE` under `use RuntimeException;` offered the MongoDB
    // extension's class ahead of PHP's and wrote its whole path on accept, because `mongodb.php`
    // sorts before `spl.php`; a script with no namespace was offered a project's own
    // `...\Exception\Exception` ahead of the `Exception` it had been writing.
    //
    // Among its namesakes and no further: where a name stands against *other* names is the
    // ranking's. And by symbol table, since a function and a class may share a name.
    for at in 0..classes.len() {
        if !classes[at].0 {
            continue;
        }
        let slots: Vec<usize> = (0..=at)
            .filter(|&other| {
                let (namesake, meant) = (&classes[other].1, &classes[at].1);
                import_kind_of(namesake.kind) == import_kind_of(meant.kind)
                    && namesake.label.eq_ignore_ascii_case(&meant.label)
            })
            .collect();
        // Up to the first place its namesakes hold, each of them moving down one.
        for slot in (0..slots.len() - 1).rev() {
            classes.swap(slots[slot], slots[slot + 1]);
        }
    }
    let classes = classes.into_iter().map(|(_, item)| item);
    // Which comes first is what each place is written with. Code names a class by importing it, so
    // a fully-qualified path there is rare and the class of the same name is what is meant -- on the
    // reference project `\Drupal::` is written 269 times in code and `\Drupal\...` 3. A docblock
    // writes the whole path every time, so there the path is the answer being typed.
    let items = match spelling {
        Spelling::Qualified => namespaces
            .into_iter()
            .chain(aliases)
            .chain(classes)
            .collect(),
        Spelling::Imported => aliases
            .into_iter()
            .chain(classes)
            .chain(namespaces)
            .collect(),
    };

    CompletionList {
        items,
        is_complete,
        ..CompletionList::default()
    }
}

/// The namespaces a qualified name being written in code can go on into: `\GuzzleHttp\Ex|` offers
/// `Exception`.
///
/// What is inserted is what was written up to the last separator, the segment, and the separator
/// after it -- the anchor stays the person's, since `\GuzzleHttp\` and an imported
/// `Guzzle\` both reach this and only one of them is the fully-qualified spelling.
fn namespaces_written_at(prefix: &str, paths: Vec<String>) -> Vec<CompletionItem> {
    let Some(head) = written_head(prefix) else {
        return Vec::new();
    };
    paths
        .into_iter()
        .map(|path| {
            let segment = path.rsplit('\\').next().unwrap_or(&path).to_owned();
            namespace_item(segment.clone(), format!("{head}{segment}\\"), Some(path))
        })
        .collect()
}

/// The names this file bound with `as`, and the namespaces it imported by name, that match a bare
/// word being typed.
///
/// First among the names, because they are this file's own: an alias exists so that this file can
/// write it. Written as the alias and nothing else -- the `use` line is already there, and a
/// docblock in the file resolves through it like the code does. One that repeats the class's own
/// short name adds nothing the catalog's row does not already say.
fn aliases_offered(view: &IndexView<'_>, prefix: &str, place: NamePlace) -> Vec<CompletionItem> {
    if prefix.contains('\\') {
        return Vec::new();
    }
    let namespaces = view
        .imported_namespaces_matching(prefix)
        .into_iter()
        .map(|(local, path)| {
            namespace_item(
                local.as_str().to_owned(),
                format!("{}\\", local.as_str()),
                Some(path),
            )
        });
    let aliases = view
        .aliases_matching(prefix, place.kinds())
        .into_iter()
        .filter(|alias| {
            !alias
                .name
                .as_str()
                .eq_ignore_ascii_case(short_name(alias.fqn.as_str()))
        })
        .map(|alias| CompletionItem {
            demoted: false,
            deprecated: false,
            label: alias.name.as_str().to_owned(),
            kind: alias.kind,
            detail: Some(alias.fqn.as_str().to_owned()),
            describe: Some(Describe::TopLevel {
                fqn: alias.fqn.as_str().to_owned(),
                kind: alias.kind,
            }),
            insert: None,
            call: None,
            import: None,
        });
    namespaces.chain(aliases).collect()
}

/// What a qualified prefix wrote up to its last separator, separator included -- the part an item
/// offered under it goes on from. `None` for a bare word.
fn written_head(prefix: &str) -> Option<&str> {
    prefix.rfind('\\').map(|separator| &prefix[..=separator])
}

/// One namespace offered: what the list shows, and what accepting it writes.
fn namespace_item(label: String, insert: String, detail: Option<String>) -> CompletionItem {
    CompletionItem {
        demoted: false,
        deprecated: false,
        label,
        kind: SymbolKind::Namespace,
        detail,
        // Nothing declares a namespace, so there is no declaration to describe.
        describe: None,
        // With the separator, because a namespace is only ever the way to something under it, and
        // the next segment is what the person types next.
        insert: Some(Insertion::plain(insert)),
        call: None,
        import: None,
    }
}

/// The paths a `use` line can take.
///
/// Two things separate an import from a name written in code, and both of them were wrong when this
/// went through [`names`].
///
/// A `use` path is **anchored at the root** however it is written: `use Drupal\Core\Entity\Node;`
/// means that class and not the one under whatever namespace the file declares. Resolved
/// relatively -- which is what an unanchored prefix asks for -- the same line inside `namespace App`
/// searched for `App\Drupal\Core\Entity\` and found nothing, so a qualified prefix in an import
/// offered an empty list.
///
/// And the **label is the whole path**, because the whole path is what the line will contain. A
/// client filters the list against the text an item replaces, which here is everything typed since
/// the keyword: an item labelled `Node` disappears from the list the moment somebody has typed
/// `App\`, however right the insertion behind it is.
pub(super) fn import_paths(view: &IndexView<'_>, offset: TextSize, prefix: &str) -> CompletionList {
    // Only a prefix with a separator in it needs saying. A single segment is looked up by short
    // name, where the anchor means nothing -- and where the whole point is to find the path a
    // reader knows only the class's name of.
    let anchored = prefix
        .contains('\\')
        .then(|| format!("\\{}", prefix.trim_start_matches('\\')));

    // The namespaces first, for the reason `namespaces_written_at` gives -- and here a single
    // segment asks too, because a `use` path is anchored at the root by the language: `use Psr`
    // means the top-level `Psr` in any file, and `use GuzzleHttp` offered nothing at all while the
    // classes it holds were a separator away.
    //
    // A `use` line can name any of the three tables -- `use function` and `use const` exist because
    // it can -- so nothing here narrows `kinds` the way `new` does. A path is one walk for both
    // halves; a single segment is a short-name lookup for the names and a walk for the namespaces.
    let (classes, namespace_paths) = match &anchored {
        Some(anchored) => {
            let found = view.path_continuations(offset, anchored, NameKinds::Any, NAME_LIMIT);
            (found.names, found.namespaces)
        }
        None => (
            view.candidates_with_prefix(offset, prefix, NameKinds::Any, NAME_LIMIT),
            view.namespaces_continuing(prefix, NAME_LIMIT),
        ),
    };
    let mut items: Vec<CompletionItem> = namespace_paths
        .into_iter()
        .map(|path| namespace_item(path.clone(), format!("{path}\\"), None))
        .collect();

    let classes = classes.into_iter().map(|candidate| CompletionItem {
        demoted: false,
        deprecated: candidate.deprecated,
        label: candidate.fqn.as_str().to_owned(),
        kind: candidate.kind,
        // The label is already the path, and a second copy of it beside the row says nothing.
        detail: None,
        describe: Some(Describe::TopLevel {
            fqn: candidate.fqn.as_str().to_owned(),
            kind: candidate.kind,
        }),
        // A `use` statement is anchored at the root by the language, so a leading separator
        // there is a syntax error rather than a clarification -- which makes the label exactly
        // what has to be written.
        insert: None,
        call: None,
        // A `use` line is the import.
        import: None,
    });
    items.extend(classes);

    let is_complete = items.len() < NAME_LIMIT;
    CompletionList {
        items,
        is_complete,
        ..CompletionList::default()
    }
}

/// What has to be written for a name to mean what was offered, and any `use` line it needs.
///
/// Two answers, in order of what a person would have typed. The short name, if it already resolves
/// to this symbol here or if an import can make it -- and the import comes back with it, because
/// inserting one without the other leaves the file naming something that is not there. Failing that,
/// the fully-qualified form with its leading separator: verbose, and always right.
///
/// The middle case is refused more often than it looks. An import changes what a short name means
/// for the whole file, so it is not offered when the name already means something else -- which is
/// exactly when the qualified spelling earns its keep.
fn written_form(
    view: &IndexView<'_>,
    offset: TextSize,
    fqn: &str,
    kind: SymbolKind,
    spelling: Spelling,
) -> (String, Option<ImportEdit>) {
    match spelling {
        // A docblock. Fully qualified and anchored, and never imported: an import is a change to
        // the code, and a comment is not a reason to make one.
        Spelling::Qualified => return (format!("\\{fqn}"), None),
        Spelling::Imported => {}
    }

    // In the table the name actually belongs to. Resolving a function or a constant against the
    // class table finds nothing, so every one of them was offered fully qualified -- and PHP falls
    // back to the global namespace for exactly those two, so `\array_map` and
    // `\JSON_THROW_ON_ERROR` were noise the language does not ask for. A class is different: a bare
    // `Exception` inside `namespace App` means `App\Exception`, so there the separator is required
    // and this check correctly declines.
    let short = short_name(fqn);
    let resolves_here = view
        .resolve_name(offset, short, NameAnchor::Relative, import_kind_of(kind))
        .is_some_and(|location| location.fqn.as_str() == fqn);
    if resolves_here {
        return (short.to_owned(), None);
    }

    match view.import_for(offset, fqn, import_kind_of(kind)) {
        Some(edit) => (short.to_owned(), Some(edit)),
        None => (format!("\\{fqn}"), None),
    }
}

/// How a name has to be written where the cursor is.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Spelling {
    /// Import it if that is possible, else fully qualify. Ordinary code.
    Imported,
    /// Fully qualified with a leading separator, and never imported: inside a docblock.
    Qualified,
}

/// Which of PHP's three symbol tables a candidate belongs to.
///
/// A `use` line has to say: `use function` and `use const` import into tables of their own, and
/// writing a plain `use` for a function imports a class nobody declared.
fn import_kind_of(kind: SymbolKind) -> phrost_view::ImportKind {
    match kind {
        SymbolKind::Function => phrost_view::ImportKind::Function,
        SymbolKind::Constant => phrost_view::ImportKind::Constant,
        _ => phrost_view::ImportKind::ClassLike,
    }
}
