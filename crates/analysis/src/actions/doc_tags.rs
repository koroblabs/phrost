//! Adding the `@param` and `@return` tags a docblock is missing, without touching a line already
//! written above it.
//!
//! Four offers over one reading of the docblock. `Add @param $x` on the parameter, and on the
//! declaration's name when it is the only thing missing; `Add @return` on the name; `Add @throws`
//! on the name and on the `throw` itself; and `Update the docblock`, on the name or on any of those
//! sites, whenever two or more things are missing -- every missing `@param`, the `@return` and every
//! undocumented `@throws` in one edit, which is what somebody standing on a signature that grew
//! wants, and is what the bulk `@param` action used to be before the rest was counted with the
//! parameters.
//!
//! `document::WriteDocblock` is the other half of this: it writes a docblock where a declaration
//! has none. A signature that grows a parameter, or gains a return type, after its docblock was
//! written ends up with one that still parses and is still wrong -- and that declaration already
//! has a docblock, so `WriteDocblock` refuses it outright. Nothing here may rewrite what is
//! already there either; only the one missing line is added, at the place the check that reports
//! it already points to.

use std::fmt::Write as _;

use phrost_model::TextSize;
use phrost_view::{Docblock, Documentable, DocumentedDeclaration, IndexView};

use super::{
    ActionKind, ActionSubject, CodeAction, DocStyle, Offer, SubjectSites, TextEdit, concerns,
};
use crate::diagnostics::docblocks::{DOC_MISSING_TAGS, DOC_MISSING_THROWS, is_throws};
use crate::docblock::{Docblock as ParsedDocblock, Tag};

pub(super) struct AddMissingDocTag;

impl Offer for AddMissingDocTag {
    fn kind(&self) -> ActionKind {
        ActionKind::QuickFix
    }

    fn subject(&self) -> ActionSubject {
        ActionSubject::Declaration
    }

    fn run(&self, sites: &SubjectSites<'_, '_>, out: &mut Vec<CodeAction>) {
        // The wider list, which the other declaration offers do not use: a throw raised by a call
        // is a position inside the body and on none of the three this offer's neighbours act on.
        // What may be offered from a body position is settled by the guards in `tags_for`, not
        // here -- see `declarations_or_their_bodies`.
        for declaration in sites.declarations_or_their_bodies() {
            Self::tags_for(sites, declaration, out);
        }
    }

    /// A tag line is read by whoever reads the docblock next and changes nothing the code does.
    fn unattended(&self) -> bool {
        true
    }
}

impl AddMissingDocTag {
    /// Everything one declaration is missing a tag for.
    ///
    /// Its own method because it gives up in a dozen places, and each of those is a `return` here
    /// where it would be a `continue` in a loop over the whole file -- the shape that let the
    /// expensive step below sit above the cheap ones without anybody noticing.
    fn tags_for(
        sites: &SubjectSites<'_, '_>,
        declaration: &DocumentedDeclaration,
        out: &mut Vec<CodeAction>,
    ) {
        let (view, at) = (sites.view(), sites.at());

        let Some(docblock) = &declaration.docblock else {
            // No docblock at all is `WriteDocblock`'s finding, not this one's.
            return;
        };
        // A framework may print the whole docblock at somebody -- Drupal's update descriptions are
        // that docblock with the stars stripped -- and then a tag offered here is not a line a
        // reader may skip but words in the middle of the sentence.
        if view.user_facing_docblock(declaration.name.as_str()) {
            return;
        }
        let parsed = ParsedDocblock::parse(&docblock.text);
        // `{@inheritdoc}` means the tags are the ones above, on the declaration this overrides, and
        // a framework may settle documentation elsewhere entirely -- neither is a gap a line here
        // can fill.
        if parsed.inherits() || view.documents_elsewhere(&parsed.prose) {
            return;
        }
        let tags: Vec<Tag> = parsed
            .tags
            .iter()
            .filter_map(|line| Tag::parse(line))
            .collect();
        let documented = subjects(&tags);

        // What each parameter and the return are *typed* as is a position query over the whole
        // file, so it is asked once and only once something is known to be missing. A declaration
        // whose docblock is complete -- most of them, in a project that documents itself -- never
        // asks at all.
        let mut typed = Typed::Unasked;

        // Every parameter with no `@param`, in the order the signature declares them -- which is the
        // order they have to be written in, and the reason they are collected before any action is
        // offered rather than one per turn of the loop.
        let mut missing: Vec<(&phrost_view::Parameter, String, String)> = Vec::new();
        for parameter in &declaration.parameters {
            if documented
                .iter()
                .any(|name| *name == parameter.name.as_str())
            {
                continue;
            }
            // The type information comes from `documentable_at`, for a method and for a free
            // function alike; a declaration it cannot type is left alone rather than offered a
            // `mixed` that was never asked for.
            let Some(typed) = typed.of(view, declaration) else {
                return;
            };
            let Some(param) = typed
                .params
                .iter()
                .find(|param| param.name.as_str() == parameter.name.as_str())
            else {
                continue;
            };
            missing.push((
                parameter,
                format!("{} ${}", param.ty.as_deref().unwrap_or("mixed"), param.name),
                super::prose::param_prose(param.name.as_str()),
            ));
        }

        // The return, when the docblock has none and the declaration hands something back. Asked
        // before anything is offered, because how many things are missing *in total* decides what
        // is offered where: one missing thing gets the action that names it, two or more get the
        // whole docblock brought up to date.
        let mut returned: Option<(String, String)> = None;
        if !tags.iter().any(is_return)
            && (declaration.returns_a_value || declaration.returns_from_body)
            && let Some(typed) = typed.of(view, declaration)
            // Untyped and not inferred: there is nothing to write that would not be invented.
            && let Some(returns) = &typed.returns
        {
            returned = Some((returns.clone(), super::prose::return_prose(returns)));
        }
        // What the declaration can throw and the docblock does not admit to, one entry per class
        // with every place it is raised -- so the fix is on offer from the site a report underlines
        // as well as from the name.
        //
        // Through the seam the check reports from, and not a second reading of it. This had its own
        // and the two had drifted: the action saw only what the body *writes*, so a finding raised
        // by a call had no fix at all, and it offered a tag for a class the interface had already
        // documented -- a fix for a finding nobody was shown.
        let documented_throws: Vec<String> = tags
            .iter()
            .filter(|tag| is_throws(tag))
            .filter_map(|tag| tag.ty.clone())
            .collect();
        let thrown = view.undocumented_throws(declaration, &documented_throws);

        let total = missing.len() + usize::from(returned.is_some()) + thrown.len();
        if total == 0 {
            return;
        }
        let at_name = concerns(declaration.name_range, at);
        let at_a_missing_parameter = missing
            .iter()
            .any(|(parameter, ..)| concerns(parameter.name_range, at));
        let at_a_throw =
            |ranges: &[phrost_model::TextRange]| ranges.iter().any(|range| concerns(*range, at));
        let at_an_undocumented_throw = thrown.iter().any(|entry| at_a_throw(&entry.sites));

        for (parameter, subject, prose) in &missing {
            // On the parameter, and on the name when it is the only thing the docblock lacks --
            // where the one action that names the fix is better than one that says "update".
            if !(concerns(parameter.name_range, at) || (at_name && total == 1)) {
                continue;
            }
            let Some(edits) = tag_edits(
                view,
                docblock,
                &[("param", vec![(subject.clone(), prose.clone())])],
            ) else {
                continue;
            };
            out.push(CodeAction {
                title: format!("Add @param ${}", parameter.name),
                edits,
                // The only tag missing anywhere in the docblock.
                preferred: total == 1,
                kind: ActionKind::QuickFix,
                fixes: Some(DOC_MISSING_TAGS),
            });
        }

        if at_name
            && let Some((returns, prose)) = &returned
            && let Some(edits) = tag_edits(
                view,
                docblock,
                &[("return", vec![(returns.clone(), prose.clone())])],
            )
        {
            out.push(CodeAction {
                title: "Add @return".to_owned(),
                edits,
                preferred: total == 1,
                kind: ActionKind::QuickFix,
                fixes: Some(DOC_MISSING_TAGS),
            });
        }

        let throws_written = |classes: &[&phrost_model::Name]| -> Vec<(String, String)> {
            classes
                .iter()
                .map(|class| {
                    let rendered = format!("\\{class}");
                    let prose = super::prose::throws_prose(&rendered);
                    (rendered, prose)
                })
                .collect()
        };
        for entry in &thrown {
            if !(at_a_throw(&entry.sites) || (at_name && total == 1)) {
                continue;
            }
            let class = &entry.class;
            let Some(edits) = tag_edits(view, docblock, &[("throws", throws_written(&[class]))])
            else {
                continue;
            };
            out.push(CodeAction {
                title: format!("Add @throws \\{class}"),
                edits,
                preferred: total == 1,
                kind: ActionKind::QuickFix,
                fixes: Some(DOC_MISSING_THROWS),
            });
        }

        // Everything at once, for the declaration a constructor promotion turns into seven
        // parameters and one line of prose, and for the signature that grew a parameter and a
        // return type since its docblock was written. One action rather than one per tag: they
        // all go to the same docblock, and somebody documenting a whole signature is not standing
        // on any one part of it -- so it is offered from the name and from any missing part.
        if total >= 2 && (at_name || at_a_missing_parameter || at_an_undocumented_throw) {
            let mut groups: Vec<(&str, Vec<(String, String)>)> = Vec::new();
            if !missing.is_empty() {
                groups.push((
                    "param",
                    missing
                        .iter()
                        .map(|(_, subject, prose)| (subject.clone(), prose.clone()))
                        .collect(),
                ));
            }
            if let Some((returns, prose)) = &returned {
                groups.push(("return", vec![(returns.clone(), prose.clone())]));
            }
            if !thrown.is_empty() {
                let classes: Vec<&phrost_model::Name> =
                    thrown.iter().map(|entry| &entry.class).collect();
                groups.push(("throws", throws_written(&classes)));
            }
            if let Some(edits) = tag_edits(view, docblock, &groups) {
                out.push(CodeAction {
                    title: "Update the docblock".to_owned(),
                    edits,
                    // On the name it is the fix a keystroke should take; on a parameter or a throw
                    // the action naming that one is.
                    preferred: at_name,
                    kind: ActionKind::QuickFix,
                    // The tags rule where any tag is missing; only a docblock short of nothing but
                    // its throws is the throws rule's alone.
                    fixes: Some(match missing.is_empty() && returned.is_none() {
                        true => DOC_MISSING_THROWS,
                        false => DOC_MISSING_TAGS,
                    }),
                });
            }
        }
    }
}

/// What a declaration's parameters and return are typed as, worked out at most once.
///
/// Three states and not an `Option<Option<_>>`: "not asked yet" and "asked, and the index had no
/// answer" lead to opposite decisions, and a shape that spells them the same way is one somebody
/// collapses by accident.
enum Typed {
    Unasked,
    Answered(Documentable),
    Unanswerable,
}

impl Typed {
    fn of(
        &mut self,
        view: &IndexView<'_>,
        declaration: &DocumentedDeclaration,
    ) -> Option<&Documentable> {
        if matches!(self, Self::Unasked) {
            *self = match view.documentable_at(declaration.name_range.start()) {
                Some(found) => Self::Answered(found),
                None => Self::Unanswerable,
            };
        }
        match self {
            Self::Answered(found) => Some(found),
            Self::Unasked | Self::Unanswerable => None,
        }
    }
}

/// The edits that insert new tags, each group in the place PHPDoc's reading order puts it.
///
/// `groups` is one entry per tag kind, in reading order, each with the lines to write. Two groups
/// whose placements coincide -- every missing `@param` and the `@return` both go at the end of the
/// tags, or both go above the same `@throws` -- are written as **one** edit, params then return,
/// with one separator between them. Two inserts at one offset would leave their order to whoever
/// applies them, and an applier working back to front puts the second first; and each would bring a
/// separator of its own, which in Drupal's style is a blank line doubled.
///
/// Refuses unless the closing line is bare: a last line carrying content as well as the closer
/// is a shape this cannot extend without guessing which part of it the new line goes after, and a
/// one-line docblock (`/** Text */`) has no closing line of its own to insert before at all.
fn tag_edits(
    view: &IndexView<'_>,
    docblock: &Docblock,
    groups: &[(&str, Vec<(String, String)>)],
) -> Option<Vec<TextEdit>> {
    let close_at = docblock.text.rfind('\n')?;
    let closing_line = &docblock.text[close_at + 1..];
    if closing_line.trim() != "*/" {
        return None;
    }
    // The whitespace the closing line is indented by is exactly what every other line in the
    // block is indented by too, so reusing it lines the new tag up without having to know the
    // declaration's own indentation separately.
    let indent: String = closing_line
        .chars()
        .take_while(|ch| ch.is_whitespace())
        .collect();

    // `DocStyle::named` is private to `document`, so the house style is read the same way it
    // reads it, rather than through a method this module cannot call.
    let style = match view.house_style() {
        Some("drupal") => DocStyle::Drupal,
        _ => DocStyle::Plain,
    };
    let drupal = matches!(style, DocStyle::Drupal);

    let mut edits: Vec<TextEdit> = Vec::new();
    // The insertion the last group went into, so the next one can join it if it lands there too.
    let mut open: Option<(usize, String, Placement)> = None;
    for (tag, written) in groups {
        let mut lines = String::new();
        for (subject, prose) in written {
            // The same writer `WriteDocblock` uses, which is what keeps a tag added to an existing
            // block laid out like the ones a generated block holds -- including the wrap a long
            // shape needs.
            lines.push_str(&super::document::tag(&indent, style, tag, subject, prose));
        }

        let placement = placement(&docblock.text, tag);
        let (Placement::After { at, .. } | Placement::Before(at) | Placement::AtTheEnd(at)) =
            placement;

        match &mut open {
            // The group before this one is being inserted at the same place: this one follows it,
            // and starts a group of its own, so Drupal's style separates the two.
            Some((open_at, text, _)) if *open_at == at => {
                if drupal {
                    let _ = writeln!(text, "{indent}*");
                }
                text.push_str(&lines);
            }
            _ => {
                if let Some(done) = open.take() {
                    edits.push(finish(docblock, &indent, drupal, done)?);
                }
                // A tag that starts a group needs the blank line that keeps a group apart from its
                // neighbours, and one that joins a group must not have one -- two `@param` lines
                // belong together. Only Drupal's style separates them at all.
                let mut text = String::new();
                let separates =
                    drupal && !matches!(placement, Placement::After { joins: true, .. });
                if separates && !blank_above(&docblock.text, at) {
                    let _ = writeln!(text, "{indent}*");
                }
                text.push_str(&lines);
                open = Some((at, text, placement));
            }
        }
    }
    if let Some(done) = open {
        edits.push(finish(docblock, &indent, drupal, done)?);
    }
    Some(edits)
}

/// One insertion, closed: a group written above a tag that reads after it is kept apart from that
/// tag by a blank line, in Drupal's style.
fn finish(
    docblock: &Docblock,
    indent: &str,
    drupal: bool,
    (at, mut text, placement): (usize, String, Placement),
) -> Option<TextEdit> {
    if drupal && matches!(placement, Placement::Before(_)) {
        let _ = writeln!(text, "{indent}*");
    }
    let relative = TextSize::try_from(at).ok()?;
    Some(TextEdit::insert(docblock.range.start() + relative, text))
}

/// Whether the line above an insertion point carries nothing but its `*`.
fn blank_above(text: &str, at: usize) -> bool {
    let before = text[..at].trim_end_matches('\n');
    is_blank(before.rsplit('\n').next().unwrap_or(before))
}

/// Where a new tag line goes.
///
/// PHPDoc's order is the reading order, and every tool that renders a docblock renders the tags in
/// the order it finds them -- so a tag written before the closing `*/`, which is where every one of
/// them went, is valid and reads wrong the moment the docblock has a tag that outranks it. The
/// reported one was a `@param` landing under a `@throws`.
///
/// Nothing already written moves: the line joins the group it belongs to, or goes above the first
/// group that reads after it, so a docblock whose own tags are out of order keeps them exactly as
/// their author left them. That is this module's rule, and the reason this is a placement rather
/// than a sort.
enum Placement {
    /// At the end of the last group that reads at or before this tag -- past its description lines
    /// and above the blank line that separates it from the next, which is where a second `@param`
    /// belongs.
    ///
    /// `joins` is whether that group is this tag's own kind. A second `@param` joins the first and
    /// must not be separated from it; a `@return` written after them starts a group of its own and
    /// must be. The difference is invisible in the placement and decides the blank line.
    After { at: usize, joins: bool },
    /// Above the first tag that reads after this one, for a docblock with no such group to join.
    Before(usize),
    /// Before the closing `*/`, for a docblock with no tag to sort against.
    AtTheEnd(usize),
}

fn placement(text: &str, tag: &str) -> Placement {
    let rank = crate::docblock::reading_rank(tag);
    let mut group_ends: Option<usize> = None;
    let mut group_rank: Option<u8> = None;
    let mut at = 0;

    for line in text.split_inclusive('\n') {
        // The closer ends every group, and is where a tag with nothing to sort against goes. The
        // caller has already refused a closing line carrying anything else.
        if line.trim() == "*/" {
            break;
        }
        if let Some(name) = opens_a_tag(line) {
            let opened = crate::docblock::reading_rank(name);
            if opened > rank {
                return after(group_ends, group_rank, rank).unwrap_or(Placement::Before(at));
            }
            group_rank = Some(opened);
        }
        // The last line with something on it: a group ends at its own text, not at the blank line
        // under it, or the tag would be inserted below a separator and the one it separates would
        // lose it.
        if group_rank.is_some() && !is_blank(line) {
            group_ends = Some(at + line.len());
        }
        at += line.len();
    }

    // Everything above is prose, or the block is empty.
    after(group_ends, group_rank, rank).unwrap_or(Placement::AtTheEnd(at))
}

fn after(ends: Option<usize>, group: Option<u8>, rank: u8) -> Option<Placement> {
    Some(Placement::After {
        at: ends?,
        joins: group? == rank,
    })
}

/// Whether a docblock line carries nothing but its `*`.
fn is_blank(line: &str) -> bool {
    let text = line.trim();
    text.is_empty() || text == "*"
}

/// The tag a line of a docblock opens, when it opens one.
///
/// Not a parser: a docblock line is `*` and then whatever the author wrote, and the one thing read
/// here is whether the first word carries the `@`. An inline `{@inheritdoc}` opens with a brace and
/// so is not one, which is right -- it is prose, and a tag inserted before it would land in the
/// middle of a sentence.
fn opens_a_tag(line: &str) -> Option<&str> {
    let text = line.trim_start();
    let text = text.strip_prefix("/**").unwrap_or(text).trim_start();
    let text = text.strip_prefix('*').unwrap_or(text).trim_start();
    let name = text.strip_prefix('@')?;
    Some(name.split_whitespace().next().unwrap_or(name))
}

/// Every `$name` a `@param` names, without the sigils a call adds -- the same reading
/// `diagnostics::docblocks` gives it, so the two agree about which parameters still need one.
fn subjects(tags: &[Tag]) -> Vec<&str> {
    tags.iter()
        .filter(|tag| is_param(tag))
        .filter_map(|tag| tag.subject.as_deref())
        .map(|subject| {
            subject
                .trim_start_matches("...")
                .trim_start_matches('&')
                .trim_start_matches('$')
        })
        .collect()
}

fn is_param(tag: &Tag) -> bool {
    matches!(tag.name.rsplit('-').next(), Some("param" | "param-out"))
        || tag.name.ends_with("param")
        || crate::diagnostics::misspelt_tags::meant_on_a_function(&tag.name) == Some("param")
}

fn is_return(tag: &Tag) -> bool {
    tag.name.ends_with("return")
        || crate::diagnostics::misspelt_tags::meant_on_a_function(&tag.name) == Some("return")
}
