//! Replacing a `@return array` that says nothing with what the body actually hands back.
//!
//! `document::WriteDocblock` writes the shape into a block that does not exist yet, and
//! `doc_tags::AddMissingDocTag` adds a `@return` that was never written -- so a declaration that
//! already carries `@return array` gets neither, and on the reference project that is most of
//! them: **866 of 2 088** declarations returning `array` or `?array` hand back a keyed literal, and
//! the ones somebody documented say the word twice over.
//!
//! # Why it is its own offer and not a case in `doc_tags`
//!
//! That module states, in its first paragraph, that nothing in it may rewrite a line already
//! written -- only the missing one is added. This *is* a rewrite, so it belongs where that rule
//! does not hold rather than as an exception inside it.
//!
//! And it is a refactor rather than a quick fix. Nothing here is wrong: `@return array` is true,
//! and a person wrote it. So there is no diagnostic behind it, nothing about it is preferred, and
//! [`Offer::unattended`] stays false -- saving a file must not rewrite what its author wrote,
//! however much better the replacement is.

use phrost_view::{Docblock, DocumentedDeclaration};

use super::{ActionKind, ActionSubject, CodeAction, Offer, SubjectSites, TextEdit, concerns};
use crate::docblock::Docblock as ParsedDocblock;

pub(super) struct RefineReturn;

impl Offer for RefineReturn {
    fn kind(&self) -> ActionKind {
        ActionKind::Refactor
    }

    fn subject(&self) -> ActionSubject {
        ActionSubject::Declaration
    }

    fn run(&self, sites: &SubjectSites<'_, '_>, out: &mut Vec<CodeAction>) {
        for declaration in sites.declarations() {
            Self::refinement(sites, declaration, out);
        }
    }
}

impl RefineReturn {
    fn refinement(
        sites: &SubjectSites<'_, '_>,
        declaration: &DocumentedDeclaration,
        out: &mut Vec<CodeAction>,
    ) {
        let (view, at) = (sites.view(), sites.at());
        if !concerns(declaration.name_range, at) {
            return;
        }
        let Some(docblock) = &declaration.docblock else {
            return;
        };
        // The same three refusals `AddMissingDocTag` makes, for the same reasons: a docblock a
        // framework prints at somebody is a sentence rather than a set of tags, `{@inheritdoc}`
        // means the tags are on the declaration above, and a framework may settle documentation
        // elsewhere entirely.
        if view.user_facing_docblock(declaration.name.as_str()) {
            return;
        }
        let parsed = ParsedDocblock::parse(&docblock.text);
        if parsed.inherits() || view.documents_elsewhere(&parsed.prose) {
            return;
        }

        let Some(written) = WrittenReturn::in_docblock(docblock) else {
            return;
        };
        if !written.says_only_array() {
            return;
        }
        // What the tag *should* say, which is where the body is read -- `Documentable::returns`
        // already refines a bare `array` into the shape, so this offer is the edit and not the
        // rule.
        let Some(shape) = sites.documentable().and_then(|found| found.returns.clone()) else {
            return;
        };
        if !shape.trim_start_matches('?').starts_with("array{") {
            return;
        }

        // Where on its line the type starts, which is what decides whether it has to wrap.
        let column = written.at - written.line_at;
        let replacement = super::document::written_type(&written.prefix, column, &shape);
        out.push(CodeAction {
            title: "Say what the returned array holds".to_owned(),
            edits: vec![TextEdit::replace(written.range(docblock), replacement)],
            preferred: false,
            kind: ActionKind::Refactor,
            fixes: None,
        });
    }
}

/// The type a docblock's `@return` is written with, and where.
struct WrittenReturn {
    /// Byte offset of the type inside the docblock's own text.
    at: usize,
    /// Byte offset of the start of the line it is on, so the column can be worked out.
    line_at: usize,
    /// The type as written.
    text: String,
    /// Everything before the `*` on that line, which a wrap has to repeat.
    prefix: String,
}

impl WrittenReturn {
    /// The one `@return` a docblock has, when it has exactly one.
    ///
    /// Read off the raw text rather than through [`crate::docblock::Tag`], because what is wanted
    /// is a *range* to replace and that parser is lexical and keeps none.
    ///
    /// Nothing is offered where a `@phpstan-return` or `@psalm-return` stands beside the plain tag.
    /// §8 records why: the prefixed one is the precise claim and the plain one is there for a
    /// reader that cannot hold it, so replacing the plain one would put two precise claims in one
    /// block, and replacing the precise one would overwrite something better than this can say.
    fn in_docblock(docblock: &Docblock) -> Option<Self> {
        let text = &docblock.text;
        let mut found: Option<Self> = None;
        let mut line_at = 0usize;

        for line in text.split_inclusive('\n') {
            let content = line.trim_start();
            let stars = content.len() - content.trim_start_matches('*').len();
            let body = content[stars..].trim_start();
            let name = body.strip_prefix('@').map(|rest| {
                let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
                &rest[..end]
            });
            match name {
                Some("return") => {
                    // A second return tag of any spelling: see the doc comment.
                    if found.is_some() {
                        return None;
                    }
                    let rest = body["@return".len()..].trim_start();
                    if rest.is_empty() {
                        return None;
                    }
                    let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
                    // Every step from `line` to `rest` trims from the left, so `rest` is a
                    // suffix of the line and the difference in length is where it starts.
                    let prefix_len = line.len() - content.len();
                    found = Some(Self {
                        at: line_at + (line.len() - rest.len()),
                        line_at,
                        text: rest[..end].to_owned(),
                        prefix: line[..prefix_len].to_owned(),
                    });
                }
                Some("phpstan-return" | "psalm-return") => return None,
                _ => {}
            }
            line_at += line.len();
        }
        found
    }

    /// Whether the tag says the value is an array and nothing whatever about what is in it.
    ///
    /// The four spellings of that, compared case-insensitively because `NULL` is how a Drupal file
    /// writes the word everywhere else and an author is consistent before they are conventional.
    fn says_only_array(&self) -> bool {
        matches!(
            self.text.to_ascii_lowercase().as_str(),
            "array" | "?array" | "array|null" | "null|array"
        )
    }

    /// Where the type sits in the file.
    fn range(&self, docblock: &Docblock) -> phrost_model::TextRange {
        let start = docblock.range.start() + phrost_model::TextSize::new(self.at as u32);
        phrost_model::TextRange::at(start, phrost_model::TextSize::new(self.text.len() as u32))
    }
}
