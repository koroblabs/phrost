//! No finding tells its reader to write mago's own vocabulary.
//!
//! Every `message` and `help` a real project's `--lint all` produced was grepped for mago's name
//! and its suppression comment; the only hit was `no-empty-catch-clause`'s `help`, which told the
//! reader to add a `@mago-expect` comment. phrost's users have never heard of mago, and the way to
//! silence a rule in phrost is naming it in `phrost.toml`, not a comment mago's own linter reads.

use expect_test::expect;

use super::*;

/// Named directly rather than by category, so the fixture's unrelated `strict-types` finding (this
/// project's own correctness rules do not run `declare(strict_types=1)` in a four-line test file)
/// does not show up in a snapshot about a different rule entirely.
fn only_rule(source: &str, code: &str) -> Vec<LintIssue> {
    lint(
        source,
        "src/Runner.php",
        &[],
        None,
        LintSelection {
            on: &[code.to_owned()],
            ..Default::default()
        },
    )
}

/// The whole result, code, message and help together -- so a change that mangles the message
/// while fixing the help (or the reverse) is caught, not just the substring this test is about.
fn render(issues: &[LintIssue]) -> String {
    issues
        .iter()
        .map(|issue| {
            format!(
                "{}: {}\n  help: {}\n",
                issue.code,
                issue.message,
                issue.help.as_deref().unwrap_or("(none)"),
            )
        })
        .collect()
}

/// `no-empty-catch-clause` on an empty `catch` block. mago's own `help` names `@mago-expect`; a
/// phrost user is told to name the rule in `phrost.toml` instead, which is the mechanism that
/// actually exists on this side of the wrapper.
#[test]
fn an_empty_catch_clause_is_told_the_phrost_way_to_silence_it() {
    let source = r#"<?php

final class Runner
{
    public function run(): void
    {
        try {
            do_the_thing();
        } catch (Exception $e) {
        }
    }
}
"#;

    expect![[r#"
        no-empty-catch-clause: Do not use empty `catch` blocks.
          help: Either log the exception, re-throw it, or handle it. If ignoring it is intentional, turn the rule off for the project instead of the finding: add its code to `phrost.toml`'s `[lint]` table, under `off`.
    "#]]
    .assert_eq(&render(&only_rule(source, "no-empty-catch-clause")));
}

/// A rule whose advice is about the code itself, not about suppressing the finding, keeps its
/// wording exactly as mago wrote it: rewriting is targeted at mago's own name, not at every
/// `help` string on principle.
#[test]
fn advice_about_the_code_itself_is_kept_verbatim() {
    let source = "<?php\n\nclass Runner\n{\n    public function run(string $code): void\n    {\n        eval($code);\n    }\n}\n";

    expect![[r#"
        no-eval: Unsafe use of `eval` construct.
          help: Avoid using `eval` unless absolutely necessary, and ensure that any dynamically generated code is properly validated and sanitized before execution.
    "#]]
    .assert_eq(&render(&only_rule(source, "no-eval")));
}
