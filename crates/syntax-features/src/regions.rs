//! Which language is written at an offset: PHP, or the text PHP copies to its output.
//!
//! A `.php` file is a template until `<?php` says otherwise, and a Drupal project's `.module`,
//! `.theme` and `.inc` files leave PHP and come back. Everything outside the tags is literal text
//! -- nothing there is a name, a call, or a keyword -- so a feature that answers as though it were
//! PHP answers about a file that does not exist.
//!
//! Asked of the lexer rather than by scanning for `<?php` and `?>`, and the difference is not
//! pedantry: `$x = '?>';` does not leave PHP and `// ?>` does, so a byte scan is wrong in both
//! directions and wrong silently. What keeps that affordable on the completion path is that a
//! file holding no `?>` at all cannot have left PHP, whatever else is in it -- so the lexer runs
//! only for a file that closes its tag, and the shortcut is checked against it rather than
//! trusted.

use mago_syntax::{lexer::Lexer, token::TokenKind};
use mago_syntax_core::input::Input;
use phrost_model::{TextRange, TextSize};

/// Whether PHP is what is being written at `offset`.
///
/// False in three places, and only the first is obvious. Outside the tags, plainly -- literal
/// text. *Within* an opening or closing tag, because the only thing being written there is the
/// tag: a caret at the end of a just-typed `<?php` has the three letters of `php` behind it, and a
/// feature reading them as a prefix offers every `PHP_*` constant in the standard library to
/// somebody who was opening a file. And inside a tag that is not finished yet, which the lexer
/// cannot see -- see [`opening_tag_typed`].
///
/// A file with no tag in it at all is literal text from its first byte, an empty one included.
/// That is what PHP does with it, and it is what makes the answer useful at the top of a new file.
pub fn writing_php(text: &str, offset: TextSize) -> bool {
    if opening_tag_typed(text, offset).is_some() {
        return false;
    }
    match settled_without_lexing(text, offset) {
        Some(settled) => settled,
        None => by_lexing(text, offset),
    }
}

/// The statement of the rule: what the tokens say, read in order.
///
/// Kept whole and reachable so [`settled_without_lexing`] can be checked against it rather than
/// argued about -- see `tests::the_shortcut_agrees_with_the_lexer_everywhere`.
fn by_lexing(text: &str, offset: TextSize) -> bool {
    let at = u32::from(offset);
    let mut lexer = Lexer::new(
        Input::new(mago_database::file::FileId::zero(), text.as_bytes()),
        mago_syntax::settings::LexerSettings::default(),
    );

    let mut php = false;
    while let Some(Ok(token)) = lexer.advance() {
        let start = token.start.offset;
        // A token beginning at the caret is after it: nothing of it has been typed yet.
        if start >= at {
            break;
        }
        let end = start + token.value.len() as u32;

        match token.kind {
            // Up to and including the tag's last character, the tag is what is being written.
            TokenKind::OpenTag | TokenKind::EchoTag | TokenKind::ShortOpenTag => {
                if at <= end {
                    return false;
                }
                php = true;
            }
            TokenKind::CloseTag => {
                if at <= end {
                    return false;
                }
                php = false;
            }
            TokenKind::InlineText => php = false,
            _ => php = true,
        }
    }

    php
}

/// The answer for a file that never leaves PHP, without lexing it.
///
/// Asked because this runs on the completion path, where lexing the file before the caret was
/// measured doubling the request -- p95 0.06 ms to 0.13 ms on a real project. The saving is
/// exact rather than a heuristic: **leaving PHP takes a `?>` token, and a `?>` token needs those
/// two bytes**, so a prefix holding neither is still in the first block whatever else is in it.
/// The same argument runs the other way for `<?`: before any tag the file is inline text, so the
/// first `<?` in it is the opening tag and nothing else can be.
///
/// `None` where those two bytes do appear, which is the one case the string-versus-comment
/// question is real and the lexer has to settle it. Modern PHP omits the closing tag entirely, so
/// on the reference project that is a handful of templates.
fn settled_without_lexing(text: &str, offset: TextSize) -> Option<bool> {
    // The whole file rather than the prefix, because a caret *inside* `?>` has only its first
    // byte behind it -- and asking the prefix put the second one out of reach, which read as a
    // file that never closes.
    if text.contains("?>") {
        return None;
    }
    let head = text.get(..usize::from(offset))?;
    let Some(tag) = head.find("<?") else {
        // No tag has been opened, so nothing before the caret is PHP.
        return Some(false);
    };

    let after = &head[tag + 2..];
    let length = match after {
        _ if after.len() >= 3 && after[..3].eq_ignore_ascii_case("php") => 5,
        _ if after.starts_with('=') => 3,
        _ => 2,
    };
    Some(head.len() > tag + length)
}

/// The `<?php` being typed at `offset`, whole, when one is: `<`, `<?`, `<?p`, `<?ph`.
///
/// The lexer cannot answer this and the reason is worth stating: `<?` is the short opening tag, so
/// `<?ph` lexes as a tag followed by an identifier and the caret lands *in PHP* one keystroke
/// before the tag it is part of exists. Every letter of `<?php` therefore reads as a name being
/// typed, which is the whole of the bug this file exists to stop.
///
/// The range runs from the `<`, because that is what an accepted `<?php` has to replace -- a
/// completion replacing only the letters would leave `<?<?php` in the file.
///
/// Nothing once the tag is finished: there is then nothing left to offer, and the caret is inside
/// a complete tag, which [`writing_php`] already answers from the token. Its limit is stated
/// rather than hidden -- a file opening with the short tag and naming something spelled like a
/// prefix of `php` gets no completion for those two characters. `<?p` meaning "echo `$p`" is not a
/// thing anybody writes, and short tags are deprecated besides.
pub fn opening_tag_typed(text: &str, offset: TextSize) -> Option<TextRange> {
    let head = text.get(..usize::from(offset))?;
    let typed = head.trim_end_matches(|letter: char| letter.is_ascii_alphabetic());
    let letters = &head[typed.len()..];

    let start = match letters.len() {
        0 if head.ends_with("<?") => head.len() - 2,
        0 if head.ends_with('<') => head.len() - 1,
        1 | 2 if typed.ends_with("<?") && "php".starts_with(&letters.to_ascii_lowercase()) => {
            typed.len() - 2
        }
        _ => return None,
    };
    Some(TextRange::new(
        TextSize::try_from(start).ok()?,
        TextSize::try_from(head.len()).ok()?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every position of a file at once, `p` where PHP is what is written.
    ///
    /// Rendered whole rather than asserted one position at a time, because what matters is where
    /// the answer *changes* and a test naming positions pins whichever boundary it was written
    /// against. One character per offset, so the last one is the caret at end of file.
    fn rendered(text: &str) -> String {
        (0..=text.len())
            .map(
                |at| match writing_php(text, TextSize::try_from(at).unwrap()) {
                    true => 'p',
                    false => '.',
                },
            )
            .collect()
    }

    /// The caret at the tag's own end is still in the tag -- that is the position a person is at
    /// the instant they finish typing `<?php`.
    #[test]
    fn the_opening_tag_is_not_php_and_what_follows_it_is() {
        assert_eq!(rendered("<?php $x;"), "......pppp");
    }

    #[test]
    fn inline_text_around_a_php_block_is_not_php() {
        assert_eq!(rendered("a<?php $x; ?>b"), ".......ppppp...");
    }

    /// The two cases a scan for `<?php` and `?>` gets wrong, in the two directions.
    #[test]
    fn a_close_tag_written_in_a_string_does_not_leave_php() {
        assert!(writing_php("<?php $x = '?>';", TextSize::new(15)));
    }

    #[test]
    fn a_close_tag_in_a_line_comment_does_leave_php() {
        assert!(!writing_php("<?php // ?>\nplain", TextSize::new(14)));
    }

    /// Every keystroke of the tag, including the one where the lexer starts reading PHP.
    #[test]
    fn a_tag_still_being_typed_is_not_php_at_any_of_its_letters() {
        for half in ["<", "<?", "<?p", "<?ph", "<?php", "<?PHP"] {
            assert!(
                !writing_php(half, TextSize::try_from(half.len()).unwrap()),
                "{half:?}"
            );
        }
    }

    /// A file with no tag is the template PHP treats it as, from the first byte.
    #[test]
    fn a_file_with_no_tag_is_text_throughout() {
        assert_eq!(rendered(""), ".");
        assert_eq!(rendered("name: Repo\n"), "............");
    }

    /// The half-written rule reads the letters, not the position, so it must not fire on a name
    /// that merely ends in one of them.
    #[test]
    fn a_name_spelled_like_the_tag_is_still_a_name() {
        let text = "<?php php_sapi_name(); $ph = 1;";
        assert!(writing_php(text, TextSize::new(14)), "inside the call");
        assert!(writing_php(text, TextSize::new(26)), "after `$ph`");
    }

    /// What an accepted `<?php` replaces: the whole of what has been typed of it.
    #[test]
    fn the_range_of_a_tag_being_typed_runs_from_its_opening_angle() {
        for (text, wanted) in [
            ("<", Some((0, 1))),
            ("<?", Some((0, 2))),
            ("<?p", Some((0, 3))),
            ("<?ph", Some((0, 4))),
            ("hello <", Some((6, 7))),
            ("<h", None),
            ("plain", None),
        ] {
            let at = TextSize::try_from(text.len()).unwrap();
            let found = opening_tag_typed(text, at).map(|range| {
                (
                    u32::from(range.start()) as usize,
                    u32::from(range.end()) as usize,
                )
            });
            assert_eq!(found, wanted, "{text:?}");
        }
    }

    /// The shortcut is an optimisation, so the lexer is the oracle and every offset is compared.
    ///
    /// Written as a sweep rather than as cases because the shortcut's whole claim is about what it
    /// may *skip*: a case list pins the ones somebody thought of, and the failure mode is the one
    /// they did not -- a caret straddling `?>`, found exactly this way.
    #[test]
    fn the_shortcut_agrees_with_the_lexer_everywhere() {
        for text in [
            "",
            "<?php $x;",
            "plain text with no tag at all",
            "a<?php $x; ?>b",
            "<?php $x = '?>'; $y;",
            "<?php // ?>\nplain<?php $z;",
            "<?= $x ?>tail",
            "<? $x; ?>\n<?php $y;",
            "<?php\n$s = <<<HTML\n?> not a tag\nHTML;\n$t;",
            "text\n<?php\n",
        ] {
            for at in 0..=text.len() {
                if !text.is_char_boundary(at) {
                    continue;
                }
                let offset = TextSize::try_from(at).unwrap();
                // The half-written rule is above both paths and is not what this compares.
                if opening_tag_typed(text, offset).is_some() {
                    continue;
                }
                assert_eq!(
                    writing_php(text, offset),
                    by_lexing(text, offset),
                    "{text:?} at {at}"
                );
            }
        }
    }

    /// A finished tag offers nothing: the caret is inside it, and there is nothing left to write.
    #[test]
    fn a_finished_tag_is_not_one_being_typed() {
        assert_eq!(opening_tag_typed("<?php", TextSize::new(5)), None);
        assert!(!writing_php("<?php", TextSize::new(5)));
    }
}
