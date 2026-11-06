//! Which names a docblock tag takes, when it is not a type.
//!
//! Every name-taking tag was read as a type, so `@use HasFactory<UserFactory>` -- Laravel's
//! spelling, on every model -- offered nothing, a trait being the one class-like nothing is an
//! instance of. Inside its brackets it is a type position again. `@global MO $l10n`, WordPress's, and `@see` were not read as taking a name at all.

use expect_test::expect;

use crate::tests::{Query, check};

const DECLARED: &str = r#"
//- /src/Kinds.php
<?php
namespace App;

trait HasFactory {}
class HasFactoryReport {}
function has_factory(): void {}
"#;

fn check_with_kinds(client: &str, queries: &[Query], expect: expect_test::Expect) {
    check(&format!("{DECLARED}\n{client}"), queries, expect);
}

#[test]
fn use_takes_a_trait() {
    check_with_kinds(
        r#"
//- /src/User.php
<?php
namespace App;

class User
{
    /** @use HasFac@here@<UserFactory> */
    use HasFactory;
}
"#,
        &[Query::completion("@use", "here")],
        expect![[r#"
            @use
            - HasFactory [trait]  App\HasFactory  insert `\App\HasFactory`
            - ... and the client must ask again
            - accepting the first leaves `/** @use \App\HasFactory<UserFactory> */`
        "#]],
    );
}

/// Inside the brackets the trait's arguments are written, and those are types like any other: the
/// factory a model's `@use HasFactory<UserFactory>` names is a class, not a trait.
#[test]
fn use_takes_types_inside_its_brackets() {
    check_with_kinds(
        r#"
//- /src/User.php
<?php
namespace App;

class User
{
    /** @use HasFactory<HasFac@here@> */
    use HasFactory;
}
"#,
        &[Query::completion("@use", "here")],
        expect![[r#"
            @use
            - HasFactoryReport [class]  App\HasFactoryReport  insert `\App\HasFactoryReport`
            - ... and the client must ask again
            - accepting the first leaves `/** @use HasFactory<\App\HasFactoryReport> */`
        "#]],
    );
}

#[test]
fn global_takes_a_type() {
    check_with_kinds(
        r#"
//- /src/load.php
<?php
namespace App;

/**
 * @global HasFac@here@ $report
 */
function load(): void {}
"#,
        &[Query::completion("@global", "here")],
        expect![[r#"
            @global
            - HasFactoryReport [class]  App\HasFactoryReport  insert `\App\HasFactoryReport`
            - ... and the client must ask again
            - accepting the first leaves `* @global \App\HasFactoryReport $report`
        "#]],
    );
}

#[test]
fn see_takes_any_declaration() {
    check_with_kinds(
        r#"
//- /src/load.php
<?php
namespace App;

/**
 * @see has_fac@here@
 */
function load(): void {}
"#,
        &[Query::completion("@see", "here")],
        expect![[r#"
            @see
            - has_factory [function]  App\has_factory  insert `\App\has_factory`
            - ... and the client must ask again
            - accepting the first leaves `* @see \App\has_factory`
        "#]],
    );
}
