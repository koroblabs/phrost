use mago_span::Span;
use phrost_model::{TextRange, TextSize};

use crate::text_range;

/// One docblock comment, with the text as written.
///
/// The delimiters are kept so that offsets inside the comment stay relative to `range`, which is
/// what lets a type parsed out of a docblock report a position the editor can navigate to.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Docblock<'a> {
    pub text: &'a str,
    pub range: TextRange,
}

/// Docblocks in one file, ordered by position, ready to be matched to declarations.
///
/// Attaching a docblock is not something the tree does: a comment is trivia, so it has no parent
/// node. The rule PHP tooling has settled on is "the docblock immediately above, with nothing but
/// whitespace between", and that needs the source text as well as the comment positions — so it
/// lives here rather than being re-derived by each caller.
#[derive(Debug, Default)]
pub(crate) struct DocblockIndex<'a> {
    /// Sorted by end offset, which is the order mago emits trivia in.
    blocks: Vec<Docblock<'a>>,
}

impl<'a> DocblockIndex<'a> {
    pub(crate) fn build(source: &'a str, trivia: impl Iterator<Item = (Span, &'a [u8])>) -> Self {
        let mut blocks: Vec<Docblock<'a>> = trivia
            .map(|(span, _)| {
                let range = text_range(span);
                Docblock {
                    text: slice(source, range),
                    range,
                }
            })
            .collect();

        blocks.sort_by_key(|block| block.range.end());
        Self { blocks }
    }

    /// Every docblock in the file, in position order.
    ///
    /// What a whole-file reader wants, as opposed to [`Self::preceding`], which answers about one
    /// declaration. Colouring what a docblock says needs all of them and needs none of them
    /// attached: a `@param` reads the same whether or not anything is declared below it.
    pub(crate) fn all(&self) -> &[Docblock<'a>] {
        &self.blocks
    }

    /// The docblock attached to a declaration starting at `decl_start`.
    ///
    /// `decl_start` must be the very first byte of the declaration including its attributes:
    /// `#[Attr]` sits between the docblock and the keyword, so measuring from the keyword would
    /// find nothing on every attributed declaration.
    pub(crate) fn preceding(&self, source: &'a str, decl_start: TextSize) -> Option<Docblock<'a>> {
        let candidate = *self.blocks.get(
            self.blocks
                .partition_point(|block| block.range.end() <= decl_start)
                .checked_sub(1)?,
        )?;

        // Anything other than whitespace and line comments in the gap means the block documents
        // something else -- a preceding statement, or nothing at all.
        let gap = source
            .get(usize::from(candidate.range.end())..usize::from(decl_start))
            .unwrap_or_default();

        is_only_a_gap(gap).then_some(candidate)
    }
}

/// Whether nothing between a docblock and a declaration separates the two.
///
/// Whitespace, and **line comments**, which is PHP's own rule: the compiler keeps the last doc
/// comment it saw and an ordinary comment does not clear it, so `getDocComment()` answers across
/// one. Drupal core relies on it -- a property documented `@var \Drupal\Core\Entity\EntityInterface`
/// with a `// phpcs:ignore` line under the block is how `ResultRow::$_entity` is written, and 215
/// properties on one checkout are written that way. Read as a separator, every one of them lost its
/// type.
///
/// A `#` that opens an attribute is not a comment and is left to reject, which keeps the answer the
/// one it has always been where a declaration's own range does not already cover its attributes.
fn is_only_a_gap(gap: &str) -> bool {
    let mut rest = gap;
    loop {
        rest = rest.trim_start();
        let line_comment = match rest.as_bytes() {
            [b'/', b'/', ..] => true,
            [b'#', next, ..] if *next != b'[' => true,
            [b'#'] => true,
            _ => false,
        };
        if !line_comment {
            return rest.is_empty();
        }
        rest = match rest.find('\n') {
            Some(end) => &rest[end..],
            // A comment running to the end of the gap: the declaration is on the same line as it,
            // which is not a shape PHP parses -- so nothing is left and nothing separates them.
            None => return true,
        };
    }
}

fn slice(source: &str, range: TextRange) -> &str {
    source
        .get(usize::from(range.start())..usize::from(range.end()))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::is_only_a_gap;

    /// PHP keeps the last doc comment it saw and an ordinary comment does not clear it, so a
    /// `// phpcs:ignore` between a block and what it documents separates nothing. Drupal core
    /// writes `ResultRow::$_entity` exactly that way, and 215 properties on one checkout do.
    #[test]
    fn a_line_comment_does_not_separate_a_docblock_from_what_it_documents() {
        for gap in [
            "\n",
            "",
            "\n    // phpcs:ignore Drupal.Classes.PropertyDeclaration\n    ",
            "\n    # a hash comment\n",
            "\n  // one\n  // and another\n  ",
            // A comment with nothing after it: the declaration would be on the same line, which
            // PHP does not parse, so there is nothing left between them.
            "\n    // to the end",
        ] {
            assert!(is_only_a_gap(gap), "read {gap:?} as a separator");
        }

        // Anything that is code, and an attribute -- `#[` is not a comment, and where a
        // declaration's own range does not already cover it the answer stays the one it was.
        for gap in ["\n $x = 1;\n", "\n#[Attribute]\n", "\n/* a block */\n", "x"] {
            assert!(!is_only_a_gap(gap), "read {gap:?} as a gap");
        }
    }
}
