//! `no-literal-password` narrowed to values that could plausibly be a credential.
//!
//! mago's rule matches on a *name* -- `password`, `token`, `secret`, `api_key` by suffix -- and asks
//! almost nothing of the *value*: any literal longer than two characters, quotes included, counts.
//! Hand-audited against a real Drupal project (see [`super::is_credential_shaped`]'s doc comment
//! for the measured counts), six of twenty-one findings held a value that was never a credential,
//! and the report that started this file -- `private string $secret = 'x';` -- is a seventh shape
//! none of the six measured ones cover. Each test below is one of those shapes as a minimal file,
//! plus two regression guards proving the rule still fires on an actual credential.

use expect_test::expect;

use super::*;

fn security_findings(source: &str) -> Vec<LintIssue> {
    lint(
        source,
        "src/Runner.php",
        &[],
        None,
        LintSelection {
            categories: &[LintCategory::Security],
            ..Default::default()
        },
    )
}

/// Renders the whole result, not a substring of it: an empty vector renders as `no findings`, so a
/// test that expects silence and gets one item still fails on content, not just on `.is_empty()`.
fn render(issues: &[LintIssue]) -> String {
    if issues.is_empty() {
        return "no findings\n".to_owned();
    }
    issues
        .iter()
        .map(|issue| format!("{:?} {}: {}\n", issue.severity, issue.code, issue.message))
        .collect()
}

/// The exact report that started this audit. `'x'` shares none of the six measured shapes below --
/// it is not a sentence, not a route, not a type name, not self-referential -- so it needs its own
/// test: a value too short to be a real password, token, secret, or key.
#[test]
fn a_placeholder_one_character_long_is_not_a_credential() {
    let source = "<?php\n\nfinal class Runner\n{\n    private string $secret = 'x';\n}\n";

    expect![[r#"
        no findings
    "#]]
    .assert_eq(&render(&security_findings(source)));
}

/// A schema mapping field names to their own PHP type. `'access_token' => 'string'` names a type,
/// not a value -- and it is exactly what the class constant in `app_core/src/Bureau/AccessToken.php`
/// does on the real project.
#[test]
fn a_schemas_type_name_is_not_a_credential() {
    let source = r#"<?php

final class AccessToken
{
    public const array SCHEMA = [
        'access_token' => 'string',
    ];
}
"#;

    expect![[r#"
        no findings
    "#]]
    .assert_eq(&render(&security_findings(source)));
}

/// An attribute-casting map. `'password' => 'hashed'` says how the field is stored, and it is the
/// first thing a fresh Laravel application's `User` model declares -- the one finding a new install
/// got.
#[test]
fn a_casting_maps_type_name_is_not_a_credential() {
    let source = r#"<?php

final class User
{
    protected function casts(): array
    {
        return [
            'password' => 'hashed',
            'api_token' => 'encrypted:array',
        ];
    }
}
"#;

    expect![[r#"
        no findings
    "#]]
    .assert_eq(&render(&security_findings(source)));
}

/// A route table. `'resetPassword' => '/secure/reset/password'` is where a request goes, not a
/// password -- the shape measured in `app_core/src/Bureau/BureauClient.php`.
#[test]
fn a_url_route_is_not_a_credential() {
    let source = r#"<?php

final class BureauClient
{
    private const array ENDPOINTS = [
        'resetPassword' => '/secure/reset/password',
    ];
}
"#;

    expect![[r#"
        no findings
    "#]]
    .assert_eq(&render(&security_findings(source)));
}

/// A pointer to a secret store, not the secret -- `$data['clientSecret'] = 'vault/path/to/it'`
/// in `app_sanctions/tests/src/Unit/RiskConfigTest.php`, reached through the assignment/array-index
/// path rather than an array literal, so it covers a different code path than the schema and route
/// cases above.
#[test]
fn a_secret_store_pointer_is_not_a_credential() {
    let source = "<?php\n\n$data['clientSecret'] = 'vault/path/to/it';\n";

    expect![[r#"
        no findings
    "#]]
    .assert_eq(&render(&security_findings(source)));
}

/// A key-to-key lookup table that happens to repeat its own name -- `'password' => 'password'` in
/// `app_core/src/PaymentGateway/PaymentGatewayManager.php`, mapping a display name to the Drupal
/// config key that holds the real value.
#[test]
fn a_value_identical_to_its_own_name_is_not_a_credential() {
    let source = r#"<?php

function payment_gateway_settings(): array
{
    return [
        'password' => 'password',
    ];
}
"#;

    expect![[r#"
        no findings
    "#]]
    .assert_eq(&render(&security_findings(source)));
}

/// An error-response key -- `['token' => 'Missing required parameter.']` in
/// `wh_consent_tracker/.../ConsentResource.php` -- names the missing parameter, and does not store
/// one. A sentence (it contains a space) is the tell.
#[test]
fn a_sentence_is_not_a_credential() {
    let source = r#"<?php

function fail(): array
{
    return ['token' => 'Missing required parameter.'];
}
"#;

    expect![[r#"
        no findings
    "#]]
    .assert_eq(&render(&security_findings(source)));
}

/// Regression guard: narrowing the rule must not turn it off. A property actually holding something
/// password-shaped is still reported.
#[test]
fn an_actual_looking_secret_is_still_reported() {
    let source =
        "<?php\n\nfinal class Runner\n{\n    private string $secret = 'Sup3rSecretValue!42';\n}\n";

    expect![[r#"
        Error no-literal-password: Literal passwords or sensitive data should not be stored in code.
    "#]]
    .assert_eq(&render(&security_findings(source)));
}

/// Regression guard for the other code path: a real credential assigned through an array index,
/// the same shape as `docroot/sites/default/default.settings.local.php`'s hardcoded passwords.
#[test]
fn an_actual_looking_secret_assigned_by_array_index_is_still_reported() {
    let source = "<?php\n\n$settings['password'] = 'AmeriFirstBeta1';\n";

    expect![[r#"
        Error no-literal-password: Literal passwords or sensitive data should not be stored in code.
    "#]]
    .assert_eq(&render(&security_findings(source)));
}

/// Regression guard for the narrowing that nearly went in blanket. base64 and bcrypt both put `/`
/// in their alphabets, so "the value contains a slash" -- which is what suppresses the route two
/// tests up -- would have thrown away the encoded secrets this rule is most worth having for, and
/// thrown them away silently. A path is a slash-separated run of lowercase words; a token carries
/// capitals or digits.
#[test]
fn a_base64_secret_with_a_slash_in_it_is_still_reported() {
    let source =
        "<?php\n\nfinal class Runner\n{\n    private string $apiSecret = 'aB3/xY9zQ1wK+7t=';\n}\n";

    expect![[r#"
        Error no-literal-password: Literal passwords or sensitive data should not be stored in code.
    "#]]
    .assert_eq(&render(&security_findings(source)));
}
