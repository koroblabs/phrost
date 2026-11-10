//! `no-missing-format-argument` held to what a call actually writes.
//!
//! The report that started it: `printf("... %6d %6d\n", 'total', '', ...array_values($totals))`,
//! eight placeholders and six of them supplied by the unpacked array, answered with "contains 8
//! placeholders, 8 values given" -- a finding that was wrong, in a sentence that contradicted
//! itself. Two defects in one rule, and [`super::format_values::FormatValues`] is where each is
//! put right.

use expect_test::expect;

use super::*;

fn findings(source: &str) -> Vec<LintIssue> {
    lint(
        source,
        "src/Runner.php",
        &[],
        LintSelection {
            on: &["no-missing-format-argument".to_owned()],
            ..Default::default()
        },
    )
}

/// The whole finding, so a correction to the message cannot quietly cost the help beside it.
fn render(issues: &[LintIssue]) -> String {
    if issues.is_empty() {
        return "no findings\n".to_owned();
    }
    issues
        .iter()
        .map(|issue| {
            format!(
                "{:?} {}: {}\n  help: {}\n",
                issue.severity,
                issue.code,
                issue.message,
                issue.help.as_deref().unwrap_or("(none)"),
            )
        })
        .collect()
}

/// The reported line as written. How many values an unpacked array holds is decided when the line
/// runs, so nothing on the page says any are missing.
#[test]
fn an_unpacked_value_may_supply_any_number_of_placeholders() {
    let source = "<?php\n\nprintf(\"   %-32s %-26s %6d %6d %6d %6d %6d %6d\\n\\n\", 'total', '', ...array_values($reportTotals));\n";

    expect![[r#"
        no findings
    "#]]
    .assert_eq(&render(&findings(source)));
}

/// The same call with the array left out: two values for eight placeholders, which is the finding
/// the rule exists for -- and the number after the comma is the two that were written.
#[test]
fn the_values_given_are_the_ones_written() {
    let source =
        "<?php\n\nprintf(\"   %-32s %-26s %6d %6d %6d %6d %6d %6d\\n\\n\", 'total', '');\n";

    expect![[r#"
        Error no-missing-format-argument: Call to `printf` contains 8 placeholders, 2 values given.
          help: Add the missing values to the call, or remove the corresponding %s placeholders from the format string.
    "#]]
    .assert_eq(&render(&findings(source)));
}

/// One value, so the sentence is singular where mago's own plural followed the wrong number.
#[test]
fn one_value_given_is_counted_as_one() {
    let source = "<?php\n\n$line = sprintf('Hello %s, you have %d messages', $name);\n";

    expect![[r#"
        Error no-missing-format-argument: Call to `sprintf` contains 2 placeholders, 1 value given.
          help: Add the missing value to the call, or remove the corresponding %s placeholders from the format string.
    "#]]
    .assert_eq(&render(&findings(source)));
}

/// `fprintf` takes its format second, so the handle before it is not a value.
#[test]
fn what_stands_before_the_format_is_not_a_value() {
    let source = "<?php\n\nfprintf($handle, 'Hello %s %s', $name);\n";

    expect![[r#"
        Error no-missing-format-argument: Call to `fprintf` contains 2 placeholders, 1 value given.
          help: Add the missing value to the call, or remove the corresponding %s placeholders from the format string.
    "#]]
    .assert_eq(&render(&findings(source)));
}

/// The call is found wherever it is written: in a method, as an argument of another call, inside a
/// closure. A filter that only reached top-level statements would leave the report standing in
/// every place a real call is.
#[test]
fn an_unpacked_value_is_seen_at_any_depth() {
    let source = r#"<?php

final class Report
{
    public function lines(array $rows): array
    {
        return array_map(
            static fn (array $row): string => trim(sprintf('%s: %d of %d', ...$row)),
            $rows,
        );
    }
}
"#;

    expect![[r#"
        no findings
    "#]]
    .assert_eq(&render(&findings(source)));
}

/// Two calls on one line, one of each kind: the unpacked one is dropped and the short one beside
/// it is still reported, so the filter judges the call a finding is about and not the line.
#[test]
fn the_call_beside_an_unpacked_one_is_still_reported() {
    let source = "<?php\n\necho sprintf('%s %s', ...$pair), sprintf('%s %s', $one);\n";

    expect![[r#"
        Error no-missing-format-argument: Call to `sprintf` contains 2 placeholders, 1 value given.
          help: Add the missing value to the call, or remove the corresponding %s placeholders from the format string.
    "#]]
    .assert_eq(&render(&findings(source)));
}

/// The other half of the rule -- a `v*printf` with no array at all -- says something else in
/// other words, and is kept exactly as written.
#[test]
fn a_missing_values_array_is_reported_as_mago_wrote_it() {
    let source = "<?php\n\n$line = vsprintf('Hello %s');\n";

    expect![[r#"
        Error no-missing-format-argument: Call to `vsprintf` expects a format string and a values array, but only 1 argument given.
          help: Provide an array of values as the second argument.
    "#]]
    .assert_eq(&render(&findings(source)));
}
