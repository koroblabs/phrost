//! Rendering hover blocks as markdown.
//!
//! This is the only place markdown is produced, which is why `phrost-analysis` returns blocks
//! instead of a string: changing a fence language or the separator between sections is one edit
//! here, not a rewrite of every hover snapshot in the feature layer.
//!
//! The escaping is the part that matters. PHP names are full of backslashes, and `App\Repo` written
//! into markdown unescaped renders as `AppRepo` — the backslash is consumed as an escape for `R`.
//! An underscore in a name turns the rest of the line into italics. So text this crate generated is
//! escaped, and text an author wrote in a docblock is not.

use phrost_analysis::{HoverBlock, HoverInfo};

/// Renders hover content as markdown.
pub(crate) fn render_hover(info: &HoverInfo) -> String {
    render_blocks(&info.blocks)
}

/// The same blocks with no range around them, which is what a completion card is.
///
/// One renderer for both, because the two cards describe the same declarations: a list is built per
/// keystroke and a card for the one item somebody highlighted, and that is the only difference
/// between them that anybody chose. The other differences -- a tag list dumped into a fence where
/// hover labels it, an origin note the list never showed -- were nobody's decision.
pub(crate) fn render_blocks(blocks: &[HoverBlock]) -> String {
    let mut out = String::new();

    for block in blocks {
        if !out.is_empty() {
            out.push_str("\n\n");
        }

        match block {
            HoverBlock::Code(code) => {
                out.push_str("```php\n");
                out.push_str(code);
                out.push_str("\n```");
            }
            // The author's own markup. Passing it through is the point: a docblock that used a list
            // or a link should render as one.
            HoverBlock::Documentation(text) => out.push_str(text),
            HoverBlock::Note(text) => out.push_str(&escape(text)),
            // The label and the type are ours, so they are escaped and the type goes in a code
            // span; the description is the author's markup and goes through untouched.
            HoverBlock::Tag {
                label,
                ty,
                subject,
                description,
            } => {
                out.push_str(&format!("**{}**", escape(label)));
                // The type first, then the name it belongs to, which is the order PHP writes them:
                // `int|string $itemNumber`.
                //
                // Not escaped, because a code span is already literal -- markdown does no escape
                // processing inside one, so a backslash written there is a backslash *shown*, and
                // `\Drupal\app\_core` is exactly what that looks like. Which is what happened.
                if let Some(ty) = ty {
                    out.push_str(&format!(" `{ty}`"));
                }
                if let Some(subject) = subject {
                    out.push_str(&format!(" `{subject}`"));
                }
                if !description.is_empty() {
                    out.push_str(" — ");
                    out.push_str(description);
                }
            }
            HoverBlock::Separator => out.push_str("---"),
        }
    }

    out
}

/// Escapes the characters that can change how a line renders.
///
/// CommonMark permits a backslash before any ASCII punctuation, so escaping everything would be
/// valid — and it is what this did first. The problem is the other direction: a client whose
/// markdown support is incomplete shows the backslash, and `src\/Repo\.php` is worse than the
/// problem it was solving. So the set is the characters that actually matter mid-line:
///
/// - `\` — the important one. PHP names are full of it, and `App\Repo` unescaped renders as
///   `AppRepo` because the backslash is consumed as an escape for `R`.
/// - `` ` `` — would open a code span.
/// - `*` and `_` — emphasis. `Repo_Base` italicises the rest of the line.
/// - `[` and `]` — link syntax.
/// - `<` and `&` — raw HTML and entities.
/// - `~` — strikethrough, where the client supports it.
///
/// Line-start constructs — a leading `-`, `#`, or `1.` — are not escaped, because every note this
/// crate produces begins with a lowercase word. If that ever stops being true, this is where it
/// gets handled.
fn escape(text: &str) -> String {
    const SIGNIFICANT: &[char] = &['\\', '`', '*', '_', '[', ']', '<', '&', '~'];

    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        if SIGNIFICANT.contains(&ch) {
            out.push('\\');
        }
        out.push(ch);
    }
    out
}

#[cfg(test)]
mod tests {
    use expect_test::expect;
    use phrost_model::{TextRange, TextSize};

    use super::*;

    fn info(blocks: Vec<HoverBlock>) -> HoverInfo {
        HoverInfo {
            range: TextRange::empty(TextSize::from(0)),
            blocks,
        }
    }

    #[test]
    fn a_php_name_survives_escaping() {
        // The bug this exists to prevent: `App\Repo` renders as `AppRepo` when the backslash is
        // read as an escape, and `Repo_Base` turns the rest of the line into italics.
        let rendered = render_hover(&info(vec![HoverBlock::Note(
            "declared in App\\Repo_Base.php".to_owned(),
        )]));

        expect![[r#"declared in App\\Repo\_Base.php"#]].assert_eq(&rendered);
    }

    #[test]
    fn a_signature_becomes_a_php_code_fence() {
        let rendered = render_hover(&info(vec![
            HoverBlock::Code("App\\Repo::find(int $id): ?User".to_owned()),
            HoverBlock::Note("declared in src/Repo.php".to_owned()),
        ]));

        expect![[r#"
            ```php
            App\Repo::find(int $id): ?User
            ```

            declared in src/Repo.php"#]]
        .assert_eq(&rendered);
    }

    #[test]
    fn documentation_passes_through_unescaped() {
        // A docblock that used a list or a link should render as one; escaping it would show the
        // markup instead.
        let rendered = render_hover(&info(vec![
            HoverBlock::Documentation("Finds a *user*.\n\n- by id\n- by name".to_owned()),
            HoverBlock::Separator,
            HoverBlock::Note("own".to_owned()),
        ]));

        expect![[r#"
            Finds a *user*.

            - by id
            - by name

            ---

            own"#]]
        .assert_eq(&rendered);
    }
    #[test]
    fn a_tag_puts_its_type_in_a_code_span_and_does_not_escape_it() {
        // A code span is already literal -- markdown does no escape processing inside one -- so a
        // backslash written there is a backslash *shown*. Escaping it produced
        // `\\Drupal\\app\_core\\Application`, which is what a reader saw and reported.
        let rendered = render_hover(&info(vec![
            HoverBlock::Tag {
                label: "Parameter".to_owned(),
                ty: Some("int|string".to_owned()),
                subject: Some("$itemNumber".to_owned()),
                description: "Item Number.".to_owned(),
            },
            HoverBlock::Tag {
                label: "Returns".to_owned(),
                ty: Some("\\Drupal\\app_core\\Application\\Application|null".to_owned()),
                subject: None,
                description: "Item application object on success.".to_owned(),
            },
        ]));

        expect![[r#"
            **Parameter** `int|string` `$itemNumber` — Item Number.

            **Returns** `\Drupal\app_core\Application\Application|null` — Item application object on success."#]]
        .assert_eq(&rendered);
    }
}
