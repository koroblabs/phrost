//! `no-error-control-operator` kept away from a deprecation raised the way a library is asked to
//! raise one. See [`super::deprecation_notice`] for why that one `@` is not the habit the rule is
//! about, and for the 368 of 483 findings in Drupal core that were it.

use expect_test::expect;

use super::*;

fn findings(source: &str) -> Vec<LintIssue> {
    lint(
        source,
        "src/Runner.php",
        &[],
        LintSelection {
            on: &["no-error-control-operator".to_owned()],
            ..Default::default()
        },
    )
}

/// The whole finding and where it starts, so two `@` on one line can be told apart.
fn render(source: &str) -> String {
    let issues = findings(source);
    if issues.is_empty() {
        return "no findings\n".to_owned();
    }
    issues
        .iter()
        .map(|issue| {
            let at = usize::from(issue.range.start());
            let rest = source[at..].lines().next().unwrap_or_default();
            format!(
                "{:?} {}: {}\n  at: {rest}\n  help: {}\n",
                issue.severity,
                issue.code,
                issue.message,
                issue.help.as_deref().unwrap_or("(none)"),
            )
        })
        .collect()
}

/// Drupal's deprecation policy, verbatim in shape: the line a module is told to write.
#[test]
fn a_silenced_deprecation_is_the_protocol_and_not_a_hidden_error() {
    let source = "<?php\n\n@trigger_error('Foo::bar() is deprecated in drupal:11.5.0 and is removed from drupal:13.0.0. Use Foo::baz() instead.', E_USER_DEPRECATED);\n";

    expect![[r#"
        no findings
    "#]]
    .assert_eq(&render(source));
}

/// Symfony's spelling of the same line: both names fully qualified, inside a namespace, the
/// message built by a call.
#[test]
fn the_fully_qualified_spelling_is_the_same_notice() {
    let source = "<?php\n\nnamespace App\\Legacy;\n\nfinal class Mailer\n{\n    public function send(): void\n    {\n        @\\trigger_error(\\sprintf('Since app 3.0: %s is deprecated.', __METHOD__), \\E_USER_DEPRECATED);\n    }\n}\n";

    expect![[r#"
        no findings
    "#]]
    .assert_eq(&render(source));
}

/// The level by its parameter's name.
#[test]
fn a_named_level_is_the_same_notice() {
    let source = "<?php\n\n@trigger_error(message: 'gone', error_level: E_USER_DEPRECATED);\n";

    expect![[r#"
        no findings
    "#]]
    .assert_eq(&render(source));
}

/// What the rule is for, untouched: an error PHP raises, hidden.
#[test]
fn a_silenced_filesystem_call_is_still_reported() {
    let source = "<?php\n\n@unlink($file);\n";

    expect![[r#"
        Error no-error-control-operator: Unsafe use of error control operator `@`.
          at: @unlink($file);
          help: Remove the `@` and use `set_error_handler` to handle errors instead.
    "#]]
    .assert_eq(&render(source));
}

/// A warning raised and silenced is a warning nobody sees. Only the deprecation level is the
/// opt-in protocol.
#[test]
fn a_silenced_notice_at_another_level_is_still_reported() {
    let source = "<?php\n\n@trigger_error('disk is full', E_USER_WARNING);\n";

    expect![[r#"
        Error no-error-control-operator: Unsafe use of error control operator `@`.
          at: @trigger_error('disk is full', E_USER_WARNING);
          help: Remove the `@` and use `set_error_handler` to handle errors instead.
    "#]]
    .assert_eq(&render(source));
}

/// No level written is `E_USER_NOTICE`, and a level in a variable is one the page cannot read.
#[test]
fn a_level_the_page_does_not_name_is_still_reported() {
    let source = "<?php\n\n@trigger_error($message);\n@trigger_error($message, $level);\n";

    expect![[r#"
        Error no-error-control-operator: Unsafe use of error control operator `@`.
          at: @trigger_error($message);
          help: Remove the `@` and use `set_error_handler` to handle errors instead.
        Error no-error-control-operator: Unsafe use of error control operator `@`.
          at: @trigger_error($message, $level);
          help: Remove the `@` and use `set_error_handler` to handle errors instead.
    "#]]
    .assert_eq(&render(source));
}

/// A method that happens to share the name is somebody's own code, and what its `@` hides is
/// whatever that code raises.
#[test]
fn a_method_of_the_same_name_is_still_reported() {
    let source = "<?php\n\n@$logger->trigger_error('gone', E_USER_DEPRECATED);\n";

    expect![[r#"
        Error no-error-control-operator: Unsafe use of error control operator `@`.
          at: @$logger->trigger_error('gone', E_USER_DEPRECATED);
          help: Remove the `@` and use `set_error_handler` to handle errors instead.
    "#]]
    .assert_eq(&render(source));
}

/// Two on one line: the read is reported and the notice beside it is not, so the filter judges
/// the operator a finding points at and not the line it is on.
#[test]
fn the_operator_beside_a_silenced_deprecation_is_still_reported() {
    let source = "<?php\n\n$read = @file_get_contents($path) ?: @trigger_error('gone', E_USER_DEPRECATED);\n";

    expect![[r#"
        Error no-error-control-operator: Unsafe use of error control operator `@`.
          at: @file_get_contents($path) ?: @trigger_error('gone', E_USER_DEPRECATED);
          help: Remove the `@` and use `set_error_handler` to handle errors instead.
    "#]]
    .assert_eq(&render(source));
}
