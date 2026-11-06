use std::path::PathBuf;

use phrost_model::{FileId, Name, SymbolKind, TextRange, TextSize};
use phrost_view::IndexView;

/// Somewhere an editor can jump to.
///
/// Three ranges, because an editor wants all three: the name to select, the whole declaration to
/// preview, and the span in the *asking* file the jump was asked from. Collapsing the first two
/// means either the preview shows one identifier or the selection covers a whole class body.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct NavigationTarget {
    pub file: FileId,
    pub name: Name,
    pub kind: SymbolKind,
    /// The declaring class, for a member.
    pub container: Option<Name>,
    /// The name alone — what the editor should select.
    pub selection_range: TextRange,
    /// The whole declaration — what the editor should preview.
    pub full_range: TextRange,
    /// The span the jump was asked from, in the file that asked.
    ///
    /// This is what a client underlines for a ctrl-click and follows the click within, and the
    /// protocol's word for it is `originSelectionRange`. Omitting it is allowed and is not free:
    /// the client then works the span out of the buffer itself, by its own idea of where a word
    /// ends — which for a name written `Drupal\\Core\\Url` is a guess about PHP made by something
    /// that is not parsing PHP.
    ///
    /// `None` where no position asked: a call hierarchy's callers, a type hierarchy's subtypes.
    pub origin: Option<TextRange>,
}

impl NavigationTarget {
    /// The same target, remembering the span a reader asked it from.
    pub(crate) fn asked_from(self, origin: Option<TextRange>) -> Self {
        Self { origin, ..self }
    }
}

/// Somewhere to jump that is not in the indexed PHP.
///
/// A route name is declared in a `*.routing.yml`, a permission in a `*.permissions.yml`. Neither is
/// PHP, neither holds a declaration this index can see, and neither should be indexed as one -- so
/// the answer is a path and a position rather than a [`NavigationTarget`], which is about symbols.
///
/// The position is a line and a character because that is what was recorded when the file was read.
/// Nothing here holds its text, so there is no offset to convert.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ExternalTarget {
    pub path: PathBuf,
    /// Zero-based, as the protocol counts them.
    pub line: u32,
    pub character: u32,
    /// The key that led here, for a client that shows what it is jumping to.
    pub key: Box<str>,
    /// The key as written in the asking file, which is the span the jump was asked from.
    pub origin: TextRange,
}

/// Where a key that names something outside the project's PHP is declared.
///
/// Asked before [`goto_definition`] rather than after it, and not as a fallback: the two are
/// disjoint by construction, because a cursor is either on a catalog key that names a place or it is
/// not, and no catalog holds both kinds of key.
pub fn goto_external(view: &IndexView<'_>, offset: TextSize) -> Option<ExternalTarget> {
    let (key, place, origin) = view.place_at(offset)?;
    Some(ExternalTarget {
        path: place.path.to_path_buf(),
        line: place.line,
        character: place.character,
        key,
        origin,
    })
}

/// Where the thing at `offset` is declared.
///
/// A `Vec` because that is the shape the protocol wants, not because PHP has several answers: a
/// name resolves to one declaration, and when two files declare it the index has already ranked
/// them. Several targets would mean goto-implementation, which is a different question.
///
/// Empty means the index has no answer — not that it gave up and should be asked again some other
/// way. There is no text scan behind this, except the one disjoint case at the end: a cursor on
/// `{@inheritdoc}` sits inside a *comment*, which nothing above ever matches, so reading the
/// buffer there is not a second attempt at a miss.
pub fn goto_definition(view: &IndexView<'_>, offset: TextSize) -> Vec<NavigationTarget> {
    // Variables first, as hover does: a local is not a declaration in any catalog, so nothing below
    // would find one. It is bound by a parameter, an assignment, a `foreach`, or a `catch`, and that
    // is where a reader wants to land.
    if let Some(variable) = view.variable_at(offset) {
        // A template's variable is bound by no statement in the template: the key that names it
        // in the call rendering the template is the nearest thing to a declaration it has.
        if variable.declared_at.is_none()
            && let Some((file, range)) = variable.rendered_at
        {
            return vec![NavigationTarget {
                file,
                name: variable.name,
                kind: SymbolKind::Variable,
                container: None,
                selection_range: range,
                full_range: range,
                origin: Some(variable.range),
            }];
        }
        return variable
            .declared_at
            .map(|(range, bound)| NavigationTarget {
                file: variable.file,
                name: variable.name,
                kind: match bound {
                    phrost_view::BindingKind::Parameter => SymbolKind::Parameter,
                    _ => SymbolKind::Variable,
                },
                container: None,
                selection_range: range,
                // A local has no wider region worth previewing than the name it is bound under: the
                // statement around it is code the reader is already looking at.
                full_range: range,
                origin: Some(variable.range),
            })
            .into_iter()
            .collect();
    }

    // A call's label goes to the function or method whose parameter it names. The parameter itself
    // has no stored range -- the index keeps a signature's types, not where it is written -- and
    // the declaration is the nearest thing that does, with the parameter in view.
    if let Some((parameter, label)) = view.named_argument_at(offset)
        && let Some(location) = view.parameter_declaration(&parameter)
    {
        return vec![target_of(location).asked_from(Some(label))];
    }

    // One read of the position, used twice: what the cursor is on says both which declaration to
    // go to and which span the client should underline to get there.
    if let Some(cursor) = view.cursor_target(offset)
        && let Some(location) = view.declaration_of(offset, &cursor)
    {
        return vec![target_of(location).asked_from(Some(cursor.range()))];
    }

    if let Some(target) = goto_documented_parameter(view, offset) {
        return vec![target];
    }

    goto_inheritdoc(view, offset).into_iter().collect()
}

/// The parameter a docblock's `@param $name` is about.
///
/// The same shape as [`goto_inheritdoc`] below and for the same reason: the cursor is inside a
/// *comment*, so no name reference and no declaration range covers it, and nothing above this can
/// answer. What makes it answerable anyway is that a docblock names things the signature under it
/// declares -- `@param string $processorType` is a claim about a parameter eight characters away,
/// and clicking the name is a reader asking to see the one it is about.
///
/// Found by asking which documented declaration's docblock the cursor is inside, rather than by
/// parsing the tag: the variable is written the same way wherever it appears in the block -- in the
/// `@param` line, in the prose under it, in an `@throws` description -- and all of those mean the
/// same parameter. Reading the tag would answer for one of them and leave the rest doing nothing,
/// which is the shape a reader notices as unreliability rather than as a rule.
///
/// Nothing for a name no parameter carries, which is the honest answer and the one the
/// `doc-param-name` check reports on separately: a docblock naming `$rows` where the signature says
/// `$items` has a mistake in it, and jumping somewhere would hide it.
fn goto_documented_parameter(view: &IndexView<'_>, offset: TextSize) -> Option<NavigationTarget> {
    let documented = documented_parameter_at(view, offset)?;
    Some(NavigationTarget {
        file: view.current_file_id()?,
        name: documented.parameter.name.clone(),
        kind: SymbolKind::Parameter,
        container: Some(documented.declaration),
        selection_range: documented.parameter.name_range,
        // A parameter has no wider region worth previewing than its own name, which is what the
        // local branch above this says about a variable for the same reason.
        full_range: documented.parameter.name_range,
        origin: Some(documented.written),
    })
}

/// A parameter a docblock names, and where the cursor found it written.
pub(crate) struct DocumentedParameter {
    pub(crate) declaration: phrost_model::Name,
    pub(crate) parameter: phrost_view::Parameter,
    /// The `$name` in the comment, sigil included.
    pub(crate) written: phrost_model::TextRange,
}

/// The parameter the `$name` under the cursor in a docblock is about -- the one reading goto and
/// hover share, so the two cannot name different parameters or answer at different places.
pub(crate) fn documented_parameter_at(
    view: &IndexView<'_>,
    offset: TextSize,
) -> Option<DocumentedParameter> {
    let (name, written) = variable_written_at(view.current_text()?, offset)?;
    let scope = view.docblock_scope_at(offset)?;
    let parameter = scope
        .parameters
        .into_iter()
        .find(|parameter| parameter.name.as_str() == name)?;
    Some(DocumentedParameter {
        declaration: scope.declaration,
        parameter,
        written,
    })
}

/// The `$name` the cursor is on, read off the buffer.
///
/// Textual, like `inheritdoc_tag_at`: inside a comment there is no node to ask. A PHP variable is a
/// `$` and then a name, so the run is bounded by what a name may hold -- and the cursor may be
/// anywhere in it, including on the `$` itself.
fn variable_written_at(text: &str, offset: TextSize) -> Option<(&str, phrost_model::TextRange)> {
    let at = usize::from(offset).min(text.len());
    let name_byte = |byte: u8| byte.is_ascii_alphanumeric() || byte == b'_';
    let bytes = text.as_bytes();

    let mut start = at;
    while start > 0 && name_byte(bytes[start - 1]) {
        start -= 1;
    }
    // On the `$` itself the loop above stopped at the sigil rather than before it.
    let dollar = match bytes.get(start.wrapping_sub(1)) {
        Some(b'$') => start - 1,
        _ if bytes.get(start) == Some(&b'$') => start,
        _ => return None,
    };
    let mut end = dollar + 1;
    while end < bytes.len() && name_byte(bytes[end]) {
        end += 1;
    }
    if end == dollar + 1 || at > end {
        return None;
    }
    Some((
        text.get(dollar + 1..end)?,
        phrost_model::TextRange::new(
            TextSize::try_from(dollar).ok()?,
            TextSize::try_from(end).ok()?,
        ),
    ))
}

/// Where the `{@inheritdoc}` tag at `offset` points: the ancestor member whose docblock this one
/// is borrowing.
///
/// Nothing above finds anything here because there is nothing to find: the cursor is inside a
/// *comment*, which holds no name reference and no declaration range. So this reads the buffer for
/// the one substring that means anything at this position, then asks the outline which declaration
/// comes right after it -- a docblock always precedes what it documents, with nothing but
/// whitespace between, and that adjacency is the only thing read here. `overridden_member` is the
/// same projection hover already climbs to resolve `{@inheritdoc}` for display; this only has to
/// find which member the tag belongs to.
///
/// `None` for a docblock that is not immediately followed by a member -- including one that
/// documents a class or a function, neither of which anything is overridden from -- rather than
/// guessing at the nearest declaration regardless of the gap.
fn goto_inheritdoc(view: &IndexView<'_>, offset: TextSize) -> Option<NavigationTarget> {
    inheritdoc_location(view, offset).map(|(tag, above)| target_of(above).asked_from(Some(tag)))
}

/// The tag's own range, and the ancestor member whose docblock it borrows.
///
/// Shared with hover, which answers the same question and renders it instead of jumping: a reader
/// whose cursor is on the tag is asking what is being inherited, and whether they want to read it
/// or to go to it is the client's gesture rather than two different searches.
pub(crate) fn inheritdoc_location(
    view: &IndexView<'_>,
    offset: TextSize,
) -> Option<(phrost_model::TextRange, phrost_view::SymbolLocation)> {
    let text = view.current_text()?;
    let (tag, comment_end) = inheritdoc_tag_at(text, offset)?;
    let (owner, member) = documented_member_after(view, text, comment_end)?;
    let kind = member_kind_of(member.kind)?;
    let above = view.overridden_member(owner.as_str(), kind, member.name.as_ref())?;
    Some((tag, above))
}

/// The tag PHPDoc uses to mean "the description above applies here too", matched the same way
/// `crate::docblock` matches it: case-insensitively, in both spellings.
const INHERIT_DOC: &str = "{@inheritdoc}";

/// The `{@inheritdoc}` tag `offset` sits on, and the end of the docblock comment holding it.
///
/// Textual, the same way `spells_a_relative_class` in `phrost-view` reads `self`/`static`/`parent`
/// off the buffer: the cursor is not on anything a name reference or a declaration range covers,
/// so nothing else here can answer without looking at the same characters a syntax highlighter
/// already did.
fn inheritdoc_tag_at(text: &str, offset: TextSize) -> Option<(phrost_model::TextRange, usize)> {
    let at = usize::from(offset).min(text.len());
    let start = text[..at].rfind("/**")?;
    let end = start + text[start..].find("*/")?.checked_add(2)?;
    if at > end {
        // The nearest `/**` before the cursor already closed before reaching it -- an earlier,
        // unrelated docblock, not the one the cursor is inside of.
        return None;
    }

    let body_lower = text[start..end].to_ascii_lowercase();
    let mut searched = 0;
    while let Some(found) = body_lower[searched..].find(INHERIT_DOC) {
        let tag_start = start + searched + found;
        let tag_end = tag_start + INHERIT_DOC.len();
        if (tag_start..=tag_end).contains(&at) {
            return Some((
                phrost_model::TextRange::new(
                    TextSize::try_from(tag_start).ok()?,
                    TextSize::try_from(tag_end).ok()?,
                ),
                end,
            ));
        }
        searched += found + INHERIT_DOC.len();
    }
    None
}

/// The member, and its owning class's fully-qualified name, whose docblock ends at `comment_end`.
///
/// Found by proximity: the outline lists every declaration in the file in source order with where
/// each begins, and a docblock is not a symbol with a link of its own to what it documents. The
/// nearest declaration at or after the comment is read as the one it documents, but only when
/// nothing but whitespace lies between -- a docblock separated from a declaration by anything else
/// does not describe it, and this says so rather than guessing.
fn documented_member_after(
    view: &IndexView<'_>,
    text: &str,
    comment_end: usize,
) -> Option<(Name, phrost_view::OutlineNode)> {
    let outline = view.outline();
    let mut candidates: Vec<(
        TextSize,
        Option<(&phrost_view::OutlineNode, &phrost_view::OutlineNode)>,
    )> = Vec::new();
    for node in &outline {
        // The declaration itself is a candidate too, so a docblock immediately above a class or a
        // function -- neither of which anything is overridden from -- is recognised as documenting
        // *something*, and answered with nothing rather than skipped past to the member below it.
        candidates.push((node.full_range.start(), None));
        for member in &node.children {
            candidates.push((member.full_range.start(), Some((node, member))));
        }
    }
    candidates.sort_by_key(|(start, _)| *start);

    let (start, found) = candidates
        .into_iter()
        .find(|(start, _)| usize::from(*start) >= comment_end)?;

    let gap = text.get(comment_end..usize::from(start))?;
    if !gap.chars().all(char::is_whitespace) {
        return None;
    }

    let (class, member) = found?;
    let owner = view.declaration_at(class.name_range.start())?.fqn;
    Some((owner, member.clone()))
}

/// The class of the *type* of the thing at `offset`, rather than where that thing is declared.
///
/// The difference is the whole feature: on `$node` in `$node = $storage->load($id)`, goto definition
/// lands on the assignment three characters away, and this lands on `Node`. On a member access it is
/// the class the member holds -- what reading it gives you -- and on a name written where a class
/// goes it is that class, because there the two questions have the same answer.
///
/// A type that names no class of its own answers with its *element*: `@var Rule[]` has no `Rule[]`
/// to jump to, and `Rule` is the only declaration a reader asking where this type is written could
/// want. Only where the type is not a class itself, so a class that iterates is still its own
/// answer.
///
/// Empty where the index cannot name a class at all: a variable with no declared type, a member
/// declared bare `array`, a name that resolves to nothing. Nothing here falls back to goto
/// definition, because a jump to somewhere other than a type is not a worse answer to this question
/// -- it is an answer to a different one, and the editor already has a key bound to it.
pub fn goto_type_definition(view: &IndexView<'_>, offset: TextSize) -> Vec<NavigationTarget> {
    // A variable first, as goto definition does: a local is in no catalog, so nothing below finds
    // one.
    // A union of classes is a type declared in several places, and each is where it is written.
    if let Some(variable) = view.variable_at(offset)
        && !variable.classes.is_empty()
    {
        return variable
            .classes
            .iter()
            .filter_map(|class| view.class_location(class.as_str()))
            .map(|location| target_of(location).asked_from(Some(variable.range)))
            .collect();
    }
    let (class, origin) = match view.variable_at(offset) {
        // What that *site* holds, which is the question the card beside this one already answered.
        // Searching backwards for the binding instead is a second reading of one rule, and it
        // disagreed at the one position a person is most likely to ask from: an assignment target
        // is typed by its own right-hand side, and a search that only looks above it finds nothing.
        //
        // The element where the type names no class of its own: there is no `Rule[]` to jump to and
        // `Rule` is the only declaration a reader asking this question could want.
        Some(variable) => (
            variable
                .class
                .clone()
                .or_else(|| variable.element_class.clone()),
            Some(variable.range),
        ),
        None => {
            let cursor = view.cursor_target(offset);
            let origin = cursor.as_ref().map(phrost_view::CursorTarget::range);
            let class = match &cursor {
                // What reading the member gives you. A property's declared type and a method's
                // return type are the same question asked of the same stored field.
                Some(phrost_view::CursorTarget::MemberAccess {
                    owner, name, kind, ..
                })
                | Some(phrost_view::CursorTarget::MemberDeclaration {
                    owner, name, kind, ..
                }) => view
                    .member_class(owner.as_str(), *kind, name.as_str())
                    .or_else(|| view.member_element_class(owner.as_str(), *kind, name.as_str())),
                // A class named where a class goes, a `self`, or a catalog key that resolved to
                // one. All three already name a type, so the answer is that type.
                Some(phrost_view::CursorTarget::RelativeClass { class, .. })
                | Some(phrost_view::CursorTarget::CatalogKey { class, .. }) => Some(class.clone()),
                _ => cursor
                    .as_ref()
                    .and_then(|cursor| view.declaration_of(offset, cursor))
                    .and_then(|location| location.kind.is_class_like().then_some(location.fqn)),
            };
            (class, origin)
        }
    };

    class
        .and_then(|class| view.class_location(class.as_str()))
        .map(|location| target_of(location).asked_from(origin))
        .into_iter()
        .collect()
}

/// Every class in the project that inherits from the class-like name at `offset`.
///
/// `None` where the question does not apply: an implementation is something a *class-like* name has,
/// and a local, a member or a catalog key has none. Answering about the class a member happens to be
/// declared on would be answering a question nobody asked.
///
/// Most valuable exactly where a codebase is written against interfaces, which is every framework
/// one: `EntityStorageInterface` has no useful definition to read and a very useful list of
/// implementers.
pub fn goto_implementation(view: &IndexView<'_>, offset: TextSize) -> Vec<NavigationTarget> {
    let Some(cursor) = view.cursor_target(offset) else {
        return Vec::new();
    };
    let Some(location) = view.declaration_of(offset, &cursor) else {
        return Vec::new();
    };
    // Every implementer is reached from the one span the reader is on, so they all carry it.
    let origin = Some(cursor.range());

    if location.kind.is_class_like() {
        return view
            .implementers_of(location.fqn.as_str(), IMPLEMENTERS)
            .into_iter()
            .filter(|found| !crate::render::is_stub_internal(found.location.fqn.as_str()))
            .map(|found| target_of(found.location).asked_from(origin))
            .collect();
    }

    // A member, which is the same question one level down: who *overrides* this. Asked from either
    // side -- the declaration in the interface, or a call written against it -- because
    // `declaration_at` resolves both to the same place.
    let Some(owner) = location.container.clone() else {
        return Vec::new();
    };
    let Some(kind) = member_kind_of(location.kind) else {
        return Vec::new();
    };

    view.implementers_of(owner.as_str(), IMPLEMENTERS)
        .into_iter()
        .filter(|found| {
            // Declared there and not inherited. Without this every subclass answers with the
            // interface's own declaration, which is where an inherited member really is declared.
            view.declares_member(found.location.fqn.as_str(), kind, location.fqn.as_str())
        })
        .filter_map(|found| {
            view.member_location(found.location.fqn.as_str(), kind, location.fqn.as_str())
        })
        .map(|location| target_of(location).asked_from(origin))
        .collect()
}

/// The member kind a symbol kind stands for, or nothing when it stands for no member.
fn member_kind_of(kind: SymbolKind) -> Option<phrost_view::MemberKind> {
    Some(match kind {
        SymbolKind::Method => phrost_view::MemberKind::Method,
        SymbolKind::Property => phrost_view::MemberKind::Property,
        SymbolKind::ClassConstant => phrost_view::MemberKind::ClassConstant,
        SymbolKind::EnumCase => phrost_view::MemberKind::EnumCase,
        _ => return None,
    })
}

pub(crate) fn target_of(location: phrost_view::SymbolLocation) -> NavigationTarget {
    NavigationTarget {
        file: location.file,
        name: location.fqn,
        kind: location.kind,
        container: location.container,
        selection_range: location.name_range,
        full_range: location.full_range,
        // Filled by whoever was asked from a position; a caller that has none says so by leaving
        // it, rather than by inventing a span in a file the reader is not looking at.
        origin: None,
    }
}

/// How many implementers from outside the project to offer.
///
/// A framework's own base interfaces have hundreds. What matters is that the project's own classes
/// are all in the list, which [`implementers_of`](phrost_view::IndexView::implementers_of)
/// guarantees separately; this bounds only what a dependency adds on top of them.
const IMPLEMENTERS: usize = 100;

/// The class-like declaration at `offset`, as the head of a hierarchy.
///
/// `None` for anything that is not one: a hierarchy is something a class has, and a local or a member
/// has none.
pub fn hierarchy_head(view: &IndexView<'_>, offset: TextSize) -> Option<NavigationTarget> {
    let location = view.declaration_at(offset)?;
    location.kind.is_class_like().then(|| target_of(location))
}

/// What `fqn` extends, implements, and uses.
///
/// All three together, because they are one question for a reader: what does this class get its shape
/// from. PHP distinguishes them and the hierarchy a person walks does not.
pub fn supertypes_of(view: &IndexView<'_>, fqn: &str) -> Vec<NavigationTarget> {
    let Some(header) = view.class_header(fqn) else {
        return Vec::new();
    };
    header
        .extends
        .iter()
        .chain(&header.implements)
        .chain(&header.uses)
        // A stub's internal marker interface -- see [`crate::render::is_stub_internal`] -- is a
        // real supertype the index owes an honest answer about, but not one worth a jump: nothing
        // written there is something a person meant to read.
        .filter(|named| !crate::render::is_stub_internal(named.as_str()))
        .filter_map(|named| view.class_location(named.as_str()))
        .map(target_of)
        .collect()
}

/// What directly inherits from `fqn`, which is [`goto_implementation`] asked about a name instead of
/// a position.
pub fn subtypes_of(view: &IndexView<'_>, fqn: &str) -> Vec<NavigationTarget> {
    view.implementers_of(fqn, IMPLEMENTERS)
        .into_iter()
        .filter(|found| !crate::render::is_stub_internal(found.location.fqn.as_str()))
        .map(|found| target_of(found.location))
        .collect()
}
