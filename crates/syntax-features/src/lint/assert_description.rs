//! `assert-description` is a house rule, so it waits to be asked for.
//!
//! `assert($value instanceof Thing);` is the line the type layer narrows through, and Drupal core
//! writes it three hundred times without a description. See the list in `lint.rs` that this rule
//! is on for the counts.

use expect_test::expect;

use super::*;

/// Strict types declared, so the fixture's only possible correctness finding is the one this is
/// about.
const NARROWING_ASSERT: &str = "<?php\n\ndeclare(strict_types=1);\n\nfunction title(object $entity): string\n{\n    assert($entity instanceof Node);\n\n    return $entity->title();\n}\n";

fn findings(selection: LintSelection<'_>) -> String {
    let issues = lint(NARROWING_ASSERT, "src/Runner.php", &[], None, selection);
    if issues.is_empty() {
        return "no findings\n".to_owned();
    }
    issues
        .iter()
        .map(|issue| format!("{:?} {}: {}\n", issue.severity, issue.code, issue.message))
        .collect()
}

/// Correctness asked for as a category does not bring it.
#[test]
fn an_assert_that_narrows_is_not_asked_to_describe_itself() {
    expect![[r#"
        no findings
    "#]]
    .assert_eq(&findings(LintSelection {
        categories: &[LintCategory::Correctness],
        ..Default::default()
    }));
}

/// Named, it is the project's own word.
#[test]
fn a_project_that_names_the_rule_gets_it() {
    expect![[r#"
        Warning assert-description: Missing description in assert function.
    "#]]
    .assert_eq(&findings(LintSelection {
        categories: &[LintCategory::Correctness],
        on: &["assert-description".to_owned()],
        ..Default::default()
    }));
}
