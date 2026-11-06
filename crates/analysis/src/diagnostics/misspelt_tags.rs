//! A docblock tag that no tool reads, written where it plainly means one that every tool does.
//!
//! `@yield \Drupal\...\Application` above a property, `@yield Segment[]` above a method: the author
//! meant `@var` and `@return`, and neither phrost nor PHPStan nor an editor reads `@yield` at all --
//! so the type written there is simply gone. On the reference project five such tags left a
//! property and four methods untyped, and every access through them with it: twenty-eight reads of
//! one property alone.
//!
//! Read for what a tag *means*, not guessed into the index. A checker that silently took `@yield`
//! for `@return` would answer where every other tool does not, and the fix -- one word -- is the
//! author's to make; so this reports it, and the code action makes it.
//!
//! Only spellings with one reading. `@returns` and `@params` are the plural a person writes out of
//! habit from another language's docs; `@retrun` is a typo; `@yield` is not a tag in any standard
//! and a type beside it is a claim about the value. A tag a framework invents -- Drupal's
//! `@FieldType`, PHPUnit's `@dataProvider`, anybody's `@internal` -- is not one of these and is
//! never reported.

use phrost_model::{TextRange, TextSize};
use phrost_view::{Docblock, IndexView};

use super::{Check, Diagnostic, Severity};

pub(crate) const DOC_MISSPELT_TAG: &str = "doc-misspelt-tag";

pub(super) struct MisspeltDocTag;

/// One tag written under a name no tool reads, and the name that says what it meant.
#[derive(Clone, PartialEq, Eq, Debug)]
pub(crate) struct MisspeltTag {
    /// The tag's name as written, `@` excluded -- what an edit replaces.
    pub(crate) range: TextRange,
    pub(crate) written: String,
    pub(crate) meant: &'static str,
    /// The meant tag's own block, when the docblock already writes one that says less -- the
    /// generated `@return array` / `The array.` beside `@yield Segment[]`. Whole lines, description
    /// and the blank line after it included, so deleting it leaves the docblock as a person would
    /// have written it.
    pub(crate) replaces: Option<TextRange>,
}

/// One line of a docblock that opens with a tag.
struct TagLine<'t> {
    /// Where the tag's name starts in the text, past the `@`.
    name_at: usize,
    name: &'t str,
    /// What follows the name on its line, trimmed.
    rest: &'t str,
    /// Where the line starts in the text.
    line_start: usize,
}

/// Types that say nothing a written element type would not say better.
fn says_little(rest: &str) -> bool {
    let ty = rest.split_whitespace().next().unwrap_or("");
    ty.is_empty()
        || [
            "array",
            "mixed",
            "object",
            "iterable",
            "?array",
            "array|null",
            "null|array",
        ]
        .iter()
        .any(|weak| ty.eq_ignore_ascii_case(weak))
}

/// What a tag written on a function or method means, when it is one of the spellings with a reading.
///
/// Asked by the rules that look for a `@return` or a `@param` too, so a misspelt one counts as the
/// tag it means: `@yield Segment[]` is this rule's finding, and "has no `@return`" beside it would
/// be a second report of the same word -- and a fix-all taking both would write the tag twice.
pub(crate) fn meant_on_a_function(written: &str) -> Option<&'static str> {
    match written.to_ascii_lowercase().as_str() {
        "yield" | "yields" | "returns" | "retrun" | "reutrn" | "retun" | "retrurn" | "rerturn"
        | "retunr" => Some("return"),
        "params" | "parm" | "pram" | "parma" | "parmas" => Some("param"),
        _ => None,
    }
}

/// The same, on a property: whatever type it names is the property's, which is `@var`.
fn meant_on_a_property(written: &str) -> Option<&'static str> {
    match written.to_ascii_lowercase().as_str() {
        "yield" | "yields" | "return" | "returns" => Some("var"),
        _ => None,
    }
}

impl MisspeltTag {
    /// Every such tag in the open file.
    pub(crate) fn in_file(view: &IndexView<'_>) -> Vec<MisspeltTag> {
        let mut found = Vec::new();
        for declaration in view.documented_declarations() {
            if let Some(docblock) = &declaration.docblock {
                Self::in_docblock(docblock, meant_on_a_function, &mut found);
            }
        }
        for property in view.documented_properties() {
            if let Some(docblock) = &property.docblock {
                Self::in_docblock(docblock, meant_on_a_property, &mut found);
            }
        }
        found.sort_by_key(|tag| tag.range.start());
        found.dedup();
        found
    }

    /// The tags of one docblock that `meant` has a reading for -- unless the docblock already
    /// writes the tag they mean, where the misspelt one is a second claim and not the only one.
    fn in_docblock(
        docblock: &Docblock,
        meant: fn(&str) -> Option<&'static str>,
        out: &mut Vec<MisspeltTag>,
    ) {
        let text = &docblock.text;
        let tags = Self::tags(text);
        for tag in &tags {
            let Some(meant) = meant(tag.name) else {
                continue;
            };
            // The meant tag written as well: only a finding where it says less than this one does,
            // and then its block is what the fix takes out.
            let already = tags
                .iter()
                .position(|other| other.name.eq_ignore_ascii_case(meant));
            let replaces = match already {
                None => None,
                Some(index) if says_little(tags[index].rest) && !says_little(tag.rest) => {
                    Self::block_of(text, &tags, index).map(|(from, to)| {
                        let base = docblock.range.start();
                        TextRange::new(base + from, base + to)
                    })
                }
                Some(_) => continue,
            };
            let (Ok(start), Ok(len)) = (
                TextSize::try_from(tag.name_at),
                TextSize::try_from(tag.name.len()),
            ) else {
                continue;
            };
            let start = docblock.range.start() + start;
            out.push(MisspeltTag {
                range: TextRange::new(start, start + len),
                written: tag.name.to_owned(),
                meant,
                replaces,
            });
        }
    }

    /// The lines of one tag's block: its own line through the line before the next tag, or through
    /// its last description line where it is the last tag -- the blank line before it taken
    /// instead, so no two blank lines are left together.
    fn block_of(text: &str, tags: &[TagLine<'_>], index: usize) -> Option<(TextSize, TextSize)> {
        let from = tags[index].line_start;
        let (from, to) = match tags.get(index + 1) {
            Some(next) => (from, next.line_start),
            None => {
                // Up to the line holding the closing `*/`, and back over a blank line above.
                let close = text.rfind("*/")?;
                let to = text[..close].rfind('\n').map_or(close, |at| at + 1);
                let before = text[..from].trim_end_matches([' ', '\t']);
                let blank_above = before
                    .strip_suffix('\n')
                    .and_then(|above| above.rfind('\n').map(|at| &above[at + 1..]))
                    .is_some_and(|line| line.trim() == "*");
                let from = match blank_above {
                    true => before[..before.len() - 1]
                        .rfind('\n')
                        .map_or(from, |at| at + 1),
                    false => from,
                };
                (from, to)
            }
        };
        Some((TextSize::try_from(from).ok()?, TextSize::try_from(to).ok()?))
    }

    /// Every tag a docblock opens a line with.
    ///
    /// At the start of a line only -- after the `*` and the spaces -- because that is where a tag
    /// is; an `@` inside prose or an inline `{@link ...}` is not one.
    fn tags(text: &str) -> Vec<TagLine<'_>> {
        let mut tags = Vec::new();
        let mut line_start = 0;
        for line in text.split_inclusive('\n') {
            let trimmed =
                line.trim_start_matches(|c: char| c.is_whitespace() || c == '*' || c == '/');
            let lead = line.len() - trimmed.len();
            if let Some(rest) = trimmed.strip_prefix('@') {
                let name_len = rest
                    .find(|c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == '_'))
                    .unwrap_or(rest.len());
                if name_len > 0 {
                    tags.push(TagLine {
                        name_at: line_start + lead + 1,
                        name: &rest[..name_len],
                        rest: rest[name_len..].trim(),
                        line_start,
                    });
                }
            }
            line_start += line.len();
        }
        tags
    }
}

impl Check for MisspeltDocTag {
    fn run(&self, view: &IndexView<'_>, out: &mut Vec<Diagnostic>) {
        for tag in MisspeltTag::in_file(view) {
            out.push(Diagnostic {
                range: tag.range,
                severity: Severity::Warning,
                code: DOC_MISSPELT_TAG,
                message: match tag.replaces {
                    None => format!(
                        "@{} is not a tag any tool reads, so the type beside it is lost -- this means @{}",
                        tag.written, tag.meant
                    ),
                    Some(_) => format!(
                        "@{} is not a tag any tool reads, so the type beside it is lost -- the @{} it means says less",
                        tag.written, tag.meant
                    ),
                },
                unnecessary: false,
                deprecated: false,
            });
        }
    }
}
