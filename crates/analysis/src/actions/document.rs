//! Writing the docblock a declaration does not have.
//!
//! Everything in it comes from the declaration itself: the parameters, the type each takes, what it
//! returns. Nothing is invented, and the one thing a generator cannot supply -- what the code is
//! *for* -- is left as a line to overwrite rather than filled with something that reads like an
//! answer.
//!
//! # The layout is a house style
//!
//! Drupal writes a `@param` and then its description indented on the next line; most other PHP puts
//! the description on the same line. That is not a preference this layer gets to have, so it comes
//! from the same seam that already decides how the file is formatted -- and where nothing says, the
//! plain layout is written, because it is the one a Drupal file would also accept without the
//! standard's own checks running.

use phrost_view::{Documentable, DocumentedParam};

use super::{ActionKind, ActionSubject, CodeAction, Offer, SubjectSites, TextEdit};
use crate::diagnostics::docblocks::DOC_MISSING_TAGS;

pub(super) struct WriteDocblock;

impl Offer for WriteDocblock {
    fn kind(&self) -> ActionKind {
        ActionKind::QuickFix
    }

    fn subject(&self) -> ActionSubject {
        ActionSubject::Declaration
    }

    fn run(&self, sites: &SubjectSites<'_, '_>, out: &mut Vec<CodeAction>) {
        let (view, at) = (sites.view(), sites.at());
        let Some(found) = sites.documentable() else {
            return;
        };
        // Offering a second one would be offering to break the file.
        if found.documented {
            return;
        }
        // A framework may print the whole docblock at somebody. Drupal's update descriptions are
        // the case, and there the block is worth *more* rather than less -- a hook with none gets
        // an empty line beside it on `update.php` -- but everything below the summary is read out
        // as part of the sentence, so nothing below the summary may be written.
        let published = view.user_facing_docblock(found.name.as_str());
        out.push(CodeAction {
            title: "Write a docblock".to_owned(),
            edits: vec![TextEdit::insert(
                found.at,
                written(found, DocStyle::named(view.house_style()), published),
            )],
            // Only when it really is the only answer. A method that overrides something has
            // `InheritDocblock` standing beside it, and both are correct -- so marking one
            // preferred would make a keystroke choose between two things a person meant to read.
            preferred: !inherits_a_docblock(view, at, found),
            kind: ActionKind::QuickFix,
            fixes: Some(DOC_MISSING_TAGS),
        });
    }
}

/// How a project lays a docblock out.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DocStyle {
    /// `@param int $id  The id.` -- the description beside the tag.
    Plain,
    /// Drupal's: the description on the next line, indented two spaces past the star, and a blank
    /// line between the parameters and the return.
    Drupal,
}

impl DocStyle {
    /// The style a house-style name asks for.
    ///
    /// Anything unrecognised gets the plain layout, which a Drupal file also accepts -- so a project
    /// naming a style nothing here knows gets a docblock rather than none.
    fn named(style: Option<&str>) -> Self {
        match style {
            Some("drupal") => Self::Drupal,
            _ => Self::Plain,
        }
    }
}

/// The text of the docblock, indented to match what it describes.
///
/// `published` is the one thing here that is not derived from the declaration: a docblock a
/// framework shows to a person is the summary and nothing else, and the summary is the line to
/// overwrite rather than one derived from the name. `wh_item_snapshot_update_8001` yields "Lvs item
/// snapshot update 8001." -- fine as a suggestion in an editor, and not fine as the description
/// beside a pending migration on a production database.
fn written(found: &Documentable, style: DocStyle, published: bool) -> String {
    let indent = &found.indent;
    let summary = match published {
        true => super::prose::PLACEHOLDER.to_owned(),
        false => super::prose::summary_of(found.name.as_str()),
    };
    // Everything before the `*` of every line, which is what `tag` lays a line out against.
    let star = format!("{indent} ");
    let mut out = format!("/**\n{indent} * {summary}\n");

    if published {
        out.push_str(&format!("{indent} */\n{indent}"));
        return out;
    }

    if !found.params.is_empty() || found.returns.is_some() || !found.throws.is_empty() {
        out.push_str(&format!("{indent} *\n"));
    }
    for param in &found.params {
        out.push_str(&tag(
            &star,
            style,
            "param",
            &param_text(param),
            &super::prose::param_prose(param.name.as_str()),
        ));
    }
    if let Some(returns) = &found.returns {
        // Drupal puts a blank line between the two groups; the plain layout runs them together.
        if style == DocStyle::Drupal && !found.params.is_empty() {
            out.push_str(&format!("{indent} *\n"));
        }
        out.push_str(&tag(
            &star,
            style,
            "return",
            returns,
            &super::prose::return_prose(returns),
        ));
    }
    // What the body throws, a group of its own after the return -- PHPDoc's reading order, which
    // `reading_rank` also states -- and separated the way the return is.
    if !found.throws.is_empty() {
        if style == DocStyle::Drupal && (!found.params.is_empty() || found.returns.is_some()) {
            out.push_str(&format!("{indent} *\n"));
        }
        for thrown in &found.throws {
            let rendered = format!("\\{thrown}");
            out.push_str(&tag(
                &star,
                style,
                "throws",
                &rendered,
                &super::prose::throws_prose(&rendered),
            ));
        }
    }

    out.push_str(&format!("{indent} */\n{indent}"));
    out
}

/// `\App\Repo $id` -- the type and the name a `@param` names, with the `$` a docblock writes.
///
/// `mixed` for a parameter nothing declares a type for, because a tag has to say something and
/// inventing a type would be inventing a claim.
fn param_text(param: &DocumentedParam) -> String {
    format!("{} ${}", param.ty.as_deref().unwrap_or("mixed"), param.name)
}

/// The column a generated docblock line stays inside.
///
/// Drupal's own sniff warns past 80 and no PHP standard puts the soft limit higher, so a tag that
/// crosses it is one the project's own program will complain about. It is a *soft* limit here in
/// the one direction that matters: a type with no place to break stays on its line rather than
/// being cut somewhere that changes what it says.
const WIDTH: usize = 80;

/// One tag, laid out the way the style asks.
///
/// `prefix` is everything before the `*`, which is the one thing the two writers of a tag line
/// spell differently -- [`written`] has the declaration's indentation and `doc_tags` reads the
/// closing line's. Everything after it is this function's, because a layout decided in two places
/// is a layout that disagrees with itself.
pub(super) fn tag(prefix: &str, style: DocStyle, name: &str, subject: &str, prose: &str) -> String {
    let opening = format!("{prefix}* @{name} ");
    let written = written_type(prefix, opening.len(), subject);
    if written.contains('\n') {
        // The description always goes on its own line under a wrapped type, in both styles: after
        // the closing `}` it reads as part of the type rather than as prose about it.
        return format!("{opening}{written}\n{prefix}*   {prose}\n");
    }
    match style {
        DocStyle::Plain => format!("{opening}{written} {prose}\n"),
        DocStyle::Drupal => format!("{opening}{written}\n{prefix}*   {prose}\n"),
    }
}

/// The type as a tag writes it: on one line, or one field to a line when it would not fit.
///
/// `column` is where the type starts, which is the tag's own opening -- the same type is wrapped
/// under `@return` and not under a shorter tag, and that is right rather than inconsistent.
///
/// Wrapped only where there is a place to break that means the same thing. A shape's fields are
/// that place; a long union's operands are not -- `A|B` broken across lines is a continuation
/// whose leading `|` no reader of ours has been taught.
pub(super) fn written_type(prefix: &str, column: usize, subject: &str) -> String {
    if column + subject.len() <= WIDTH {
        return subject.to_owned();
    }
    wrapped_shape(prefix, subject).unwrap_or_else(|| subject.to_owned())
}

/// `array{a: int, b: string}` written one field to a line, or nothing when the type is not a shape.
///
/// Only a type that is a shape *whole* -- optionally behind a `?` -- because those are the two
/// spellings whose fields are the top level. A shape inside a union has the union's operands
/// between it and the margin, and breaking at its fields would leave the operands unbroken beside
/// them, which reads as one long line with a hole in it.
///
/// The trailing comma is deliberate: PHPStan reads it, mago reads it, and it makes adding a key a
/// one-line diff rather than a two-line one.
fn wrapped_shape(prefix: &str, subject: &str) -> Option<String> {
    let head = subject.strip_prefix('?').unwrap_or(subject);
    let body = head.strip_prefix("array{")?.strip_suffix('}')?;
    let mut out = format!(
        "{}array{{\n",
        if subject.starts_with('?') { "?" } else { "" }
    );
    for field in top_level_fields(body) {
        out.push_str(&format!("{prefix}*   {field},\n"));
    }
    out.push_str(&format!("{prefix}* }}"));
    Some(out)
}

/// A shape's fields, split at the commas that separate them and not at the ones inside a field.
///
/// `array{rows: array<int, string>, when: array{at: int}}` has two fields and four commas. Depth
/// is counted over the three brackets a docblock type can nest -- a quoted key cannot hold one,
/// because [`phrost_ty::shape_key`] only quotes what a PHP array key can be spelled with.
fn top_level_fields(body: &str) -> Vec<&str> {
    let mut fields = Vec::new();
    let mut depth = 0usize;
    let mut start = 0usize;
    for (at, ch) in body.char_indices() {
        match ch {
            '{' | '<' | '(' => depth += 1,
            '}' | '>' | ')' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                fields.push(body[start..at].trim());
                start = at + 1;
            }
            _ => {}
        }
    }
    let last = body[start..].trim();
    if !last.is_empty() {
        fields.push(last);
    }
    fields
}

/// Whether `{@inheritdoc}` is an answer here too.
///
/// The same question `InheritDocblock` asks, and asked here for one reason: whether *this* offer is
/// the only one. Two offers cannot both be preferred, and a keystroke that picks between them picks
/// at random.
fn inherits_a_docblock(
    view: &phrost_view::IndexView<'_>,
    at: phrost_model::TextRange,
    found: &phrost_view::Documentable,
) -> bool {
    view.enclosing_class(at.start()).is_some_and(|owner| {
        view.overridden_member(
            owner.as_str(),
            phrost_view::MemberKind::Method,
            found.name.as_str(),
        )
        .is_some()
    })
}
