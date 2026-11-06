//! A literal key a catalog does not list, where the catalog lists only the exceptions.
//!
//! Laravel's `trans()`: the line a key names, the array a group or a nested list is, and the key
//! itself where it names nothing -- so only the keys holding arrays are filed, and every other
//! literal is a string. A key holding the catalog's `outside` text is not its to answer, and a key
//! that is not a literal leaves the declaration's `array|string` standing.

use expect_test::expect;
use phrost_model::BuiltinTy;

use super::{Query, check_view_ingested_with};

#[test]
fn an_unlisted_literal_holds_what_the_catalog_says() {
    check_view_ingested_with(
        r#"
//- /vendor/helpers.php
<?php
function trans(?string $key = null): array|string {}

//- /src/run.php
<?php
function run(string $dynamic): void
{
    $line = trans('auth.failed');
    $@line@line;
    $group = trans('auth');
    $@group@group;
    $nested = trans('validation.between');
    $@nested@nested;
    $package = trans('vendor::auth.failed');
    $@package@package;
    $written = trans($dynamic);
    $@written@written;
}
"#,
        &[
            Query::variable_type("a key nothing lists", "line"),
            Query::variable_type("a group", "group"),
            Query::variable_type("a nested list", "nested"),
            Query::variable_type("outside the catalog", "package"),
            Query::variable_type("not a literal", "written"),
        ],
        |world| {
            let contracts = world.contracts_mut();
            contracts.bind_function_argument("trans", 0, "translations");
            contracts.catalog_shape("translations", "auth", BuiltinTy::Array);
            contracts.catalog_shape("translations", "validation.between", BuiltinTy::Array);
            contracts.bind_unlisted_key("translations", BuiltinTy::String, "::");
        },
        expect![[r#"
            a key nothing lists
            - string
            a group
            - array
            a nested list
            - array
            outside the catalog
            - array|string
            not a literal
            - array|string
        "#]],
    );
}
