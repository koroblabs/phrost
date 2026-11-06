//! The tripwire for a re-vendor, which is the one way this documentation disappears silently.
//!
//! The vendored set carries precise types and no prose, so a hover over `sort` showed a signature
//! and nothing else, and 14 of its 4 422 methods had a description. **945 functions and 1 199
//! methods** now carry a summary, 138 of them a worked example: the 193 one real checkout's own code
//! actually calls -- 100 % of its calls into the standard library -- plus everything else in the
//! extensions PHP itself bundles, which is the principled boundary, since every installation has
//! them. `uv`, `gearman`, `xcache` and `com` are deliberately left out: nobody hovers `xcache_get`.
//!
//! Functions and methods are counted separately because they were written by separate passes and a
//! re-vendor could plausibly lose one and not the other -- and because the method pass needed a
//! different placing rule, several stub files wrapping everything in a braced `namespace { ... }`
//! block that indents every class inside it.
//!
//! That makes the prose a **local edit**, and `README.md` records why local edits are dangerous
//! here: taking a newer `mago-prelude` overwrites these files wholesale, and a docblock that is
//! simply gone fails nothing. So this counts.
//!
//! A count rather than a list of 184 names, because what has to be caught is the *wholesale* loss --
//! a re-vendor takes every summary at once and the count goes to single figures. The named handful
//! below is not a second copy of that check; it is there so the failure says which functions to look
//! at instead of only a number. Together they cost nothing and need no maintenance when the next
//! summary is written.

use std::collections::BTreeSet;

use crate::PRELUDE;

/// How many free functions in the set carry a prose line above their tags.
///
/// The floor is deliberately well under the real 947: it exists to catch a revert, not to be edited
/// every time a name is added.
const FUNCTION_FLOOR: usize = 900;

/// The same for methods, of which 1 199 carry one, keyed `Class::method`.
///
/// Held well under that for the reason the function floor is, and it had to be *lowered* once: a
/// re-vendor removes declarations as well as adding them, and 1.47.2 -> 1.48.0 took five that
/// carried a summary -- `Reflector::__toString`, `ReflectionZendExtension::export` and the three
/// enum-reflection constructors, each either gone from PHP 8 or deduped against the parent that
/// declares it. Prose with no declaration left to sit on is not a loss this is watching for, and a
/// floor four under the real count is an exact-count assertion wearing a tripwire's name.
const METHOD_FLOOR: usize = 1150;

/// Every declaration in the set that carries a prose summary, split by whether it is a free function
/// or a method.
///
/// A method is keyed `Class::method`, and that is load-bearing rather than tidy: `count`, `current`,
/// `next`, `key` and `offsetGet` are declared by dozens of classes, so a set of bare names collapses
/// 1 253 methods into 648 and the floor below reads as a failure. Keying by owner is also what caught
/// six invented methods on the way in -- `SplDoublyLinkedList::enqueue` and `IteratorAggregate::
/// current` among them -- which a bare-name match would have placed on whichever class happened to
/// declare something of that name.
///
/// The two kinds are counted apart because they were written by separate passes over separate
/// slices, and a re-vendor could lose one without the other.
fn documented() -> (BTreeSet<String>, BTreeSet<String>) {
    let (mut functions, mut methods) = (BTreeSet::new(), BTreeSet::new());
    for (_, contents) in PRELUDE {
        let lines: Vec<&str> = contents.lines().collect();
        let (mut owner, mut namespaced) = (None, false);
        for (at, line) in lines.iter().enumerate() {
            let trimmed = line.trim_start();
            if let Some(rest) = trimmed.strip_prefix("namespace") {
                // A braced `namespace { ... }` is the global one; `namespace Dom {` is not. Several
                // stub files use the braced form, which indents every class inside it.
                namespaced = !rest.trim_start().starts_with(['{', ';']);
            }
            if let Some(name) = declares_type(trimmed) {
                owner = Some(name);
            }
            let Some((indented, name)) = declared(line) else {
                continue;
            };
            // Back over the attributes a declaration may carry to whatever precedes it, then over
            // the docblock to its opener, reading the lines that are not tags.
            let mut above = at;
            while above > 0 && lines[above - 1].trim_start().starts_with("#[") {
                above -= 1;
            }
            if above == 0 || lines[above - 1].trim() != "*/" {
                continue;
            }
            let mut top = above - 1;
            while top > 0 && lines[top].trim() != "/**" {
                top -= 1;
            }
            // The run before the FIRST tag, not any non-tag line anywhere in the block. A
            // multi-line conditional type continues on lines opening with `?` and `:`, which read
            // as prose to a whole-block scan -- the script that placed these summaries had exactly
            // that bug and silently skipped nine functions, `array_map` among them.
            let prose = lines[top + 1..above - 1]
                .iter()
                .map(|line| line.trim().trim_start_matches('*').trim())
                .take_while(|text| !text.starts_with('@'))
                .any(|text| !text.is_empty());
            if !prose {
                continue;
            }
            match (indented, owner) {
                (true, Some(class)) if !namespaced => {
                    methods.insert(format!("{class}::{name}"));
                }
                (false, _) => {
                    functions.insert(name.to_owned());
                }
                _ => {}
            }
        }
    }
    (functions, methods)
}

/// The name a `class`/`interface`/`trait`/`enum` line declares.
fn declares_type(trimmed: &str) -> Option<&str> {
    let rest = ["final ", "abstract ", "readonly "]
        .iter()
        .fold(trimmed, |text, modifier| {
            text.strip_prefix(modifier).unwrap_or(text).trim_start()
        });
    ["class ", "interface ", "trait ", "enum "]
        .iter()
        .find_map(|keyword| rest.strip_prefix(*keyword))
        .and_then(|name| name.trim_start().split([' ', '{', ':']).next())
        .filter(|name| !name.is_empty())
}

/// `(is indented, its name)` for a line declaring a function, or `None` for anything else.
fn declared(line: &str) -> Option<(bool, &str)> {
    let trimmed = line.trim_start();
    let rest = [
        "final ",
        "abstract ",
        "static ",
        "public ",
        "private ",
        "protected ",
        "readonly ",
    ]
    .iter()
    .fold(trimmed, |text, modifier| {
        text.strip_prefix(modifier).unwrap_or(text).trim_start()
    });
    let name = rest
        .strip_prefix("function ")?
        .trim_start_matches('&')
        .split(['(', ' '])
        .next()?;
    (!name.is_empty()).then_some((trimmed.len() != line.len(), name))
}

#[test]
fn a_re_vendor_that_drops_the_written_summaries_fails_here() {
    let (functions, methods) = documented();
    assert!(
        functions.len() >= FUNCTION_FLOOR,
        "only {} stub functions carry a prose summary, expected at least {FUNCTION_FLOOR}. A \
         re-vendor overwrites third_party/php-prelude wholesale; re-apply the local edits its \
         README lists.",
        functions.len()
    );
    assert!(
        methods.len() >= METHOD_FLOOR,
        "only {} stub methods carry a prose summary, expected at least {METHOD_FLOOR}. A re-vendor \
         overwrites third_party/php-prelude wholesale; re-apply the local edits its README lists.",
        methods.len()
    );

    // The most-called names on the reference checkout, so the failure names something to look at.
    // One per shape of trap the summaries exist to record: a loose comparison, an in-place sort, a
    // false-on-failure return, a byte-not-character operation, an internal pointer, an array whose
    // keys are documented nowhere else.
    let named_functions = [
        "sprintf",
        "in_array",
        "is_array",
        "count",
        "sort",
        "usort",
        "array_splice",
        "reset",
        "strpos",
        "strlen",
        "json_decode",
        "preg_match",
        "localtime",
        "stat",
        "getimagesize",
        "debug_backtrace",
    ];
    // And for methods, the pair whose difference is the commonest bug in the language.
    let named_methods = [
        "DateTime::modify",
        "DateTimeImmutable::modify",
        "SplFileInfo::getRealPath",
        "PDOStatement::fetch",
        "ReflectionClass::getMethods",
        "SplObjectStorage::attach",
        "ZipArchive::close",
    ];
    let missing: Vec<&str> = named_functions
        .iter()
        .filter(|name| !functions.contains(**name))
        .chain(
            named_methods
                .iter()
                .filter(|name| !methods.contains(**name)),
        )
        .copied()
        .collect();
    assert!(
        missing.is_empty(),
        "these stub declarations lost their prose summary: {missing:?}"
    );
}
