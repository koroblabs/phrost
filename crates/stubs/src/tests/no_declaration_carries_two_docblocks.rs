//! The tripwire for the tags a re-vendor silently drops on the floor.
//!
//! Forty declarations in the vendored set were written with **two** docblocks in a row:
//!
//! ```text
//! /** @return DOMNode|false */
//! /**
//!  * Adds node as the last child of this node and returns it. ...
//!  */
//! public function appendChild(DOMNode $node) {}
//! ```
//!
//! PHP attaches the *nearer* one, and so does every other tool, so the first block and its tags are
//! lost. `DOMNode::appendChild()` has no native return type either, which left it completely
//! untyped -- and with it every chain through it, which is most of how anyone builds a document.
//! Thirty-three of the forty are `dom.php`.
//!
//! The tags are merged into the block that keeps them, which makes them a **local edit**: taking a
//! newer `mago-prelude` overwrites these files wholesale and a tag that is simply gone fails
//! nothing. So this counts them, and the count is zero.
//!
//! A shape rather than a list of names, because what has to be caught is the wholesale return of
//! the generator artifact rather than any one method. The failure names the file and line, which is
//! what somebody re-running the merge needs.

use crate::PRELUDE;

/// Whether a line closes a doc comment -- `/** ... */`, not a plain `/* ... */`.
///
/// Told apart by walking back to the opener, because several stub files write a section header as
/// `/* mysqli_real_connect flags */` immediately above a real docblock, and counting those would
/// make this fail on eight lines that are perfectly correct.
fn closes_a_docblock(lines: &[&str], at: usize) -> bool {
    if !lines[at].trim_end().ends_with("*/") {
        return false;
    }
    (0..=at)
        .rev()
        .find(|line| lines[*line].contains("/*"))
        .is_some_and(|opener| lines[opener].trim_start().starts_with("/**"))
}

#[test]
fn no_declaration_in_the_set_carries_two_docblocks() {
    let mut found: Vec<String> = Vec::new();

    for (name, contents) in PRELUDE {
        let lines: Vec<&str> = contents.lines().collect();
        for at in 0..lines.len().saturating_sub(1) {
            if lines[at + 1].trim_start().starts_with("/**") && closes_a_docblock(&lines, at) {
                found.push(format!("{name}:{}", at + 1));
            }
        }
    }

    assert!(
        found.is_empty(),
        "a docblock is followed by another, so its tags reach nothing -- merge them:\n  {}",
        found.join("\n  ")
    );
}
