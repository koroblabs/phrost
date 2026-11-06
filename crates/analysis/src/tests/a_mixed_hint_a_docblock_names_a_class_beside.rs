//! A `: mixed` hint beside a docblock that writes `mixed|SomeClass`.
//!
//! BookStack's `setting(): mixed` is documented `@return mixed|SettingService` -- the service with no
//! key, a value with one. Navigation follows the class; a judgement reads the union as the `mixed`
//! it holds and says nothing about where the value is passed.

use expect_test::expect;

use super::{Query, check, diagnostics};

const FIXTURE: &str = r#"
//- /src/helpers.php
<?php
namespace App;

class SettingService
{
    public function get(string $key): mixed {}
}

/** @return mixed|SettingService */
function setting(?string $key = null): mixed {}

function takes(string $value): void {}

function run(): void
{
    setting()->g@get@et('locale');
    takes(setting('locale'));
    takes([1]);
}
"#;

#[test]
fn the_class_beside_mixed_is_followed_and_nothing_is_judged() {
    check(
        FIXTURE,
        &[Query::goto(
            "goto through the class the docblock names",
            "get",
        )],
        expect![[r#"
            goto through the class the docblock names
            - method App\SettingService::get @ src/helpers.php 6:21-6:24 (full 6:5-6:47)
        "#]],
    );
}

#[test]
fn a_call_documented_mixed_beside_a_class_is_not_judged() {
    // The second call is the control: a check that runs, refusing an array where a string goes.
    let fixture = FIXTURE.replace(
        "setting()->g@get@et('locale');",
        "setting()->get('locale');",
    );
    diagnostics::check(
        &fixture,
        expect![[r#"
        18:11-18:14 error [incompatible-argument] $value takes string, and this is array{0: int}
    "#]],
    );
}
