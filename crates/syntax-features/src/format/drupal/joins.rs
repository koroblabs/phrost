//! A chain merging an array, written on its operators' lines.
//!
//! The one respelling that joins rather than breaks: mago lays an operator chain out on a line per
//! operator, and Drupal's array sniffs leave nowhere for those lines to go.

use mago_span::HasSpan;
use mago_syntax::cst::{Binary, Expression, Node};
use mago_syntax::token::GetPrecedence;
use phrost_model::TextSize;
use phrost_syntax::ParseSession;

use super::WIDTH;

/// A guard against a pathological input rather than a limit real code reaches.
///
/// Each pass makes every join that shares no line with another and re-parses, so a pass is needed
/// per link of the longest chain rather than per join in the file. The loop ends on its own -- a
/// pass that joins nothing stops it -- and the cap is only about how long a save may take; a file
/// past it keeps the formatter's layout for the rest, which is valid PHP the sniff complains about.
const MOST_PASSES: usize = 32;

/// `$defaults + [` stays on one line, which is where Drupal writes the array it merges.
///
/// mago breaks before an operator whose array operand spans lines -- `$options\n  + [\n ... ]` --
/// and `Drupal.Arrays.Array.ArrayIndentation` rejects the result. The sniff takes the first token on
/// the line the enclosing array's `[` is on and demands every later line of that array start two
/// columns further in, and the `+` line is one of them: for an element written at eight spaces the
/// continuation lands at ten and the sniff asks for eight. There is nowhere for the operator to go
/// that satisfies both it and the array it opens, so the break itself is what has to go.
///
/// Which is also how Drupal writes it -- 105 files of core 11 write `X + [` at the end of a line --
/// and it is what mago already does for the operators whose right operand it inlines (`??`, `||`,
/// a comparison). An arithmetic operator is the gap, and `+` is the only one an array reaches.
///
/// The value such a chain is assigned to comes back as well. mago moves a chain below its `=` or
/// `=>` unless the chain's last operand is what breaks -- so `[...] + $defaults` -- and from 1.49 any
/// chain of two operators too, `[...] + $field + [...]`, on the grounds that the chain's own breaks
/// would otherwise sit flush with the assignment. Once those breaks are joined that reason is gone,
/// and what is left is `$settings =` on a line of its own above an array Drupal writes beside it.
/// Three files of the reference project write each shape.
///
/// Drupal-only, because Drupal is the style whose sniffs have been read. The break is no better
/// under the other presets, but nothing there has asked.
pub(super) fn hugging_array_operands(text: &str) -> String {
    if !a_line_starts_with_an_operator(text) {
        return text.to_owned();
    }

    let mut text = text.to_owned();
    for _ in 0..MOST_PASSES {
        let Some(joined) = joined(&text) else {
            break;
        };
        text = joined;
    }
    text
}

/// Whether anything in `text` could be an operator continuation, without parsing to find out.
///
/// The pass below parses, and nearly every file has nothing for it to find. A leading `-` also
/// starts a member access chain, so this lets more through than it needs to; what it buys is that a
/// file with no broken operator at all pays one scan of its bytes.
fn a_line_starts_with_an_operator(text: &str) -> bool {
    text.lines().any(|line| {
        let trimmed = line.trim_start();
        trimmed.starts_with([
            '+', '-', '*', '/', '%', '.', '?', '&', '|', '^', '<', '>', '=',
        ]) || ["and ", "or ", "xor ", "instanceof "]
            .iter()
            .any(|word| trimmed.starts_with(word))
    })
}

/// `text` with every break undone that does not depend on another, or nothing when it holds none.
///
/// Breaks of one chain are undone one at a time, re-parsing between: the second is measured against
/// a line the first one moved, and re-reading it is cheaper to be sure of than an arithmetic that
/// has to predict it. `] + $field + [` is the shape that needs two passes. Breaks on lines nothing
/// else reads are undone together -- a test of Drupal core merges `$attributes + [` thirty-nine times,
/// and one join a pass ran it past the cap with seven breaks left.
fn joined(text: &str) -> Option<String> {
    let session = ParseSession::new();
    let parsed = session.parse(text);
    // The formatter's own output, so it parses. If it somehow does not, it is left exactly as it is.
    if !parsed.errors().is_empty() {
        return None;
    }

    let mut found = Vec::new();
    // Each node with the chain it is an operand of, if any: that chain's precedence, and whether it
    // holds a multi-line array. A chain is judged once at its root and every operator of the same
    // precedence inherits the answer -- the whole chain and not only the operator beside the array,
    // because `$defaults + $y + [` is one expression and joining just its last `+` left `$defaults`
    // above the rest. An operand of another precedence is a chain of its own: the `&&`s between
    // `$a === [` comparisons are not what the array is merged by.
    let mut pending = vec![(Node::Program(parsed.program()), None)];
    while let Some((node, chain)) = pending.pop() {
        let mut inherited = None;
        let join = match node {
            Node::Binary(binary) => {
                let level = binary.operator.precedence();
                let broken = match chain {
                    Some((parent, broken)) if parent == level => broken,
                    _ => holds_a_broken_array(text, binary),
                };
                inherited = Some((level, broken));
                match broken {
                    true => Join::of(text, binary.lhs, binary.operator.span(), binary.rhs),
                    false => None,
                }
            }
            Node::Expression(_) => {
                inherited = chain;
                None
            }
            Node::Assignment(assignment) => Join::of_value(
                text,
                assignment.lhs.span(),
                assignment.operator.span(),
                assignment.rhs,
            ),
            Node::KeyValueArrayElement(element) => Join::of_value(
                text,
                element.key.span(),
                element.double_arrow,
                element.value,
            ),
            Node::ClassLikeConstantItem(item) => {
                Join::of_value(text, item.name.span(), item.equals, item.value)
            }
            Node::ConstantItem(item) => {
                Join::of_value(text, item.name.span(), item.equals, item.value)
            }
            Node::PropertyConcreteItem(item) => {
                Join::of_value(text, item.variable.span(), item.equals, item.value)
            }
            _ => None,
        };
        found.extend(join);
        node.visit_children(|child| pending.push((child, inherited)));
    }

    // Earliest first, so of two that share a line the one a single pass used to make is made.
    found.sort_by_key(|join| join.gap.0);
    let mut made: Vec<(Join, String)> = Vec::new();
    for join in found {
        if made.iter().any(|(held, _)| held.shares_a_line(&join)) {
            continue;
        }
        // A region that cannot be brought back keeps the formatter's layout, and the rest go on.
        if let Some(joined) = join.joined(text) {
            made.push((join, joined));
        }
    }
    if made.is_empty() {
        return None;
    }

    let mut out = String::with_capacity(text.len());
    let mut at = 0;
    for (join, joined) in &made {
        out.push_str(text.get(at..join.gap.0)?);
        out.push_str(joined);
        at = join.tail.1;
    }
    out.push_str(text.get(at..)?);
    Some(out)
}

/// One break to undo: the whitespace to replace, and the region whose indentation it decided.
struct Join {
    /// The whitespace run holding the newline, which becomes a single space.
    gap: (usize, usize),
    /// From the end of `gap` to the end of the right operand -- the lines the formatter indented
    /// because the operator was on its own line.
    tail: (usize, usize),
    /// How far those lines have to come back.
    dedent: usize,
    /// Every line the join reads or rewrites, from the start of the first to the end of the last.
    lines: (usize, usize),
}

impl Join {
    /// The join this binary expression needs, or nothing when it needs none.
    ///
    /// Asked only of a chain holding an array that spans lines. That is the whole of what the
    /// formatter breaks for here -- the line is nearly always short enough -- and it is what keeps
    /// the join from making a line nothing can break: the chain still ends up beside a `[`.
    fn of(
        text: &str,
        lhs: &Expression<'_>,
        operator: mago_span::Span,
        rhs: &Expression<'_>,
    ) -> Option<Self> {
        let (lhs_end, op_start, op_end, rhs_start) = (
            lhs.span().end.offset as usize,
            operator.start.offset as usize,
            operator.end.offset as usize,
            rhs.span().start.offset as usize,
        );
        let (before, after) = (text.get(lhs_end..op_start)?, text.get(op_end..rhs_start)?);
        // A comment either side is why the formatter broke, and joining would swallow the line
        // after it. Whitespace both sides, and exactly one of them holding the newline.
        if !before.trim().is_empty() || !after.trim().is_empty() {
            return None;
        }
        let gap = match (before.contains('\n'), after.contains('\n')) {
            (true, false) => (lhs_end, op_start),
            (false, true) => (op_end, rhs_start),
            _ => return None,
        };
        if !fits(text, gap) {
            return None;
        }

        // What the array's own lines are measured against once the join is made: the line the left
        // operand ends on, which is where the operator lands.
        let host = indent_of(text, lhs_end)?;
        let dedent = indent_of(text, op_start)?.saturating_sub(host);

        let tail = (gap.1, rhs.span().end.offset as usize);
        Some(Self {
            gap,
            tail,
            dedent,
            lines: lines_between(text, lhs_end, tail.1),
        })
    }

    /// The join a value placed below its `=` or `=>` needs, or nothing when it needs none.
    ///
    /// Only a chain with a multi-line array among its operands: that is the value the chains above
    /// join and so the one whose break stops meaning anything, where a long `??` chain below its `=`
    /// is a layout that holds. The value's lines were indented under the operator's line, and come
    /// back by however far that was.
    fn of_value(
        text: &str,
        target: mago_span::Span,
        operator: mago_span::Span,
        value: &Expression<'_>,
    ) -> Option<Self> {
        let Expression::Binary(chain) = value else {
            return None;
        };
        if !holds_a_broken_array(text, chain) {
            return None;
        }

        let (target_end, op_start, op_end, value_start) = (
            target.end.offset as usize,
            operator.start.offset as usize,
            operator.end.offset as usize,
            value.span().start.offset as usize,
        );
        let (before, after) = (
            text.get(target_end..op_start)?,
            text.get(op_end..value_start)?,
        );
        // The operator on the target's line and the value below it, with nothing but whitespace
        // between -- a comment there is the author's, and the line after it is not ours to take.
        if !before.trim().is_empty() || before.contains('\n') {
            return None;
        }
        if !after.trim().is_empty() || !after.contains('\n') {
            return None;
        }

        let gap = (op_end, value_start);
        if !fits(text, gap) {
            return None;
        }

        let dedent = indent_of(text, value_start)?.saturating_sub(indent_of(text, op_start)?);
        let tail = (value_start, value.span().end.offset as usize);
        Some(Self {
            gap,
            tail,
            dedent,
            lines: lines_between(text, op_start, tail.1),
        })
    }

    /// Whether `other` reads or rewrites a line this one does, which makes it a join to measure
    /// again after this one is made rather than beside it.
    fn shares_a_line(&self, other: &Self) -> bool {
        self.lines.0 <= other.lines.1 && other.lines.0 <= self.lines.1
    }

    /// What replaces `gap` and `tail`: the break as a single space, and the region behind it brought
    /// back in.
    ///
    /// Two kinds of line in that region are not layout, and they need opposite treatment -- which
    /// is why the question is asked of the lexer rather than of the indentation. A string written
    /// over several lines carries its own newline, so its later lines *are* the value and must not
    /// move; the lexer's range covers them, and they stay. A heredoc's body is the other way round:
    /// PHP strips the closing marker's indentation from every line of it, so the body and the
    /// marker moving together leaves the value exactly as it was -- and the lexer says so by
    /// starting each part *after* that indentation, so those line starts are outside every range.
    ///
    /// Nothing when a line has less indentation than it would have to give back. That is the
    /// heredoc written at column zero, and the answer for it is the formatter's own output: valid
    /// PHP that the sniff complains about, which is the one outcome that is never wrong.
    fn joined(&self, text: &str) -> Option<String> {
        let tail = text.get(self.tail.0..self.tail.1)?;
        let inside_a_string = match self.dedent {
            0 => Vec::new(),
            _ => string_lines(text, self.tail),
        };

        let mut out = String::with_capacity(tail.len() + 1);
        out.push(' ');

        let mut at = self.tail.0;
        for (nth, line) in tail.split_inclusive('\n').enumerate() {
            // The first piece is the remainder of the operator's own line, which moves whole. A
            // line the lexer says begins inside a string is text the program prints, and a blank
            // one has nothing to bring back.
            let moves = nth > 0 && !inside_a_string.contains(&at) && !line.trim().is_empty();
            let kept = match moves {
                true => line.strip_prefix(&" ".repeat(self.dedent))?,
                false => line,
            };
            out.push_str(kept);
            at += line.len();
        }
        Some(out)
    }
}

/// From the start of the line holding `from` to the end of the line holding `to`.
fn lines_between(text: &str, from: usize, to: usize) -> (usize, usize) {
    let start = text[..from].rfind('\n').map_or(0, |at| at + 1);
    let end = text[to..].find('\n').map_or(text.len(), |at| to + at);
    (start, end)
}

/// Whether the line a join of `gap` makes stays within the width the preset prints at.
///
/// A chain's operands are not only arrays, and two long calls joined around a `+` make a line the
/// formatter would have broken -- the break is then the formatter's, and stays.
fn fits(text: &str, gap: (usize, usize)) -> bool {
    let start = text[..gap.0].rfind('\n').map_or(0, |at| at + 1);
    let end = text[gap.1..].find('\n').map_or(text.len(), |at| gap.1 + at);
    text[start..gap.0].chars().count() + 1 + text[gap.1..end].chars().count() <= WIDTH
}

/// Whether one of the operands of the chain `binary` roots -- its operators of one precedence -- is
/// an array written over more than one line.
fn holds_a_broken_array(text: &str, binary: &Binary<'_>) -> bool {
    let level = binary.operator.precedence();
    let mut pending = vec![binary.lhs, binary.rhs];
    while let Some(operand) = pending.pop() {
        match operand {
            Expression::Binary(inner) if inner.operator.precedence() == level => {
                pending.extend([inner.lhs, inner.rhs]);
            }
            operand if spans_lines(text, operand) => return true,
            _ => {}
        }
    }
    false
}

/// Whether `expression` is an array written over more than one line.
fn spans_lines(text: &str, expression: &Expression<'_>) -> bool {
    if !matches!(
        expression,
        Expression::Array(_) | Expression::LegacyArray(_) | Expression::List(_)
    ) {
        return false;
    }
    let span = expression.span();
    text.get(span.start.offset as usize..span.end.offset as usize)
        .is_some_and(|written| written.contains('\n'))
}

/// The spaces the line holding `offset` opens with.
///
/// Nothing when it opens with anything else. A tab is not a thing the preset writes -- `use_tabs`
/// is off and the indent is two spaces -- and a respelling that cannot count the indentation must
/// not guess at it.
fn indent_of(text: &str, offset: usize) -> Option<usize> {
    let start = text.get(..offset)?.rfind('\n').map_or(0, |at| at + 1);
    let line = text.get(start..)?;
    let spaces = line.bytes().take_while(|byte| *byte == b' ').count();
    match line.as_bytes().get(spaces) {
        Some(b'\t') => None,
        _ => Some(spaces),
    }
}

/// The start offset of every line in `region` that begins inside a string literal.
///
/// From the lexer rather than the tree, because what is being asked is not what a node is but
/// whether a byte is part of a value -- see [`Join::applied`] for what that decides.
fn string_lines(text: &str, region: (usize, usize)) -> Vec<usize> {
    let strings = phrost_syntax::string_ranges(text);
    let Some(inside) = text.get(region.0..region.1) else {
        return Vec::new();
    };

    let mut starts = Vec::new();
    let mut at = region.0;
    for line in inside.split_inclusive('\n') {
        let held = TextSize::try_from(at).unwrap_or_default();
        if at > region.0 && strings.iter().any(|range| range.contains(held)) {
            starts.push(at);
        }
        at += line.len();
    }
    starts
}

#[cfg(test)]
mod tests {
    use expect_test::expect;

    use crate::format::drupal::tests::drupal;
    use crate::format::{FormatStyle, formatted};

    /// The shape a Drupal codebase writes wherever it merges defaults into an array, and the one
    /// the sniff rejects when the operator is left on its own line.
    #[test]
    fn an_array_a_binary_merges_comes_back_to_the_operators_line() {
        let out = drupal(
            "<?php\n\nclass Repo {\n\n  public function build(UserInfo $info): array {\n    return [\n      'context' => [\n        'browser' => $info->getBrowserInfo()\n          + [\n            'allowed' => $info->isSupportedBrowser(),\n          ],\n      ],\n    ];\n  }\n\n}\n",
        );

        expect![[r#"
            <?php

            class Repo {

              public function build(UserInfo $info): array {
                return [
                  'context' => [
                    'browser' => $info->getBrowserInfo() + [
                      'allowed' => $info->isSupportedBrowser(),
                    ],
                  ],
                ];
              }

            }
        "#]]
        .assert_eq(&out);
    }

    /// Every place the idiom is written: a return, an assignment, an argument and an array element
    /// each put the operator somewhere else, and the join is measured from the line the left
    /// operand ends on rather than from the statement.
    #[test]
    fn the_join_is_measured_from_the_line_the_left_operand_ends_on() {
        let out = drupal(
            "<?php\n\nclass Repo {\n\n  public function f(array $options, array $field): array {\n    $log = $options\n    + [\n      'channel' => 'test',\n    ];\n    return parent::f()\n    + [\n      'a' => [\n        'b' => 1,\n      ]\n        + $field\n        + [\n          'c' => $log,\n        ],\n    ];\n  }\n\n}\n",
        );

        expect![[r#"
            <?php

            class Repo {

              public function f(array $options, array $field): array {
                $log = $options + [
                  'channel' => 'test',
                ];
                return parent::f() + [
                  'a' => [
                    'b' => 1,
                  ] + $field + [
                    'c' => $log,
                  ],
                ];
              }

            }
        "#]]
        .assert_eq(&out);
    }

    /// A heredoc moves whole -- body and closing marker together -- because PHP reads its value
    /// relative to that marker. Two spaces off both is the same string.
    #[test]
    fn a_heredoc_inside_the_array_moves_with_its_closing_marker() {
        let out = drupal(
            "<?php\n\nclass Repo {\n\n  public function f(array $base): array {\n    return [\n      'body' => $base\n        + [\n          'text' => <<<EOT\n  kept\n    exactly\nEOT,\n        ],\n    ];\n  }\n\n}\n",
        );

        expect![[r#"
            <?php

            class Repo {

              public function f(array $base): array {
                return [
                  'body' => $base + [
                    'text' => <<<EOT
                        kept
                          exactly
                      EOT,
                  ],
                ];
              }

            }
        "#]]
        .assert_eq(&out);
    }

    /// A comment is why the formatter broke the line, and joining it would put the code after it
    /// behind a `//`.
    #[test]
    fn a_comment_between_the_operands_keeps_the_break() {
        let out = drupal(
            "<?php\n\nclass Repo {\n\n  public function f(array $base): array {\n    return $base\n    // The order matters.\n    + [\n      'a' => 1,\n    ];\n  }\n\n}\n",
        );

        assert!(out.contains("// The order matters."), "{out}");
        assert!(out.contains("\n    + ["), "the break stands: {out}");
    }

    /// The rule is about an array operand and nothing else: an expression that breaks because it is
    /// long still breaks.
    #[test]
    fn a_long_expression_with_no_array_operand_still_breaks() {
        let out = drupal(
            "<?php\n\nclass Repo {\n\n  public function f(string $a, string $b): string {\n    return $this->translateTheFirstPartOfTheSentence($a) . $this->translateTheSecondPartOfTheSentence($b) . $this->translateTheThirdPartOfTheSentence($a) . $this->translateTheFourthPartOfTheSentence($b) . $this->translateTheFifthPartOfTheSentence($a);\n  }\n\n}\n",
        );

        assert!(out.contains("\n      . $this->"), "still broken: {out}");
    }

    /// Drupal-only. The other presets get mago's own answer, which is the break.
    #[test]
    fn another_style_keeps_the_break() {
        let out = formatted(
            "<?php\n\nclass Repo\n{\n    public function f(array $options): array\n    {\n        return $options + [\n            'a' => 1,\n        ];\n    }\n}\n",
            FormatStyle::Default,
        );

        assert!(out.contains("return $options\n        + ["), "{out}");
    }

    /// Every place a value is assigned, and every shape of chain: the array first, last, between,
    /// and beside a `??`. Each is written the way Drupal writes it, and each comes back unchanged --
    /// where mago moves the chain below its `=`, and where a join made only beside the array kept
    /// the chain's first operand on a line of its own.
    #[test]
    fn a_chain_holding_an_array_stays_beside_what_it_is_assigned_to() {
        let out = drupal(
            "<?php\n\nclass Shapes {\n\n  const C = [\n    'a' => 1,\n  ] + self::B + [\n    'c' => 2,\n  ];\n\n  public array $p = [\n    'a' => 1,\n  ] + self::B + [\n    'c' => 2,\n  ];\n\n  public function f(array $defaults, array $y, ?array $a, ?array $b): array {\n    $x = $defaults + $y + [\n      'k' => 1,\n    ];\n    $z = $a ?? $b ?? [\n      'k' => 1,\n    ];\n    $w = $defaults + [\n      'k' => 1,\n    ] + $y;\n    $v = [\n      'k' => 1,\n    ] + $defaults + $y;\n    $config['nested'] += [\n      'one' => 1,\n    ] + $defaults + [\n      'two' => 2,\n    ];\n    return [\n      'a' => $defaults + $y + [\n        'k' => 1,\n      ],\n    ];\n  }\n\n}\n",
        );

        expect![[r#"
            <?php

            class Shapes {

              const C = [
                'a' => 1,
              ] + self::B + [
                'c' => 2,
              ];

              public array $p = [
                'a' => 1,
              ] + self::B + [
                'c' => 2,
              ];

              public function f(array $defaults, array $y, ?array $a, ?array $b): array {
                $x = $defaults + $y + [
                  'k' => 1,
                ];
                $z = $a ?? $b ?? [
                  'k' => 1,
                ];
                $w = $defaults + [
                  'k' => 1,
                ] + $y;
                $v = [
                  'k' => 1,
                ] + $defaults + $y;
                $config['nested'] += [
                  'one' => 1,
                ] + $defaults + [
                  'two' => 2,
                ];
                return [
                  'a' => $defaults + $y + [
                    'k' => 1,
                  ],
                ];
              }

            }
        "#]]
        .assert_eq(&out);
    }

    /// A join is refused where the line it makes would pass the width the preset prints at: the
    /// operands are not only arrays, and two long calls side by side are a line the formatter broke
    /// for a reason.
    #[test]
    fn a_join_past_the_width_keeps_the_break() {
        let out = drupal(
            "<?php\n\nclass Repo {\n\n  public function f(array $base): array {\n    return $this->aMethodWithAVeryLongNameThatTakesSeveralArguments($base, $base, $base, $base, $base, $base, $base, $base) + $this->anotherMethodWithAVeryLongNameThatTakesArguments($base, $base, $base, $base) + [\n      'a' => 1,\n    ];\n  }\n\n}\n",
        );

        expect![[r#"
            <?php

            class Repo {

              public function f(array $base): array {
                return (
                  $this->aMethodWithAVeryLongNameThatTakesSeveralArguments($base, $base, $base, $base, $base, $base, $base, $base)
                  + $this->anotherMethodWithAVeryLongNameThatTakesArguments($base, $base, $base, $base) + [
                    'a' => 1,
                  ]
                );
              }

            }
        "#]].assert_eq(&out);
    }

    /// The value comes back beside its `=` only when it is a chain merging an array. A long chain
    /// of anything else is a layout mago chose and Drupal does not object to.
    #[test]
    fn a_long_chain_with_no_array_stays_below_its_assignment() {
        let out = drupal(
            "<?php\n\nclass Repo {\n\n  public function f(object $event): ?string {\n    $queue = $this->getAttributeValueFromTheEventItself($event, QueueAttribute::class, 'queue') ?? $this->resolveQueueFromTheConfiguredQueueRoute($event) ?? $this->fallbackQueueNameFromTheConfiguration() ?? $this->defaultQueueName;\n    return $queue;\n  }\n\n}\n",
        );

        expect![[r#"
            <?php

            class Repo {

              public function f(object $event): ?string {
                $queue =
                  $this->getAttributeValueFromTheEventItself($event, QueueAttribute::class, 'queue')
                  ?? $this->resolveQueueFromTheConfiguredQueueRoute($event)
                  ?? $this->fallbackQueueNameFromTheConfiguration()
                  ?? $this->defaultQueueName;
                return $queue;
              }

            }
        "#]]
        .assert_eq(&out);
    }

    /// A chain is its operators of one precedence. The `&&`s between comparisons against arrays
    /// keep a line each: what merges the array is the `===` beside it, and that join is made.
    #[test]
    fn a_chain_of_another_precedence_around_the_array_keeps_its_breaks() {
        let out = drupal(
            "<?php\n\nclass Check {\n\n  public function f(array $values): bool {\n    return (\n      ($values['status'] ?? NULL) === 'pending'\n      && ($values['first'] ?? NULL) === 'Janet'\n      && ($values['reasons'] ?? NULL) === ['mismatch']\n      && ($values['nested'] ?? NULL) === [\n        'mismatch' => ['invalid'],\n      ]\n      && ($values['message'] ?? NULL) === 'Mismatch.'\n    );\n  }\n\n}\n",
        );

        expect![[r#"
            <?php

            class Check {

              public function f(array $values): bool {
                return (
                  ($values['status'] ?? NULL) === 'pending'
                  && ($values['first'] ?? NULL) === 'Janet'
                  && ($values['reasons'] ?? NULL) === ['mismatch']
                  && ($values['nested'] ?? NULL) === [
                    'mismatch' => ['invalid'],
                  ]
                  && ($values['message'] ?? NULL) === 'Mismatch.'
                );
              }

            }
        "#]]
        .assert_eq(&out);
    }

    /// Joins on lines nothing else reads are made in one pass, so a file is not limited by how many
    /// it has: a test in Drupal core merges `$attributes + [` thirty-nine times, and one join a pass
    /// left the last seven as the formatter broke them.
    #[test]
    fn every_merge_in_a_long_file_is_joined() {
        let situations: String = (0..40)
            .map(|nth| {
                format!(
                    "    $situations[] = [\n      'attributes' => $attributes + [\n        'data-nth' => '{nth}',\n      ],\n    ];\n"
                )
            })
            .collect();
        let source = format!(
            "<?php\n\nclass Filter {{\n\n  public function situations(array $attributes): array {{\n    $situations = [];\n{situations}    return $situations;\n  }}\n\n}}\n"
        );

        assert_eq!(drupal(&source), source);
    }
}
