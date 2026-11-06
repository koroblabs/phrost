//! Text outside the PHP tags is output, and a reprint leaves it as it was written.
//!
//! The formatter settles a file's end on one newline and drops a closing tag at the end, and both
//! are style rules for a file ending in code -- for one ending outside the tags each is a change of
//! output. Laravel's mail templates are `.blade.php` files with no `<?php` in them at all, and one
//! of them lost its last blank line; Symfony's translation fixtures end `... null); ?>` and a blank
//! line, and dropping the tag turned the blank line from output into whitespace. Found by comparing
//! what a reprinted checkout means with what it meant.

use mago_span::HasSpan;
use mago_syntax::cst::Statement;
use phrost_syntax::ParseSession;

/// `printed`, ending on the output `written` ends on when `written` ends outside the tags.
pub(super) fn trailing_output_kept(written: &str, printed: String) -> String {
    // Nothing at all is not a file ending in code: a newline added to it is a newline of output,
    // one byte where including the file used to print none.
    if written.is_empty() {
        return String::new();
    }
    let Some(output_at) = trailing_output(written) else {
        return printed;
    };
    let kept = printed.trim_end();
    match trailing_output(&printed) {
        // Still output: only the whitespace the formatter settles can have moved.
        Some(_) => format!("{kept}{}", &written[written.trim_end().len()..]),
        // The tag that made it output is gone, so it goes back, with everything written after it.
        None => match written[..output_at].rfind("?>") {
            Some(closed_at) => format!("{kept} {}", &written[closed_at..]),
            None => printed,
        },
    }
}

/// Where the text after the last closing tag begins, when the file ends outside the tags with
/// something PHP prints.
///
/// One newline straight after `?>` belongs to the tag -- PHP's lexer takes it -- so a file ending
/// `?>` and a newline prints nothing, and its tag is the one a style may drop.
fn trailing_output(text: &str) -> Option<usize> {
    let session = ParseSession::new();
    let parsed = session.parse(text);
    let Some(statement @ Statement::Inline(_)) = parsed.program().statements.last() else {
        return None;
    };
    let start = statement.span().start.offset as usize;
    let inline = text.get(start..)?;
    // Only when the text begins right at the tag: where the tag's own token took the newline, the
    // text after it is already what PHP prints.
    let printed = match text[..start].ends_with("?>") {
        true => inline
            .strip_prefix("\r\n")
            .or_else(|| inline.strip_prefix('\n'))
            .unwrap_or(inline),
        false => inline,
    };
    (!printed.is_empty()).then_some(start)
}
