//! Writing a docblock annotation as a PHP attribute instead.
//!
//! `@RestResource(id = "x", label = @Translation("X"))` and `#[RestResource(id: "x", label: new
//! TranslatableMarkup("X"))]` say the same thing to the same plugin manager, and a framework that
//! has both spellings is moving from the first to the second. The conversion is mechanical and
//! nobody does it by hand happily: the arguments have to be repunctuated, the nested annotations
//! turned into constructor calls, the classes imported, and the docblock left holding only its
//! prose.
//!
//! Mechanical is not the same as safe, so two things are refused rather than guessed. An annotation
//! whose name matches no attribute class in the index is not converted -- it is a docblock tag this
//! framework does not have an attribute for, or a project where no framework was detected at all.
//! And an annotation carrying a *nested* one nothing can resolve is refused whole, because writing
//! `new Whatever(...)` for it would be an import nobody can add.
//!
//! A refactor rather than a quick fix: nothing here is wrong. Nothing marks it preferred either --
//! a keystroke should not silently rewrite a plugin's declaration.

mod doctrine;

use std::ops::Range;

use phrost_model::TextSize;
use phrost_view::ImportKind;

use self::doctrine::Annotation;
use super::{ActionKind, ActionSubject, CodeAction, Offer, SubjectSites, TextEdit};

pub(super) struct AnnotationAsAttribute;

impl Offer for AnnotationAsAttribute {
    fn kind(&self) -> ActionKind {
        ActionKind::Refactor
    }

    fn subject(&self) -> ActionSubject {
        ActionSubject::ClassDeclaration
    }

    fn run(&self, sites: &SubjectSites<'_, '_>, out: &mut Vec<CodeAction>) {
        let view = sites.view();
        for class in sites.declared_classes() {
            // An annotation is written in a docblock, so a class with none has none.
            let Some(docblock) = class.docblock.as_ref() else {
                continue;
            };
            let Some(annotation) = Annotation::first_in(&docblock.text) else {
                continue;
            };

            // The values a framework named come first: `@Translation` resolves to a class that is
            // not an attribute and never was, and a project with an attribute of that name would
            // otherwise win the search and be written out in its place.
            let class_of = |name: &str| {
                view.class_for_annotation_value(name)
                    .map(str::to_owned)
                    .or_else(|| {
                        view.attribute_for_annotation(name)
                            .map(|fqn| fqn.as_str().to_owned())
                    })
            };
            let Some(attribute) = annotation.as_attribute(&class.indent, &class_of) else {
                continue;
            };

            let indent = &class.indent;
            let text = match docblock_without(&docblock.text, &annotation.span) {
                Some(prose) => format!("{prose}\n{indent}{}", attribute.text),
                None => attribute.text,
            };

            let mut edits = vec![TextEdit::replace(docblock.range, text)];
            edits.extend(imports_for(
                sites,
                class.name_range.start(),
                &attribute.imports,
            ));

            out.push(CodeAction {
                title: format!("Convert @{} to an attribute", annotation.name),
                edits,
                preferred: false,
                kind: ActionKind::Refactor,
                fixes: None,
            });
        }
    }
}

/// The `use` lines the attribute's own text now needs, as one edit per insertion point.
///
/// One edit and not one per class: every class-like import in a file anchors to the end of the same
/// `use` block, so two of them would be two edits at one position -- which the protocol leaves in no
/// defined order, and which a client is entitled to reject as overlapping.
fn imports_for(sites: &SubjectSites<'_, '_>, at: TextSize, classes: &[String]) -> Vec<TextEdit> {
    let mut lines: Vec<(TextSize, String)> = Vec::new();
    for class in classes {
        let Some(edit) = sites.view().import_for(at, class, ImportKind::ClassLike) else {
            continue;
        };
        match lines.iter_mut().find(|(anchor, _)| *anchor == edit.at) {
            Some((_, held)) => held.push_str(&edit.text),
            None => lines.push((edit.at, edit.text)),
        }
    }
    lines
        .into_iter()
        .map(|(at, text)| TextEdit::insert(at, text))
        .collect()
}

/// The docblock with one annotation's lines taken out of it.
///
/// `None` when the annotation was all it said, in which case the whole comment goes: an empty
/// docblock above a declaration is worse than no docblock, and a standard that asks for one is
/// asking for the sentence rather than for the delimiters.
fn docblock_without(text: &str, span: &Range<usize>) -> Option<String> {
    let from = text[..span.start].rfind('\n').map_or(0, |at| at + 1);
    let to = text[span.end..]
        .find('\n')
        .map_or(text.len(), |at| span.end + at + 1);
    let kept = format!("{}{}", &text[..from], &text[to..]);

    // A blank `*` line separates two things, and one of them has just left. Two in a row is what
    // removing a middle paragraph leaves; one before the closing delimiter is what removing the
    // last one leaves, which is the common case.
    let mut lines: Vec<&str> = Vec::new();
    for line in kept.split('\n') {
        if is_blank(line) && lines.last().is_some_and(|previous| is_blank(previous)) {
            continue;
        }
        lines.push(line);
    }
    while lines.len() > 1 && is_blank(lines[lines.len() - 2]) {
        lines.remove(lines.len() - 2);
    }

    (!lines.iter().all(|line| says_nothing(line))).then(|| lines.join("\n"))
}

/// A docblock line that carries nothing but its own `*`.
fn is_blank(line: &str) -> bool {
    line.trim() == "*"
}

/// A line that is only the comment's own punctuation.
fn says_nothing(line: &str) -> bool {
    matches!(line.trim(), "" | "*" | "/**" | "*/")
}
