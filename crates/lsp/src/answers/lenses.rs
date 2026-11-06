//! How many places use the thing declared on this line.
//!
//! The question a reader asks of a method before changing it, and the one they otherwise answer by
//! invoking a search and reading the count off the panel. It is worth a line above the declaration
//! only because it is now cheap to answer: a reference sweep was 59.6 ms at the median and is
//! 2.8 ms, which is what made this affordable rather than a good idea somebody kept refusing.
//!
//! **Two requests, and the split is the whole design.** A file declares tens of things and a
//! sweep costs milliseconds each, so answering them all on `textDocument/codeLens` would put
//! tenths of a second between an editor opening a file and painting it. So that request answers
//! with *places* -- an outline walk, no sweeping -- and the count is computed in
//! `codeLens/resolve`, which a client asks only for the lenses it is about to show. Scrolling
//! pays for what comes into view and nothing else.
//!
//! Neither half needs a budget row of its own, and that is the point of the split rather than an
//! omission: the listing *is* `document_symbols`, which has one at 12 ms, and the resolve is the
//! same sweep find-references runs. Measured on the largest file of a real project -- 184 lenses,
//! listing 1.1 ms, resolve 1.7 ms at the median and 11.5 at p90. The 64 ms worst case is a method
//! with 1 392 uses, where the number is large because the answer is.
//!
//! A title and no command. Making the lens clickable means `workspace/executeCommand`, which this
//! server deliberately does not implement -- see `server.rs` on why a configuration notification
//! is refused for the same kind of reason -- and a reader who wants the list has "find references"
//! on the same word.

use lspkit_runtime::protocol::{jsonrpc::Result, ls_types::*};

use crate::{convert, server::Backend};

/// What travels from the listing to the resolve, which is only where to ask again.
///
/// A position rather than a byte offset, and rather than anything derived from the index: a client
/// may resolve a lens after the buffer has moved on, and a position is the one currency both ends
/// of the round trip already agree on. A lens resolved against a changed document answers about
/// whatever is at that position now, which is what re-requesting the lenses would have said
/// anyway.
struct Anchor {
    uri: Uri,
    position: Position,
}

impl Anchor {
    /// Written by hand rather than derived, which keeps `serde`'s derive out of this crate's
    /// dependencies for three fields nothing else reads.
    fn value(&self) -> serde_json::Value {
        serde_json::json!({
            "uri": self.uri.as_str(),
            "line": self.position.line,
            "character": self.position.character,
        })
    }

    fn of(data: &serde_json::Value) -> Option<Self> {
        Some(Self {
            uri: data["uri"].as_str()?.parse().ok()?,
            position: Position {
                line: u32::try_from(data["line"].as_u64()?).ok()?,
                character: u32::try_from(data["character"].as_u64()?).ok()?,
            },
        })
    }
}

impl Backend {
    pub(crate) async fn answer_code_lens(&self, uri: &Uri) -> Result<Option<Vec<CodeLens>>> {
        // Silent rather than refused while indexing: a client asks for these the moment a file
        // opens, and an error there is a notification in front of somebody who has not asked for
        // anything. It re-asks when the index reports itself ready.
        if !self.workspace.read().await.is_ready() {
            return Ok(None);
        }
        let Some((_, file)) = self.file_of(uri).await else {
            return Ok(None);
        };
        let workspace = self.workspace.read().await;
        let Some((text, lines)) = workspace.document_text(file) else {
            return Ok(None);
        };

        let view = workspace.view(file);
        let mut out = Vec::new();
        for entry in phrost_analysis::document_symbols(&view) {
            // Declarations a reader asks the question of. Not a property, a constant or an enum
            // case: they are used constantly and in numbers nobody acts on, and a lens over every
            // one of them is a second gutter rather than an answer.
            let wanted = |kind| {
                matches!(
                    kind,
                    phrost_model::SymbolKind::Class
                        | phrost_model::SymbolKind::Interface
                        | phrost_model::SymbolKind::Trait
                        | phrost_model::SymbolKind::Enum
                        | phrost_model::SymbolKind::Function
                        | phrost_model::SymbolKind::Method
                )
            };
            for at in std::iter::once(&entry)
                .chain(&entry.children)
                .filter(|entry| wanted(entry.kind))
            {
                let range = convert::range_of(&text, &lines, at.selection_range);
                out.push(CodeLens {
                    range,
                    command: None,
                    data: Some(
                        Anchor {
                            uri: uri.clone(),
                            position: range.start,
                        }
                        .value(),
                    ),
                });
            }
        }
        Ok(Some(out))
    }

    /// The count for one lens, asked when the client is about to show it.
    pub(crate) async fn answer_code_lens_resolve(&self, lens: CodeLens) -> Result<CodeLens> {
        let Some(anchor) = lens.data.as_ref().and_then(Anchor::of) else {
            return Ok(lens);
        };
        let Some(count) = self.reference_count(&anchor).await else {
            return Ok(lens);
        };

        Ok(CodeLens {
            command: Some(Command {
                title: match count {
                    1 => "1 reference".to_owned(),
                    _ => format!("{count} references"),
                },
                // Deliberately empty: a client renders the title and offers nothing to click.
                command: String::new(),
                arguments: None,
            }),
            ..lens
        })
    }

    /// How many places reach the declaration at `anchor`, not counting the declaration itself.
    async fn reference_count(&self, anchor: &Anchor) -> Option<usize> {
        let (_, file) = self.file_of(&anchor.uri).await?;
        let (target, declaration, outside, needles) = {
            let workspace = self.workspace.read().await;
            let (text, lines) = workspace.document_text(file)?;
            let offset = convert::offset_of(&text, &lines, anchor.position);
            let view = workspace.view(file);
            let target = phrost_analysis::reference_target(&view, offset)?;
            // The places outside PHP, counted here because find-references lists them: a lens
            // that says three over a service class whose list opens with four is a lens a reader
            // stops believing. Through `config_sites_for`, which is the one place that decides
            // which lookup a target kind gets -- this answer had no copy of that rule at all.
            let outside = super::config_sites_for(&workspace, &target).len();
            let needles = target.sweep_query(&view);
            (target, view.declaration_at(offset), outside, needles)
        };

        let found = match needles.is_empty() {
            false => super::sweep_references(&self.workspace, &target, &needles).await,
            true => self.references_here(file, &target).await,
        };
        // The declaration is one of what a sweep finds, and it is not a use of itself. Matched by
        // where it is rather than counted off, because a sweep of an overridden method finds the
        // other declarations too and those are not uses either.
        Some(
            found
                .into_iter()
                .filter(|reference| {
                    declaration.as_ref().is_none_or(|at| {
                        at.file != reference.file || at.name_range != reference.range
                    })
                })
                .count()
                + outside,
        )
    }
}
