//! Changing a name everywhere it is used, and refusing when that cannot be done safely.

use std::collections::HashMap;

use lspkit_runtime::protocol::{
    jsonrpc::{Error, Result},
    ls_types::*,
};

use crate::{
    answers::{positions::Positions, sweep_references},
    convert,
    server::Backend,
};

impl Backend {
    pub(crate) async fn answer_prepare_rename(
        &self,
        params: TextDocumentPositionParams,
    ) -> Result<Option<PrepareRenameResponse>> {
        self.require_ready().await?;

        let Some((_, file)) = self.file_of(&params.text_document.uri).await else {
            return Ok(None);
        };
        let workspace = self.workspace.read().await;
        let Some((text, lines)) = workspace.document_text(file) else {
            return Ok(None);
        };
        let offset = convert::offset_of(&text, &lines, params.position);

        let view = workspace.view(file);
        // The declarations above first, which are a header read each: a member something outside
        // the project also declares is refused whatever lies below it, and the walk below is a
        // pass over the class table per class -- every test, for a `setUp()`.
        if let Some(above) = phrost_analysis::rename_target_above(&view, offset)
            && let Some(why) = outside_the_project(&workspace, &view, &above)
        {
            return Err(cannot_rename(&why));
        }
        let Some(target) = phrost_analysis::rename_target(&view, offset) else {
            return Ok(None);
        };
        if !workspace.is_own_code(home_of(&view, offset, file, &target)) {
            return Err(cannot_rename(
                "this declaration is not in this project's own code",
            ));
        }
        if let Some(why) = outside_the_project(&workspace, &view, &target) {
            return Err(cannot_rename(&why));
        }
        // Before the dialogue opens, not after a name has been typed into it: a rename that cannot
        // finish should never look available.
        if let Some(why) = phrost_analysis::rename_refusal(&view, &target, None) {
            return Err(cannot_rename(&why));
        }

        // The range at the cursor, when the cursor is on something the catalogs do not hold.
        //
        // A local is always that. So is a *member*, when a promoted constructor parameter put the
        // property's own name in the body: `$across` there is neither a declaration nor a member
        // access, which is both of the things the lookup below searches, so the dialogue refused to
        // open at a position the rename itself handles correctly.
        let here = match &target {
            phrost_analysis::ReferenceTarget::Alias { name } => view
                .alias_occurrences(name.as_str())
                .into_iter()
                .find(|range| range.contains_inclusive(offset)),
            // Whichever occurrence the cursor is on, a docblock's own `$name` included: the rename
            // edits it, so the dialogue has to open there too. A promoted property is reached from
            // its constructor's `@param` the same way a plain parameter is.
            phrost_analysis::ReferenceTarget::Local { .. }
            | phrost_analysis::ReferenceTarget::MemberFamily {
                kind: phrost_view::MemberKind::Property,
                ..
            } => view
                .variable_at(offset)
                .map(|variable| variable.range)
                .or_else(|| {
                    phrost_analysis::references_in(&view, &target)
                        .into_iter()
                        .find(|range| range.contains_inclusive(offset))
                }),
            _ => view.variable_at(offset).map(|variable| variable.range),
        };
        if let Some(range) = here {
            let at = phrost_analysis::rename_range(&text, range);
            return Ok(Some(PrepareRenameResponse::RangeWithPlaceholder {
                range: convert::range_of(&text, &lines, at),
                placeholder: text[usize::from(at.start())..usize::from(at.end())].to_owned(),
            }));
        }
        if matches!(
            target,
            phrost_analysis::ReferenceTarget::Local { .. }
                | phrost_analysis::ReferenceTarget::Alias { .. }
        ) {
            return Ok(None);
        }

        // The one answer to "what would a rename replace here", so the dialogue cannot open where
        // the rename does nothing -- or refuse where it does forty edits. See `rename_span`.
        let Some(at) = phrost_analysis::rename_span(&view, offset) else {
            return Ok(None);
        };
        let at = phrost_analysis::rename_range(&text, at);

        Ok(Some(PrepareRenameResponse::RangeWithPlaceholder {
            range: convert::range_of(&text, &lines, at),
            placeholder: text[usize::from(at.start())..usize::from(at.end())].to_owned(),
        }))
    }

    pub(crate) async fn answer_rename(
        &self,
        params: RenameParams,
    ) -> Result<Option<WorkspaceEdit>> {
        self.require_ready().await?;

        // Refused rather than applied and regretted: a name PHP would not accept leaves every file
        // it touched unparseable, and an editor's undo is one step per file.
        if !phrost_analysis::is_valid_name(&params.new_name) {
            return Err(cannot_rename("that is not a name PHP would accept"));
        }

        let Some((_, file)) = self
            .file_of(&params.text_document_position.text_document.uri)
            .await
        else {
            return Ok(None);
        };

        let workspace = self.workspace.read().await;
        let Some((text, lines)) = workspace.document_text(file) else {
            return Ok(None);
        };
        let offset = convert::offset_of(&text, &lines, params.text_document_position.position);

        let view = workspace.view(file);
        // The declarations above first, which are a header read each: a member something outside
        // the project also declares is refused whatever lies below it, and the walk below is a
        // pass over the class table per class -- every test, for a `setUp()`.
        if let Some(above) = phrost_analysis::rename_target_above(&view, offset)
            && let Some(why) = outside_the_project(&workspace, &view, &above)
        {
            return Err(cannot_rename(&why));
        }
        let Some(target) = phrost_analysis::rename_target(&view, offset) else {
            return Ok(None);
        };
        if !workspace.is_own_code(home_of(&view, offset, file, &target)) {
            return Err(cannot_rename(
                "this declaration is not in this project's own code",
            ));
        }
        if let Some(why) = outside_the_project(&workspace, &view, &target) {
            return Err(cannot_rename(&why));
        }
        // Asked again with the name, because two of the reasons to decline need it and one does
        // not: a reserved word is about what was typed, an override family is about what was
        // clicked, and `prepareRename` has only the second.
        if let Some(why) = phrost_analysis::rename_refusal(&view, &target, Some(&params.new_name)) {
            return Err(cannot_rename(&why));
        }
        let needles = target.sweep_query(&view);
        drop(view);
        drop(workspace);

        // A target that cannot leave its file has nothing to sweep for, and sweeping anyway would be
        // worse than slow: the sweep matches a *name*, and a local's name means a different variable
        // in every function that shares the spelling.
        let found = match needles.is_empty() {
            false => sweep_references(&self.workspace, &target, &needles).await,
            true => self.references_here(file, &target).await,
        };

        // The lock is taken again, and only to turn ranges into positions.
        let workspace = self.workspace.read().await;

        // Grouped by file, because that is the shape the protocol wants and because each file's
        // ranges have to be turned into positions with that file's own line map.
        let mut positions = Positions::over(&workspace);
        let mut edits: HashMap<Uri, Vec<TextEdit>> = HashMap::new();
        for reference in found {
            // Only the last segment: replacing the whole of `App\Repo` would import a class in the
            // global namespace.
            let Some(uri) = positions.uri(reference.file) else {
                continue;
            };
            let Some(at) = positions.rename_range(reference.file, reference.range) else {
                continue;
            };
            edits.entry(uri).or_default().push(TextEdit {
                range: at,
                new_text: params.new_name.clone(),
            });
        }

        // The places outside PHP, where the two halves of a `Class::method` are renamed
        // differently. Left out, a renamed service class keeps a `services.yml` pointing at a
        // class that is gone and a renamed controller action keeps a route pointing at a method
        // that is -- neither of which anything reports, because the file is not PHP.
        // Where they are is `config_sites_for`'s answer and the same one find-references and the
        // code lens read. What is left here is *what to replace*, which is a different question
        // and the only part that is a rename's business.
        let sites = super::config_sites_for(&workspace, &target);
        let outside: Vec<_> = match &target {
            // The last segment, taken from the end: a class is written qualified there and only
            // its final word is what a rename replaces.
            phrost_analysis::ReferenceTarget::ClassUnderItsOwnName(fqn) => sites
                .into_iter()
                .map(|(path, range)| {
                    let short = u32::try_from(short_name(fqn.as_str()).len()).unwrap_or_default();
                    let at = phrost_model::TextRange::new(
                        range.end() - phrost_model::TextSize::from(short),
                        range.end(),
                    );
                    (path, at)
                })
                .collect(),
            // A method is written whole, so its range is already the word to replace.
            _ => sites,
        };
        for (path, at) in outside {
            let Some(at) = positions.locate_path(&path, at) else {
                continue;
            };
            edits.entry(at.uri).or_default().push(TextEdit {
                range: at.range,
                new_text: params.new_name.clone(),
            });
        }

        Ok(Some(WorkspaceEdit {
            changes: Some(one_edit_per_place(edits)),
            document_changes: None,
            change_annotations: None,
        }))
    }
}

/// One edit per place, which the protocol requires and an editor does not check.
///
/// Two references can be one range here even though they are two in the index: `rename_range`
/// replaces the *last segment* of what is written, so a site written `App\\Repo` and a site
/// written `Repo` land on the same characters, and a docblock's `{@see Repo}` beside the name
/// reference it carries does too. A client applies a workspace edit as it is given -- so the
/// second edit replaced the text the first had just written, and `DustState` renamed and renamed
/// back came out `ZzDustStateRenamedteRenamed`. Found on psl by
/// `tools/a-rename-goes-there-and-back.py`: 5 of 19 round trips, every one of them a file left
/// holding text nobody wrote.
///
/// Overlap and not only equality, because the reason two edits meet is that one range is derived
/// from another and nothing guarantees they agree on where it ends. The first is kept, which is
/// the leftmost: the ranges are sorted, so a wider one starting earlier wins over what it covers.
fn one_edit_per_place(edits: HashMap<Uri, Vec<TextEdit>>) -> HashMap<Uri, Vec<TextEdit>> {
    edits
        .into_iter()
        .map(|(uri, mut here)| {
            here.sort_by_key(|edit| (edit.range.start, edit.range.end));
            let mut kept: Vec<TextEdit> = Vec::with_capacity(here.len());
            for edit in here {
                if kept
                    .last()
                    .is_none_or(|previous| previous.range.end <= edit.range.start)
                {
                    kept.push(edit);
                }
            }
            (uri, kept)
        })
        .collect()
}

/// The file whose ownership decides whether a rename is allowed.
///
/// For a class or a member that is where it was declared: one in `vendor/` is not this project's to
/// rename, and an edit there is undone by the next `composer install`. A local has no declaration
/// anywhere else, and neither has an alias -- a `use ... as ...` line binds a name in the file that
/// wrote it -- so for both, the file the request came from is the one that has to qualify.
/// A declaration the rename would have to move that the project does not own, when there is one.
///
/// Only a family can have one: the class at the cursor was already checked, and every *other*
/// declaration the rename moves has to be checked the same way. An interface under `vendor/`
/// declaring the same method makes the whole rename one the next `composer install` undoes, and
/// renaming only the half that is ours leaves a contract nothing satisfies.
fn outside_the_project(
    workspace: &crate::workspace::Workspace,
    view: &phrost_view::IndexView<'_>,
    target: &phrost_analysis::ReferenceTarget,
) -> Option<String> {
    let phrost_analysis::ReferenceTarget::MemberFamily {
        declarers,
        kind,
        name,
        ..
    } = target
    else {
        return None;
    };

    let outside = declarers.iter().find(|declared_by| {
        view.member_location(declared_by.as_str(), *kind, name.as_str())
            .is_none_or(|location| !workspace.is_own_code(location.file))
    })?;

    Some(format!(
        "`{name}` is also declared by `{}`, which is not in this project's own code -- renaming it          there would be undone by the next install, and renaming only the rest leaves a contract          half moved",
        short_name(outside.as_str())
    ))
}

fn home_of(
    view: &phrost_view::IndexView<'_>,
    offset: phrost_model::TextSize,
    file: phrost_model::FileId,
    target: &phrost_analysis::ReferenceTarget,
) -> phrost_model::FileId {
    match target {
        phrost_analysis::ReferenceTarget::Local { .. }
        | phrost_analysis::ReferenceTarget::Alias { .. } => file,
        phrost_analysis::ReferenceTarget::Parameter(parameter) => view
            .parameter_declaration(parameter)
            .map_or(file, |location| location.file),
        _ => view
            .declaration_at(offset)
            .map_or(file, |location| location.file),
    }
}

/// The last segment of a qualified name, which is the part a rename replaces.
fn short_name(fqn: &str) -> &str {
    fqn.rsplit('\\').next().unwrap_or(fqn)
}

/// A refusal the editor shows as a message, before there is anything to undo.
fn cannot_rename(why: &str) -> Error {
    Error {
        code: lspkit_runtime::protocol::jsonrpc::ErrorCode::InvalidRequest,
        message: why.to_owned().into(),
        data: None,
    }
}
