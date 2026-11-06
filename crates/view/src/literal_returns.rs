//! Standard-library functions that cannot fail on a literal the caller wrote out.
//!
//! `strtotime()` returns `int|false`, and the `false` is what it answers for a string it cannot
//! parse. When the string is written at the call site -- `strtotime('-25 days')` -- there is nothing
//! left to be uncertain about: the format is right there, and a checker that reports the failure
//! branch is reporting a run that cannot happen. Sixty of one real project's hundred and seventeen
//! argument findings were exactly this, which is the largest single family the checker produces.
//! `hex2bin('400a637573746f6d')` is the same statement about a different alphabet, and 42 of psl's
//! hundred and thirteen.
//!
//! # What is deliberately not here
//!
//! `unpack('c', $bytes)` looks like the same shape and is not: the format being well formed is only
//! half of what it needs, and `false` is also what it answers when the *data* runs out -- which is
//! why psl guards the length above every one of its calls. A rule reading only the format would
//! silence a report the length guard is the whole reason for.
//!
//! # Why this is not a stub
//!
//! A conditional return type tests an argument's *type*, and the docblock language has no way to
//! say "and the value is a date PHP can read". So the test is a recognizer, written here, and the
//! table naming which function it applies to is beside it.
//!
//! # Why it is safe
//!
//! It only ever *removes* the failure branch, and only for a literal it can recognise. A format it
//! does not know keeps the union, so a genuine `strtotime('yesterdya')` typo is still reported --
//! the recogniser refuses rather than approximates, which is the same discipline every other
//! judgement here follows. The worst it can do is miss a finding.
//!
//! # A string with a hole in it
//!
//! `strtotime("-$i months")` is the same statement as `strtotime('-25 days')` and it is how an
//! application actually writes one: fifteen sites on one real checkout against **none** in
//! WordPress, Laravel, Symfony or Slim, because a framework does date arithmetic through objects
//! and an application writes the phrase. So the count slot may be a hole, and PHP settles what that
//! costs -- `-3 months`, `--3 months`, `-+3 months`, `-0 months` and `-03 months` all parse, so
//! *any* integer in that slot leaves a date, whatever sign is written beside it.
//!
//! Only the count slot, and only for a hole the index can show holds an integer. A hole anywhere
//! else is a spelling -- a month name, four digits of a year, a unit -- and no type says a value
//! renders as one: `strtotime("$year-$month-$day")` really can fail, and so can
//! `strtotime("$days[$dW] $which week")`, and both are on the same checkout as the twelve that
//! cannot.

use phrost_body::value_path_of;
use phrost_hir::{ExprId, body::INTERPOLATION_HOLE};
use phrost_model::BuiltinTy;

use crate::{resolve::Held, symbols::CallFacts, view::IndexView};

/// What a literal has to look like for the failure branch to be impossible.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum LiteralShape {
    /// A string PHP's own date parser reads. Deliberately narrower than that parser: what is
    /// recognised is what real code writes, and everything else keeps the union.
    DateTime,
    /// An even number of hexadecimal digits, which is the whole of what `hex2bin` asks of its
    /// input: an odd length or a character outside `0-9a-fA-F` is the only way it answers `false`.
    HexPairs,
    /// A `sprintf` format, whose text outside its conversions is in every string it builds: what
    /// the call returns holds at least what that text does. Not a failure branch removed but a
    /// string refined, so it answers with the text's holding rather than with `well_formed`.
    Format,
}

/// One function, the argument that decides, and what it answers when that argument is well formed.
struct LiteralNarrowing {
    function: &'static str,
    argument: usize,
    shape: LiteralShape,
    /// The branch that survives. The union's other operand is the failure this removes.
    well_formed: BuiltinTy,
}

static NARROWINGS: &[LiteralNarrowing] = &[
    LiteralNarrowing {
        function: "strtotime",
        argument: 0,
        shape: LiteralShape::DateTime,
        well_formed: BuiltinTy::Int,
    },
    LiteralNarrowing {
        function: "hex2bin",
        argument: 0,
        shape: LiteralShape::HexPairs,
        well_formed: BuiltinTy::String,
    },
    LiteralNarrowing {
        function: "sprintf",
        argument: 0,
        shape: LiteralShape::Format,
        well_formed: BuiltinTy::String,
    },
    LiteralNarrowing {
        function: "vsprintf",
        argument: 0,
        shape: LiteralShape::Format,
        well_formed: BuiltinTy::String,
    },
];

impl IndexView<'_> {
    /// What `function` returns for the arguments this call wrote, when a recogniser settles it.
    pub(crate) fn narrowed_by_what_the_call_wrote(
        &self,
        function: &str,
        at: &CallFacts<'_>,
    ) -> Option<BuiltinTy> {
        // The name first: a scan of one short string today, and the answer for every other call.
        // Which is also what keeps the hole walk below off the hot path -- it runs for a call to one
        // of these two functions and for nothing else.
        let narrowing = NARROWINGS
            .iter()
            .find(|entry| function.eq_ignore_ascii_case(entry.function))?;

        let written = at
            .literals
            .iter()
            .find(|literal| usize::from(literal.position) == narrowing.argument)?;
        // A format is read only when it is written out whole: a hole may hold a `%` of its own,
        // which turns the text after it into a conversion.
        if narrowing.shape == LiteralShape::Format {
            if !matches!(written.kind, phrost_hir::body::ArgumentLiteral::Text) {
                return None;
            }
            return match format_text_holds(written.value.as_str())? {
                BuiltinTy::String => None,
                holds => Some(holds),
            };
        }
        let holes = match written.kind {
            phrost_hir::body::ArgumentLiteral::Interpolated => {
                self.integral_holes(at, written.position)?
            }
            // A keyword is `null`, `true` or `false` -- none of them a date.
            _ if written.wrote_its_value() => Vec::new(),
            _ => return None,
        };
        if holes.len() != hole_count(written.value.as_str()) {
            return None;
        }
        match narrowing.shape {
            LiteralShape::DateTime => reads_as_a_date(written.value.as_str(), &holes),
            // No type says a value renders as a pair of hex digits, so a skeleton never settles
            // this one -- `is_hex_pairs` refuses the hole marker for being outside the alphabet,
            // which is the same answer for the same reason.
            LiteralShape::HexPairs => is_hex_pairs(written.value.as_str()),
            LiteralShape::Format => unreachable!("a format is answered above"),
        }
        .then_some(narrowing.well_formed)
    }

    /// Whether each hole of an interpolated argument holds an integer, in the order they were
    /// written.
    ///
    /// `None` when the holes cannot be lined up with the skeleton at all. The skeleton comes from
    /// lowering and the holes from the argument's own nested expressions, so agreeing on the count
    /// is what makes "the third marker" and "the third expression" the same thing -- and a walk that
    /// matched by shape and quietly disagreed is the failure this refuses rather than guesses past.
    fn integral_holes(&self, at: &CallFacts<'_>, position: u16) -> Option<Vec<bool>> {
        let argument = *at.arguments()?.get(usize::from(position))?;
        // Either shape an interpolated string has been lowered to. It is an `Operator` producing a
        // string now -- because a value assigned one was otherwise untyped -- and the holes are the
        // same nested expressions they were as an `Unmodelled`. Both arms named rather than a `_`
        // fall-through, which is what the note above this function asks for.
        let (phrost_hir::body::ExprData::Unmodelled { inner, .. }
        | phrost_hir::body::ExprData::Operator { inner, .. }) = at.hir.expr(argument)
        else {
            return None;
        };
        Some(
            at.hir
                .expr_list(*inner)
                .iter()
                .map(|hole| self.holds_an_integer(at, *hole))
                .collect(),
        )
    }

    /// Whether one hole's value is an integer, and nothing wider.
    ///
    /// Nothing wider is the whole of it: `?int` renders the empty string for its null, and
    /// `strtotime('- months')` is one of the few relative phrases PHP really does refuse. So a union
    /// answers no here, which [`Self::scalar_named`] already does by returning nothing for one.
    fn holds_an_integer(&self, at: &CallFacts<'_>, hole: ExprId) -> bool {
        self.held_by(at, hole)
            .and_then(|held| self.scalar_named(at.hir, &held))
            == Some(BuiltinTy::Int)
    }

    /// What one expression written inside a call holds.
    fn held_by(&self, at: &CallFacts<'_>, expr: ExprId) -> Option<Held> {
        let path = value_path_of(at.hir, expr)?;
        self.resolve_path(at.offset, at.hir, &path, at.budget.checked_sub(1)?)
    }
}

/// What every string a `sprintf` format builds holds: what its text does once each conversion is
/// taken out, `%%` being the one that leaves a `%` behind. `None` for a format with a conversion
/// this does not read, which PHP refuses with a `ValueError` anyway.
///
/// A conversion is `%`, an optional `N$` argument number, flags, a width, a precision and one
/// letter. What it renders is the argument's and is not known -- but a width is the least it
/// renders, padding included, so `%02s` is never shorter than two characters and stands in the text
/// as two of them. `'0'` is what they are counted as, the one character that makes a single one
/// falsy, so a width of one says the string holds something and no more.
fn format_text_holds(format: &str) -> Option<BuiltinTy> {
    let mut text = String::new();
    let mut chars = format.chars().peekable();
    while let Some(char) = chars.next() {
        if char != '%' {
            text.push(char);
            continue;
        }
        if chars.peek() == Some(&'%') {
            chars.next();
            text.push('%');
            continue;
        }
        let mut digits = String::new();
        // `1$` numbers the argument and says nothing about the width.
        while let Some(&next) = chars.peek()
            && next.is_ascii_digit()
        {
            digits.push(next);
            chars.next();
        }
        if chars.peek() == Some(&'$') {
            chars.next();
            digits.clear();
        }
        // Flags: `-`, `+`, a space, `0`, and `'x` for a padding character of the writer's own.
        loop {
            match chars.peek() {
                Some('-' | '+' | ' ') => {
                    chars.next();
                }
                Some('0') if digits.is_empty() => {
                    chars.next();
                }
                Some('\'') => {
                    chars.next();
                    chars.next();
                }
                _ => break,
            }
        }
        while let Some(&next) = chars.peek()
            && next.is_ascii_digit()
        {
            digits.push(next);
            chars.next();
        }
        if chars.peek() == Some(&'.') {
            chars.next();
            while chars.peek().is_some_and(char::is_ascii_digit) {
                chars.next();
            }
        }
        match chars.next() {
            Some(
                'b' | 'c' | 'd' | 'e' | 'E' | 'f' | 'F' | 'g' | 'G' | 'h' | 'H' | 'o' | 's' | 'u'
                | 'x' | 'X',
            ) => {}
            _ => return None,
        }
        let width = digits.parse::<usize>().unwrap_or(0);
        text.extend(std::iter::repeat_n('0', width.min(2)));
    }
    Some(BuiltinTy::string_holding(text.as_bytes()))
}

/// The hole markers a skeleton holds, which has to match the expressions written into it.
fn hole_count(written: &str) -> usize {
    written.matches(INTERPOLATION_HOLE).count()
}

/// Whether PHP's date parser certainly reads this string.
///
/// A recogniser and not a parser: it says yes only to shapes whose meaning is not in doubt, and no
/// to everything else including strings PHP would in fact accept. Being wrong in that direction
/// costs a finding nobody gets; being wrong in the other would put an error on working code.
///
/// `holes` says what each marker in an interpolated skeleton holds, in the order they were written,
/// and is empty for a string the caller wrote out. Only [`relative_phrase`] consults it: every other
/// shape here turns on a *spelling* rather than a count, and a type cannot promise a value renders
/// as a month name or as four digits of a year.
fn reads_as_a_date(written: &str, holes: &[bool]) -> bool {
    let text = written.trim().to_ascii_lowercase();
    if text.is_empty() {
        return false;
    }
    // `@1700000000` -- a timestamp, and the one spelling with no ambiguity at all.
    if let Some(digits) = text.strip_prefix('@') {
        return !digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_digit());
    }
    if matches!(
        text.as_str(),
        "now" | "today" | "tomorrow" | "yesterday" | "midnight" | "noon"
    ) {
        return true;
    }
    if text.starts_with("first day of ") || text.starts_with("last day of ") {
        // The remainder is a month or a relative phrase, and both are read. The prefix alone is
        // enough to be sure the whole is a date expression rather than a mistake -- unless a hole is
        // in it, because then the remainder is whatever the value turns out to be and `first day of
        // $month` really can fail.
        return !text.contains(INTERPOLATION_HOLE);
    }
    if is_iso_date(&text) {
        return true;
    }
    relative_phrase(&text, holes)
}

/// `2024-01-31`, with an optional time after it.
fn is_iso_date(text: &str) -> bool {
    let date = text.split_whitespace().next().unwrap_or(text);
    let mut parts = date.split('-');
    let (Some(year), Some(month), Some(day), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return false;
    };
    year.len() == 4
        && [year, month, day]
            .iter()
            .all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
}

/// `-25 days`, `+1 week`, `2 hours 30 minutes` -- one or more signed counts with a unit.
///
/// The count may be a hole, and the unit may not. Which is the whole of what the interpolated case
/// adds: an integer written into a count slot always leaves a phrase PHP reads, sign and all, and
/// nothing about a type says a value spells `months`.
fn relative_phrase(text: &str, holes: &[bool]) -> bool {
    const UNITS: &[&str] = &[
        "sec",
        "secs",
        "second",
        "seconds",
        "min",
        "mins",
        "minute",
        "minutes",
        "hour",
        "hours",
        "day",
        "days",
        "week",
        "weeks",
        "fortnight",
        "fortnights",
        "month",
        "months",
        "year",
        "years",
    ];

    let mut words = text.split_whitespace().peekable();
    let mut pairs = 0;
    let mut holes = holes.iter();
    while let Some(count) = words.next() {
        if !is_a_count(count, &mut holes) {
            return false;
        }
        // `ago` may close the phrase, and only after at least one count.
        let Some(unit) = words.next() else {
            return false;
        };
        if !UNITS.contains(&unit) {
            return false;
        }
        pairs += 1;
        if words.peek() == Some(&"ago") {
            words.next();
            return words.next().is_none() && pairs > 0;
        }
    }
    // Every hole has to have been consumed by a count, or one of them sat somewhere this walk never
    // looked -- which is the case it must refuse rather than the case it settles.
    pairs > 0 && holes.next().is_none()
}

/// Whether one word is a count PHP reads: digits, or a hole that holds an integer.
///
/// A signed hole counts, and that is a statement about PHP rather than a convenience: `--3 months`
/// and `-+3 months` both parse, so a negative value under a written `-` is still a date. What does
/// not count is a hole mixed with digits -- `-{$i}0 months` would read, and the recogniser refuses
/// what it is not sure of rather than reasoning about a rendering.
fn is_a_count<'a>(word: &str, holes: &mut impl Iterator<Item = &'a bool>) -> bool {
    let unsigned = word.strip_prefix(['+', '-']).unwrap_or(word);
    if unsigned == INTERPOLATION_HOLE.to_string() {
        return holes.next() == Some(&true);
    }
    !unsigned.is_empty() && unsigned.bytes().all(|byte| byte.is_ascii_digit())
}

/// Whether `hex2bin` certainly reads this string.
///
/// The whole of its contract: pairs of hexadecimal digits, and `false` for anything else. Unlike
/// the date recogniser beside it this one is exact rather than narrow, because the rule it is
/// reading is exact -- so an empty literal is accepted, `hex2bin('')` being `''`.
fn is_hex_pairs(written: &str) -> bool {
    written.len().is_multiple_of(2) && written.bytes().all(|byte| byte.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::{is_hex_pairs, reads_as_a_date};

    #[test]
    fn the_hex_a_literal_writes_is_read_and_anything_else_keeps_the_union() {
        for written in ["400a", "DEADBEEF", "0f0F", ""] {
            assert!(is_hex_pairs(written), "refused `{written}`");
        }
        // An odd length, a character outside the alphabet, and whitespace -- every one of which
        // really does answer `false`, so the report stays.
        for written in ["400", "zz", "40 0a", "0x40", "40\n"] {
            assert!(!is_hex_pairs(written), "accepted `{written}`");
        }
    }

    /// A skeleton whose one hole holds an integer, which is the shape the interpolated case is for.
    const INTEGER: &[bool] = &[true];

    #[test]
    fn the_shapes_real_code_writes_are_read() {
        for written in [
            "-25 days",
            "+1 week",
            "now",
            "TODAY",
            "  tomorrow  ",
            "2 hours 30 minutes",
            "3 days ago",
            "@1700000000",
            "2024-01-31",
            "2024-01-31 13:45:00",
            "first day of next month",
            "last day of february",
        ] {
            assert!(reads_as_a_date(written, &[]), "refused `{written}`");
        }
    }

    #[test]
    fn a_count_written_as_a_hole_is_read_when_the_hole_holds_an_integer() {
        // Every one of these is on the reference checkout, and PHP reads all of them for any
        // integer -- including a negative one under the written `-`, which renders `--3 months`.
        for written in ["-\u{1} months", "\u{1} days", "+\u{1} days", "-\u{1} month"] {
            assert!(reads_as_a_date(written, INTEGER), "refused `{written}`");
        }
        // The same strings with a hole that holds anything else. `strtotime('- months')` is false,
        // so a `?int` or a string really can fail here.
        for written in ["-\u{1} months", "\u{1} days"] {
            assert!(
                !reads_as_a_date(written, &[false]),
                "accepted `{written}` for a hole that is not an integer"
            );
        }
    }

    #[test]
    fn a_hole_anywhere_but_a_count_keeps_the_union() {
        // Both are on the reference checkout beside the ones above, and both really can fail:
        // `"$year-$month-$day"` for a month outside 1-12, and `"$days[$dW] $which week"` for a unit
        // that is not one. A third holds the count twice over, which renders two numbers running
        // together and is not something to reason about.
        for (written, holes) in [
            ("\u{1}-\u{1}-\u{1}", &[true, true, true][..]),
            ("\u{1} \u{1} week", &[true, true][..]),
            ("-\u{1}\u{1} months", &[true, true][..]),
            ("first day of \u{1}", &[true][..]),
            ("@\u{1}", &[true][..]),
            ("\u{1} months ago please", &[true][..]),
        ] {
            assert!(
                !reads_as_a_date(written, holes),
                "accepted `{written}` with a hole outside a count"
            );
        }
    }

    #[test]
    fn anything_it_cannot_be_sure_of_keeps_the_union() {
        for written in [
            // The typo this must still report.
            "yesterdya",
            "-25 dyas",
            "",
            "   ",
            // A unit with no count and a count with no unit.
            "days",
            "-25",
            // Trailing rubbish after a phrase it would otherwise read.
            "3 days ago please",
            "@",
            "@notdigits",
            // Shapes PHP does read and this deliberately does not: being wrong here costs a
            // finding nobody gets, and being wrong the other way puts an error on working code.
            "next thursday",
            "31/01/2024",
        ] {
            assert!(!reads_as_a_date(written, &[]), "accepted `{written}`");
        }
    }
}
