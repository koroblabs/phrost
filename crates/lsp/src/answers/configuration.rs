//! Requests in a file that is not PHP.
//!
//! A framework project's YAML is full of names the index already holds -- a service's class, a
//! route's controller, a menu link's route -- and none of them was reachable, because the buffer is
//! not indexed and has no `FileId` for the ordinary path to start from.
//!
//! So this is a second entry, taken *before* the ordinary one and disjoint from it: a buffer is
//! either one the index holds or one held here as text, never both. What it answers with is the
//! same [`IndexView`] everything else uses, asked with no current file -- a catalog and the def-map
//! are project-wide, and nothing a configuration value names is answered from a file.

use std::path::PathBuf;

use lspkit_runtime::protocol::ls_types::*;
use phrost_view::{ConfigCandidateKind, ConfigReference, ConfigReferent, IndexView};

use crate::{Workspace, server::Backend};

impl Backend {
    /// The path of an open configuration buffer this request is about, if it is one.
    pub(crate) async fn config_of(&self, uri: &Uri) -> Option<PathBuf> {
        let path = uri.to_file_path()?.to_path_buf();
        self.workspace
            .read()
            .await
            .config_text(&path)
            .is_some()
            .then_some(path)
    }
}

impl Backend {
    /// Where the name at `position` in a configuration buffer is declared, and the name's own span.
    pub(crate) async fn jump_in_config(
        &self,
        path: &std::path::Path,
        position: Position,
    ) -> Option<(Range, Location)> {
        let workspace = self.workspace.read().await;
        if let Some((found, text)) = reference_at(&workspace, path, position) {
            return Some((span_of(&text, &found)?, jump(&workspace, &found)?));
        }
        // A template's variable or attribute, where the position is on no name a configuration
        // rule reads.
        let (found, text, lines) = super::twig::symbol_at(&workspace, path, position)?;
        Some((
            super::twig::span_of(&text, &lines, &found)?,
            super::twig::jump(&workspace, path, &text, &lines, &found)?,
        ))
    }

    /// What the name at `position` in a configuration buffer is.
    pub(crate) async fn hover_in_config(
        &self,
        path: &std::path::Path,
        position: Position,
    ) -> Option<Hover> {
        let workspace = self.workspace.read().await;
        if let Some((found, text)) = reference_at(&workspace, path, position) {
            return Some(Hover {
                contents: HoverContents::Markup(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: describe(&workspace.project_view(), &found),
                }),
                range: Some(span_of(&text, &found)?),
            });
        }
        let (found, text, lines) = super::twig::symbol_at(&workspace, path, position)?;
        Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: super::twig::describe(&workspace, &workspace.project_view(), &found),
            }),
            range: Some(super::twig::span_of(&text, &lines, &found)?),
        })
    }

    /// The names that could be written at `position` in a configuration buffer.
    pub(crate) async fn complete_in_config(
        &self,
        path: &std::path::Path,
        position: Position,
    ) -> Option<CompletionResponse> {
        let workspace = self.workspace.read().await;
        let text = workspace.config_text(path)?.clone();
        let name = path.file_name()?.to_str()?;
        let index = phrost_model::LineIndex::new(&text);
        let offset = crate::convert::offset_of(&text, &index, position);

        let view = workspace.project_view();
        let Some((replaced, items)) =
            view.config_completions(name, &text, usize::from(offset), CONFIGURED_KEYS)
        else {
            // A template's variable or attribute being written, where no template name is.
            let (replaced, items) =
                view.twig_completions(path, &text, usize::from(offset), CONFIGURED_KEYS)?;
            let replace = crate::convert::range_of(
                &text,
                &index,
                phrost_model::TextRange::new(
                    phrost_model::TextSize::try_from(replaced.start).ok()?,
                    phrost_model::TextSize::try_from(replaced.end).ok()?,
                ),
            );
            return Some(CompletionResponse::List(CompletionList {
                is_incomplete: items.len() == CONFIGURED_KEYS,
                items: items
                    .into_iter()
                    .enumerate()
                    .map(|(rank, item)| super::twig::item(item, rank, replace))
                    .collect(),
            }));
        };
        let replace = crate::convert::range_of(
            &text,
            &index,
            phrost_model::TextRange::new(
                phrost_model::TextSize::try_from(replaced.start).ok()?,
                phrost_model::TextSize::try_from(replaced.end).ok()?,
            ),
        );

        // A service id is typed with dots in it here as in PHP, and the client filters by the word
        // after the last one -- see `client_word`.
        let typed = text
            .get(replaced.start..usize::from(offset))
            .unwrap_or_default();
        let head = super::client_word::TypedHead::of(typed);

        Some(CompletionResponse::List(CompletionList {
            // The list is cut at `CONFIGURED_KEYS`, so the client must keep asking as the prefix
            // narrows rather than filtering what it already has -- and so must one filtered by what
            // was typed, which the client would narrow with a function of its own.
            is_incomplete: items.len() == CONFIGURED_KEYS || !typed.is_empty(),
            items: items
                .into_iter()
                .enumerate()
                .map(|(rank, item)| CompletionItem {
                    // The order the list was built in, for where a client's scores tie -- the
                    // reason the PHP path gives.
                    sort_text: Some(format!("{rank:05}")),
                    filter_text: head.filter_text(&item.name),
                    label: item.name.to_string(),
                    detail: (!item.detail.is_empty()).then(|| item.detail.to_string()),
                    kind: Some(match item.kind {
                        ConfigCandidateKind::CatalogKey => CompletionItemKind::VALUE,
                        ConfigCandidateKind::Class => CompletionItemKind::CLASS,
                        ConfigCandidateKind::Namespace => CompletionItemKind::MODULE,
                        ConfigCandidateKind::Method => CompletionItemKind::METHOD,
                    }),
                    // An explicit edit over what is already typed, for the reason the PHP path
                    // gives: a client works the replaced word out from its own idea of one, and a
                    // service id is full of dots and slashes that most of them break on.
                    text_edit: Some(CompletionTextEdit::Edit(TextEdit {
                        range: replace,
                        new_text: item.name.to_string(),
                    })),
                    ..CompletionItem::default()
                })
                .collect(),
        }))
    }
}

/// How many keys one configuration completion offers.
///
/// One real project holds 1 856 service ids and 1 350 route names, and an empty prefix matches every
/// one of them. The same reasoning the PHP side records: the caller says what it can afford and
/// tells the client the list is partial.
const CONFIGURED_KEYS: usize = 200;

/// What one position in a configuration buffer refers to, with the file's text for converting
/// positions back.
pub(crate) fn reference_at(
    workspace: &Workspace,
    path: &std::path::Path,
    position: Position,
) -> Option<(ConfigReference, std::sync::Arc<str>)> {
    let text = workspace.config_text(path)?.clone();
    let name = path.file_name()?.to_str()?;
    let index = phrost_model::LineIndex::new(&text);
    let offset = crate::convert::offset_of(&text, &index, position);

    let view = workspace.project_view();
    let found = view.config_reference_at(name, &text, usize::from(offset))?;
    Some((found, text))
}

/// The name a configuration reference is about, as the protocol spells a range in the buffer.
///
/// What a hover marks and what a goto says was clicked, and both need it from the server: a YAML
/// client's word stops at the dots of a service id and the backslashes of a class, so left to its
/// own idea it marks `mine` in `'@mine.repo'`.
fn span_of(text: &str, found: &ConfigReference) -> Option<Range> {
    let index = phrost_model::LineIndex::new(text);
    let range = phrost_model::TextRange::new(
        phrost_model::TextSize::try_from(found.range.start).ok()?,
        phrost_model::TextSize::try_from(found.range.end).ok()?,
    );
    Some(crate::convert::range_of(text, &index, range))
}

/// Where a configuration reference points, as the protocol spells a jump.
pub(crate) fn jump(workspace: &Workspace, found: &ConfigReference) -> Option<Location> {
    match &found.referent {
        ConfigReferent::Declaration(location) => {
            let (text, index) = workspace.text_for_positions(location.file)?;
            let uri = Uri::from_file_path(workspace.path_of(location.file))?;
            Some(Location::new(
                uri,
                crate::convert::range_of(&text, &index, location.name_range),
            ))
        }
        ConfigReferent::CatalogClass { location, .. } => {
            let (text, index) = workspace.text_for_positions(location.file)?;
            let uri = Uri::from_file_path(workspace.path_of(location.file))?;
            Some(Location::new(
                uri,
                crate::convert::range_of(&text, &index, location.name_range),
            ))
        }
        // Declared in another file that is not PHP, so the position was recorded when it was read
        // and there is no text here to convert. The key's own length selects it, which is its length
        // in characters because a route name and a permission are written in ASCII.
        ConfigReferent::CatalogEntry { place, .. } => {
            let place = place.as_ref()?;
            let uri = Uri::from_file_path(&place.path)?;
            let at = Position::new(place.line, place.character);
            let end = Position::new(
                place.line,
                place.character + u32::try_from(found.name.chars().count()).unwrap_or(0),
            );
            Some(Location::new(uri, Range::new(at, end)))
        }
        ConfigReferent::Unregistered { .. } => None,
    }
}

/// What to say about a configuration reference, in the same shape a hover over PHP is written in.
pub(crate) fn describe(view: &IndexView<'_>, found: &ConfigReference) -> String {
    match &found.referent {
        ConfigReferent::Declaration(location) => match &location.container {
            Some(class) => format!("```php\n{class}::{}\n```", location.fqn),
            None => format!("```php\n{}\n```", location.fqn),
        },
        // The id and the class it hands back, because a reader looking at `'@app.repo'` wants the
        // class and has to be told which id gave it -- the same two halves the PHP hover over a
        // catalog key already shows.
        ConfigReferent::CatalogClass { class, location } => {
            let mut out = format!("```php\n{class}\n```");
            if let Some(summary) = view.documentation(location) {
                out.push_str("\n\n");
                out.push_str(&summary);
            }
            out
        }
        ConfigReferent::CatalogEntry { detail, .. } => match detail.is_empty() {
            true => format!("`{}`", found.name),
            false => format!("`{}`\n\n{detail}", found.name),
        },
        ConfigReferent::Unregistered { catalog } => {
            format!("`{}`\n\nNothing registers this {catalog}.", found.name)
        }
    }
}
