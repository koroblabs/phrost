//! The words PHP allows where a type is written.
//!
//! A closed list of seventeen, and none of them was ever offered. `function run(): st|` answered
//! `Stack`, `stat` and eighteen `stats_cdf_*` functions and not `string`; `vo|` answered `Volatile`
//! and not `void`; `private ar|` answered forty `array_*` functions and not `array`. The commonest
//! thing anybody types in PHP, missing from the one position where it is the only thing that can be
//! typed.
//!
//! Which of them is legal depends on the place, and the differences are the language's rather than a
//! ranking: `void` and `never` describe a return and nothing else, `static` as a type means "the
//! class this was called on" and only a return can say it, and an enum's backing type is two words
//! long. Offering a keyword somewhere PHP refuses it is offering a fatal error, which is the same
//! reason a capture list leaves out `$this`.

use phrost_model::SymbolKind;

use super::CompletionItem;
use crate::context::TypePlace;

/// The types every position takes: PHP's scalars and the containers beside them.
const EVERYWHERE: &[&str] = &[
    "array", "bool", "float", "int", "iterable", "mixed", "object", "string",
];

/// Written as an operand of a union and nowhere alone -- `?T` and `T|null` say the same thing, and
/// `false` and `true` are only a type as one arm of one.
///
/// Offered anyway, because the list is what somebody reads to find out what they may write, and a
/// `string|fal` with nothing to complete reads as the type not existing.
const IN_A_UNION: &[&str] = &["false", "null", "true"];

/// The class the code is written in, by relative name.
const RELATIVE: &[&str] = &["self", "parent"];

/// What only a docblock can say.
///
/// The static-analysis vocabulary the type tags share, which PHP has no syntax for and which this
/// project's own docblocks are written in. Not every one either checker accepts -- the ones a person
/// reaches for, the same discipline the statement keywords follow.
const IN_A_DOCBLOCK: &[&str] = &[
    "array-key",
    "callable-string",
    "class-string",
    "int-mask-of",
    "key-of",
    "list",
    "literal-string",
    "negative-int",
    "non-empty-array",
    "non-empty-list",
    "non-empty-string",
    "non-negative-int",
    "numeric-string",
    "positive-int",
    "resource",
    "scalar",
    "value-of",
];

impl TypePlace {
    /// The words PHP allows here, which is not the same list twice.
    fn keywords(self) -> Vec<&'static str> {
        // An enum's backing type is two words and no others -- not `float`, not a class, not `null`.
        if self == Self::EnumBacking {
            return vec!["int", "string"];
        }
        // A `catch` takes a class and nothing else at all: no scalar is throwable.
        if self == Self::Catch {
            return Vec::new();
        }

        let mut words: Vec<&'static str> = EVERYWHERE.to_vec();
        words.extend(IN_A_UNION);
        words.extend(RELATIVE);
        if self == Self::Docblock {
            // A docblock says things PHP's own types cannot, and the words for them are what a
            // person reaches into the tag language for. `void`, `never` and `static` come with
            // them, since a `@return` writes all three.
            words.extend(IN_A_DOCBLOCK);
            words.extend(["void", "never", "static", "callable"]);
            words.sort_unstable();
            words.dedup();
            return words;
        }
        // `callable` is refused as a property's type, because PHP cannot tell a stored string from
        // a call to it -- the one place the language leaves it out.
        if self != Self::Property {
            words.push("callable");
        }
        if self == Self::Return {
            // What only a return can say: nothing comes back, nothing comes back ever, and the
            // class this was called on.
            words.extend(["void", "never", "static"]);
        }
        words.sort_unstable();
        words
    }
}

/// The type keywords matching what has been typed, for the place the cursor is in.
///
/// Case-**sensitively**, unlike the statement keywords beside them, and the difference is what the
/// two lists are for. `RETURN` is written in old PHP and runs, so that list has to answer it; a type
/// keyword is lowercase in every project and every formatter, and a capital letter at a type
/// position is somebody reaching for a class. Matching loosely put `resource` above `Repo` for a
/// typed `Re`, which is the whole cost of getting this backwards.
pub(super) fn type_keywords(place: TypePlace, prefix: &str) -> Vec<CompletionItem> {
    place
        .keywords()
        .into_iter()
        .filter(|word| word.starts_with(prefix))
        .map(|word| CompletionItem {
            demoted: false,
            deprecated: false,
            label: word.to_owned(),
            kind: SymbolKind::Keyword,
            // Nothing to say about a keyword that the word does not already say.
            detail: None,
            describe: None,
            insert: None,
            call: None,
            import: None,
        })
        .collect()
}
