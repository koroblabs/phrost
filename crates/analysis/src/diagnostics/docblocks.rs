//! Whether a declaration's docblock says what the declaration says.
//!
//! Both rules here are about *agreement*, not about presence: a docblock that documents a parameter
//! the declaration does not have, or leaves one out, is worse than no docblock at all -- it reads as
//! authoritative and describes code that was renamed underneath it.
//!
//! Off unless a project asks. A house standard that requires a full docblock on every function is a
//! decision only the project can make -- Drupal's requires one and most projects do not -- so these
//! are named in `phrost.toml` rather than reported by default. See [`Check::asked_for`].
//!
//! `{@inheritdoc}` ends both: the tag means the declaration above documents this one, and demanding
//! the tags again is demanding the copy the tag exists to avoid.

use phrost_view::{DocumentedDeclaration, IndexView};

use crate::docblock::{Docblock as ParsedDocblock, Tag};

use super::{Check, Diagnostic, Severity};

/// A parameter with no `@param`, or a return with no `@return`.
/// Named because `actions::document` offers the fix for it, and because two sites here emit it.
pub(crate) const DOC_MISSING_TAGS: &str = "doc-missing-tags";

pub(crate) const DOC_PARAM_NAME: &str = "doc-param-name";
pub(crate) const DOC_MISSING_THROWS: &str = "doc-missing-throws";
pub(crate) const DOC_REDUNDANT_TYPE: &str = "doc-redundant-type";

pub(super) struct MissingDocTags;

/// A `@param` naming something the declaration does not declare.
pub(super) struct WrongDocParam;

/// A `throw new X` the docblock above it does not admit to.
///
/// Its own rule rather than a third arm of [`MissingDocTags`], because a project that documents its
/// parameters may still leave its exceptions to the reader -- Drupal's standard asks for the first
/// and not the second -- and a rule bundled with the tags could not be turned on without them.
pub(super) struct UndocumentedThrow;

/// A written union one of whose operands swallows another, so the tag says only the wider one.
///
/// `@return Reseller[]|array` is the shape it was built for, and it is not a style note: `array`
/// covers `Reseller[]`, so the union *is* `array` and the element type the author wrote is gone. On
/// the reference project one such tag left sixteen of the project's three hundred and fifty-two
/// unresolved receivers with nothing to resolve to -- every `$reseller->id()` in every callback over
/// what the method returns.
///
/// Reported by default, unlike the two rules above: they are a house standard about what a docblock
/// must contain, and this is a mistake in one that was written.
pub(super) struct RedundantDocType;

impl Check for MissingDocTags {
    fn asked_for(&self) -> Option<&'static str> {
        Some("doc-missing-tags")
    }

    fn run(&self, view: &IndexView<'_>, out: &mut Vec<Diagnostic>) {
        for declaration in view.documented_declarations() {
            if user_facing(view, &declaration) {
                continue;
            }
            let Some(tags) = documented(view, &declaration) else {
                continue;
            };
            let documented_names = subjects(&tags);
            for parameter in &declaration.parameters {
                if documented_names
                    .iter()
                    .any(|name| *name == parameter.name.as_str())
                {
                    continue;
                }
                out.push(Diagnostic {
                    range: parameter.name_range,
                    severity: Severity::Warning,
                    code: DOC_MISSING_TAGS,
                    message: format!("${} has no @param", parameter.name),
                    unnecessary: false,
                    deprecated: false,
                });
            }

            let returns = declaration.returns_a_value || declaration.returns_from_body;
            if returns && !tags.iter().any(is_return) {
                out.push(Diagnostic {
                    range: declaration.name_range,
                    severity: Severity::Warning,
                    code: DOC_MISSING_TAGS,
                    message: "returns a value and has no @return".to_owned(),
                    unnecessary: false,
                    deprecated: false,
                });
            }
        }
    }
}

impl Check for WrongDocParam {
    fn asked_for(&self) -> Option<&'static str> {
        Some("doc-param-name")
    }

    fn run(&self, view: &IndexView<'_>, out: &mut Vec<Diagnostic>) {
        for declaration in view.documented_declarations() {
            let Some(tags) = documented(view, &declaration) else {
                continue;
            };
            // The declaration is what runs, so it is the authority: a `@param` naming a variable it
            // does not declare is a rename that stopped half way, and the tag now documents nothing.
            for name in subjects(&tags) {
                if declaration
                    .parameters
                    .iter()
                    .any(|parameter| parameter.name.as_str() == name)
                {
                    continue;
                }
                out.push(Diagnostic {
                    // The declaration's name, because the docblock's own range is a comment and an
                    // editor underlining a comment reads as a problem with the prose.
                    range: declaration.name_range,
                    severity: Severity::Warning,
                    code: DOC_PARAM_NAME,
                    message: format!("@param ${name} names no parameter of this declaration"),
                    unnecessary: false,
                    deprecated: false,
                });
            }
        }
    }
}

impl Check for UndocumentedThrow {
    fn asked_for(&self) -> Option<&'static str> {
        Some("doc-missing-throws")
    }

    fn run(&self, view: &IndexView<'_>, out: &mut Vec<Diagnostic>) {
        for declaration in view.documented_declarations() {
            if user_facing(view, &declaration) {
                continue;
            }
            let Some(tags) = documented(view, &declaration) else {
                continue;
            };
            // One seam, shared with the quick fix that offers a tag for each of these. It owns what
            // a declaration can throw beyond what it wrote -- a call's declared throws, a contract
            // inherited from what it overrides, a class a framework runs and catches -- because two
            // readers working that out separately is §8.1's oldest form, and the action's copy had
            // neither of the last two.
            let documented: Vec<String> = tags
                .iter()
                .filter(|tag| is_throws(tag))
                .filter_map(|tag| tag.ty.clone())
                .collect();
            for undocumented in view.undocumented_throws(&declaration, &documented) {
                // The first place it is raised, not the declaration's name: the thing to read is
                // which exception, and it is written there.
                let Some(&at) = undocumented.sites.first() else {
                    continue;
                };
                out.push(Diagnostic {
                    range: at,
                    severity: Severity::Warning,
                    code: DOC_MISSING_THROWS,
                    // The call is named when the throw is not written here. Without it the
                    // report sits on `return new self(...$fields);` -- a line holding no `throw`
                    // at all -- and reads as an accusation with the evidence left out, which is
                    // how a true finding gets called spam.
                    message: match &undocumented.through {
                        Some(through) if undocumented.through_declares => format!(
                            "throws \\{}, which {through} declares and the docblock does not",
                            undocumented.class
                        ),
                        // A callee in this file that throws it and does not say so yet: the fix for
                        // that one is in the same file, and the same save makes both.
                        Some(through) => format!(
                            "throws \\{}, which {through} throws and the docblock does not declare",
                            undocumented.class
                        ),
                        None => format!(
                            "throws \\{}, which the docblock does not declare",
                            undocumented.class
                        ),
                    },
                    unnecessary: false,
                    deprecated: false,
                });
            }
        }
    }
}

/// The tags of the comment above a declaration, when it has one worth comparing against.
///
/// `None` for a declaration with no docblock -- that is a rule about *presence* and neither of these
/// is one -- and for one that defers to what it overrides.
fn documented(view: &IndexView<'_>, declaration: &DocumentedDeclaration) -> Option<Vec<Tag>> {
    let docblock = declaration.docblock.as_ref()?;
    let parsed = ParsedDocblock::parse(&docblock.text);
    // `{@inheritdoc}` is PHP's own way of saying the documentation is elsewhere; a framework can have
    // a second, and only the framework knows what it reads like.
    if parsed.inherits() || view.documents_elsewhere(&parsed.prose) {
        return None;
    }
    Some(
        parsed
            .tags
            .iter()
            .filter_map(|line| Tag::parse(line))
            .collect(),
    )
}

/// Whether a framework prints this declaration's docblock at somebody rather than storing it.
///
/// Drupal's `hook_update_N` is the case: the description beside a pending update on `update.php` is
/// the whole docblock, so a tag added to satisfy a rule here is read as part of the sentence. Both
/// rules that demand a tag are off, and `WrongDocParam` deliberately is not -- it is about
/// agreement, so it only speaks about a tag somebody chose to write.
fn user_facing(view: &IndexView<'_>, declaration: &DocumentedDeclaration) -> bool {
    view.user_facing_docblock(declaration.name.as_str())
}

/// Every `$name` a `@param` names, without the `$`.
fn subjects(tags: &[Tag]) -> Vec<&str> {
    tags.iter()
        .filter(|tag| is_param(tag))
        .filter_map(|tag| tag.subject.as_deref())
        // `...$rest` and `&$out` are the same parameter written with what the call does to it. The
        // sigils belong to the signature, not to the name a tag is about.
        .map(|subject| {
            subject
                .trim_start_matches("...")
                .trim_start_matches('&')
                .trim_start_matches('$')
        })
        .collect()
}

/// Whether a tag is a `@param`, under any of the prefixes the tools use.
fn is_param(tag: &Tag) -> bool {
    matches!(tag.name.rsplit('-').next(), Some("param" | "param-out"))
        || tag.name.ends_with("param")
        || crate::diagnostics::misspelt_tags::meant_on_a_function(&tag.name) == Some("param")
}

pub(crate) fn is_throws(tag: &Tag) -> bool {
    matches!(tag.name.rsplit('-').next(), Some("throws" | "throw"))
}

fn is_return(tag: &Tag) -> bool {
    tag.name.ends_with("return")
        || crate::diagnostics::misspelt_tags::meant_on_a_function(&tag.name) == Some("return")
}

impl Check for RedundantDocType {
    fn run(&self, view: &IndexView<'_>, out: &mut Vec<Diagnostic>) {
        for declaration in view.documented_declarations() {
            let Some(tags) = documented(view, &declaration) else {
                continue;
            };
            for tag in &tags {
                let Some(written) = tag.ty.as_deref() else {
                    continue;
                };
                let Some(swallowed) = swallowed_operand(written) else {
                    continue;
                };
                out.push(Diagnostic {
                    // The declaration's name, for the reason `doc-param-name` gives: an editor
                    // underlining a comment reads as a problem with the prose.
                    range: declaration.name_range,
                    severity: Severity::Warning,
                    code: DOC_REDUNDANT_TYPE,
                    message: format!(
                        "@{} {written} says only {}, which covers {swallowed} -- {}",
                        tag.name,
                        WIDER,
                        repair(&swallowed),
                    ),
                    unnecessary: false,
                    deprecated: false,
                });
            }
        }
    }
}

/// The one wider word this rule knows, spelled once.
const WIDER: &str = "array";

/// The operand a bare `array` in the same union swallows, if there is one.
///
/// Only `array` for now, and only beside an operand that states what the container holds. It is the
/// shape that occurs -- `T[]|array`, `array|T[]` -- and the one whose cost is measurable, because
/// the element type is exactly what every reader downstream wanted. A wider subsumption rule
/// (`Child|Parent`, `string|scalar`) is the same idea and needs the class hierarchy, so it is not
/// decided here on text.
///
/// `mixed` is deliberately not the second word: `@return mixed|null` and friends are written to
/// *emphasise* the null rather than to claim an element type, and there is nothing to lose there.
fn swallowed_operand(written: &str) -> Option<String> {
    let operands = union_operands(written);
    if operands.len() < 2 {
        return None;
    }
    if !operands.iter().any(|part| part.eq_ignore_ascii_case(WIDER)) {
        return None;
    }
    operands
        .iter()
        .find(|part| states_what_it_holds(part))
        .map(|part| (*part).to_owned())
}

/// Which operand the author has to edit, which the swallowed one decides.
///
/// Both operands collapse to `array`, so the algebra is the same either way -- but what deleting the
/// wider word *claims* is not. `Reseller[]`, `array<K, V>` and `list<T>` hold the empty array
/// already, so the `array` beside one of them was written out of habit and dropping it says nothing
/// new. A shape with a required key does not hold it: `array{id: int}|array` is an author saying
/// "the shape, or nothing", and deleting the `array` promises the keys are always there -- a
/// stronger claim than a body with an early `return []` makes, and one that reads at every call site
/// as the `?? ''` guarding those keys being redundant. Which is how this rule cost somebody 54
/// findings from another checker for taking the repair it named.
fn repair(swallowed: &str) -> String {
    match excludes_the_empty_array(swallowed) {
        true => format!("{WIDER}{{}} is the empty case it leaves out"),
        false => format!("the {WIDER} adds nothing {swallowed} does not already hold"),
    }
}

/// Whether a written array type has no empty value.
///
/// The question `repair` turns on, and the only two spellings that answer yes are a shape with a key
/// that is not optional -- `array{id: int}`, and a positional `array{int, string}` whose slots are
/// all required -- and a `non-empty-` container, which says so in its name.
fn excludes_the_empty_array(written: &str) -> bool {
    let bare = written.trim_start_matches('?');
    if ["non-empty-array<", "non-empty-list<"]
        .iter()
        .any(|opener| opens_with(bare, opener))
    {
        return true;
    }
    if !opens_with(bare, "array{") || !bare.ends_with('}') {
        return false;
    }
    let entries = split_at_top_level(&bare["array{".len()..bare.len() - 1], ',');
    entries.iter().any(|entry| {
        // `...` is room for keys the shape does not name and demands none of its own.
        if *entry == "..." {
            return false;
        }
        // A positional slot has no key to mark optional, so it is required by being written.
        match split_at_top_level(entry, ':').first() {
            Some(key) if *entry != *key => !key.ends_with('?'),
            _ => true,
        }
    })
}

/// Whether a written type opens with a marker, whatever case it was written in.
fn opens_with(written: &str, opener: &str) -> bool {
    written.len() > opener.len() && written[..opener.len()].eq_ignore_ascii_case(opener)
}

/// Whether a written type is an array that says what it holds.
fn states_what_it_holds(written: &str) -> bool {
    let bare = written.trim_start_matches('?');
    bare.ends_with("[]")
        || [
            "array<",
            "list<",
            "non-empty-array<",
            "non-empty-list<",
            "array{",
        ]
        .iter()
        .any(|opener| opens_with(bare, opener))
}

/// A written union split at its top-level `|`, with generics and shapes left whole.
///
/// `array<string, A|B>|array` is two operands and not three: a `|` inside brackets belongs to
/// whatever opened them. A DNF operand's own parentheses count the same way.
fn union_operands(written: &str) -> Vec<&str> {
    split_at_top_level(written, '|')
}

/// A written type split at one separator, with generics and shapes left whole.
///
/// `array<string, A|B>|array` is two operands and not three: a `|` inside brackets belongs to
/// whatever opened them. A DNF operand's own parentheses count the same way, and a shape's entries
/// split the same way at their commas.
fn split_at_top_level(written: &str, separator: char) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0_i32;
    let mut start = 0;
    for (at, ch) in written.char_indices() {
        match ch {
            '<' | '(' | '{' | '[' => depth += 1,
            '>' | ')' | '}' | ']' => depth -= 1,
            _ if ch == separator && depth == 0 => {
                parts.push(written[start..at].trim());
                start = at + ch.len_utf8();
            }
            _ => {}
        }
    }
    parts.push(written[start..].trim());
    parts.retain(|part| !part.is_empty());
    parts
}
