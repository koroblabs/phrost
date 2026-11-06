//! Fitting a rendered type into the room the feature drawing it has.
//!
//! A shape is the one type whose spelling grows without a bound: `array{a: int, b: string}` names
//! every key the value holds, so a real one out of a project's own code runs to twelve fields and
//! 250 characters. Drawn as a hint it pushes the code off the right of the screen; drawn on one
//! line of a hover card it wraps into a paragraph of punctuation. The type is right in both places
//! and only the room differs, which is why this is here rather than in a renderer: the type table
//! has one answer to give and both callers want it.
//!
//! Over the rendered text, like the two other tests a hint makes before it draws anything. The
//! question is how many characters land on the screen, and that is what the text says.

/// One line of PHP in a popup, in characters.
///
/// A hover card is as wide as the client makes it, so this is a reading width rather than a
/// measurement of anything: past about here a line of types is scanned rather than read, and every
/// client wraps it somewhere of its own choosing anyway.
pub(crate) const WIDEST_LINE: usize = 72;

/// How far each nesting level of a broken shape is pushed in. PSR-12's indent, since the card is
/// PHP.
const INDENT: usize = 4;

/// How many of a shape's fields a card writes before it says how many are left.
///
/// A card is read, not scrolled. Sixteen is above every shape a project writes -- the largest one
/// measured in real code has twelve fields -- and below the one type that has no natural size:
/// `$_SERVER` names forty-two keys, and a hover over the commonest variable in PHP covering the
/// screen is worse than one that shows the first sixteen and says so.
const MOST_FIELDS: usize = 16;

/// Every shape's fields replaced by an ellipsis: `?array{a: int, b: string}` -> `?array{…}`.
///
/// For a place with one line and no scrollbar. The elision is visible on purpose -- a bare `?array`
/// would say the value has no keys worth knowing, which is the opposite of what a shape means.
pub(crate) fn folded_shapes(ty: &str) -> String {
    let mut out = String::with_capacity(ty.len());
    let mut rest = ty;

    while let Some((open, close)) = inline_group(rest) {
        out.push_str(&rest[..=open]);
        // `array{}` names no fields, so there is nothing an ellipsis could stand for.
        if !rest[open + 1..close].trim().is_empty() {
            out.push('…');
        }
        out.push('}');
        rest = &rest[close + 1..];
    }

    out.push_str(rest);
    out
}

/// The same type with the union operands past `widest` replaced by an ellipsis: `A|B|C|D` ->
/// `A|B|…`.
///
/// The other type that grows without a bound, and it grows for a reason a reader recognises: a
/// `match` over eight product types answers with eight classes, and the type of what it returns
/// names every one of them. One real factory's hint ran to 135 characters of union pushed into the
/// line of code it was drawn beside.
///
/// At least one operand always survives, because a hint reading `…` says less than no hint at all,
/// and the ellipsis is deliberately visible: what it stands for is a gesture away in the tooltip
/// rather than something a reader has to guess at.
///
/// Only the operands of the *outermost* union. A `|` inside `array<A|B>` or a shape's field belongs
/// to that group, and eliding it would make the type unreadable rather than shorter.
pub(crate) fn folded_operands(ty: &str, widest: usize) -> String {
    let operands = top_level_operands(ty);
    if operands.len() < 2 {
        return ty.to_owned();
    }

    let mut out = String::with_capacity(ty.len());
    for (position, operand) in operands.iter().enumerate() {
        // The separator and the ellipsis this would need if it stopped here, so the answer is never
        // wider than it was asked to be.
        let would_be = out.chars().count() + operand.chars().count() + 2;
        if position > 0 && would_be > widest {
            out.push_str("|…");
            return out;
        }
        if position > 0 {
            out.push('|');
        }
        out.push_str(operand);
    }
    out
}

/// The operands of the outermost union, or the whole type where it is not one.
fn top_level_operands(ty: &str) -> Vec<&str> {
    let mut found = Vec::new();
    let mut depth = 0_u32;
    let mut start = 0;

    for (at, byte) in ty.bytes().enumerate() {
        match byte {
            b'<' | b'{' | b'(' | b'[' => depth += 1,
            b'>' | b'}' | b')' | b']' => depth = depth.saturating_sub(1),
            b'|' | b'&' if depth == 0 => {
                found.push(&ty[start..at]);
                start = at + 1;
            }
            _ => {}
        }
    }
    found.push(&ty[start..]);
    found
}

/// The same type with any shape too wide for `widest` written down the page, one field a line.
///
/// Takes whole PHP text rather than a type, because that is what a hover card holds: the shape a
/// reader complains about arrives inside `?array{…} $limits`, or inside a method signature nested
/// in the class that declares it. Only a group already written on one line is a candidate, which is
/// what keeps the braces of that class alone -- they are a brace group too, and expanding them
/// would rewrite the card instead of the type in it.
pub(crate) fn over_lines(php: &str, widest: usize) -> String {
    let mut out = String::with_capacity(php.len());

    for (position, line) in php.lines().enumerate() {
        if position > 0 {
            out.push('\n');
        }
        let indent = line.chars().take_while(|char| *char == ' ').count();
        out.push_str(&line[..indent]);
        write_wrapped(&mut out, &line[indent..], indent, widest);
    }
    if php.ends_with('\n') {
        out.push('\n');
    }

    out
}

/// Writes `content`, which starts at column `indent`, breaking the first shape that does not fit.
///
/// Recursive on both halves of what it finds: a field that is itself a wide shape is broken again
/// one level further in, and whatever follows the closing brace is looked at the same way, since a
/// signature can name two shapes.
fn write_wrapped(out: &mut String, content: &str, indent: usize, widest: usize) {
    if indent + content.chars().count() <= widest {
        out.push_str(content);
        return;
    }
    let Some((open, close)) = inline_group(content) else {
        // No shape to break, so the other unbounded type gets its turn: a union of eight classes
        // is written one operand a line, the way a formatter would break it.
        //
        // Only where every operand but the last is a type and nothing else. The last may carry a
        // name -- a variable's card is `<type> $name`, and that is the line a reader complains
        // about -- while a space anywhere earlier means the line is the card's own text rather than
        // one type: `function f(A|B $x): void` keeps its shape, and its `|` is inside the
        // parentheses anyway, which the operand scan already leaves alone.
        let operands = top_level_operands(content);
        let breakable = operands.len() > 1
            && operands
                .iter()
                .rev()
                .skip(1)
                .all(|operand| !operand.contains(' '));
        if breakable {
            for (position, operand) in operands.iter().enumerate() {
                if position > 0 {
                    out.push('\n');
                    for _ in 0..indent {
                        out.push(' ');
                    }
                    out.push('|');
                }
                out.push_str(operand);
            }
            return;
        }
        out.push_str(content);
        return;
    };
    let fields = fields(&content[open + 1..close]);
    if fields.is_empty() {
        out.push_str(&content[..=close]);
        write_wrapped(out, &content[close + 1..], indent, widest);
        return;
    }

    out.push_str(&content[..=open]);
    let inner = indent + INDENT;
    for (position, field) in fields.iter().take(MOST_FIELDS).enumerate() {
        if position > 0 {
            out.push(',');
        }
        out.push('\n');
        for _ in 0..inner {
            out.push(' ');
        }
        write_wrapped(out, field, inner, widest);
    }
    if let Some(rest) = fields
        .len()
        .checked_sub(MOST_FIELDS)
        .filter(|rest| *rest > 0)
    {
        out.push(',');
        out.push('\n');
        for _ in 0..inner {
            out.push(' ');
        }
        out.push_str(&format!("… {rest} more"));
    }
    out.push('\n');
    for _ in 0..indent {
        out.push(' ');
    }
    out.push('}');
    write_wrapped(out, &content[close + 1..], indent, widest);
}

/// The braces of the first shape written on one line, as byte offsets of the braces themselves.
///
/// Not a parser, and the same bargain [`super::hover`]'s value reader makes: it looks for one
/// delimiter and steps over quotes and nesting blindly, which is enough because the text is known
/// to be a rendered type and being wrong costs a card that reads as it did before.
fn inline_group(text: &str) -> Option<(usize, usize)> {
    let mut quote = None::<u8>;
    let mut escaped = false;
    let mut open = None::<usize>;
    let mut depth = 0_u32;

    for (at, byte) in text.bytes().enumerate() {
        match quote {
            Some(char) => match byte {
                _ if escaped => escaped = false,
                b'\\' => escaped = true,
                _ if byte == char => quote = None,
                _ => {}
            },
            None => match byte {
                b'\'' | b'"' => quote = Some(byte),
                b'{' => {
                    open.get_or_insert(at);
                    depth += 1;
                }
                b'}' => {
                    depth = depth.saturating_sub(1);
                    if depth == 0
                        && let Some(start) = open
                    {
                        return Some((start, at));
                    }
                }
                // A group whose brace opened on an earlier line is somebody else's: a class body,
                // written across lines by the card that nests a member in it.
                b'\n' => {
                    open = None;
                    depth = 0;
                }
                _ => {}
            },
        }
    }

    None
}

/// A shape's fields, split at the commas that belong to it.
///
/// `data: array<string, int>` holds a comma that is the generic's, and `run: callable(...): void`
/// one that could be a parameter list's, so the split counts every bracket rather than only the
/// braces.
fn fields(inner: &str) -> Vec<&str> {
    let mut found = Vec::new();
    let mut quote = None::<u8>;
    let mut escaped = false;
    let mut depth = 0_i32;
    let mut start = 0;

    for (at, byte) in inner.bytes().enumerate() {
        match quote {
            Some(char) => match byte {
                _ if escaped => escaped = false,
                b'\\' => escaped = true,
                _ if byte == char => quote = None,
                _ => {}
            },
            None => match byte {
                b'\'' | b'"' => quote = Some(byte),
                b'{' | b'<' | b'(' => depth += 1,
                b'}' | b'>' | b')' => depth -= 1,
                b',' if depth == 0 => {
                    found.push(inner[start..at].trim());
                    start = at + 1;
                }
                _ => {}
            },
        }
    }

    let last = inner[start..].trim();
    if !last.is_empty() {
        found.push(last);
    }
    found.retain(|field| !field.is_empty());
    found
}

#[cfg(test)]
mod tests {
    use expect_test::expect;

    use super::{WIDEST_LINE, folded_operands, folded_shapes, over_lines};

    /// The type from the report this exists for, as an editor drew it into the line.
    const LIMITS: &str = "?array{package_type: string, weight_limit: string, distance_limit: int, gift_wrap: bool, gift_value: int, gift_amount: int, max_days_since_order: int, discount_reduction: float, rebate_rate: float, rebate_rate_floor: float, payout: bool, payout_max: int}";

    #[test]
    fn a_shape_folds_to_its_brace() {
        assert_eq!(folded_shapes(LIMITS), "?array{…}");
        assert_eq!(
            folded_shapes("array<string, array{a: int}>"),
            "array<string, array{…}>"
        );
        // Nothing to stand for, and nothing to fold in a type that has no shape at all.
        assert_eq!(folded_shapes("array{}"), "array{}");
        assert_eq!(folded_shapes("?Repo|int"), "?Repo|int");
    }

    /// A shape with no natural size says how much it left out.
    ///
    /// `$_SERVER` is the one that forces this: forty-two documented keys, and a card that writes
    /// them all covers the editor. The count is part of the answer -- a truncation that does not
    /// say it truncated reads as the type being smaller than it is.
    #[test]
    fn a_shape_past_the_cap_says_how_many_are_left() {
        let fields = (0..20)
            .map(|n| format!("k{n}: string"))
            .collect::<Vec<_>>()
            .join(", ");
        let broken = over_lines(&format!("array{{{fields}}}"), WIDEST_LINE);
        let lines = broken.lines().collect::<Vec<_>>();

        assert_eq!(
            lines.len(),
            19,
            "the brace, sixteen fields, the tail, the close"
        );
        assert_eq!(lines[17].trim(), "… 4 more");
        assert_eq!(lines[1].trim(), "k0: string,");
    }

    #[test]
    fn a_wide_shape_is_written_down_the_page() {
        expect![[r#"
            ?array{
                package_type: string,
                weight_limit: string,
                distance_limit: int,
                gift_wrap: bool,
                gift_value: int,
                gift_amount: int,
                max_days_since_order: int,
                discount_reduction: float,
                rebate_rate: float,
                rebate_rate_floor: float,
                payout: bool,
                payout_max: int
            } $limits"#]]
        .assert_eq(&over_lines(&format!("{LIMITS} $limits"), WIDEST_LINE));
    }

    #[test]
    fn a_type_that_fits_is_left_alone() {
        let narrow = "array{app: Repo, count: int} $shape";
        assert_eq!(over_lines(narrow, WIDEST_LINE), narrow);
    }

    /// The braces of the class a member is nested in opened on an earlier line, so they are not a
    /// group this may touch -- and the field that is itself a wide shape is broken again, indented
    /// from where its own line starts rather than from the left.
    #[test]
    fn a_nested_shape_breaks_from_the_column_it_starts_at() {
        let card = "<?php\nclass Rules\n{\n    public function limits(): array{first: array{package_type: string, weight_limit: string, distance_limit: int}, second: int}\n}";

        expect![[r#"
            <?php
            class Rules
            {
                public function limits(): array{
                    first: array{
                        package_type: string,
                        weight_limit: string,
                        distance_limit: int
                    },
                    second: int
                }
            }"#]]
        .assert_eq(&over_lines(card, WIDEST_LINE));
    }

    /// A generic's comma is not a field's, and neither is a callable's.
    #[test]
    fn the_split_counts_brackets() {
        expect![[r#"
            array{
                data: array<string, int>,
                run: callable(...): void,
                rows: list<array<int, string>>
            }"#]]
        .assert_eq(&over_lines(
            "array{data: array<string, int>, run: callable(...): void, rows: list<array<int, string>>}",
            WIDEST_LINE,
        ));
    }

    /// A union of classes, kept up to the width it was given.
    ///
    /// One real factory's `match` over eight product types, and the hint drawn from it is longer
    /// than the line of code it sits in.
    #[test]
    fn a_wide_union_keeps_the_operands_that_fit() {
        let ty = "DigitalGoodsCoordinator|PerishableCoordinator|BulkOrderCoordinator|ExpressOvernightCoordinator";

        assert_eq!(
            folded_operands(ty, 48),
            "DigitalGoodsCoordinator|PerishableCoordinator|…"
        );
        // Never wider than it was asked to be, and never narrower than one operand: an ellipsis on
        // its own says less than no hint.
        assert_eq!(folded_operands(ty, 4), "DigitalGoodsCoordinator|…");
        assert_eq!(folded_operands("Repo", 48), "Repo");
    }

    /// A `|` inside a group belongs to that group.
    #[test]
    fn a_union_inside_a_generic_is_not_an_operand() {
        let ty = "array<int, DigitalGoodsCoordinator|PerishableCoordinator|BulkOrderCoordinator>";

        assert_eq!(folded_operands(ty, 20), ty);
    }

    /// The card gets the whole type, one operand a line, because nothing in a union is a space to
    /// wrap at.
    #[test]
    fn a_wide_union_on_a_card_is_written_down_the_page() {
        let ty = "DigitalGoodsCoordinator|PerishableCoordinator|BulkOrderCoordinator|ExpressOvernightCoordinator";

        assert_eq!(
            over_lines(ty, WIDEST_LINE),
            "DigitalGoodsCoordinator\n|PerishableCoordinator\n|BulkOrderCoordinator\n|ExpressOvernightCoordinator"
        );
    }

    /// And a line that is not only a type is left alone: a signature's own `|` is inside its
    /// parentheses, where the operand scan does not look.
    #[test]
    fn a_line_that_is_not_only_a_type_keeps_its_shape() {
        let signature = "function make(DigitalGoodsCoordinator|PerishableCoordinator|BulkOrderCoordinator $u): void";

        assert_eq!(over_lines(signature, WIDEST_LINE), signature);
    }

    /// A variable's card is `<type> $name`, and the name rides on the last operand: that line is
    /// the 207-character one a reader actually meets.
    #[test]
    fn a_named_union_breaks_and_keeps_its_name_on_the_last_operand() {
        let card =
            "DigitalGoodsCoordinator|PerishableCoordinator|ClearanceLotCoordinator $coordinator";

        assert_eq!(
            over_lines(card, WIDEST_LINE),
            "DigitalGoodsCoordinator\n|PerishableCoordinator\n|ClearanceLotCoordinator $coordinator"
        );
    }
}
