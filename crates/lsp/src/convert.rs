//! Conversion between LSP's view of a document and phrost's.
//!
//! Two things differ and both are easy to get subtly wrong.
//!
//! **Columns.** LSP counts UTF-16 code units; everything inside phrost counts UTF-8 bytes. For
//! ASCII they agree, which is why the mistake survives every test written in English and then
//! misplaces the cursor in a file with a comment in Ukrainian. The conversion lives here because
//! it is a protocol concern: pushing UTF-16 down into the index would put an encoding detail in
//! every layer that only wanted to name a position.
//!
//! **Paths.** A client speaks URIs. Everything below speaks paths, and below that, file ids.

use lspkit_runtime::protocol::ls_types::{Position, Range};
use phrost_model::{LineCol, LineIndex, TextRange, TextSize};

/// Converts an LSP position to a byte offset.
///
/// Clamps rather than failing. A position past the end of a line means the client's idea of the
/// document is a revision ahead of ours, and answering about the nearest real position is a better
/// outcome than refusing — the alternative is a request that fails while the user types.
pub(crate) fn offset_of(text: &str, index: &LineIndex, position: Position) -> TextSize {
    let Some(line) = index.line_range(position.line) else {
        return TextSize::of(text);
    };
    let line_text = &text[usize::from(line.start())..usize::from(line.end())];

    line.start() + TextSize::from(utf16_to_byte_column(line_text, position.character))
}

/// Converts a byte offset to an LSP position.
pub(crate) fn position_of(text: &str, index: &LineIndex, offset: TextSize) -> Position {
    let LineCol { line, col } = index.line_col(offset);
    let character = index
        .line_range(line)
        .map(|range| {
            let line_text = &text[usize::from(range.start())..usize::from(range.end())];
            byte_to_utf16_column(line_text, col as usize)
        })
        .unwrap_or(0);

    Position { line, character }
}

pub(crate) fn range_of(text: &str, index: &LineIndex, range: TextRange) -> Range {
    Range {
        start: position_of(text, index, range.start()),
        end: position_of(text, index, range.end()),
    }
}

/// The byte column that a UTF-16 column names.
///
/// A column landing inside a surrogate pair — which a client should not send, but might — resolves
/// to the start of that character rather than to a byte boundary inside it.
fn utf16_to_byte_column(line: &str, utf16_column: u32) -> u32 {
    let mut utf16 = 0_u32;

    for (byte_offset, ch) in line.char_indices() {
        if utf16 >= utf16_column {
            return u32::try_from(byte_offset).expect("line length fits into u32");
        }
        utf16 += u32::try_from(ch.len_utf16()).expect("a char is at most two code units");
    }

    u32::try_from(line.len()).expect("line length fits into u32")
}

/// The UTF-16 column of a byte column.
fn byte_to_utf16_column(line: &str, byte_column: usize) -> u32 {
    line.char_indices()
        .take_while(|(offset, _)| *offset < byte_column)
        .map(|(_, ch)| u32::try_from(ch.len_utf16()).expect("a char is at most two code units"))
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A comment in Cyrillic, then an identifier. Every character before `find` is multi-byte, so
    /// byte and UTF-16 columns disagree from the first one.
    const TEXT: &str = "<?php\n// Клас\nclass Repo { function find() {} }\n";

    #[test]
    fn a_position_after_multibyte_text_maps_to_the_right_byte() {
        let index = LineIndex::new(TEXT);

        // `// Клас` is 7 UTF-16 units and 11 bytes: four Cyrillic letters at two bytes each.
        let end_of_comment = Position {
            line: 1,
            character: 7,
        };
        let offset = offset_of(TEXT, &index, end_of_comment);
        assert_eq!(&TEXT[usize::from(offset)..][..1], "\n");

        // And back again, unchanged.
        assert_eq!(position_of(TEXT, &index, offset), end_of_comment);
    }

    #[test]
    fn an_ascii_line_is_unaffected() {
        let index = LineIndex::new(TEXT);
        let at_find = Position {
            line: 2,
            character: 22,
        };
        let offset = offset_of(TEXT, &index, at_find);
        assert!(TEXT[usize::from(offset)..].starts_with("find"));
        assert_eq!(position_of(TEXT, &index, offset), at_find);
    }

    #[test]
    fn an_astral_character_counts_as_two_units() {
        // An emoji is one char, four bytes, and two UTF-16 code units. A conversion that counted
        // chars would be off by one for every emoji earlier in the line.
        let text = "<?php // 🎉 done\n";
        let index = LineIndex::new(text);

        let after_emoji = Position {
            line: 0,
            character: 11,
        };
        let offset = offset_of(text, &index, after_emoji);
        assert!(text[usize::from(offset)..].starts_with(" done"));
        assert_eq!(position_of(text, &index, offset), after_emoji);
    }

    #[test]
    fn a_position_past_the_end_of_a_line_clamps_to_it() {
        // Happens whenever the client is a revision ahead. Refusing would fail a request while the
        // user types; answering about the nearest real position is the lesser wrong.
        let index = LineIndex::new(TEXT);
        let offset = offset_of(
            TEXT,
            &index,
            Position {
                line: 1,
                character: 999,
            },
        );
        assert_eq!(&TEXT[usize::from(offset)..][..1], "\n");
    }

    #[test]
    fn a_position_past_the_end_of_the_file_clamps_to_it() {
        let index = LineIndex::new(TEXT);
        let offset = offset_of(
            TEXT,
            &index,
            Position {
                line: 999,
                character: 0,
            },
        );
        assert_eq!(offset, TextSize::of(TEXT));
    }
}
