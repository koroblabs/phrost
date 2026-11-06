//! What the Drupal standard asks for and the preset cannot say.
//!
//! mago's Drupal preset is several dozen settled decisions and these are the ones left over: a
//! construct Drupal's own sniffs judge, that no setting reaches. Each is a respelling of the
//! formatter's output rather than a second formatter -- text in, text out, with the reason the
//! standard gives written beside it.
//!
//! Both are text-level for the same reason: the formatter is a closed decision tree over a tree
//! phrost may not see. Where a respelling needs to know what a construct *is* rather than what it
//! looks like, it parses the formatter's output and reads spans off that -- which is what keeps
//! `hugging_array_operands` from mistaking a `+` inside a heredoc for an operator.

use mago_span::HasSpan;
use mago_syntax::cst::{Expression, Literal, Node};
use phrost_syntax::ParseSession;

mod joins;

/// The formatter's output, respelled for the sniffs.
pub(super) fn respelled(formatted: &str) -> String {
    let hugged = joins::hugging_array_operands(formatted);
    let spaced = spaced_catch_unions(&hugged);
    let concatenated = broken_string_concat(&spaced);
    let matched = broken_single_arm_match(&concatenated);
    sniffed_array_indentation(&matched)
}

/// The width the Drupal preset is printed at -- see the formatter's settings for why it is not 120.
pub(super) const WIDTH: usize = 200;

/// Whether anything in `text` could be an array declaration past the column limit, cheaply.
///
/// The other half of the same gate. A line shorter than the limit cannot hold an array that reaches
/// it, whatever is on the line.
fn a_line_reaches_the_limit(text: &str) -> bool {
    text.lines().any(|line| line.len() >= LONGEST_DECLARATION)
}

/// Whether anything in `text` could be a hugged closing bracket, without parsing to find out.
///
/// This reads the formatter's *own* output, and there a multi-line array either closes on a line of
/// its own or closes inside a run of closing delimiters -- `))]);` -- so a line whose first
/// character is a closer and which holds a `]` further along is the whole of the shape. A hand-
/// written `'a' => 1];` would slip past this, and cannot arrive here.
fn a_line_closes_over_a_bracket(text: &str) -> bool {
    text.lines().any(|line| {
        let trimmed = line.trim_start();
        trimmed.starts_with([')', ']']) && trimmed.get(1..).is_some_and(|rest| rest.contains(']'))
    })
}

/// `catch (A|B $e)` written `catch (A | B $e)`, which is how Drupal writes it.
///
/// Drupal's sniff reads the `|` between two caught classes as an operator and asks for a space on
/// either side -- core writes `catch (FileException | InvalidStreamWrapperException $e)` throughout
/// -- and the formatter has no setting for it, so the one construct is respelled here. Only inside a
/// `catch (...)` whose parentheses hold nothing but class names, `|` and the variable: a union in a
/// signature stays `int|string`, which the same sniff leaves alone.
fn spaced_catch_unions(formatted: &str) -> String {
    let mut out = String::with_capacity(formatted.len());
    let mut rest = formatted;
    while let Some(at) = rest.find("catch (") {
        let (before, from) = rest.split_at(at + "catch (".len());
        out.push_str(before);
        let Some(close) = from.find(')') else {
            rest = from;
            break;
        };
        let (inside, after) = from.split_at(close);
        let is_a_type_list = inside.contains('|')
            && !inside.contains(" |")
            && !inside.contains("| ")
            && inside
                .chars()
                .all(|ch| ch.is_alphanumeric() || matches!(ch, '\\' | '|' | '_' | '$' | ' '));
        match is_a_type_list {
            true => out.push_str(&inside.replace('|', " | ")),
            false => out.push_str(inside),
        }
        rest = after;
    }
    out.push_str(rest);
    out
}

/// The column `Drupal.Arrays.Array.LongLineDeclaration` allows a one-line array to reach.
///
/// Named for what it is: the *only* error-level line limit the standard has, and it is about an
/// array declaration rather than about a line -- which is why the formatter may run to 200 columns
/// and an array may not.
const LONGEST_DECLARATION: usize = 120;

/// `text` with a line break written into the first array literal the sniffs refuse, or nothing when
/// it holds none.
///
/// `Squiz.Arrays.ArrayDeclaration.CloseBraceNewLine`: an array declaration that spans lines must
/// put its closing bracket on a line of its own. mago hugs instead -- an argument list keeps a
/// trailing array literal inline and breaks *deeper*, so
/// `responseError($status, [sprintf($message, implode(', ', $states))])` comes back with the
/// `implode(` broken and the array's `]` welded to `)]);`. The author's own line was 190 characters
/// and had no error; the reprint has one.
///
/// Two elements in that array and mago breaks it properly, which is what says the shape is the
/// single-element hug rather than anything about arrays. Measured: **0** of these written by hand in
/// the reference project's 1 673 files and **2** produced by formatting them -- and 15 written by
/// hand in one Symfony component, which is why this is Drupal's respelling and not everyone's.
///
/// The break is written into the formatter's *input* rather than its output because expanding an
/// array means re-indenting everything inside it, and that is the formatter's job rather than a
/// respelling's. One newline after the `[` is enough: `preserve_breaking_array_like` reads a break
/// an author wrote as intent, so the next pass keeps the array open and lays the inside out itself.
pub(super) fn hinted_array_break(text: &str) -> Option<String> {
    if !a_line_closes_over_a_bracket(text) && !a_line_reaches_the_limit(text) {
        return None;
    }

    let session = ParseSession::new();
    let parsed = session.parse(text);
    if !parsed.errors().is_empty() {
        return None;
    }

    let mut first: Option<usize> = None;
    let mut pending = vec![Node::Program(parsed.program())];
    while let Some(node) = pending.pop() {
        // A `list(...)` is a target rather than a declaration, so the sniff has nothing to say
        // about it and nothing here writes into one.
        let brackets = match node {
            Node::Array(array) => Some((array.left_bracket, array.right_bracket)),
            Node::LegacyArray(array) => Some((array.left_parenthesis, array.right_parenthesis)),
            _ => None,
        };
        if let Some((open, close)) = brackets {
            let open = open.end.offset as usize;
            let close = close.start.offset as usize;
            let line = text.get(..close)?.rfind('\n').map_or(0, |at| at + 1);
            // Two shapes, and one hint settles both. Either it spans lines and the closing bracket
            // has code before it on its own line -- `CloseBraceNewLine` -- or it is written on one
            // line that reaches past column 120, which is `LongLineDeclaration`: the one
            // error-level line limit Drupal has, and it is about an array rather than a line.
            let hugged = line > open && !text.get(line..close)?.trim().is_empty();
            let too_wide = line == text.get(..open)?.rfind('\n').map_or(0, |at| at + 1)
                && close - line >= LONGEST_DECLARATION;
            if (hugged || too_wide) && first.is_none_or(|held| open < held) {
                first = Some(open);
            }
        }
        node.visit_children(|child| pending.push(child));
    }

    let at = first?;
    let mut out = String::with_capacity(text.len() + 1);
    out.push_str(text.get(..at)?);
    out.push('\n');
    out.push_str(text.get(at..)?);
    Some(out)
}

/// A `match` written on one line gets a line for its arm and one for each brace.
///
/// `return match ($this) { self::TELECHECK_FAILED => 'x' };` is three findings -- one for the space
/// after the opening brace, one for the space before the closing one, and one for the closing brace
/// not being on a line of its own. mago keeps a `match` inline unless it has several arms or one of
/// them spans lines, so the single-arm case is the whole of it.
///
/// A respelling rather than a hint, and that difference is the lesson: hinting it -- a newline
/// inside the arm, which is the fact mago reads -- works for one pass and is undone by the next,
/// because the formatter reprints the arm on one line again and reads *that* as inline. Anything
/// the formatter would undo has to be done after it, every time.
fn broken_single_arm_match(text: &str) -> String {
    let mut text = text.to_owned();
    for _ in 0..MOST_BREAKS {
        let Some(broken) = broken_match_once(&text) else {
            break;
        };
        text = broken;
    }
    text
}

/// `text` with the first one-line `match` broken open.
fn broken_match_once(text: &str) -> Option<String> {
    if !text.contains("match (") {
        return None;
    }

    let session = ParseSession::new();
    let parsed = session.parse(text);
    if !parsed.errors().is_empty() {
        return None;
    }

    let mut first: Option<(usize, usize)> = None;
    let mut pending = vec![Node::Program(parsed.program())];
    while let Some(node) = pending.pop() {
        if let Node::Match(r#match) = node {
            let open = r#match.left_brace.end.offset as usize;
            let close = r#match.right_brace.start.offset as usize;
            if !text.get(open..close)?.contains('\n')
                && !text.get(open..close)?.trim().is_empty()
                && first.is_none_or(|(held, _)| open < held)
            {
                first = Some((open, close));
            }
        }
        node.visit_children(|child| pending.push(child));
    }

    let (open, close) = first?;
    let (_, indent) = line_at(text, open);
    let arm = text.get(open..close)?.trim();
    let comma = match arm.ends_with(',') {
        true => "",
        false => ",",
    };
    let mut out = String::with_capacity(text.len() + indent * 2 + 4);
    out.push_str(text.get(..open)?);
    out.push('\n');
    out.push_str(&" ".repeat(indent + 2));
    out.push_str(arm);
    out.push_str(comma);
    out.push('\n');
    out.push_str(&" ".repeat(indent));
    out.push_str(text.get(close..)?);
    Some(out)
}

/// A guard against a pathological input, like the cap on joins: each pass adds one break.
const MOST_BREAKS: usize = 64;

/// Two written-out strings joined by a `.` keep a line each.
///
/// `Generic.Strings.UnnecessaryStringConcat`, which Drupal's standard includes, reports `'a' . 'b'`
/// on one line and says to write one string instead. The author wrote them on two lines *because*
/// the line would otherwise be long, and the formatter joins them: 6 findings on the reference
/// project, and **none of them there before the reprint**. So the break goes back.
///
/// Only where the operands either side of the `.` are both written-out strings, which is the whole
/// of what the sniff reports -- `'a' . $b` says something a single string cannot and is left alone.
/// The continuation takes its line's own indentation, which is how the author wrote it and what
/// reads best; the sniff accepts that and two columns further in alike.
fn broken_string_concat(text: &str) -> String {
    let mut text = text.to_owned();
    for _ in 0..MOST_BREAKS {
        let Some(broken) = broken_once(&text) else {
            break;
        };
        text = broken;
    }
    text
}

/// `text` with the first joined pair of strings put back on two lines.
fn broken_once(text: &str) -> Option<String> {
    if !text.contains("' . '") && !text.contains("\" . \"") && !text.contains("' . \"") {
        return None;
    }

    let session = ParseSession::new();
    let parsed = session.parse(text);
    if !parsed.errors().is_empty() {
        return None;
    }

    let mut first: Option<usize> = None;
    let mut pending = vec![Node::Program(parsed.program())];
    while let Some(node) = pending.pop() {
        if let Node::Binary(binary) = node
            && binary.operator.is_concatenation()
            && written_string(binary.rhs)
            && rightmost_written_string(binary.lhs)
        {
            let at = binary.operator.span().start.offset as usize;
            let from = binary.lhs.span().end.offset as usize;
            if !text.get(from..at)?.contains('\n') && first.is_none_or(|held| at < held) {
                first = Some(at);
            }
        }
        node.visit_children(|child| pending.push(child));
    }

    let at = first?;
    let (_, indent) = line_at(text, at);
    let mut out = String::with_capacity(text.len() + indent + 1);
    out.push_str(text.get(..at)?.trim_end_matches(' '));
    out.push('\n');
    out.push_str(&" ".repeat(indent));
    out.push_str(text.get(at..)?);
    Some(out)
}

/// Whether the expression is a string written out in the source, through any parentheses.
fn written_string(expression: &Expression<'_>) -> bool {
    match expression {
        Expression::Parenthesized(parenthesized) => written_string(parenthesized.expression),
        Expression::Literal(Literal::String(_)) => true,
        _ => false,
    }
}

/// Whether a concatenation chain *ends* in a written-out string, which is the operand the next `.`
/// sits beside.
fn rightmost_written_string(expression: &Expression<'_>) -> bool {
    match expression {
        Expression::Parenthesized(parenthesized) => {
            rightmost_written_string(parenthesized.expression)
        }
        Expression::Binary(binary) if binary.operator.is_concatenation() => {
            rightmost_written_string(binary.rhs)
        }
        _ => written_string(expression),
    }
}

/// Every line of a multi-line array indented the way `Drupal.Arrays.Array.ArrayIndentation` asks.
///
/// The one respelling that reimplements a sniff, and it earns that because mago's indentation model
/// and Drupal's disagree in a way no setting reaches. The sniff's rule is short: take the first
/// token on the line the array's `[` is on, and every later line of that array starts two columns
/// further in -- four for a line opening with `->`, `?` or `:` -- with the closing bracket back at
/// the first. It jumps over a nested array (checked in its own right), over anything inside
/// parentheses, and over a heredoc.
///
/// mago instead indents a continuation from the construct that broke, which for a binary operand or
/// a ternary inside an array element is two columns further in than the sniff allows. Measured with
/// phpcs itself over the reference project's 1 673 files: it is the single largest source of errors
/// the formatter introduced, and every one of them is this arithmetic.
///
/// Applied to the formatter's own output, after the joins above, and outermost array first so that a
/// nested one is measured from where its opening line *will* be.
pub(super) fn sniffed_array_indentation(text: &str) -> String {
    let session = ParseSession::new();
    let parsed = session.parse(text);
    if !parsed.errors().is_empty() {
        return text.to_owned();
    }

    let mut arrays: Vec<(usize, usize)> = Vec::new();
    let mut skips: Vec<(usize, usize)> = Vec::new();
    let mut pending = vec![Node::Program(parsed.program())];
    while let Some(node) = pending.pop() {
        match node {
            Node::Array(array) => arrays.push((
                array.left_bracket.start.offset as usize,
                array.right_bracket.start.offset as usize,
            )),
            Node::LegacyArray(array) => arrays.push((
                array.left_parenthesis.start.offset as usize,
                array.right_parenthesis.start.offset as usize,
            )),
            // What the sniff steps over: a call's or a declaration's parentheses, a grouped
            // expression, and the bodies that carry their own indentation.
            Node::ArgumentList(_)
            | Node::Parenthesized(_)
            | Node::FunctionLikeParameterList(_)
            | Node::DocumentString(_)
            | Node::Closure(_)
            | Node::ArrowFunction(_)
            | Node::AnonymousClass(_)
            | Node::Match(_) => {
                // To the closer's *start*, not past it: the sniff reads the token that opens a line
                // and a closing delimiter is no longer inside the pair it closes -- so a `),` line
                // is checked, and the argument lines above it are not.
                let span = node.span();
                let closer = (span.end.offset as usize).saturating_sub(1);
                skips.push((span.start.offset as usize, closer));
            }
            _ => {}
        }
        node.visit_children(|child| pending.push(child));
    }
    // A line whose first byte is inside a string is the string's, not the layout's.
    for range in phrost_syntax::string_ranges(text) {
        skips.push((usize::from(range.start()), usize::from(range.end())));
    }
    arrays.sort_unstable();

    let mut wanted: std::collections::BTreeMap<usize, usize> = std::collections::BTreeMap::new();
    for (open, close) in &arrays {
        let (open_line, written) = line_at(text, *open);
        let (close_line, _) = line_at(text, *close);
        if open_line == close_line {
            continue;
        }
        // Where the opening line will be once the array around this one has had its say.
        let base = wanted.get(&open_line).copied().unwrap_or(written);

        let mut at = open_line;
        while at <= close_line {
            let (start, indent) = line_at(text, at);
            let end = text[start..]
                .find('\n')
                .map_or(text.len(), |offset| start + offset);
            let written = text.get(start + indent..end).unwrap_or_default();
            // A nested array is stepped over whole, its closing bracket's line included: that line
            // is the nested array's own to answer for. Everything else is measured from the line's
            // *first token* rather than its start, because a closing delimiter is no longer inside
            // the pair it closes -- which is why a `),` line is checked and the argument lines
            // above it are not.
            let first_token = start + indent;
            let nested = arrays
                .iter()
                .any(|(other, until)| other != open && *other < start && start < *until)
                || skips
                    .iter()
                    .any(|(from, until)| *from < first_token && first_token < *until);
            if start > open_line && !written.is_empty() && !nested {
                let extra = match written.starts_with("->")
                    || written.starts_with('?')
                    || written.starts_with(':')
                {
                    true => 4,
                    false => 2,
                };
                let target = match start == close_line {
                    true => base,
                    false => base + extra,
                };
                wanted.insert(start, target);
            }
            at = end + 1;
        }
    }

    reindented(text, &wanted)
}

/// `text` with the leading spaces of each named line replaced.
///
/// A line that opens with anything but spaces is left alone: the preset writes no tabs, and a
/// respelling that cannot count the indentation must not guess at it.
fn reindented(text: &str, wanted: &std::collections::BTreeMap<usize, usize>) -> String {
    let mut out = String::with_capacity(text.len());
    let mut at = 0;
    for (start, target) in wanted {
        let (_, indent) = line_at(text, *start);
        if indent == *target || *start < at {
            continue;
        }
        let Some(before) = text.get(at..*start) else {
            continue;
        };
        out.push_str(before);
        out.push_str(&" ".repeat(*target));
        at = start + indent;
    }
    out.push_str(text.get(at..).unwrap_or_default());
    out
}

/// The start offset of the line holding `at`, and how many spaces it opens with.
fn line_at(text: &str, at: usize) -> (usize, usize) {
    let start = text
        .get(..at)
        .and_then(|before| before.rfind('\n'))
        .map_or(0, |offset| offset + 1);
    let indent = text
        .get(start..)
        .map(|line| line.bytes().take_while(|byte| *byte == b' ').count())
        .unwrap_or_default();
    (start, indent)
}

#[cfg(test)]
mod tests {
    use expect_test::expect;

    use crate::format::{FormatStyle, format, formatted};

    /// What a caller gets for Drupal, and the assertion that the style is a fixpoint: a second
    /// format has nothing to do. Every test here goes through it, because a respelling that is not
    /// stable is worse than none -- format-on-save would rewrite the file on every save.
    pub(super) fn drupal(source: &str) -> String {
        let out = formatted(source, FormatStyle::Drupal);
        assert_eq!(format(&out, FormatStyle::Drupal), None, "not a fixpoint");
        out
    }

    /// `Drupal.Arrays.Array.ArrayIndentation` measures every line of an array from the line its
    /// `[` is on, and mago measures a continuation from the construct that broke -- two columns
    /// further in for a binary operand or a ternary inside an element. This is the arithmetic the
    /// sniff asks for, applied to the formatter's own output.
    #[test]
    fn every_line_of_an_array_is_two_columns_in_from_where_it_opened() {
        let out = drupal(
            "<?php\n\nclass Repo {\n\n  public function f(array $q, array $rows): array {\n    return [\n      'seen' => is_array($q) && in_array('a very long needle indeed, long enough on its own that the whole element has to be broken somewhere by the formatter, and long enough again that no width short of nonsense would hold it', $q, TRUE),\n      'count' => count($rows) > 0 ? $this->countEveryOneOfTheRowsThatMatterHereAndAlsoOverThere($rows) : $this->fallbackCountForAnEmptySetOfRowsInsteadOfTheOther($rows),\n    ];\n  }\n\n}\n",
        );

        expect![[r#"
            <?php

            class Repo {

              public function f(array $q, array $rows): array {
                return [
                  'seen' =>
                  is_array($q)
                  && in_array(
                        'a very long needle indeed, long enough on its own that the whole element has to be broken somewhere by the formatter, and long enough again that no width short of nonsense would hold it',
                        $q,
                        TRUE,
                  ),
                  'count' => count($rows) > 0 ? $this->countEveryOneOfTheRowsThatMatterHereAndAlsoOverThere($rows) : $this->fallbackCountForAnEmptySetOfRowsInsteadOfTheOther($rows),
                ];
              }

            }
        "#]]
        .assert_eq(&out);
    }

    /// The one error-level line limit Drupal has is about an array declaration rather than a line:
    /// `LongLineDeclaration` past column 120. So the formatter may run to 200 and an array may not.
    #[test]
    fn an_array_declaration_past_column_120_is_broken_open() {
        let out = drupal(
            "<?php\n\nclass Repo {\n\n  public function f(): bool {\n    return $this->decided() && in_array($this->stage(), [Stage::APPROVED, Stage::DECLINED, Stage::WITHDRAWN, Stage::CANCELLED], TRUE);\n  }\n\n}\n",
        );

        expect![[r#"
            <?php

            class Repo {

              public function f(): bool {
                return $this->decided()
                && in_array(
                  $this->stage(),
                  [
                    Stage::APPROVED,
                    Stage::DECLINED,
                    Stage::WITHDRAWN,
                    Stage::CANCELLED,
                  ],
                  TRUE,
                );
              }

            }
        "#]]
        .assert_eq(&out);
    }

    /// `Generic.Strings.UnnecessaryStringConcat` reports two written-out strings joined on one
    /// line, and the author put them on two *because* the line would otherwise be long. Six
    /// findings on the reference project, none of them there before the reprint.
    #[test]
    fn two_written_out_strings_keep_a_line_each() {
        let out = drupal(
            "<?php\n\nclass Repo {\n\n  public function f(): void {\n    throw new \\RuntimeException('Warehouse config `app_core.app_settings` is missing `version` or ' . '`companyName` values required for the User Agent header.');\n  }\n\n}\n",
        );

        expect![[r#"
            <?php

            class Repo {

              public function f(): void {
                throw new \RuntimeException('Warehouse config `app_core.app_settings` is missing `version` or '
                . '`companyName` values required for the User Agent header.');
              }

            }
        "#]]
        .assert_eq(&out);
    }

    /// A one-line `match` is three findings: the space after `{`, the space before `}`, and the
    /// closing brace not being on a line of its own.
    #[test]
    fn a_match_written_on_one_line_gets_a_line_for_each_brace() {
        let out = drupal(
            "<?php\n\nclass Repo {\n\n  public function label(): string {\n    return match ($this) { self::TELECHECK_FAILED => 'Telecheck report failed' };\n  }\n\n}\n",
        );

        expect![[r#"
            <?php

            class Repo {

              public function label(): string {
                return match ($this) {
                  self::TELECHECK_FAILED => 'Telecheck report failed',
                };
              }

            }
        "#]]
        .assert_eq(&out);
    }

    /// The other half of the same rule: a string written over several lines carries its own
    /// newlines, so its later lines are the value and stay exactly where they are while the array
    /// around them comes back.
    #[test]
    fn a_string_written_over_several_lines_keeps_its_own_bytes() {
        let out = drupal(
            "<?php\n\nclass Repo {\n\n  public function f(array $base): array {\n    return [\n      'body' => $base\n        + [\n          'text' => 'line one\nline two',\n        ],\n    ];\n  }\n\n}\n",
        );

        expect![[r#"
            <?php

            class Repo {

              public function f(array $base): array {
                return [
                  'body' => $base + [
                    'text' => 'line one
            line two',
                  ],
                ];
              }

            }
        "#]]
        .assert_eq(&out);
    }

    /// `Squiz.Arrays.ArrayDeclaration.CloseBraceNewLine`: an array that spans lines closes on a
    /// line of its own. mago hugs a trailing single-element array to the argument list and breaks
    /// *deeper*, so the author's one long line came back as an error -- and the way out is to write
    /// the break into the formatter's input and ask again, because expanding an array means
    /// re-laying out its contents and that is the formatter's job.
    #[test]
    fn an_array_that_spans_lines_closes_on_a_line_of_its_own() {
        let out = drupal(
            "<?php\n\nclass Repo {\n\n  public function run(array $states): object {\n    return $this->error(static::STATUS, [sprintf('Product type bulk_dispatches is not configured for `%s` in the regional distribution, so the fulfillment cannot continue.', implode(', ', $states), $this->distributorName(), $this->productName())]);\n  }\n\n}\n",
        );

        expect![[r#"
            <?php

            class Repo {

              public function run(array $states): object {
                return $this->error(static::STATUS, [
                  sprintf(
                    'Product type bulk_dispatches is not configured for `%s` in the regional distribution, so the fulfillment cannot continue.',
                    implode(', ', $states),
                    $this->distributorName(),
                    $this->productName(),
                  ),
                ]);
              }

            }
        "#]]
        .assert_eq(&out);
    }

    /// The hint is written once per array and the formatter keeps it, so a file already in the
    /// style needs no edit -- which is what makes format-on-save safe here.
    #[test]
    fn an_array_already_closing_on_its_own_line_needs_no_hint() {
        let already = "<?php\n\nclass Repo {\n\n  public function run(array $states): object {\n    return $this->error(static::STATUS, [\n      sprintf(\n        'Product type bulk_dispatches is not configured for `%s` in the regional distribution, so the fulfillment cannot continue.',\n        implode(', ', $states),\n        $this->distributorName(),\n        $this->productName(),\n      ),\n    ]);\n  }\n\n}\n";

        assert_eq!(format(already, FormatStyle::Drupal), None);
    }

    /// Drupal-only, and this one is measured rather than assumed: Symfony's Console component
    /// writes fifteen of these by hand, so the shape is a house-style choice and not a mistake.
    #[test]
    fn another_style_keeps_the_hug() {
        let out = formatted(
            "<?php\n\nclass Repo\n{\n    public function run(array $states): object\n    {\n        return $this->error(static::STATUS, [sprintf('Product type bulk_dispatches is not configured for `%s` in the regional distribution.', implode(', ', $states))]);\n    }\n}\n",
            FormatStyle::Default,
        );

        assert!(out.contains(")]);"), "the hug stands: {out}");
    }
    /// Drupal's sniff reads the `|` between caught classes as an operator, and core writes the
    /// spaces; a union in a signature is left as the formatter wrote it.
    #[test]
    fn drupal_spaces_the_classes_a_catch_lists() {
        let out = formatted(
            "<?php\n\nfunction f(int|string $x): void {\n  try {\n    g();\n  }\n  catch (\\RuntimeException|\\LogicException $e) {\n    h($e);\n  }\n  catch (Missing|\\Exception) {\n  }\n}\n",
            FormatStyle::Drupal,
        );
        assert!(
            out.contains("catch (\\RuntimeException | \\LogicException $e)"),
            "spaced: {out:?}"
        );
        assert!(
            out.contains("catch (Missing | \\Exception)"),
            "spaced: {out:?}"
        );
        assert!(
            out.contains("int|string $x"),
            "signature untouched: {out:?}"
        );
        // Idempotent: the spaced form is what the style asks for, so a second pass has nothing to do.
        assert_eq!(format(&out, FormatStyle::Drupal), None);
    }
}
