use phrost_model::{SymbolKind, TextRange, TextSize, namespace_of, short_name};
use phrost_view::{ClassHeader, IndexView, SymbolLocation};

use crate::{
    docblock::{Docblock, Tag},
    type_width::{self, WIDEST_LINE},
};

/// What to show when the pointer rests on something.
///
/// Blocks rather than a finished string, and specifically **not** markdown. The protocol layer
/// renders these; that keeps two things out of here that would otherwise cause churn:
///
/// - Formatting. Changing a fence language, adding a rule between sections, or reordering blocks is
///   then one change in one place, instead of a rewrite of every hover snapshot in this crate.
/// - Escaping, which is not uniform. A docblock's prose is markup the author wrote and has to pass
///   through untouched; a line this crate generated has to be escaped, or a class name with an
///   underscore renders as italics. Conflating the two into one "text" block is the mistake that
///   forces a rewrite later, so the distinction is in the type.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct HoverInfo {
    /// The source range the hover describes, so the client can highlight it.
    pub range: TextRange,
    pub blocks: Vec<HoverBlock>,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum HoverBlock {
    /// PHP source, to be rendered as a highlighted code block.
    Code(String),
    /// Prose from a docblock, in whatever markup its author used. Passed through, not escaped.
    Documentation(String),
    /// A short fact this crate generated, such as where a member came from. Escaped when rendered.
    Note(String),
    /// One docblock tag as a labelled line.
    ///
    /// Its own kind because the halves are different: the label and the type are this crate's to
    /// format and escape, and the description is the author's markup and is passed through. Dumping
    /// the whole tag line into a `php` fence, which is what this replaced, made a sentence of prose
    /// read as code and put the type nowhere in particular.
    Tag {
        /// `Returns`, `Throws`, or the tag as written for anything with no name in English.
        label: String,
        /// The type the tag names.
        ty: Option<String>,
        /// `$name`, for a tag about one parameter or property.
        subject: Option<String>,
        description: String,
    },
    /// A visual break between groups of blocks.
    Separator,
}

/// Describes the thing at `offset`.
///
/// Built from the same projection goto uses, so the two cannot disagree about what the cursor is
/// on — which they will, eventually, if each finds its own way there.
///
/// The declaration comes first and the documentation after it. Documentation runs to ten lines on a
/// well-commented project and would push the thing being described out of a small popup; the
/// declaration is short and always present.
pub fn hover(view: &IndexView<'_>, offset: TextSize) -> Option<HoverInfo> {
    // Variables first, because they are not declarations and nothing below would find one. A local
    // has no entry in any catalog: it exists because a parameter or an assignment put it there.
    if let Some(variable) = view.variable_at(offset) {
        return Some(variable_hover(&variable));
    }

    // A call's label: the parameter it names, as declared, and whose it is.
    if let Some((parameter, label)) = view.named_argument_at(offset) {
        let mut blocks = Vec::new();
        if let Some(written) = view.parameter_signature(&parameter) {
            blocks.push(HoverBlock::Code(written));
        }
        let owner = match &parameter.container {
            Some(class) => format!("{}::{}()", class, parameter.callable),
            None => format!("{}()", parameter.callable),
        };
        blocks.push(HoverBlock::Note(format!("parameter of {owner}")));
        return Some(HoverInfo {
            range: label,
            blocks,
        });
    }

    // A cursor inside a *comment* matches nothing here: a docblock holds no name reference and no
    // declaration range. `{@inheritdoc}` is the one thing written there that names a declaration,
    // and it is read off the text -- see [`crate::navigation::inheritdoc_location`].
    let Some(target) = view.cursor_target(offset) else {
        return inheritdoc_hover(view, offset).or_else(|| documented_parameter_hover(view, offset));
    };
    // A name written in a comment that no declaration answers for. Its own branch rather than a
    // rendering of the location below, because there is nothing to render as a declaration: what
    // exists is a call, and what a reader of an implementation needs is where it is and what it
    // passes -- see [`dispatched_blocks`].
    if let phrost_view::CursorTarget::DispatchedName { name, range } = &target {
        let mut blocks = vec![HoverBlock::Code(format!("{name}()"))];
        blocks.extend(dispatched_blocks(view, name));
        return Some(HoverInfo {
            range: *range,
            blocks,
        });
    }

    // A key whose value is a *place* rather than a class: a route name, a permission id, a field, a
    // render element. Its own branch for `DispatchedName`'s reason -- there is no PHP declaration to
    // render, and what a reader wants is what the key resolves to and where it was declared.
    //
    // Worth having beyond the description: a route literal hovering *nothing* is what made a silent
    // near-miss report unexplainable from the outside. A key the catalog does not hold now reads as
    // a key the catalog does not hold.
    if let phrost_view::CursorTarget::CatalogPlace {
        key,
        detail,
        place,
        range,
    } = &target
    {
        let mut blocks = vec![HoverBlock::Code(format!("'{key}'"))];
        if !detail.is_empty() {
            blocks.push(HoverBlock::Note(detail.to_string()));
        }
        // Where it is declared, for a key some file declares. A generated one says nothing here
        // rather than pointing at the declaration it was derived from, which is not the route.
        if let Some(place) = place {
            blocks.push(HoverBlock::Note(format!(
                "declared at {}:{}",
                file_name(&place.path),
                place.line + 1
            )));
        }
        return Some(HoverInfo {
            range: *range,
            blocks,
        });
    }

    let location = view.declaration_at(offset)?;
    let mut blocks = declaration_blocks(view, &location);

    // What the call or the read gives back *here*, under the declaration that says what it gives
    // back everywhere: `get(): T` is true of every call and a `User` is what this one is. Only where
    // the two differ, so a card whose declaration already says it does not say it twice.
    let reached = matches!(
        target,
        phrost_view::CursorTarget::MemberAccess { .. }
            | phrost_view::CursorTarget::WrittenName {
                kind: phrost_view::ImportKind::Function,
                ..
            }
    );
    let card = match blocks.first() {
        Some(HoverBlock::Code(code)) => code.clone(),
        _ => String::new(),
    };
    if reached
        && let Some(here) = view.value_named_at(offset)
        && !declared_alike(location.kind, &card, &here)
    {
        let verb = match location.kind {
            SymbolKind::Property => "holds",
            _ => "returns",
        };
        blocks.insert(1, HoverBlock::Note(format!("{verb} {here} here")));
    }

    Some(HoverInfo {
        range: target.range(),
        blocks,
    })
}

/// Whether the card's declaration already says `here`, spelling aside: `?User` and `User|null` are
/// one type, and a leading separator or a namespace in front of a class is not a difference.
///
/// Read off the card as rendered rather than off the stored signature, because the card is what the
/// reader compares it with -- and a declaration writing no type of its own is rendered with the one
/// it inherits, which the stored signature does not hold.
/// A word PHP writes in front of a property's type.
fn is_a_modifier(word: &str) -> bool {
    // `private(set)` is a modifier too, written with what it restricts.
    let word = word.split('(').next().unwrap_or(word);
    [
        "public",
        "protected",
        "private",
        "static",
        "readonly",
        "var",
        "final",
        "abstract",
    ]
    .iter()
    .any(|modifier| word.eq_ignore_ascii_case(modifier))
}

fn declared_alike(kind: SymbolKind, card: &str, here: &str) -> bool {
    let declared = match kind {
        // The type is what is left once the modifiers in front of it are: `public readonly
        // array<int, string> $x` has spaces inside the type as well as before it.
        SymbolKind::Property => card.rsplit_once(" $").map(|(written, _)| {
            let mut rest = written.rsplit('\n').next().unwrap_or(written).trim();
            while let Some((word, after)) = rest.split_once(' ')
                && is_a_modifier(word)
            {
                rest = after.trim_start();
            }
            rest
        }),
        // Over the whole card, because a long signature is wrapped and its return is then on a
        // line of its own.
        _ => card
            .rsplit_once("): ")
            .map(|(_, ty)| ty.split('\n').next().unwrap_or(ty).trim()),
    };
    let Some(declared) = declared else {
        return false;
    };
    let operands = |ty: &str| {
        let mut set = ty
            .split('|')
            .flat_map(|operand| match operand.trim().strip_prefix('?') {
                Some(rest) => vec![rest.to_owned(), "null".to_owned()],
                None => vec![operand.trim().to_owned()],
            })
            .map(|operand| {
                phrost_model::short_name(operand.trim_start_matches('\\')).to_ascii_lowercase()
            })
            .collect::<Vec<_>>();
        set.sort();
        set.dedup();
        set
    };
    operands(declared) == operands(here)
}

/// What a declaration's card holds: the declaration, where it came from, and its documentation.
///
/// Its own function because three positions ask for it -- the declaration itself, a
/// `{@inheritdoc}` tag borrowing another one's, and the card a completion item resolves to -- and a
/// card that differed between them would be several answers to one question. It did: the resolve
/// card dumped its tag lines into a fence where this labels them, and never said where an inherited
/// member came from.
pub(crate) fn declaration_blocks(
    view: &IndexView<'_>,
    location: &SymbolLocation,
) -> Vec<HoverBlock> {
    let mut blocks = vec![HoverBlock::Code(type_width::over_lines(
        &render_declaration(view, location),
        WIDEST_LINE,
    ))];

    // Where a member came from is the fact a reader most often wants and least often has: a method
    // that looks like it belongs to this class may have arrived from three classes up or from a
    // trait, and that changes where they should go to change it.
    if let Some(origin) = location.origin.as_ref() {
        blocks.push(HoverBlock::Note(origin.to_string()));
    }

    // Said plainly, because the alternative is a reader following goto into a comment and wondering
    // what went wrong.
    if location.from_docblock {
        blocks.push(HoverBlock::Note("declared by a docblock tag".to_owned()));
    }

    let parsed = documentation(view, location).filter(|parsed| !parsed.is_empty());

    // Only where the docblock does not already say it. `@deprecated` is rendered as a labelled tag
    // below with whatever the author wrote beside it, and PHP 8.4's `#[\Deprecated]` is rendered
    // nowhere -- so without this the attribute spelling of the fact was invisible on the card while
    // the docblock spelling was not. One fact, two spellings, and only one of them was reaching a
    // reader.
    if location.deprecated
        && !parsed
            .as_ref()
            .is_some_and(|parsed| parsed.tags.iter().any(|tag| names_deprecated(tag)))
    {
        blocks.push(HoverBlock::Note("deprecated".to_owned()));
    }

    if let Some(parsed) = parsed.filter(|parsed| !parsed.is_empty()) {
        blocks.push(HoverBlock::Separator);
        // Read before the prose is moved into a block, because the sentence naming the other
        // declaration *is* the prose.
        let elsewhere = elsewhere_blocks(view, location, &parsed.prose);

        if !parsed.prose.is_empty() {
            blocks.push(HoverBlock::Documentation(parsed.prose));
        }
        // Tags go in a code block rather than into the prose, and not for looks: `@param
        // array<int, Foo> $x` is full of angle brackets, which markdown reads as raw HTML and
        // swallows. Inside a fence they survive, and a monospaced tag list reads better anyway.
        let tags = tag_blocks(&parsed.tags);
        if !tags.is_empty() {
            blocks.extend(tags);
        }
        blocks.extend(elsewhere);
    }

    blocks
}

/// Whether one docblock tag line is the `@deprecated` one.
///
/// The text, because that is the shape a parsed docblock hands tags back in -- see
/// [`crate::docblock::Tag`], which is what renders them.
fn names_deprecated(line: &str) -> bool {
    crate::docblock::Tag::parse(line).is_some_and(|tag| tag.name.eq_ignore_ascii_case("deprecated"))
}

/// The card for a `$name` a docblock writes: the parameter it is about, as hovering the parameter
/// itself shows it.
///
/// Goto already went there, and a name that goes somewhere and describes nothing reads as broken.
fn documented_parameter_hover(view: &IndexView<'_>, offset: TextSize) -> Option<HoverInfo> {
    let documented = crate::navigation::documented_parameter_at(view, offset)?;
    let variable = view.variable_at(documented.parameter.name_range.start())?;
    Some(HoverInfo {
        range: documented.written,
        ..variable_hover(&variable)
    })
}

/// The card for a `{@inheritdoc}` tag: the declaration whose documentation it is borrowing.
///
/// Goto already goes there, and this is the same search rendered instead of followed -- which is
/// the difference a reader wants, because the whole point of the tag is that the words are
/// somewhere else and reading them should not cost leaving the file. Said out loud first, since
/// the card describes a declaration that is not the one under the cursor.
fn inheritdoc_hover(view: &IndexView<'_>, offset: TextSize) -> Option<HoverInfo> {
    let (range, above) = crate::navigation::inheritdoc_location(view, offset)?;
    // Named as PHP writes it, because the member alone is the word already under the cursor: what
    // a reader is asking is *whose* documentation this is.
    let from = match above.container.as_ref() {
        Some(container) => format!("{}::{}", short_name(container.as_str()), above.fqn),
        None => above.fqn.to_string(),
    };
    let mut blocks = vec![HoverBlock::Note(format!("inherited from {from}"))];
    blocks.extend(declaration_blocks(view, &above));
    Some(HoverInfo { range, blocks })
}

/// How far `{@inheritdoc}` is followed up a hierarchy.
///
/// Deep enough for any real one -- Drupal's longest interface chains are four or five -- and finite,
/// because a cyclic hierarchy is text an editor sees while it is being written.
const INHERIT_LIMIT: u8 = 8;

/// What a variable is: its type where one is known, and what bound it.
///
/// Says so when nothing declares a type, rather than showing `mixed`. That absence is the reason
/// completion offers nothing on the variable, and a reader who is told can fix it by writing one.
fn variable_hover(variable: &phrost_view::VariableInfo) -> HoverInfo {
    // As PHP writes it, type first: the card is rendered inside a `php` fence, and a client
    // highlights what it can parse. `$repo: Repo` reads fine and highlights as nothing.
    //
    // Over lines where it has to be: a shape names every key the value holds, and one out of real
    // code is 250 characters that a popup shows as a paragraph of punctuation.
    let mut blocks = vec![HoverBlock::Code(match &variable.ty {
        Some(ty) => type_width::over_lines(&format!("{ty} ${}", variable.name), WIDEST_LINE),
        None => format!("${}", variable.name),
    })];

    if let Some(binding) = variable.binding {
        blocks.push(HoverBlock::Note(binding.as_str().to_owned()));
    }
    if variable.ty.is_none() {
        blocks.push(HoverBlock::Note("no declared type".to_owned()));
    }

    HoverInfo {
        blocks,
        range: variable.range,
    }
}

/// The tags of a docblock as labelled lines, in the order a reader wants them.
///
/// Deprecation first, because it is a warning and everything below it is about how to use something
/// that should not be used. Then the parameters and what comes back, which is what the reader came
/// for; then what it throws; then everything else in the order it was written.
fn tag_blocks(tags: &[String]) -> Vec<HoverBlock> {
    let parsed = tags
        .iter()
        .filter_map(|line| Tag::parse(line))
        .collect::<Vec<_>>();

    let mut ordered = parsed;
    // The same order the quick fix writes a new tag in -- see `docblock::reading_rank`. Ties keep
    // source order, which is what `sort_by_key` guarantees.
    ordered.sort_by_key(|tag| crate::docblock::reading_rank(&tag.name));
    ordered
        .into_iter()
        .map(|tag| match tag.name.eq_ignore_ascii_case("code") {
            // A usage example is source, not prose about source. As a tag it rendered as
            // `**code** — $form['author'] = [ ... ];`, one line of PHP dressed as a sentence; the
            // line breaks are kept for it in `docblock.rs` and this is what makes them visible.
            //
            // The label goes with it: a fenced block needs no heading saying it is code, and
            // `@endcode` is dropped where the lines are read.
            true => HoverBlock::Code(tag.description),
            false => HoverBlock::Tag {
                label: tag.label(),
                ty: tag.ty,
                subject: tag.subject,
                description: tag.description,
            },
        })
        .collect()
}

/// The documentation written on the declaration this one's docblock names, if it names one.
///
/// The sideways move `{@inheritdoc}` makes upwards. A framework can have a convention that says the
/// documentation is on another declaration entirely -- Drupal's hook implementations carry
/// `Implements hook_form_alter().` and nothing else, because the twenty lines saying what the
/// parameters mean are written once, on the hook -- and the index already holds the declaration the
/// sentence names. Without this a reader gets the sentence and has to go and find the rest by hand.
///
/// Attributed out loud, because the words are somebody else's: a card that showed them unlabelled
/// would read as documentation of the function under the cursor, and the difference matters the
/// moment a reader decides to edit it.
///
/// One hop and no loop. The claim points at a *declaration*, and a declaration that points onwards
/// is not a shape any convention describes -- where PHP has that shape it spells it `{@inheritdoc}`,
/// which [`documentation`] has already resolved by the time this is asked.
fn elsewhere_blocks(
    view: &IndexView<'_>,
    own: &SymbolLocation,
    prose: &str,
) -> Vec<crate::HoverBlock> {
    let Some(named) = view.documented_by(prose) else {
        return Vec::new();
    };
    // The declaration and not the sentence, because a convention may let the two be spelled
    // differently: `Implements hook_node_delete()` is documented on `hook_ENTITY_TYPE_delete`, and
    // naming the sentence in the note below would credit the words to a declaration that has none.
    let Some(declared) = view.documented_declaration(named) else {
        let found = dispatched_blocks(view, named);
        if found.is_empty() {
            return found;
        }
        return std::iter::once(HoverBlock::Separator)
            .chain(found)
            .collect();
    };
    let Some(location) = view.function_location(declared.as_str()) else {
        return Vec::new();
    };
    // A declaration whose docblock names itself has nothing to add, and saying it twice would read
    // as two sources agreeing.
    if location.file == own.file && location.name_range == own.name_range {
        return Vec::new();
    }
    let Some(parsed) = documentation(view, &location).filter(|parsed| !parsed.is_empty()) else {
        return Vec::new();
    };

    let mut blocks = vec![
        HoverBlock::Separator,
        HoverBlock::Note(format!("documentation below is from {declared}()")),
    ];
    if !parsed.prose.is_empty() {
        blocks.push(HoverBlock::Documentation(parsed.prose));
    }
    blocks.extend(tag_blocks(&parsed.tags));
    blocks
}

/// Where an extension point the project dispatches is dispatched from.
///
/// What is left to say when a claim names something nothing declares. There is no docblock to
/// borrow, no `@param` lines and no signature -- a dispatched name has none of those and inventing
/// them would be describing a declaration that does not exist. What there is is where the call is
/// and what it passes, which is the whole of what a reader of the implementation needs: the
/// arguments the hook receives are the arguments written at the call.
///
/// Every call, not the first: two modules dispatching one point is how a project grows a second
/// place the arguments come from, and hiding one would say the first is the contract.
fn dispatched_blocks(view: &IndexView<'_>, named: &str) -> Vec<crate::HoverBlock> {
    let sites = view.dispatched_declaration(named);
    if sites.is_empty() {
        return Vec::new();
    }

    let mut blocks = vec![HoverBlock::Note(format!(
        "{named}() is not declared anywhere -- it is dispatched from:"
    ))];
    for site in sites {
        let where_from = view
            .file_path(site.file)
            .map(str::to_owned)
            .unwrap_or_default();
        blocks.push(HoverBlock::Note(where_from));
        if let Some(source) = view.source_between(site.file, site.range) {
            blocks.push(HoverBlock::Code(source));
        }
    }
    blocks
}

/// The documentation for a declaration, with `{@inheritdoc}` followed.
///
/// Most implementations of a Drupal interface document themselves with nothing but that tag, so
/// without this a hover on the method people actually call says `{@inheritdoc}` and nothing else.
fn documentation(view: &IndexView<'_>, location: &SymbolLocation) -> Option<Docblock> {
    let mut parsed = view
        .documentation(location)
        .map(|raw| Docblock::parse(&raw))
        .unwrap_or_default();

    // Only a member can inherit: a class's docblock has nothing above it to inherit from, and a
    // free function has no hierarchy at all.
    let (mut container, kind) = match (&location.container, location.member_kind()) {
        (Some(container), Some(kind)) => (container.clone(), kind),
        _ => return Some(parsed),
    };

    for _ in 0..INHERIT_LIMIT {
        if !parsed.inherits() {
            break;
        }
        let Some(above) = view.overridden_member(container.as_str(), kind, location.fqn.as_str())
        else {
            break;
        };
        let inherited = view
            .documentation(&above)
            .map(|raw| Docblock::parse(&raw))
            .unwrap_or_default();

        // The ancestor's own `{@inheritdoc}` is resolved by the next turn of this loop, from the
        // ancestor's position in the hierarchy rather than from where the hover started.
        parsed.resolve_inherited(&inherited);
        match above.container {
            Some(next) => container = next,
            None => break,
        }
    }

    Some(parsed)
}

/// Renders a declaration as the PHP someone would have written.
///
/// It opens with `<?php`, which is not decoration: the PHP grammar most clients highlight with
/// starts in HTML and only switches to PHP at the open tag, so a fenced block without it is shown
/// unhighlighted. Verified against a real client.
///
/// The namespace goes on its own line rather than into the name. A fully-qualified name inline makes
/// one very long line that wraps badly in a popup, and the namespace is the part a reader scans for
/// rather than reads.
fn render_declaration(view: &IndexView<'_>, location: &SymbolLocation) -> String {
    let mut out = String::from("<?php\n");

    // For a member, the namespace and class are the container's; for a top-level symbol, its own.
    let subject = location
        .container
        .as_ref()
        .map_or(location.fqn.as_str(), |container| container.as_str());
    if let Some(namespace) = namespace_of(subject).filter(|namespace| !namespace.is_empty()) {
        out.push_str(&format!("namespace {namespace};\n\n"));
    }

    match (&location.container, view.class_header(subject)) {
        // A member: show the class it lives in, then the member itself, as PHP nests them.
        (Some(_), header) => {
            let kind = header
                .as_ref()
                .map_or("class", |header| header.kind.as_str());
            out.push_str(&format!("{kind} {}\n{{\n    ", short_name(subject)));
            out.push_str(&crate::render::member_declaration(
                location.member_modifiers,
                location.kind,
                &initialised(view, location),
            ));
            out.push_str("\n}");
        }
        // A class-like declaration.
        (None, Some(header)) => out.push_str(&render_class(&header)),
        // Something with no header of its own: a function or a constant. A function has a
        // signature to show; a constant's name is all there is.
        (None, None) => out.push_str(&format!(
            "{} {}",
            location.kind.as_str(),
            location
                .signature
                .as_deref()
                .unwrap_or_else(|| short_name(location.fqn.as_str()))
        )),
    }

    out
}

/// A constant or a case as written, with the value it was given.
///
/// The stored signature holds a name and a declared type and nothing else -- it is composed from
/// index rows, and PHP's grammar is not something the index keeps -- so `case SHIPPED` and
/// `const int LIMIT` arrived with the half a reader actually wants missing. Every other PHP tool
/// shows `case SHIPPED = 'shipped'`, and they are right: the value *is* the meaning of a constant.
///
/// No new storage for it. The declaration's range already survives compaction and the disk cache,
/// like every range does, and the characters behind it are read once for the one declaration
/// somebody is looking at -- the same bargain [`IndexView::documentation`] already makes for
/// docblock text, and for the same reason.
fn initialised(view: &IndexView<'_>, location: &SymbolLocation) -> String {
    let signature = location
        .signature
        .as_deref()
        .unwrap_or_else(|| location.fqn.as_str());

    if !matches!(
        location.kind,
        phrost_model::SymbolKind::ClassConstant | phrost_model::SymbolKind::EnumCase
    ) {
        return resolved_signature(view, location).unwrap_or_else(|| signature.to_owned());
    }

    match initialiser(view, location) {
        Some(value) => format!("{signature} = {value}"),
        None => signature.to_owned(),
    }
}

/// The declaration rendered with the type an override inherits, for one that writes none itself.
///
/// Drupal's whole API is `{@inheritdoc}` over a signature that repeats nothing, so the card for
/// `ContentEntityBase::get()` read `public function get($field_name)` -- everything the reader
/// could already see, and nothing they came for -- while go-to-type-definition at the same site
/// landed on `FieldItemListInterface`. Two descriptions of one declaration, which is the shape
/// §8.1 calls two constructors of one rule.
///
/// Only for a method: it is the kind whose type is written after the parameters, and a property
/// that declares none is rare enough that the second rule is not worth having.
fn resolved_signature(view: &IndexView<'_>, location: &SymbolLocation) -> Option<String> {
    if location.kind != phrost_model::SymbolKind::Method {
        return None;
    }
    let container = location.container.as_ref()?;
    view.resolved_signature(
        container.as_str(),
        phrost_view::MemberKind::Method,
        location.fqn.as_str(),
        // A card describes the declaration it found, and the receiver is not in a
        // `SymbolLocation`: `static` renders as `static`, which is what the declaration says.
        None,
    )
}

/// How much of a value is worth putting on a hover card's first line.
///
/// A constant holding a forty-entry array is a real thing to write, and pasting it into a popup
/// buries the declaration it is supposed to be describing. Past this, and for anything written
/// across lines, the name alone is the better answer.
const VALUE_LIMIT: usize = 80;

/// The initialiser a member was written with, or `None` when it has none worth showing.
///
/// Read from the end of the *name* rather than the start of the declaration, because
/// `const A = 1, B = 2;` gives every constant in it the same `full_range` -- the whole statement's.
/// Starting at the name means `B` begins after its own; stopping at the first comma outside every
/// bracket and quote means `A` ends before `B` begins.
fn initialiser(view: &IndexView<'_>, location: &SymbolLocation) -> Option<String> {
    let range = TextRange::new(location.name_range.end(), location.full_range.end());
    let text = view.source_between(location.file, range)?;
    let rest = text.trim_start().strip_prefix('=')?.trim_start();

    let value = rest[..end_of_value(rest)]
        .trim_end()
        .trim_end_matches(';')
        .trim_end();

    (!value.is_empty() && !value.contains('\n') && value.len() <= VALUE_LIMIT)
        .then(|| value.to_owned())
}

/// Where a value ends: the first comma that is not inside brackets or a string.
///
/// Not a parser and not trying to be one -- it looks for one delimiter, and everything it steps over
/// it steps over blindly. That is enough because the text is known to be one constant expression,
/// and being wrong costs a hover card showing too much rather than a wrong answer anywhere.
fn end_of_value(text: &str) -> usize {
    let mut depth = 0_i32;
    let mut quote = None::<u8>;
    let mut escaped = false;

    for (at, byte) in text.bytes().enumerate() {
        match quote {
            Some(open) => match byte {
                _ if escaped => escaped = false,
                b'\\' => escaped = true,
                _ if byte == open => quote = None,
                _ => {}
            },
            None => match byte {
                b'\'' | b'"' => quote = Some(byte),
                b'(' | b'[' | b'{' => depth += 1,
                b')' | b']' | b'}' => depth -= 1,
                b',' if depth == 0 => return at,
                _ => {}
            },
        }
    }
    text.len()
}

/// `abstract class Repo extends Base implements Countable` plus a `use` line for traits.
///
/// Parents are shown by short name. The namespace line above already places the subject, and a
/// header carrying five fully-qualified trait names — which Drupal's controller base really has — is
/// unreadable. Anyone who needs the full name can navigate to it.
pub(crate) fn render_class(header: &ClassHeader) -> String {
    let mut out = String::new();
    for modifier in &header.modifiers {
        out.push_str(modifier);
        out.push(' ');
    }
    out.push_str(&format!(
        "{} {}",
        header.kind.as_str(),
        short_name(header.fqn.as_str())
    ));

    // Dropped before anything below counts a collision or joins a list, so a stub's internal
    // marker interface -- `__INTERNAL_DO_NOT_USE__StringBackedEnum` and its `Int` sibling, which
    // narrow a backed enum's `$value` for the type checker and were never meant for a person to
    // read -- cannot influence either and never reaches the card.
    let extends = visible(&header.extends);
    let implements = visible(&header.implements);
    let uses = visible(&header.uses);

    let ambiguous = colliding_short_names(header.fqn.as_str(), &extends, &implements, &uses);
    for (keyword, names) in [("extends", &extends), ("implements", &implements)] {
        if names.is_empty() {
            continue;
        }
        out.push_str(&format!(" {keyword} {}", join(names, &ambiguous)));
    }

    if !uses.is_empty() {
        out.push_str(&format!("\n{{\n    use {};\n}}", join(&uses, &ambiguous)));
    }

    out
}

/// [`ClassHeader`]'s own list, with a stub's internal marker interfaces removed.
fn visible(names: &[phrost_model::Name]) -> Vec<phrost_model::Name> {
    names
        .iter()
        .filter(|name| !crate::render::is_stub_internal(name.as_str()))
        .cloned()
        .collect()
}

/// Short names that more than one class in this header would answer to.
///
/// Shortening is a display choice, and it must not turn the card into a false statement.
/// `Drupal\app_core\Drupal\AccountProxy extends Drupal\Core\Session\AccountProxy` is real code --
/// which is why its file aliases the parent -- and shortened it reads `class AccountProxy extends
/// AccountProxy`, a class extending itself.
fn colliding_short_names<'a>(
    own_fqn: &str,
    extends: &'a [phrost_model::Name],
    implements: &'a [phrost_model::Name],
    uses: &'a [phrost_model::Name],
) -> Vec<&'a str> {
    let referenced = extends.iter().chain(implements.iter()).chain(uses.iter());

    let own = short_name(own_fqn);
    let mut counted: Vec<(&str, usize)> = Vec::new();
    for name in referenced {
        let short = short_name(name.as_str());
        match counted.iter_mut().find(|(seen, _)| *seen == short) {
            Some((_, count)) => *count += 1,
            None => counted.push((short, 1)),
        }
    }

    counted
        .into_iter()
        .filter(|(short, count)| *count > 1 || *short == own)
        .map(|(short, _)| short)
        .collect()
}

/// Joins names, writing the ones a short form would not identify in full.
fn join(names: &[phrost_model::Name], ambiguous: &[&str]) -> String {
    names
        .iter()
        .map(|name| {
            let short = short_name(name.as_str());
            if ambiguous.contains(&short) {
                format!("\\{name}")
            } else {
                short.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// The last segment of a path, which is what identifies a configuration file to a reader.
///
/// The whole path is an absolute one on this machine and says nothing a reader of a hover card
/// wants: `node.routing.yml` is the fact, and where the checkout happens to live is not.
fn file_name(path: &std::path::Path) -> std::borrow::Cow<'_, str> {
    path.file_name()
        .map(|name| name.to_string_lossy())
        .unwrap_or_else(|| path.to_string_lossy())
}
