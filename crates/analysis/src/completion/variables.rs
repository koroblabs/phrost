//! Locals and superglobals, for a `$` that has been typed -- and for a closure's capture list.

use phrost_model::{SymbolKind, TextSize};
use phrost_view::IndexView;

use super::{CompletionItem, CompletionList};

/// Where the variables are being offered, and what that place refuses.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum VariablePlace<'a> {
    /// After a `$`, where every variable the position binds can stand.
    Expression,
    /// A closure's `use (...)`, which takes a variable of the enclosing scope -- and not `$this`,
    /// which a closure binds by itself and which PHP refuses outright as a lexical variable, and
    /// not one the list already holds, which is a fatal error of its own.
    Capture { already: &'a [String] },
}

/// Locals and superglobals, for a `$` that has been typed.
///
/// Case-sensitively, unlike every other completion here: PHP variable names are case-sensitive, and
/// offering `$Name` for `$na` would suggest a variable that does not exist.
pub(super) fn variables(
    view: &IndexView<'_>,
    offset: TextSize,
    prefix: &str,
    place: VariablePlace<'_>,
) -> CompletionList {
    let already: &[String] = match place {
        VariablePlace::Capture { already } => already,
        VariablePlace::Expression => &[],
    };
    let mut items: Vec<CompletionItem> = view
        .locals_in_scope(offset)
        .into_iter()
        .filter(|name| name.starts_with(prefix))
        .filter(|name| !already.iter().any(|written| written.as_str() == &**name))
        .map(|name| CompletionItem {
            demoted: false,
            deprecated: false,
            label: format!("${name}"),
            kind: SymbolKind::Variable,
            detail: None,
            // A local's declaration is one line up and a reader is looking at it.
            describe: None,
            // The `$` included, and the label used as-is. A client computes the range it replaces
            // from its own idea of a word, and PHP's includes the `$` -- so inserting only the rest
            // of the name replaces the `$` with it and `$ap` becomes `app`.
            insert: None,
            call: None,
            // A local is never imported.
            import: None,
        })
        .collect();

    // `$this` is bound by the call rather than by anything in the body, so it is in no scope and
    // `locals_in_scope` has never yielded it -- which left the most-typed variable in PHP the one
    // name completion would not offer. First, because it is the one a person is reaching for; and
    // never inside a static method, where writing it fatals.
    if matches!(place, VariablePlace::Expression)
        && "this".starts_with(prefix)
        && view.binds_this(offset)
    {
        items.insert(
            0,
            CompletionItem {
                demoted: false,
                deprecated: false,
                label: "$this".to_owned(),
                kind: SymbolKind::Variable,
                detail: None,
                describe: None,
                insert: None,
                call: None,
                import: None,
            },
        );
    }

    CompletionList {
        items,
        is_complete: true,
        ..CompletionList::default()
    }
}

/// The parameters a docblock can name, for a `$` typed inside it: `@param string $te|`.
///
/// What goto and hover already read the same `$name` as -- see `docblock_scope_at`. Nothing else
/// is in scope inside a comment, so the list was empty at the one place a person writes a parameter
/// name out by hand.
///
/// On a `@param` line, the parameters another `@param` already documents are left out: a second
/// tag for one parameter is the mistake `doc-param-name` reports, and offering it is offering that
/// mistake. In the prose of the block every parameter is fair, since a sentence may mention any.
pub(super) fn documented_parameters(
    view: &IndexView<'_>,
    offset: TextSize,
    prefix: &str,
) -> Option<CompletionList> {
    let scope = view.docblock_scope_at(offset)?;
    let docblock = &scope.docblock;
    let at = usize::from(offset - docblock.range.start());
    let line_start = docblock.text.get(..at)?.rfind('\n').map_or(0, |at| at + 1);
    let line_end = docblock.text[at..]
        .find('\n')
        .map_or(docblock.text.len(), |end| at + end);
    let on_a_param_line = after_param_tag(&docblock.text[line_start..at]).is_some();

    let documented: Vec<&str> = match on_a_param_line {
        true => docblock.text[..line_start]
            .lines()
            .chain(docblock.text[line_end..].lines())
            .filter_map(param_tag_subject)
            .collect(),
        false => Vec::new(),
    };

    Some(CompletionList {
        items: scope
            .parameters
            .iter()
            .filter(|parameter| parameter.name.as_str().starts_with(prefix))
            .filter(|parameter| !documented.contains(&parameter.name.as_str()))
            .map(|parameter| CompletionItem {
                demoted: false,
                deprecated: false,
                label: format!("${}", parameter.name),
                kind: SymbolKind::Variable,
                detail: None,
                describe: None,
                insert: None,
                call: None,
                import: None,
            })
            .collect(),
        is_complete: true,
        ..CompletionList::default()
    })
}

/// What follows the tag on a `@param` line, when the line is one.
fn after_param_tag(line: &str) -> Option<&str> {
    let (_, after) = line.split_once('@')?;
    let rest = after
        .strip_prefix("phpstan-")
        .or_else(|| after.strip_prefix("psalm-"))
        .unwrap_or(after)
        .strip_prefix("param")?;
    rest.starts_with(char::is_whitespace).then_some(rest)
}

/// The `$name` a `@param` line documents.
fn param_tag_subject(line: &str) -> Option<&str> {
    let (_, name) = after_param_tag(line)?.split_once('$')?;
    let end = name
        .find(|ch: char| !(ch.is_alphanumeric() || ch == '_'))
        .unwrap_or(name.len());
    Some(&name[..end])
}

/// The locals a `compact()` string can name: `compact('fr|')` offers `from`.
///
/// Written without the `$`, which is how the string spells the variable -- and never a superglobal,
/// which `compact()` cannot collect from a function's scope at all.
pub(super) fn compacted_names(
    view: &IndexView<'_>,
    offset: TextSize,
    prefix: &str,
) -> CompletionList {
    CompletionList {
        items: view
            .locals_in_scope(offset)
            .into_iter()
            .filter(|name| !phrost_model::SUPERGLOBALS.contains(&&**name))
            .filter(|name| name.starts_with(prefix))
            .map(|name| CompletionItem {
                demoted: false,
                deprecated: false,
                label: name.into_string(),
                kind: SymbolKind::Variable,
                detail: None,
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
