//! A rule that needs a PHP version is asked, and held to the right one.
//!
//! mago's own default is PHP 8.0, and until this crate said otherwise every rule needing more was
//! never built -- six of the seven deprecation rules among them, with nothing to show for it but
//! a category that never spoke. See [`super::php_floor`] for which version each kind is held to.

use expect_test::expect;
use phrost_model::PhpVersion;

use super::*;

fn findings(source: &str, php: Option<PhpVersion>, selection: LintSelection<'_>) -> String {
    let issues = lint(source, "src/Runner.php", &[], php, selection);
    if issues.is_empty() {
        return "no findings\n".to_owned();
    }
    issues
        .iter()
        .map(|issue| format!("{:?} {}: {}\n", issue.severity, issue.code, issue.message))
        .collect()
}

/// `Message $message = null` has been a deprecation notice since PHP 8.4.
const IMPLICITLY_NULLABLE: &str = "<?php\n\nfunction send(Message $message = null): void\n{\n}\n";

fn deprecations(php: Option<PhpVersion>) -> String {
    findings(
        IMPLICITLY_NULLABLE,
        php,
        LintSelection {
            categories: &[LintCategory::Deprecation],
            ..Default::default()
        },
    )
}

/// The rule needs PHP 8.4, so under mago's default of 8.0 it was never built and this file was
/// reported clean.
#[test]
fn a_deprecation_newer_than_php_8_0_is_reported() {
    expect![[r#"
        Warning explicit-nullable-param: Parameter `$message` is implicitly nullable and relies on a deprecated feature.
    "#]]
    .assert_eq(&deprecations(None));
}

/// A project that requires `^8.1` runs on 8.4 too, and `?Message` is legal on all of it: the
/// deprecation is about where the code is going, not about the oldest PHP it still supports.
#[test]
fn a_deprecation_is_not_held_to_the_oldest_php_a_project_supports() {
    expect![[r#"
        Warning explicit-nullable-param: Parameter `$message` is implicitly nullable and relies on a deprecated feature.
    "#]]
    .assert_eq(&deprecations(Some(PhpVersion::PHP_8_1)));
}

/// `(integer)` is deprecated from PHP 8.5 and runs on all of them. mago's rule says error; a
/// line that runs is a warning here, and a project still on 8.3 has nothing wrong with it yet.
#[test]
fn a_deprecation_is_a_warning_whatever_its_rule_calls_it() {
    expect![[r#"
        Warning deprecated-cast: Usage of deprecated cast `(integer)`.
    "#]]
    .assert_eq(&findings(
        "<?php\n\n$count = (integer) $given;\n",
        Some(PhpVersion::PHP_8_3),
        LintSelection {
            categories: &[LintCategory::Deprecation],
            ..Default::default()
        },
    ));
}

/// `0o755` is PHP 8.1 syntax, so advice to write it is only advice where 8.1 is the least a
/// project runs on.
const OCTAL: &str = "<?php\n\nchmod($path, 0755);\n";

fn octal_advice(php: Option<PhpVersion>) -> String {
    findings(
        OCTAL,
        php,
        LintSelection {
            on: &["explicit-octal".to_owned()],
            ..Default::default()
        },
    )
}

#[test]
fn advice_to_write_newer_syntax_is_given_where_the_oldest_php_has_it() {
    expect![[r#"
        Warning explicit-octal: Use explicit octal numeral notation.
    "#]]
    .assert_eq(&octal_advice(Some(PhpVersion::PHP_8_1)));
}

/// On a project that still supports 8.0 the same advice would be a parse error there, even named
/// on: the floor is a fact about the project, and a rule's name does not change it.
#[test]
fn advice_to_write_newer_syntax_is_withheld_below_the_php_that_added_it() {
    expect![[r#"
        no findings
    "#]]
    .assert_eq(&octal_advice(Some(PhpVersion::PHP_8_0)));
}

/// A project that states no floor is not read as stating the oldest one.
#[test]
fn a_project_that_states_no_php_is_not_held_to_the_oldest() {
    expect![[r#"
        Warning explicit-octal: Use explicit octal numeral notation.
    "#]]
    .assert_eq(&octal_advice(None));
}

/// `#[SensitiveParameter]` needs PHP 8.2, so the rule that asks for it never ran either -- and
/// what it asks is decided by a parameter's name. It stays out of what a reader is shown unasked;
/// see the list it is on for the fourteen findings read.
const NAMED_FOR_A_SECRET: &str =
    "<?php\n\nfunction login(string $user, string $password): void\n{\n}\n";

#[test]
fn a_parameter_named_for_a_secret_is_not_reported_unasked() {
    expect![[r#"
        no findings
    "#]]
    .assert_eq(&findings(
        NAMED_FOR_A_SECRET,
        None,
        LintSelection {
            categories: &[LintCategory::Security],
            ..Default::default()
        },
    ));
}

#[test]
fn a_parameter_named_for_a_secret_is_reported_when_the_rule_is_named() {
    expect![[r#"
        Error sensitive-parameter: Parameters that may contain sensitive information should be marked with the `#[SensitiveParameter]` attribute.
    "#]]
    .assert_eq(&findings(
        NAMED_FOR_A_SECRET,
        None,
        LintSelection {
            on: &["sensitive-parameter".to_owned()],
            ..Default::default()
        },
    ));
}
