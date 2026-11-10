//! Describing what is under the cursor, and offering what can be written at it.

use lspkit_runtime::protocol::{jsonrpc::Result, ls_types::*};
use phrost_analysis::{Describe, MemberKind, MemberRef, completions, describe_completion, hover};

use crate::{convert, markdown, server::Backend};

impl Backend {
    pub(crate) async fn answer_hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        self.require_ready().await?;

        let position = params.text_document_position_params;
        // A buffer the index holds nothing about, asked first and disjoint from what follows: a
        // path is either one of those or one with a `FileId`, never both.
        if let Some(path) = self.config_of(&position.text_document.uri).await {
            return Ok(self.hover_in_config(&path, position.position).await);
        }
        let Some((_, file)) = self.file_of(&position.text_document.uri).await else {
            return Ok(None);
        };

        let workspace = self.workspace.read().await;
        let Some((text, index)) = workspace.document_text(file) else {
            return Ok(None);
        };
        let offset = convert::offset_of(&text, &index, position.position);

        Ok(hover(&workspace.view(file), offset).map(|info| Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: markdown::render_hover(&info),
            }),
            range: Some(convert::range_of(&text, &index, info.range)),
        }))
    }

    pub(crate) async fn answer_completion(
        &self,
        params: CompletionParams,
    ) -> Result<Option<CompletionResponse>> {
        self.require_ready().await?;

        let position = params.text_document_position;
        // A configuration buffer, asked first and disjoint from what follows.
        if let Some(path) = self.config_of(&position.text_document.uri).await {
            return Ok(self.complete_in_config(&path, position.position).await);
        }
        let Some((_, file)) = self.file_of(&position.text_document.uri).await else {
            return Ok(None);
        };

        let workspace = self.workspace.read().await;
        let Some((text, index)) = workspace.document_text(file) else {
            return Ok(None);
        };
        let offset = convert::offset_of(&text, &index, position.position);
        let list = completions(&workspace.view(file), offset);
        // Whether this client will read `$0` as a cursor or as two literal characters. Asked of the
        // client rather than assumed, because assuming produces `getFoo($0)` in the buffer.
        let snippets = self
            .supports_snippets
            .load(std::sync::atomic::Ordering::Relaxed);

        // An explicit edit over the text already typed, rather than an insertion the client places
        // itself. A client works the range out from its own idea of a word, and PHP's names do not
        // match it: in `\App\Nested\Rep` it sees the word `Rep`, replaces only that with the
        // qualified name it was offered, and leaves `\App\Nested\App\Nested\Repo` behind.
        let replace = convert::range_of(&text, &index, list.replace);

        // What the client filters by is the word at the cursor -- see `client_word`.
        let typed = text
            .get(usize::from(list.replace.start())..usize::from(offset))
            .unwrap_or_default();
        let head = super::client_word::TypedHead::of(typed);

        Ok(Some(CompletionResponse::List(CompletionList {
            // A truncated list has to say so, or the client filters what it has as the prefix grows
            // and concludes a name does not exist.
            is_incomplete: !list.is_complete,
            items: list
                .items
                .into_iter()
                .enumerate()
                .map(|(rank, item)| {
                    // Worked out before the item is taken apart, because it reads three of its
                    // fields -- and the format has to agree with the text, since a client told
                    // `SNIPPET` reads every `$` in it as syntax.
                    let (new_text, format) = written(&item, snippets);
                    // Before the text is moved into the edit. A snippet is not what gets written
                    // -- its `$` are syntax -- so only plain text can stand in for the label.
                    let filter_text = head.filter_text(
                        &item.label,
                        (format != Some(InsertTextFormat::SNIPPET)).then_some(new_text.as_str()),
                    );
                    CompletionItem {
                        kind: Some(completion_kind(item.kind)),
                        // What makes a client draw the row struck through. The list has to carry
                        // it: a tag that arrived with `completionItem/resolve` arrives after the
                        // person has picked the item it was warning them about.
                        tags: item.deprecated.then(|| vec![CompletionItemTag::DEPRECATED]),
                        text_edit: Some(CompletionTextEdit::Edit(TextEdit {
                            range: replace,
                            new_text,
                        })),
                        detail: item.detail,
                        // Not the declaration -- the address of the member it would describe. The
                        // client asks for the one row it is showing, over `completionItem/resolve`.
                        data: item.describe.map(|member| resolve_data(&member)),
                        insert_text_format: format,
                        // The `use` line, applied with the name and not instead of it: a client applies
                        // all of an item's edits or none, which is the only way the file stays valid.
                        additional_text_edits: item.import.map(|import| {
                            let at = convert::range_of(
                                &text,
                                &index,
                                phrost_model::TextRange::empty(import.at),
                            );
                            vec![TextEdit {
                                range: at,
                                new_text: import.text,
                            }]
                        }),
                        // The order this list was built in, which a client consults only where its
                        // own match scores tie -- and Zed, given none, ties on kind and then on the
                        // label, which undid every order made on purpose here: the members every
                        // operand of a union answers ahead of the rest, the namespaces a docblock
                        // writes ahead of classes. Mostly alphabetical anyway, since that is how the
                        // lists are built. And a demoted row behind everything else: `~` sorts after
                        // every digit, so it is past the rest whatever its place. See
                        // `CompletionItem::demoted`.
                        sort_text: Some(match item.demoted {
                            true => format!("~{}", item.label),
                            false => format!("{rank:05}"),
                        }),
                        filter_text,
                        label: item.label,
                        ..CompletionItem::default()
                    }
                })
                .collect(),
        })))
    }
}

impl Backend {
    /// Fills in the declaration for the one item a client is showing.
    ///
    /// Everything else on the item comes back untouched: the protocol says a resolve returns the
    /// same item with more on it, and a client applies the edits from what it already has.
    ///
    /// Answered against the project and not against the buffer the list came from. Every lookup a
    /// description makes -- the member table, the class header, a function's or a constant's
    /// declaration, and the docblock read from wherever it was written -- is a project-wide one, so
    /// naming the requesting file decided nothing and only ever *withheld* an answer, when the
    /// buffer had been closed between the list and the resolve.
    pub(crate) async fn answer_completion_resolve(
        &self,
        mut item: CompletionItem,
    ) -> Result<CompletionItem> {
        let Some(target) = item.data.as_ref().and_then(resolve_target) else {
            return Ok(item);
        };

        let workspace = self.workspace.read().await;
        // A PHP fence, and the `<?php` that makes a client highlight it -- the same reason hover
        // opens that way, and verified against a real client.
        item.documentation =
            describe_completion(&workspace.project_view(), &target).map(|blocks| {
                Documentation::MarkupContent(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: markdown::render_blocks(&blocks),
                })
            });
        Ok(item)
    }
}

/// What a resolve needs, as the protocol carries it: opaque to the client, ours to read back.
///
/// The member and nothing else. This used to name the requesting file too, on the grounds that
/// which class a name belongs to is a question about a buffer -- and the *list* is built that way,
/// but by the time an address is written down the buffer has already been consulted. Every lookup a
/// resolve then makes is project-wide, so the file was answering nothing.
///
/// It was also the single largest thing on the wire: 200 items each carrying the same absolute path
/// is 25 KB of a 135 KB response, sent on every keystroke. The measurement is what makes this worth
/// stating -- redundant is cheap until it is repeated two hundred times.
fn resolve_data(target: &Describe) -> serde_json::Value {
    match target {
        Describe::Member(member) => serde_json::json!({
            "target": "member",
            "class": member.class,
            "kind": kind_tag(member.kind),
            "name": member.name,
        }),
        Describe::TopLevel { fqn, kind } => serde_json::json!({
            "target": "top-level",
            "class": fqn,
            "kind": symbol_tag(*kind),
        }),
        Describe::DocumentedKey { class, name } => serde_json::json!({
            "target": "documented-key",
            "class": class,
            "name": name,
        }),
    }
}

/// Reads back what [`resolve_data`] wrote. `None` for anything else, including an item some other
/// server produced -- a client is free to send one, and guessing at its shape is how a resolve
/// describes the wrong member.
fn resolve_target(data: &serde_json::Value) -> Option<Describe> {
    let class = data.get("class")?.as_str()?.to_owned();

    // The tag is required rather than inferred from which fields are present. An item some other
    // server produced is free to carry a `class` of its own meaning, and reading one of those as
    // ours is how a resolve describes the wrong thing.
    let target = match data.get("target")?.as_str()? {
        "member" => Describe::Member(MemberRef {
            class,
            kind: kind_of(data.get("kind")?.as_str()?)?,
            name: data.get("name")?.as_str()?.to_owned(),
        }),
        "top-level" => Describe::TopLevel {
            fqn: class,
            kind: symbol_of(data.get("kind")?.as_str()?)?,
        },
        "documented-key" => Describe::DocumentedKey {
            class,
            name: data.get("name")?.as_str()?.to_owned(),
        },
        _ => return None,
    };
    Some(target)
}

/// The wire spelling of a top-level kind, and its reverse. Written out for the same reason the
/// member one is: the protocol must not change shape because an enum gained a variant, and a kind
/// this does not recognise means the item came from somewhere else and is not ours to describe.
fn symbol_tag(kind: phrost_model::SymbolKind) -> &'static str {
    use phrost_model::SymbolKind;

    match kind {
        SymbolKind::Class => "class",
        SymbolKind::Interface => "interface",
        SymbolKind::Trait => "trait",
        SymbolKind::Enum => "enum",
        SymbolKind::Function => "function",
        SymbolKind::Constant => "constant",
        _ => "other",
    }
}

fn symbol_of(tag: &str) -> Option<phrost_model::SymbolKind> {
    use phrost_model::SymbolKind;

    Some(match tag {
        "class" => SymbolKind::Class,
        "interface" => SymbolKind::Interface,
        "trait" => SymbolKind::Trait,
        "enum" => SymbolKind::Enum,
        "function" => SymbolKind::Function,
        "constant" => SymbolKind::Constant,
        _ => return None,
    })
}

/// The wire spelling of a member kind. Written out rather than derived, so the protocol does not
/// change shape when an enum gains a variant.
fn kind_tag(kind: MemberKind) -> &'static str {
    match kind {
        MemberKind::Method => "method",
        MemberKind::Property => "property",
        MemberKind::ClassConstant => "constant",
        MemberKind::EnumCase => "case",
    }
}

fn kind_of(tag: &str) -> Option<MemberKind> {
    match tag {
        "method" => Some(MemberKind::Method),
        "property" => Some(MemberKind::Property),
        "constant" => Some(MemberKind::ClassConstant),
        "case" => Some(MemberKind::EnumCase),
        _ => None,
    }
}

fn completion_kind(kind: phrost_model::SymbolKind) -> CompletionItemKind {
    use phrost_model::SymbolKind;

    match kind {
        SymbolKind::Class => CompletionItemKind::CLASS,
        SymbolKind::Interface => CompletionItemKind::INTERFACE,
        SymbolKind::Trait => CompletionItemKind::INTERFACE,
        SymbolKind::Enum => CompletionItemKind::ENUM,
        SymbolKind::EnumCase => CompletionItemKind::ENUM_MEMBER,
        SymbolKind::Function | SymbolKind::Method => CompletionItemKind::METHOD,
        // The same choice the outline makes, and for the same reason: PHP declares properties,
        // and `FIELD` is the protocol's kind for a struct member.
        SymbolKind::Property => CompletionItemKind::PROPERTY,
        SymbolKind::Constant | SymbolKind::ClassConstant => CompletionItemKind::CONSTANT,
        SymbolKind::Namespace => CompletionItemKind::MODULE,
        SymbolKind::Variable | SymbolKind::Parameter => CompletionItemKind::VARIABLE,
        SymbolKind::Keyword => CompletionItemKind::KEYWORD,
        // A value the container answers to, which is what `VALUE` is for -- and it draws with an
        // icon a reader will not mistake for a class.
        SymbolKind::CatalogKey => CompletionItemKind::VALUE,
        // Never offered: completion offers what a name could be, and a file is not written as one.
        SymbolKind::File => CompletionItemKind::FILE,
    }
}

/// What accepting an item writes, and whether the client has to read it as a snippet.
///
/// A method becomes a call, because a name somebody then has to add brackets to is half an answer.
/// The cursor goes between the brackets when there is an argument to type and after them when there
/// is not -- which is the difference between finishing the call and having to press End.
///
/// The `;` is written when the analysis layer says the call is the whole of a statement that has
/// none. Where it is written the cursor still ends up after it, so the line is finished rather than
/// merely closed.
///
/// A generated declaration carries its own cursor instead, at an offset into text that is already
/// PHP -- so everything around the mark is escaped. `$0` is the only thing in the result a client
/// may read as syntax; `$id` in a parameter list is not.
///
/// Without snippet support the brackets, the `;` and the whole declaration are still written; only
/// the cursor placement is lost, and a literal `$0` in the buffer would be worse than that.
fn written(
    item: &phrost_analysis::CompletionItem,
    snippets: bool,
) -> (String, Option<InsertTextFormat>) {
    use phrost_analysis::CallShape;

    let insertion = item.insert.as_ref();
    let text = insertion.map_or_else(|| item.label.clone(), |insertion| insertion.text.clone());

    if let Some(at) = insertion.and_then(|insertion| insertion.cursor)
        && snippets
    {
        let (head, tail) = text.split_at(at);
        return (
            format!("{}$0{}", snippet_escaped(head), snippet_escaped(tail)),
            Some(InsertTextFormat::SNIPPET),
        );
    }

    let Some(call) = item.call.as_ref() else {
        return (text, None);
    };
    let end = if call.terminates { ";" } else { "" };
    match (call.shape, snippets) {
        // No snippet, so no placeholder either: a client that reads `${1:$body}` as itself would be
        // handed the syntax as text, which is worse than an empty pair of brackets.
        (CallShape::Empty | CallShape::Arguments, false) => (format!("{text}(){end}"), None),
        (CallShape::Empty, true) => (
            format!("{}(){end}$0", snippet_escaped(&text)),
            Some(InsertTextFormat::SNIPPET),
        ),
        // A call with no *required* parameter keeps what it always had: brackets, and the caret
        // between them. Which is right for `page(int $size = 20)` -- there is somewhere to type and
        // nothing that has to be typed.
        (CallShape::Arguments, true) if call.parameters.is_empty() => (
            format!("{}($0){end}", snippet_escaped(&text)),
            Some(InsertTextFormat::SNIPPET),
        ),
        // And one that does gets a stop per parameter, then a last one past the brackets: tabbing
        // through the arguments has to end up somewhere, and it is where the statement continues.
        (CallShape::Arguments, true) => (
            format!(
                "{}({}){end}$0",
                snippet_escaped(&text),
                placeholders(&call.parameters)
            ),
            Some(InsertTextFormat::SNIPPET),
        ),
    }
}

/// The parameters as a snippet's tab stops: `${1:$body}, ${2:$urgent}`.
///
/// Numbered from one, which is where a client starts, and with the `$` a person would have typed --
/// the name is stored without it everywhere else, because that is how PHP declares it and how every
/// other reader wants it. That `$` is escaped, or the client reads the placeholder's own text as a
/// second variable reference and inserts nothing.
///
/// A call with no *required* parameter gets `$0` instead, from the caller: the brackets are written
/// either way and the caret goes between them, which is what an optional argument deserves.
fn placeholders(parameters: &[phrost_model::Name]) -> String {
    parameters
        .iter()
        .enumerate()
        .map(|(at, name)| format!("${{{}:\\${}}}", at + 1, snippet_escaped(name.as_str())))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Text a client must read as itself, inside a string it will read as a snippet.
///
/// The three characters the grammar reserves. `\` first, or the backslashes this adds would be
/// escaped again -- and a PHP stub is full of them, since every class it names is fully qualified.
fn snippet_escaped(text: &str) -> String {
    text.replace('\\', "\\\\")
        .replace('$', "\\$")
        .replace('}', "\\}")
}
