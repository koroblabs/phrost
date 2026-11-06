//! Requests about a variable or an attribute in a Twig template.
//!
//! The second half of what a template buffer answers, after the template names a configuration
//! reference covers: `{{ node.label }}` is a variable handed over by what renders the template and
//! a getter read off it, and the view says which -- this only turns the answer into the protocol's
//! shape.

use std::{path::Path, sync::Arc};

use lspkit_runtime::protocol::ls_types::*;
use phrost_model::{LineIndex, TextRange, TextSize};
use phrost_view::{
    IndexView,
    twig::{TwigCandidate, TwigCandidateKind, TwigOrigin, TwigSymbol, TwigSymbolKind},
};

use crate::{Workspace, convert};

/// What the name at `position` in a template buffer is, with the buffer's text and lines.
pub(crate) fn symbol_at(
    workspace: &Workspace,
    path: &Path,
    position: Position,
) -> Option<(TwigSymbol, Arc<str>, LineIndex)> {
    let text = workspace.config_text(path)?.clone();
    let lines = LineIndex::new(&text);
    let offset = convert::offset_of(&text, &lines, position);
    let found = workspace
        .project_view()
        .twig_symbol_at(path, &text, usize::from(offset))?;
    Some((found, text, lines))
}

/// The name's own span, as the protocol spells a range in the buffer.
pub(crate) fn span_of(text: &str, lines: &LineIndex, found: &TwigSymbol) -> Option<Range> {
    let range = TextRange::new(
        TextSize::try_from(found.range.start).ok()?,
        TextSize::try_from(found.range.end).ok()?,
    );
    Some(convert::range_of(text, lines, range))
}

/// Where a variable gets its value, or where the member an attribute reads is declared.
pub(crate) fn jump(
    workspace: &Workspace,
    path: &Path,
    text: &str,
    lines: &LineIndex,
    found: &TwigSymbol,
) -> Option<Location> {
    match &found.kind {
        TwigSymbolKind::Variable { origin, .. } => match origin {
            TwigOrigin::Bound(range) => Some(Location::new(
                Uri::from_file_path(path)?,
                convert::range_of(
                    text,
                    lines,
                    TextRange::new(
                        TextSize::try_from(range.start).ok()?,
                        TextSize::try_from(range.end).ok()?,
                    ),
                ),
            )),
            TwigOrigin::Rendered { file, range } => {
                let (text, lines) = workspace.text_for_positions(*file)?;
                Some(Location::new(
                    Uri::from_file_path(workspace.path_of(*file))?,
                    convert::range_of(&text, &lines, *range),
                ))
            }
            TwigOrigin::Included { template, range } => {
                let text = workspace.held_template_text(template)?;
                let lines = LineIndex::new(text);
                Some(Location::new(
                    Uri::from_file_path(template)?,
                    convert::range_of(
                        text,
                        &lines,
                        TextRange::new(
                            TextSize::try_from(range.start).ok()?,
                            TextSize::try_from(range.end).ok()?,
                        ),
                    ),
                ))
            }
            TwigOrigin::Global | TwigOrigin::Unbound => None,
        },
        TwigSymbolKind::Member { location, .. } => {
            let (text, lines) = workspace.text_for_positions(location.file)?;
            Some(Location::new(
                Uri::from_file_path(workspace.path_of(location.file))?,
                convert::range_of(&text, &lines, location.name_range),
            ))
        }
        TwigSymbolKind::Key { .. } => None,
        // What runs it, or -- for a closure -- where the extension declares it.
        TwigSymbolKind::Called(called) => {
            let (file, range) = match &called.runs {
                Some(location) => (location.file, location.name_range),
                None => called.declared,
            };
            let (text, lines) = workspace.text_for_positions(file)?;
            Some(Location::new(
                Uri::from_file_path(workspace.path_of(file))?,
                convert::range_of(&text, &lines, range),
            ))
        }
    }
}

/// What to say about a name in a template, in the shape a hover over PHP is written in.
pub(crate) fn describe(workspace: &Workspace, view: &IndexView<'_>, found: &TwigSymbol) -> String {
    let ty = found.ty.as_deref();
    match &found.kind {
        TwigSymbolKind::Variable { name, origin } => {
            let mut out = match ty {
                Some(ty) => format!("```php\n{ty} ${name}\n```"),
                None => format!("```php\n${name}\n```"),
            };
            let said = match origin {
                TwigOrigin::Bound(_) => Some("Bound in this template.".to_owned()),
                TwigOrigin::Rendered { file, range } => {
                    let full = workspace.path_of(*file);
                    let shown = full.file_name().map_or_else(
                        || full.display().to_string(),
                        |name| name.to_string_lossy().into_owned(),
                    );
                    let line = workspace.text_for_positions(*file).map(|(text, lines)| {
                        convert::range_of(&text, &lines, *range).start.line + 1
                    });
                    Some(match line {
                        Some(line) => format!("Handed over in `{shown}:{line}`."),
                        None => format!("Handed over in `{shown}`."),
                    })
                }
                TwigOrigin::Included { template, range } => {
                    let shown = template.file_name().map_or_else(
                        || template.display().to_string(),
                        |name| name.to_string_lossy().into_owned(),
                    );
                    let line = workspace
                        .held_template_text(template)
                        .and_then(|text| text.get(..range.start))
                        .map(|before| before.matches('\n').count() + 1);
                    Some(match line {
                        Some(line) => format!("Handed over by `{shown}:{line}`."),
                        None => format!("Handed over by `{shown}`."),
                    })
                }
                TwigOrigin::Global => Some("Every template has it.".to_owned()),
                TwigOrigin::Unbound => None,
            };
            if let Some(said) = said {
                out.push_str("\n\n");
                out.push_str(&said);
            }
            out
        }
        TwigSymbolKind::Member { location, .. } => {
            let class = location
                .container
                .as_ref()
                .map_or("", |class| class.as_str());
            let mut out = match &location.signature {
                Some(signature) => format!("```php\n{class}\n{signature}\n```"),
                None => format!("```php\n{class}::{}\n```", location.fqn),
            };
            if let Some(summary) = view.documentation(location) {
                out.push_str("\n\n");
                out.push_str(&summary);
            }
            out
        }
        TwigSymbolKind::Called(called) => {
            let kind = match called.kind {
                phrost_view::twig::TwigCallableKind::Filter => "filter",
                phrost_view::twig::TwigCallableKind::Function => "function",
                phrost_view::twig::TwigCallableKind::Test => "test",
            };
            let mut out = match &called.runs {
                Some(location) => match (&location.container, &location.signature) {
                    (Some(class), Some(signature)) => format!("```php\n{class}\n{signature}\n```"),
                    _ => format!("```php\n{}\n```", location.fqn),
                },
                None => format!("```twig\n{}\n```", called.name),
            };
            out.push_str(&format!(
                "\n\nTwig {kind} `{}`, declared by `{}`.",
                called.name,
                phrost_model::short_name(&called.declarer)
            ));
            if let Some(summary) = called
                .runs
                .as_ref()
                .and_then(|location| view.documentation(location))
            {
                out.push_str("\n\n");
                out.push_str(&summary);
            }
            out
        }
        TwigSymbolKind::Key { name } => match ty {
            Some(ty) => format!("```php\n{ty}\n```\n\nThe key `{name}`."),
            None => format!("The key `{name}`."),
        },
    }
}

/// A candidate as the protocol's completion item, replacing `replace`.
pub(crate) fn item(candidate: TwigCandidate, rank: usize, replace: Range) -> CompletionItem {
    CompletionItem {
        // The order the list was built in, nearest binding first, for where a client's scores tie.
        sort_text: Some(format!("{rank:05}")),
        label: candidate.name.to_string(),
        detail: candidate.detail.map(|detail| detail.to_string()),
        kind: Some(match candidate.kind {
            TwigCandidateKind::Variable => CompletionItemKind::VARIABLE,
            TwigCandidateKind::Property => CompletionItemKind::PROPERTY,
            TwigCandidateKind::Method => CompletionItemKind::METHOD,
            TwigCandidateKind::Key => CompletionItemKind::FIELD,
            TwigCandidateKind::Filter | TwigCandidateKind::Function => CompletionItemKind::FUNCTION,
        }),
        text_edit: Some(CompletionTextEdit::Edit(TextEdit {
            range: replace,
            new_text: candidate.name.to_string(),
        })),
        ..CompletionItem::default()
    }
}
