//! `no-insecure-comparison` narrowed away from dispatch.
//!
//! The rule matches on a *name* -- `token`, `secret`, `password` -- exactly as its neighbour
//! [`super::is_credential_shaped`] does, and so it reports a value being routed on as a credential
//! being checked. Measured over five projects: 56 findings, and the two shapes
//! [`super::Comparisons::is_dispatch`] states cover 26 of them -- 24 a comparison against a string
//! written out in the source, 2 a `foreach` variable named for one element of a collection. The
//! word `token` belongs to a lexer as much as to an authenticator, and every project has a lexer.

use expect_test::expect;

use super::*;

fn security_findings(source: &str) -> Vec<LintIssue> {
    lint(
        source,
        "src/Runner.php",
        &[],
        LintSelection {
            categories: &[LintCategory::Security],
            ..Default::default()
        },
    )
}

fn render(issues: &[LintIssue]) -> String {
    if issues.is_empty() {
        return "no findings\n".to_owned();
    }
    issues
        .iter()
        .map(|issue| format!("{:?} {}: {}\n", issue.severity, issue.code, issue.message))
        .collect()
}

/// A form field routed on by name. `'change-secret'` is a case label, not a credential.
#[test]
fn a_field_name_matched_against_a_literal_is_dispatch() {
    let source =
        "<?php\n\n$element = ['#name' => 'x'];\nif ($element['#name'] === 'change-secret') {\n}\n";

    expect![[r#"
        no findings
    "#]]
    .assert_eq(&render(&security_findings(source)));
}

/// The same shape with the sensitive-looking name on the literal side.
#[test]
fn a_method_name_matched_against_a_literal_is_dispatch() {
    let source = "<?php\n\nfunction run(string $method): void\n{\n    if ($method === 'token') {\n    }\n}\n";

    expect![[r#"
        no findings
    "#]]
    .assert_eq(&render(&security_findings(source)));
}

/// Any string written out in the source, not just the empty one mago stops at. `$token === ')'` is
/// Laravel's Blade compiler, and the same line in Symfony's JSON lexer, its argv parser and
/// WordPress's rewrite rules -- 24 of the 56 findings measured.
#[test]
fn a_token_compared_to_a_written_string_is_a_lexer() {
    let source =
        "<?php\n\nfunction run(string $token): void\n{\n    if ($token === ')') {\n    }\n}\n";

    expect![[r#"
        no findings
    "#]]
    .assert_eq(&render(&security_findings(source)));
}

/// The shape both real reports were: a word of an address compared to the word beside it. `$token`
/// is bound by the `foreach`, so the name is the author's word for one element and says nothing
/// about what the collection holds.
#[test]
fn a_name_a_foreach_binds_is_not_evidence() {
    let source = "<?php\n\nfunction compare(array $street1, array $street2): bool\n{\n    foreach ($street1 as $index => $token) {\n        $other = $street2[$index];\n        if ($token === $other) {\n            continue;\n        }\n        return false;\n    }\n\n    return true;\n}\n";

    expect![[r#"
        no findings
    "#]]
    .assert_eq(&render(&security_findings(source)));
}

/// The same rule with the iterated name on the other side: WordPress looks for an HTML token in its
/// stack of open elements, and the parameter is what carries the sensitive-looking name.
#[test]
fn a_parameter_compared_to_an_iterated_item_is_a_search() {
    let source = "<?php\n\nfunction contains(object $token, array $stack): bool\n{\n    foreach ($stack as $item) {\n        if ($token === $item) {\n            return true;\n        }\n    }\n\n    return false;\n}\n";

    expect![[r#"
        no findings
    "#]]
    .assert_eq(&render(&security_findings(source)));
}

/// Two bare locals with nothing iterating them stay reported -- WordPress's installer checks the
/// password against its confirmation this way, and it is the rule's own shape.
#[test]
fn two_locals_that_no_loop_binds_are_still_reported() {
    let source = "<?php\n\nfunction install(string $admin_password, string $admin_password_check): void\n{\n    if ($admin_password !== $admin_password_check) {\n    }\n}\n";

    expect![[r#"
        Error no-insecure-comparison: Insecure comparison of sensitive data.
    "#]]
    .assert_eq(&render(&security_findings(source)));
}

/// And a credential fetched from somewhere stays reported inside a loop, which is what keeps the
/// rule about the shape rather than about the loop: every genuine finding in the corpora has a
/// call, a property or a subscript on one side of the comparison.
#[test]
fn a_credential_fetched_from_somewhere_is_reported_inside_a_loop_too() {
    let source = "<?php\n\nfunction check(array $users, string $given): bool\n{\n    foreach ($users as $user) {\n        if ($user->token === $given) {\n            return true;\n        }\n    }\n\n    return false;\n}\n";

    expect![[r#"
        Error no-insecure-comparison: Insecure comparison of sensitive data.
    "#]]
    .assert_eq(&render(&security_findings(source)));
}

/// Regression guard: the finding this rule is worth having for. A webhook token compared to what
/// the server holds, without `hash_equals`, is a timing attack and neither side is a literal.
#[test]
fn a_token_compared_to_a_held_secret_is_still_reported() {
    let source = "<?php\n\nclass Runner\n{\n    public function held(): string { return ''; }\n\n    public function run(string $token): void\n    {\n        if ($token !== $this->held()) {\n        }\n    }\n}\n";

    expect![[r#"
        Error no-insecure-comparison: Insecure comparison of sensitive data.
    "#]]
    .assert_eq(&render(&security_findings(source)));
}
