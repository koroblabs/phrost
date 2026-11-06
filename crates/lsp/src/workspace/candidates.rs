//! Whether one file's text may hold a use of what a sweep is looking for.
//!
//! Asked of every file a word set admits, before any of them is analysed: reading bytes is far
//! cheaper than lowering them, and analysing is what a sweep spends its time on.

use phrost_analysis::{ConstructionQuery, SweepQuery};
use rustc_hash::FxHashMap;

/// A [`SweepQuery`] made ready to be asked of thousands of files' bytes.
pub(crate) struct TextNeedles {
    words: Vec<Box<[u8]>>,
    construction: Option<ConstructionNeedles>,
}

impl TextNeedles {
    pub(crate) fn of(query: &SweepQuery) -> Self {
        Self {
            words: query
                .words
                .iter()
                .map(|word| word.as_bytes().into())
                .collect(),
            construction: query.construction.as_ref().map(ConstructionNeedles::of),
        }
    }

    pub(crate) fn found_in(&self, text: &[u8]) -> bool {
        self.words
            .iter()
            .any(|word| holds_ignoring_case(text, word))
            || self
                .construction
                .as_ref()
                .is_some_and(|construction| construction.reached_in(text))
    }
}

/// Whether `haystack` holds `needle`, ignoring ASCII case.
///
/// First byte first, rather than comparing at every offset: on a file the needle is not in -- which
/// is most of them, and the whole point of the filter -- almost every position is settled by one
/// comparison instead of by a slice compare that has to be set up first.
fn holds_ignoring_case(haystack: &[u8], needle: &[u8]) -> bool {
    let Some(first) = needle.first() else {
        return false;
    };
    let (lower, upper) = (first.to_ascii_lowercase(), first.to_ascii_uppercase());

    haystack.windows(needle.len()).any(|window| {
        (window[0] == lower || window[0] == upper) && window.eq_ignore_ascii_case(needle)
    })
}

/// Whether a class's own declaration is a place its constructor is written.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Declares {
    /// It declares the constructor, or stands in for one nothing declares.
    Constructor,
    /// It runs a parent's.
    Nothing,
}

/// Where in a file's text a constructor can be reached, told apart from where a class that runs it
/// is only named.
///
/// A class name counts wherever it is written, except in the places that cannot construct by
/// themselves:
///
/// - `extends Child`, and `use Child` -- an import, or a trait taken into a class body -- unless
///   the file also writes `new class`, an anonymous class constructed on the spot, or `new parent`.
///   An import that gives an alias always counts, since `new Kid` then constructs under it;
/// - `class Child`, or `trait` or `interface` or `enum`, for one that inherits the constructor --
///   unless the file also writes `new self` or `new static`. The declaring class's own `class Repo`
///   counts, because that file is where the constructor is written.
///
/// Which `use` is an import and which takes a trait in is a question about where it stands, which
/// bytes cannot settle -- so both are read the way the trait needs, and the import pays for it
/// only in a file that also writes `new class`.
///
/// The member's own name counts after `->`, an object re-running one, and after `::` only in a file
/// that names one of the classes: `parent::__construct()` reaches it only from a class whose parent
/// is one of them, and `self::`, `static::` and `Child::` only from inside or about one. Never where
/// it is declared, which is some other class's constructor. Anywhere else -- a string, a comment --
/// it counts, because a prefilter that is unsure has to say yes.
///
/// Measured on Symfony for `TestCase::__construct`: 5 713 files admitted by words, 407 by this.
struct ConstructionNeedles {
    /// Every class that runs it, folded.
    classes: FxHashMap<Box<[u8]>, Declares>,
    /// The length of the longest, so a longer word is passed over without being folded.
    longest: usize,
}

/// What one file's text says about a constructor, gathered in one pass and settled at the end --
/// a class's declaration may come before the `new self` that makes it count.
#[derive(Default)]
struct Evidence {
    /// One of the classes is written somewhere.
    named: bool,
    /// `::__construct`.
    called_statically: bool,
    /// `class Child`, for a class inheriting the constructor.
    inheriting_declaration: bool,
    /// `extends Child` or `use Child`: a declaration taking it in.
    composed: bool,
    /// `new self` or `new static`.
    new_self: bool,
    /// `new parent` or `new class`.
    new_parent: bool,
}

impl Evidence {
    fn reaches(&self) -> bool {
        (self.called_statically && self.named)
            || (self.inheriting_declaration && self.new_self)
            || (self.composed && self.new_parent)
    }
}

impl ConstructionNeedles {
    fn of(query: &ConstructionQuery) -> Self {
        let mut classes: FxHashMap<Box<[u8]>, Declares> = FxHashMap::default();
        // Inherited first, so a name that is both -- two classes sharing a short name in two
        // namespaces -- keeps the reading that admits more.
        for (names, declares) in [
            (&query.inherited_by, Declares::Nothing),
            (&query.declared_by, Declares::Constructor),
        ] {
            for name in names {
                classes.insert(name.to_ascii_lowercase().into_bytes().into(), declares);
            }
        }
        let longest = classes
            .keys()
            .map(|name| name.len())
            .chain([ConstructionQuery::MEMBER.len()])
            .max()
            .unwrap_or(0);
        Self { classes, longest }
    }

    fn reached_in(&self, text: &[u8]) -> bool {
        let mut evidence = Evidence::default();
        let mut folded = Vec::with_capacity(self.longest);
        let mut at = 0;
        while let Some((start, end)) = next_word(text, at) {
            at = end;
            let word = &text[start..end];
            if word.len() > self.longest || (start > 0 && text[start - 1] == b'$') {
                continue;
            }
            if word.eq_ignore_ascii_case(b"new") {
                let constructed = next_word(text, end)
                    .filter(|&(next, _)| text[end..next].iter().all(u8::is_ascii_whitespace))
                    .map(|(next, after)| &text[next..after]);
                match constructed {
                    Some(word) if is_any_of(word, &[b"self", b"static"]) => {
                        evidence.new_self = true
                    }
                    Some(word) if is_any_of(word, &[b"parent", b"class"]) => {
                        evidence.new_parent = true;
                    }
                    _ => {}
                }
                continue;
            }
            if word.eq_ignore_ascii_case(ConstructionQuery::MEMBER.as_bytes()) {
                match operator_before(text, start) {
                    Some(b"->") => return true,
                    Some(b"::") => evidence.called_statically = true,
                    _ if keyword_before(text, start).eq_ignore_ascii_case(b"function") => {}
                    _ => return true,
                }
                continue;
            }

            folded.clear();
            folded.extend(word.iter().map(u8::to_ascii_lowercase));
            let Some(&declares) = self.classes.get(folded.as_slice()) else {
                continue;
            };
            evidence.named = true;
            let keyword = keyword_before(text, start);
            if is_any_of(keyword, &[b"class", b"trait", b"interface", b"enum"]) {
                match declares {
                    Declares::Constructor => return true,
                    Declares::Nothing => evidence.inheriting_declaration = true,
                }
            } else if keyword.eq_ignore_ascii_case(b"use") {
                if word_after(text, end).eq_ignore_ascii_case(b"as") {
                    return true;
                }
                evidence.composed = true;
            } else if keyword.eq_ignore_ascii_case(b"extends") {
                evidence.composed = true;
            } else {
                return true;
            }
        }
        evidence.reaches()
    }
}

/// The next run of identifier characters at or after `from`, as the word set splits them.
fn next_word(text: &[u8], from: usize) -> Option<(usize, usize)> {
    let start = from + text.get(from..)?.iter().position(|&byte| in_word(byte))?;
    let end = text[start..]
        .iter()
        .position(|&byte| !in_word(byte))
        .map_or(text.len(), |length| start + length);
    Some((start, end))
}

const fn in_word(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

fn is_any_of(word: &[u8], keywords: &[&[u8]]) -> bool {
    keywords
        .iter()
        .any(|keyword| word.eq_ignore_ascii_case(keyword))
}

/// The word written before the name starting at `start`, past any namespace it is qualified with
/// -- the `use` of `use App\Child;` -- and past the `&` of a by-reference declaration.
fn keyword_before(text: &[u8], start: usize) -> &[u8] {
    let mut at = start;
    while at > 0 && (in_word(text[at - 1]) || text[at - 1] == b'\\') {
        at -= 1;
    }
    while at > 0 && (text[at - 1].is_ascii_whitespace() || text[at - 1] == b'&') {
        at -= 1;
    }
    let end = at;
    while at > 0 && in_word(text[at - 1]) {
        at -= 1;
    }
    &text[at..end]
}

/// The `->` or `::` right before `start`, whitespace allowed between.
fn operator_before(text: &[u8], start: usize) -> Option<&[u8]> {
    let end = text[..start]
        .iter()
        .rposition(|byte| !byte.is_ascii_whitespace())?
        + 1;
    let operator = text.get(end.checked_sub(2)?..end)?;
    matches!(operator, b"->" | b"::").then_some(operator)
}

/// The word right after `end`, whitespace allowed between.
fn word_after(text: &[u8], end: usize) -> &[u8] {
    match next_word(text, end) {
        Some((start, after)) if text[end..start].iter().all(u8::is_ascii_whitespace) => {
            &text[start..after]
        }
        _ => &[],
    }
}

#[cfg(test)]
mod tests {
    use expect_test::expect;
    use phrost_analysis::{ConstructionQuery, SweepQuery};

    use super::TextNeedles;

    /// Each text alone, against `Repo::__construct` with `Child` inheriting it -- and every line
    /// of the answer, so a text that starts or stops being admitted is a diff someone reads.
    #[test]
    fn a_constructor_is_looked_for_only_where_a_file_can_reach_it() {
        let needles = TextNeedles::of(&SweepQuery {
            words: Vec::new(),
            construction: Some(ConstructionQuery {
                declared_by: vec!["Repo".to_owned()],
                inherited_by: vec!["Child".to_owned()],
            }),
        });
        let texts = [
            "new Child();",
            "new \\App\\Child;",
            "NEW child();",
            "Child::class;",
            "function f(Child $c) {}",
            "use App\\Child;",
            "use App\\Child as Kid; new Kid;",
            "use App\\Child ;",
            "class Leaf extends Child {}",
            "class Leaf extends \\App\\Child {}",
            "class Leaf extends Child { function f() { return new parent; } }",
            "$x = new class extends Child {};",
            "$x = new class { use Child; };",
            "use App\\Child; $x = new class {};",
            "class Child {}",
            "class Child { static function make() { return new static(); } }",
            "final class Child { function f() { return new self; } }",
            "class Repo {}",
            "trait Repo {}",
            "interface Child {}",
            "trait Child {}",
            "function __construct() {}",
            "function &__construct() {}",
            "parent::__construct();",
            "class Leaf extends Child { function __construct() { parent::__construct(); } }",
            "$x->__construct();",
            "$x?->__construct();",
            "Other::__construct();",
            "'__construct'",
            "$child = 1; $Child = 2;",
            "ChildOfMine::make();",
            "new ChildOfMine;",
        ];
        let rendered = texts
            .iter()
            .map(|text| {
                let admitted = needles.found_in(text.as_bytes());
                format!("{} {text}\n", if admitted { "yes" } else { "no " })
            })
            .collect::<String>();

        expect![[r#"
            yes new Child();
            yes new \App\Child;
            yes NEW child();
            yes Child::class;
            yes function f(Child $c) {}
            no  use App\Child;
            yes use App\Child as Kid; new Kid;
            no  use App\Child ;
            no  class Leaf extends Child {}
            no  class Leaf extends \App\Child {}
            yes class Leaf extends Child { function f() { return new parent; } }
            yes $x = new class extends Child {};
            yes $x = new class { use Child; };
            yes use App\Child; $x = new class {};
            no  class Child {}
            yes class Child { static function make() { return new static(); } }
            yes final class Child { function f() { return new self; } }
            yes class Repo {}
            yes trait Repo {}
            no  interface Child {}
            no  trait Child {}
            no  function __construct() {}
            no  function &__construct() {}
            no  parent::__construct();
            yes class Leaf extends Child { function __construct() { parent::__construct(); } }
            yes $x->__construct();
            yes $x?->__construct();
            no  Other::__construct();
            yes '__construct'
            no  $child = 1; $Child = 2;
            no  ChildOfMine::make();
            no  new ChildOfMine;
        "#]]
        .assert_eq(&rendered);
    }

    #[test]
    fn a_word_is_looked_for_anywhere() {
        let needles = TextNeedles::of(&SweepQuery {
            words: vec!["find".to_owned()],
            construction: None,
        });
        assert!(needles.found_in(b"$repo->FIND();"));
        assert!(
            needles.found_in(b"// findAll"),
            "a substring, as it always was"
        );
        assert!(!needles.found_in(b"$repo->get();"));
    }
}
