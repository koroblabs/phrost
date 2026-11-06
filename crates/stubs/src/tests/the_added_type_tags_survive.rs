//! The tripwire for type tags this project added to the vendored set.
//!
//! A conditional return type is the one thing the docblock language can say that a signature cannot:
//! `base64_decode()` is declared `string|false`, and the `false` is reachable **only** when `$strict`
//! is set -- left out, which is how nearly every call writes it, invalid characters are discarded and
//! a string always comes back. Writing that as `($strict is true ? string|false : string)` is a claim
//! the set does not make and this repo added.
//!
//! Which makes it a **local edit**, and `README.md` records why those are dangerous here: taking a
//! newer `mago-prelude` overwrites these files wholesale, and a tag that is simply gone fails
//! nothing at all -- the reports come back and read as a regression somewhere else entirely.
//!
//! An entry leaves this table when upstream writes the tag itself: 1.58.2 took `base64_decode()`'s
//! conditional and the `class_parents()`/`class_uses()`/`class_implements()` maps, so they are the
//! set's own now and a re-vendor keeps them.
//!
//! `DOMNode::$childNodes` is the same statement about a property rather than a return, and it is
//! here for a sharper version of the same reason: the tag it lost does not make the type *absent*,
//! it makes it the template's bound. `DOMNodeList` is `@template-covariant TNode of
//! DOMNode|DOMNameSpaceNode`, so an unparameterised one answers `DOMNode|DOMNameSpaceNode` -- a
//! plausible union, reported as an argument mismatch at every `saveHTML($child)`, and nothing
//! anywhere says a tag went missing.
//!
//! A table rather than a scan for a shape, because unlike the two-docblock artifact there is nothing
//! structural to recognise: each of these is one sentence about one declaration, and the next one is
//! a line here.

use crate::PRELUDE;

/// One declaration, and the tag the set has to carry above it.
struct AddedTag {
    file: &'static str,
    declares: &'static str,
    tag: &'static str,
}

static ADDED: &[AddedTag] = &[
    AddedTag {
        file: "dom.php",
        declares: "public DOMNodeList $childNodes;",
        tag: "@var DOMNodeList<DOMNode>",
    },
    AddedTag {
        file: "dom.php",
        declares: "public function query(string $expression, ?DOMNode $contextNode",
        tag: "@return DOMNodeList<DOMNode>|false",
    },
];

#[test]
fn every_type_tag_this_project_added_is_still_in_the_set() {
    let mut lost = Vec::new();

    for added in ADDED {
        let Some((_, contents)) = PRELUDE.iter().find(|(name, _)| *name == added.file) else {
            lost.push(format!("{} is not in the set at all", added.file));
            continue;
        };
        let lines: Vec<&str> = contents.lines().collect();
        let Some(at) = lines
            .iter()
            .position(|line| line.trim_start().starts_with(added.declares))
        else {
            lost.push(format!(
                "{}: nothing declares `{}`",
                added.file, added.declares
            ));
            continue;
        };
        // Upwards from the declaration to its docblock's opener: the tag has to be in the block PHP
        // attaches, which is the nearer one. Anything above that opener belongs to somebody else.
        let carries = lines[..at]
            .iter()
            .rev()
            .take_while(|line| !line.trim_start().starts_with("/**"))
            .any(|line| line.contains(added.tag));
        if !carries {
            lost.push(format!(
                "{}:{}: `{}` lost `{}`",
                added.file,
                at + 1,
                added.declares,
                added.tag
            ));
        }
    }

    assert!(
        lost.is_empty(),
        "a re-vendor took a tag this project added -- put it back:\n  {}",
        lost.join("\n  ")
    );
}
