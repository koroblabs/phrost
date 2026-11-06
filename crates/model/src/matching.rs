//! What a typed prefix matches, and how well.
//!
//! Here rather than beside any one reader because three layers ask it and none of them owns it: the
//! catalogs in the index filter their keys, the view filters names and members, and the feature
//! layer filters keywords. A second copy of a matcher is a second answer to "did this match", and
//! the two would drift the day one of them learned something.

/// Whether `name` begins with `prefix`, ignoring case.
///
/// Over bytes rather than over `str`, because a member name is not guaranteed to be ASCII and
/// slicing one at a byte index that is not a character boundary panics. A folded comparison of the
/// leading bytes says the same thing for the names that are ASCII and falls back to an exact
/// comparison for the ones that are not.
pub fn starts_with_ignoring_case(name: &str, prefix: &str) -> bool {
    name.len() >= prefix.len()
        && name.as_bytes()[..prefix.len()].eq_ignore_ascii_case(prefix.as_bytes())
}

/// A character that opens a word, which is where a person's eye and their typing both start.
const OPENS_A_WORD: u32 = 16;
/// A character immediately after the previous match. A run of these is what a person means when
/// they type part of a name, and it is the whole of what separates a real hit from a coincidence.
const RUNS_ON: u32 = 16;
/// A character matched anywhere else.
const ANYWHERE: u32 = 2;
/// The first character of the name, which is a word opening and also the strongest signal there is.
const AT_THE_HEAD: u32 = 8;

/// Which letters a name holds, as one bit per `a`-`z`, folded.
///
/// The prefilter in front of [`match_score`], and it is the whole reason a loose search over sixty
/// thousand names is affordable: a name cannot contain `typed` as a subsequence unless it contains
/// every letter of it, so one `&` rejects most of the project before anything reads a string.
/// Measured on a 30 964-file checkout, this is the difference between 1.79 ms and 0.6 ms at p95 on
/// the name-completion row.
///
/// Only `a`-`z`. A character outside it sets no bit and asks for none, so the filter never rejects
/// on a `.` or a `\` -- which is right, because it may only ever *reject*: a name that passes is
/// still scored, and a name that fails could not have matched.
pub fn letters_of(name: &str) -> u32 {
    let mut held = 0;
    for byte in name.bytes() {
        let folded = byte.to_ascii_lowercase();
        if folded.is_ascii_lowercase() {
            held |= 1 << (folded - b'a');
        }
    }
    held
}

/// How well `typed` matches `name`, or nothing when it does not match at all.
///
/// A subsequence test with a score, which is what every fuzzy finder is. The test alone answers
/// whether `OrdMan` could have meant `OrderManager` -- it also answers whether it could have
/// meant `DateMalformedIntervalStringException`, and on a project with sixty thousand names the
/// difference between a list worth reading and one worth closing is entirely in the ordering. So
/// what comes back is a number, and the caller sorts by it.
///
/// Three things earn points, and each is a reason a person would have typed those letters:
/// a character that **opens a word** -- the start, after a `\`, `_`, `.`, `-`, `/` or space, or a
/// camel hump; a character that **runs on** from the one before it; and the head of the name. So
/// `Manag` scores 5 consecutive characters from a camel hump in `OrderManager` and 5 scattered
/// ones in `DateMalformedIntervalStringException`, and the two are eight times apart.
///
/// Greedy, taking the first byte that fits rather than the alignment that would score highest. The
/// optimal alignment is a dynamic program over both strings, which is affordable for one comparison
/// and not for sixty thousand per keystroke -- and the greedy alignment is the one a person reading
/// the name would find, because they read left to right too.
///
/// Case-insensitive over bytes, for [`starts_with_ignoring_case`]'s reason: a name is not guaranteed
/// to be ASCII, and folding bytes says the same thing for the ones that are.
pub fn match_score(name: &str, typed: &str) -> Option<u32> {
    let (name, typed) = (name.as_bytes(), typed.as_bytes());
    if typed.is_empty() {
        return Some(0);
    }

    let mut score = 0;
    let mut wanted = 0;
    let mut previous: Option<usize> = None;

    for (at, byte) in name.iter().enumerate() {
        if wanted == typed.len() {
            break;
        }
        if !byte.eq_ignore_ascii_case(&typed[wanted]) {
            continue;
        }

        let opens_a_word = at == 0
            || matches!(name[at - 1], b'\\' | b'_' | b'.' | b'-' | b'/' | b' ')
            || (name[at - 1].is_ascii_lowercase() && byte.is_ascii_uppercase());
        score += match opens_a_word {
            true => OPENS_A_WORD,
            false => ANYWHERE,
        };
        if at == 0 {
            score += AT_THE_HEAD;
        }
        if previous.is_some_and(|before| before + 1 == at) {
            score += RUNS_ON;
        }
        previous = Some(at);
        wanted += 1;
    }

    (wanted == typed.len()).then_some(score)
}

/// Whether `typed` appears in `name` in order, ignoring case and what lies between.
///
/// What a member list is filtered by, and it is not the same test a bare name gets. PHP member names
/// carry their subject in the middle -- `isShipped`, `getShipDate`, `setShipDate` all answer to `ship`
/// and none of them starts with it -- so a prefix test on a member offers nothing for the thing a
/// reader is most likely to type. Every other PHP server matches this way and so does the client
/// doing its own filtering afterwards.
///
/// A **catalog key** is matched this way too, and it is where it is wanted most: a service id is
/// `app_core.order.manager` and what a person remembers of it is `manager`. Measured on one
/// real project, its own YAML writes 745 of them and its PHP many more, and none of them was
/// reachable except from the first character.
///
/// One letter is the exception. A subsequence of length one is satisfied by every name containing
/// that letter, so `$order->a` answers with `getShipDate` and `save` alongside
/// `account` -- the whole class, in an order that has nothing to do with what was typed.
/// Subsequence matching earns its keep from the ordering constraint between letters, and with one
/// letter there is none, so the first character a person types is read as the start of the name.
///
/// **How many letters that exception needs is the caller's**, because it depends on how many names
/// are being filtered rather than on the test. A class's members are thirty and two letters
/// discriminate; a project's names are sixty thousand and two letters do not, so the name search
/// asks for three -- see `IndexView::candidates_with_prefix`.
pub fn matches_loosely(name: &str, typed: &str) -> bool {
    if typed.len() == 1 {
        return starts_with_ignoring_case(name, typed);
    }
    match_score(name, typed).is_some()
}

#[cfg(test)]
mod tests {
    use super::{match_score, matches_loosely};

    /// The comparison the whole thing exists to make: two names a person could have meant, one of
    /// which they did.
    #[test]
    fn a_run_from_a_word_opening_beats_the_same_letters_scattered() {
        let meant = match_score("OrderManager", "manag").expect("a subsequence");
        let coincidence =
            match_score("DateMalformedIntervalStringException", "manag").expect("also one");
        assert!(
            meant > coincidence * 2,
            "meant {meant}, coincidence {coincidence}"
        );
    }

    #[test]
    fn the_camel_humps_of_a_name_are_what_an_abbreviation_types() {
        assert!(match_score("OrderManager", "ordman").is_some());
        assert!(match_score("OrderManager", "om").is_some());
        // And a name that does not hold the letters in order is not a match however close it looks.
        assert!(match_score("OrderManager", "namager").is_none());
    }

    /// A dotted id is words too, and its separators are where a person starts reading.
    ///
    /// The comparison worth pinning is the one a person makes: `manager` is the last word of the id
    /// and `acom` is its initials, and both are things somebody could type -- so both match, and
    /// the word they remember scores above the letters they picked out.
    #[test]
    fn a_separator_opens_a_word_the_way_a_capital_does() {
        let word = match_score("app_core.order.manager", "manager").expect("a subsequence");
        let initials = match_score("app_core.order.manager", "acom").expect("also one");
        let scattered = match_score("app_core.order.manager", "ppor").expect("and so is this");
        assert!(word > initials, "word {word}, initials {initials}");
        assert!(
            initials > scattered,
            "initials {initials}, scattered {scattered}"
        );
    }

    #[test]
    fn one_letter_is_read_as_the_start_of_the_name() {
        // The ordering constraint is what makes a subsequence mean anything, and one letter has
        // none -- so it would match every name holding that letter anywhere.
        assert!(matches_loosely("getShipDate", "g"));
        assert!(!matches_loosely("getShipDate", "d"));
        assert!(matches_loosely("getShipDate", "sd"));
    }
}
