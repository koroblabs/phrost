//! What a docblock says, for colouring.
//!
//! The one place this crate reads something a grammar cannot even see. To an editor's PHP grammar a
//! docblock is a comment, so `@phpstan-assert-if-true \App\Special $this` is one grey run — the tag,
//! the class and the variable all the same colour as the prose beside them. Nothing about that is a
//! guess a grammar could avoid: the contents are a second language, and reading it needs a second
//! parser.
//!
//! Three things are emitted and no more: the tag, the type it names, and the variable it is about.
//! The description keeps the colour it had, which is right — it is prose.
//!
//! The type is one token over the whole of what was written, not one per name inside it. `\App\Repo`
//! and `?int` and `list<Repo>` are all *a type*, and that is what a reader wants distinguished from
//! the sentence after it; colouring `null` differently from `Repo` inside a `@param` is a
//! distinction nobody is reading for. It also means this needs no walk over the sixty-odd shapes a
//! docblock type can take.
//!
//! A tag whose value is not read here keeps the colour the grammar gave it. That is the same rule as
//! the rest of this crate: emitting less is how it avoids being worse than what it replaces.

use mago_allocator::{Arena as _, LocalArena};
use mago_phpdoc_syntax::{
    PHPDocParser,
    cst::{Element, Tag, TagValue, TextSegment, Variable, r#type::Type},
};
use mago_span::{HasSpan, Span};
use phrost_model::{TextRange, TextSize};
use phrost_syntax::Docblock;

use super::{SemanticToken, TokenKind, TokenModifiers};

/// Emits tokens for every docblock in the file.
pub(super) fn from_docblocks(blocks: &[Docblock<'_>], found: &mut Vec<SemanticToken>) {
    for block in blocks {
        // A `@` is the cheapest possible gate and it is exact: a docblock without one holds nothing
        // this reads, and most docblocks in a project are prose.
        if !block.text.contains('@') {
            continue;
        }
        let from = found.len();
        from_docblock(*block, found);
        break_at_line_ends(*block, found, from);
    }
}

/// Splits every token this block produced that crosses a line, in place.
///
/// The protocol encodes a token as a line, a start column and a length, and has no way to say "and
/// it continues below" -- so the encoder computes the length as a difference of *columns*. Handed a
/// token that starts on one line and ends on another it produces a nonsense length, and where the
/// closing column is the smaller of the two it saturates to zero and the token disappears. An
/// array shape written down the page is exactly that shape:
///
/// ```text
/// @param array{
///   context_values?: mixed,
/// } $options
/// ```
///
/// One token per line instead, so the invariant holds by construction here rather than by every
/// caller remembering it. Each piece is trimmed past the leading `*`, which belongs to the comment
/// and keeps the colour the grammar gave it.
fn break_at_line_ends(block: Docblock<'_>, found: &mut Vec<SemanticToken>, from: usize) {
    if !found[from..]
        .iter()
        .any(|token| crosses_a_line(block, token))
    {
        return;
    }

    let base = block.range.start();
    let mut split = Vec::with_capacity(found.len() - from + 4);
    for token in &found[from..] {
        let start = usize::from(token.range.start() - base);
        let Some(text) = block.text.get(start..usize::from(token.range.end() - base)) else {
            split.push(*token);
            continue;
        };
        if !text.contains('\n') {
            split.push(*token);
            continue;
        }
        let mut at = start;
        for line in text.split_inclusive('\n') {
            let trimmed = line.trim_end();
            // Past the indentation and the `*` that continues the comment. Only for a line after
            // the first: the first begins where the type does, just after the tag.
            let content = match at == start {
                true => trimmed,
                false => trimmed
                    .trim_start()
                    .strip_prefix('*')
                    .map_or_else(|| trimmed.trim_start(), str::trim_start),
            };
            if !content.is_empty()
                && let Some(offset) = trimmed.find(content)
                && let Ok(offset) = u32::try_from(at + offset)
                && let Ok(len) = u32::try_from(content.len())
            {
                let piece = base + TextSize::from(offset);
                split.push(SemanticToken {
                    range: TextRange::new(piece, piece + TextSize::from(len)),
                    kind: token.kind,
                    modifiers: token.modifiers,
                });
            }
            at += line.len();
        }
    }
    found.truncate(from);
    found.extend(split);
}

/// Whether a token spans more than the one line the protocol can encode.
fn crosses_a_line(block: Docblock<'_>, token: &SemanticToken) -> bool {
    let base = block.range.start();
    block
        .text
        .get(usize::from(token.range.start() - base)..usize::from(token.range.end() - base))
        .is_some_and(|text| text.contains('\n'))
}

fn from_docblock(block: Docblock<'_>, found: &mut Vec<SemanticToken>) {
    let arena = LocalArena::new();
    let content = arena.alloc_slice_copy(block.text.as_bytes());
    // A synthetic file id: the parser only stamps spans with it, and every span is rebased onto the
    // real file below.
    let document = PHPDocParser::parse(
        &arena,
        mago_database::file::FileId::new(b"docblock"),
        content,
    );

    let base = block.range.start();
    for element in &document.elements {
        match element {
            Element::Tag(tag) => {
                emit(
                    found,
                    base,
                    tag.at.join(tag.name.span()),
                    TokenKind::Decorator,
                );
                from_tag(found, base, tag);
                from_reference(found, base, block.text, tag);
                from_annotation(found, base, block.text, tag);
            }
            // `{@inheritdoc}`, `{@see Foo::bar()}` -- a tag written *inside* the prose, which the
            // parser hands back as a segment of the text rather than as an element of its own. The
            // commonest tag in a framework project is this one, and to everything above it is a
            // sentence: nothing else in the pipeline distinguishes it, so left out here it is the
            // only tag in a docblock with no colour at all.
            Element::Text(text) => {
                for segment in text.segments {
                    let TextSegment::InlineTag(inline) = segment else {
                        continue;
                    };
                    // The braces belong to the tag rather than to the prose, and they are emitted
                    // as two runs around whatever the value contributes rather than as one span
                    // over the whole of it -- two tokens over one character have no meaning in this
                    // protocol, which is the rule `ty` states at length.
                    emit(
                        found,
                        base,
                        inline.left_brace.join(inline.tag.name.span()),
                        TokenKind::Decorator,
                    );
                    from_tag(found, base, inline.tag);
                    from_reference(found, base, block.text, inline.tag);
                    emit(found, base, inline.right_brace, TokenKind::Decorator);
                }
            }
            Element::Code(_) => {}
        }
    }
}

/// The name a `@see` points at, block or inline.
///
/// `@param`, `@return` and `@throws` colour the class they name, and `@see` is the one tag that
/// names a class without it being the type of anything -- so its target was the only navigable
/// name in a docblock drawn as prose. A framework writes the tag eleven thousand times.
///
/// `@see` alone, and not `@link` or `@uses` beside it, because lowering records a name reference
/// for exactly this tag: colouring a second one would say "this can be followed" where nothing
/// follows it. The rule that finds the name lives in `phrost-syntax`, below both readers, for the
/// same reason.
fn from_reference(found: &mut Vec<SemanticToken>, base: TextSize, text: &str, tag: &Tag<'_>) {
    if !tag.name.value.eq_ignore_ascii_case(b"see") {
        return;
    }
    let TagValue::Generic(value) = &tag.value else {
        return;
    };
    let span = value.value.span;
    let (from, to) = (span.start.offset as usize, span.end.offset as usize);
    let Some(written) = text.get(from..to) else {
        return;
    };
    let Some((within, kind)) = phrost_syntax::see_reference(written) else {
        return;
    };

    let start = base + TextSize::from(span.start.offset) + within.start();
    found.push(SemanticToken {
        range: TextRange::at(start, within.len()),
        kind: match kind {
            // The same kind a type gets in every other tag, so a reference reads as a reference
            // wherever it is written.
            phrost_syntax::ReferenceKind::ClassLike => TokenKind::Type,
            phrost_syntax::ReferenceKind::Function => TokenKind::Function,
            // The protocol has no constant, and a global one is a variable nothing may write to.
            phrost_syntax::ReferenceKind::Constant => TokenKind::Variable,
        },
        modifiers: TokenModifiers::default(),
    });
}

/// The type and the variable one tag names.
///
/// Grouped by what a reader sees rather than by what the tag means: every arm here either points at
/// a type, or at a variable, or at both. The `Method` arm is the one that does more, because a
/// `@method` tag is a whole signature and reads as one.
fn from_tag(found: &mut Vec<SemanticToken>, base: TextSize, tag: &Tag<'_>) {
    match &tag.value {
        // A type on its own.
        TagValue::Return(value) | TagValue::RealReturn(value) => ty(found, base, value.r#type),
        TagValue::Throws(value) => ty(found, base, value.r#type),
        TagValue::Mixin(value) => ty(found, base, value.r#type),
        TagValue::SelfOut(value) => ty(found, base, value.r#type),
        TagValue::Extends(value) => ty(found, base, value.r#type),
        TagValue::Implements(value) => ty(found, base, value.r#type),
        TagValue::Use(value) => ty(found, base, value.r#type),
        TagValue::RequireExtends(value) => ty(found, base, value.r#type),
        TagValue::RequireImplements(value) => ty(found, base, value.r#type),
        TagValue::Assert(value)
        | TagValue::AssertIfTrue(value)
        | TagValue::AssertIfFalse(value) => {
            if let mago_phpdoc_syntax::cst::AssertPattern::Type(asserted) = value.pattern {
                ty(found, base, asserted);
            }
            subject(found, base, &value.subject);
        }

        // A type and the parameter it is about.
        TagValue::Param(value) => {
            ty(found, base, value.r#type);
            if let Some(parameter) = &value.parameter {
                variable(found, base, parameter, TokenKind::Parameter);
            }
        }
        TagValue::ParamOut(value) => {
            ty(found, base, value.r#type);
            variable(found, base, &value.parameter, TokenKind::Parameter);
        }
        TagValue::TypelessParam(value) => {
            variable(found, base, &value.parameter, TokenKind::Parameter);
        }

        // A type and the variable it is about. `@var` on a property is still a variable as written.
        TagValue::Var(value) => {
            ty(found, base, value.r#type);
            if let Some(written) = &value.variable {
                variable(found, base, written, TokenKind::Variable);
            }
        }
        TagValue::Property(value)
        | TagValue::PropertyRead(value)
        | TagValue::PropertyWrite(value) => {
            if let Some(written) = value.r#type {
                ty(found, base, written);
            }
            variable(found, base, &value.variable, TokenKind::Property);
        }

        // `@template T of Bound = Fallback`: the parameter is a type name that exists only in the
        // docblock, so nothing but this will ever colour it.
        TagValue::Template(value) => {
            emit(found, base, value.name.span(), TokenKind::Type);
            if let Some(bound) = &value.bound {
                ty(found, base, bound.r#type);
            }
            if let Some(default) = &value.default {
                ty(found, base, default.r#type);
            }
        }

        // `@phpstan-type Result array{ok: bool}` declares a type name the language has none of,
        // and `@phpstan-import-type Result from \App\Comparator as Comparison` moves one between
        // files. The same case as `@template` above, and the one a reader notices first: the alias
        // is coloured as a type everywhere it is *used*, so a definition drawn as prose reads as
        // the feature having missed the line rather than as a tag nobody taught it.
        TagValue::TypeAlias(value) => {
            emit(found, base, value.alias.span(), TokenKind::Type);
            ty(found, base, value.r#type);
        }
        TagValue::TypeAliasImport(value) => {
            emit(found, base, value.imported_alias.span(), TokenKind::Type);
            emit(found, base, value.imported_from.span(), TokenKind::Type);
            if let Some(renamed) = &value.imported_as {
                emit(found, base, renamed.local.span(), TokenKind::Type);
            }
        }

        // `@where T is Bound`, which constrains a template parameter the tag names again.
        TagValue::Where(value) => {
            emit(found, base, value.name.span(), TokenKind::Type);
            ty(found, base, value.r#type);
        }

        // A class named without being anything's type, the way `@extends` names one.
        TagValue::Sealed(value) => ty(found, base, value.r#type),
        TagValue::Inheritors(value) => ty(found, base, value.r#type),

        // The rest of the `@param` family, each about one parameter and some about its type too.
        TagValue::ParamClosureThis(value) => {
            ty(found, base, value.r#type);
            variable(found, base, &value.parameter, TokenKind::Parameter);
        }
        TagValue::ParamImmediatelyInvokedCallable(value) => {
            variable(found, base, &value.parameter, TokenKind::Parameter);
        }
        TagValue::ParamLaterInvokedCallable(value) => {
            variable(found, base, &value.parameter, TokenKind::Parameter);
        }
        TagValue::PureUnlessCallableIsImpure(value) => {
            variable(found, base, &value.parameter, TokenKind::Parameter);
        }
        TagValue::Trace(value) => variable(found, base, &value.variable, TokenKind::Variable),

        // A whole signature.
        TagValue::Method(value) => {
            if let Some(returned) = value.return_type {
                ty(found, base, returned);
            }
            emit(found, base, value.name.span(), TokenKind::Method);
            for parameter in value.parameters.entries.iter() {
                if let Some(written) = parameter.r#type {
                    ty(found, base, written);
                }
                variable(found, base, &parameter.parameter, TokenKind::Parameter);
            }
        }

        // Every remaining tag is a marker or a sentence -- it says something about the declaration
        // without naming a type or a variable, so the prose keeps the colour the grammar gave it.
        // `Generic` is every tag mago does not model, `@see` among them, and that one is read
        // above by name rather than by shape.
        //
        // Listed rather than swept up by a `_`, which is the whole point of the arms above it: a
        // `@phpstan-type` has been modelled since before this file existed and went uncoloured
        // anyway, because a wildcard answers "not one of these" for a variant nobody added. Adding
        // one now stops the build instead.
        TagValue::Deprecated(_)
        | TagValue::NotDeprecated(_)
        | TagValue::Final(_)
        | TagValue::Internal(_)
        | TagValue::Api(_)
        | TagValue::Experimental(_)
        | TagValue::Pure(_)
        | TagValue::Impure(_)
        | TagValue::Readonly(_)
        | TagValue::MustUse(_)
        | TagValue::NoNamedArguments(_)
        | TagValue::EnumInterface(_)
        | TagValue::ConsistentConstructor(_)
        | TagValue::ConsistentTemplates(_)
        | TagValue::SealProperties(_)
        | TagValue::NoSealProperties(_)
        | TagValue::SealMethods(_)
        | TagValue::NoSealMethods(_)
        | TagValue::MutationFree(_)
        | TagValue::ExternalMutationFree(_)
        | TagValue::SuspendsFiber(_)
        | TagValue::IgnoreNullableReturn(_)
        | TagValue::IgnoreFalsableReturn(_)
        | TagValue::InheritDoc(_)
        | TagValue::Generic(_)
        | TagValue::Invalid(_) => {}
    }
}

/// The insides of a Doctrine-style annotation:
/// `@ContentEntityType(id = "node", label = @Translation("Node"))`.
///
/// Drupal's older plugin syntax, and to every grammar it is one comment -- one long grey run
/// holding the id a whole subsystem is keyed by. The tag's own name is coloured above like any
/// other; this is the part in the parentheses, which mago models as nothing because it is
/// Doctrine's syntax rather than PHPDoc's.
///
/// Three kinds and no more: the key of an entry, a quoted string, and a nested annotation's name.
/// The punctuation holding them together keeps the comment's own colour, which is what it is.
///
/// A second walk over the same text `phrost_lower::annotations` walks, and deliberately: that one
/// asks what the id *is*, for a catalog, and this asks where each piece is, for a colour. They
/// also live in crates that cannot see each other -- this one may not read the lowering, by the
/// dependency rule that keeps PHP semantics out of here.
fn from_annotation(found: &mut Vec<SemanticToken>, base: TextSize, text: &str, tag: &Tag<'_>) {
    // An annotation is a tag whose name is followed *immediately* by `(`. Every PHPDoc tag whose
    // value this file already reads is followed by a space and a type, so the test is exact rather
    // than a guess -- and a tag with no parentheses at all is a bare marker with nothing inside.
    let from = usize::try_from(tag.name.span().end.offset).unwrap_or(usize::MAX);
    let to = usize::try_from(tag.span().end.offset).unwrap_or(usize::MAX);
    let Some(body) = text.get(from..to) else {
        return;
    };
    if !body.starts_with('(') {
        return;
    }

    let bytes = body.as_bytes();
    let mut at = 0;
    while at < bytes.len() {
        let start = at;
        match bytes[at] {
            // A string, in either quote, with its own quotes included: what a reader picks out of
            // an annotation is the values, and the quotes are part of seeing where one ends.
            quote @ (b'"' | b'\'') => {
                at += 1;
                let mut escaped = false;
                while at < bytes.len() {
                    match bytes[at] {
                        _ if escaped => escaped = false,
                        b'\\' => escaped = true,
                        byte if byte == quote => {
                            at += 1;
                            break;
                        }
                        _ => {}
                    }
                    at += 1;
                }
                emit_run(found, base, from + start, from + at, TokenKind::Str);
            }
            // A nested annotation -- `@Translation("...")` -- whose own arguments the loop reaches
            // on the next turns, since nothing here steps over a parenthesis.
            b'@' => {
                at += 1;
                while at < bytes.len() && is_name_byte(bytes[at]) {
                    at += 1;
                }
                emit_run(found, base, from + start, from + at, TokenKind::Decorator);
            }
            byte if is_name_byte(byte) => {
                while at < bytes.len() && is_name_byte(bytes[at]) {
                    at += 1;
                }
                // A word is a key only when an `=` follows it. Anything else is a bare constant or
                // a word of the prose a tag can carry, and colouring it as a key would say it names
                // something it does not.
                let rest = body[at..].trim_start();
                if rest.starts_with('=') && !rest.starts_with("==") {
                    emit_run(found, base, from + start, from + at, TokenKind::Property);
                }
            }
            _ => at += 1,
        }
        // Every arm above consumes at least the byte it started on, except the ones that fall
        // through -- and those advance here.
        if at == start {
            at += 1;
        }
    }
}

/// Whether a byte can be part of an annotation's key or of a nested annotation's name.
fn is_name_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'\\'
}

/// [`run`] over offsets into the docblock's own text.
fn emit_run(
    found: &mut Vec<SemanticToken>,
    base: TextSize,
    from: usize,
    to: usize,
    kind: TokenKind,
) {
    let (Ok(from), Ok(to)) = (u32::try_from(from), u32::try_from(to)) else {
        return;
    };
    run(found, base, from, to, kind);
}

/// What an `@assert` tag is about: a parameter, or something reached through one.
///
/// Recursive because the subject is: `$this->config->cache()` nests a method inside a property
/// inside a parameter, and each step paints the name at its own end. A subject that reads only the
/// variable at the root would leave every step between it and the leaf uncoloured.
fn subject(
    found: &mut Vec<SemanticToken>,
    base: TextSize,
    of: &mago_phpdoc_syntax::cst::AssertSubject<'_>,
) {
    use mago_phpdoc_syntax::cst::AssertSubject;

    match of {
        AssertSubject::Parameter { variable: written } => {
            variable(found, base, written, TokenKind::Parameter);
        }
        AssertSubject::Method { object, method, .. } => {
            subject(found, base, object);
            emit(found, base, method.span(), TokenKind::Method);
        }
        AssertSubject::Property {
            object, property, ..
        } => {
            subject(found, base, object);
            emit(found, base, property.span(), TokenKind::Property);
        }
    }
}

/// A type as written, whole.
fn ty(found: &mut Vec<SemanticToken>, base: TextSize, written: &Type<'_>) {
    let mut keys = Vec::new();
    shape_keys(written, &mut keys);
    if keys.is_empty() {
        emit(found, base, written.span(), TokenKind::Type);
        return;
    }

    // Carved rather than overlaid: two tokens over the same character have no meaning in this
    // protocol -- the client reads a flat sequence of deltas, so an overlap moves everything after
    // it. So the type is emitted as the runs *between* the keys.
    let whole = written.span();
    let mut at = whole.start.offset;
    for key in keys {
        run(found, base, at, key.start.offset, TokenKind::Type);
        run(
            found,
            base,
            key.start.offset,
            key.end.offset,
            TokenKind::Property,
        );
        at = key.end.offset;
    }
    run(found, base, at, whole.end.offset, TokenKind::Type);
}

/// One token over a stretch of the docblock, given offsets into it.
///
/// Offsets rather than a `Span`, because a carved run is not any node's span and building one would
/// mean trusting `Span`'s own arithmetic to mean what this needs -- it does not: `subspan` counts
/// from the span's start, not from the file's.
fn run(found: &mut Vec<SemanticToken>, base: TextSize, from: u32, to: u32, kind: TokenKind) {
    if to <= from {
        return;
    }
    found.push(SemanticToken {
        range: TextRange::new(base + TextSize::from(from), base + TextSize::from(to)),
        kind,
        modifiers: TokenModifiers::default(),
    });
}

/// The key of every field of every array shape in a type, in source order.
///
/// A key is the one thing inside a docblock type that is not a type: `array{name: string}` names a
/// field and then says what it holds, and a reader scans the names. Everything else stays one
/// `Type` token, for the reason the module documentation gives.
///
/// Recursion follows a field's value only when that value is itself a shape, which is the nesting
/// people actually write. A shape reached through a generic argument -- `list<array{a: int}>` --
/// keeps its keys the colour of the type around them, and that is the honest limit of not walking
/// the sixty-odd shapes a docblock type can take.
fn shape_keys(written: &Type<'_>, out: &mut Vec<Span>) {
    let Type::Shape(shape) = written else {
        return;
    };
    for field in shape.fields.iter() {
        if let Some(key) = &field.key {
            out.push(key.key.span());
        }
        shape_keys(field.value, out);
    }
}

fn variable(
    found: &mut Vec<SemanticToken>,
    base: TextSize,
    written: &Variable<'_>,
    kind: TokenKind,
) {
    emit(found, base, written.span(), kind);
}

/// Moves a docblock-relative span onto the file and records it.
///
/// Dropped when it would cross a line: the protocol cannot encode such a token, and a type written
/// across two lines of a docblock is rare enough that losing its colour costs nothing.
fn emit(found: &mut Vec<SemanticToken>, base: TextSize, span: Span, kind: TokenKind) {
    let range = TextRange::new(
        base + TextSize::from(span.start.offset),
        base + TextSize::from(span.end.offset),
    );
    if range.is_empty() {
        return;
    }
    found.push(SemanticToken {
        range,
        kind,
        modifiers: TokenModifiers::default(),
    });
}
