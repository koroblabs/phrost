//! A global or namespaced constant read as the type its value states.
//!
//! `const LIMIT = 5` and `define('ABSPATH', __DIR__ . '/')` were read as nothing at all, so every
//! local assigned one was untyped and every argument written as one was judged by nobody. The
//! type is the value's, read the way a class constant's is and kept in the index -- the value is
//! text in a file a closed buffer does not have.

use expect_test::expect;

use super::{Query, check_view};

#[test]
fn each_spelling_reaches_the_declaration_and_its_value() {
    check_view(
        r#"
//- /stubs/core.php  stub
<?php
const PHP_INT_MAX = 9223372036854775807;
const PHP_EOL = PHP_EOL;
/** @var string */
const DIRECTORY_SEPARATOR = DIRECTORY_SEPARATOR;

//- /src/config.php
<?php
namespace App;

define('ROOT', __DIR__ . '/');
/** @var string */
define('HOME', getenv('HOME'));
const LIMIT = 5;
const SIZES = ['thumb' => 200];

//- /src/app.php
<?php
namespace App;

function run(): void
{
    $d@defined@efined = ROOT;
    $c@namespaced@onst = LIMIT;
    $q@qualified@ualified = \App\LIMIT;
    $g@global@lobal = PHP_INT_MAX;
    $t@table@able = SIZES;
    $s@unsaid@elf = PHP_EOL;
    $v@tagged@ar = DIRECTORY_SEPARATOR;
    $h@tagdefine@ome = HOME;
}
"#,
        &[
            Query::variable_type("a define() is global", "defined"),
            Query::variable_type("a namespaced const", "namespaced"),
            Query::variable_type("written in full", "qualified"),
            Query::variable_type("the global one, reached by fallback", "global"),
            Query::variable_type("a table", "table"),
            Query::variable_type("a value naming another constant says nothing", "unsaid"),
            Query::variable_type("unless a `@var` says it", "tagged"),
            Query::variable_type(
                "a `@var` above a define() whose value says nothing",
                "tagdefine",
            ),
        ],
        expect![[r#"
            a define() is global
            - non-falsy-string
            a namespaced const
            - int
            written in full
            - int
            the global one, reached by fallback
            - int
            a table
            - array<non-falsy-string, int>
            a value naming another constant says nothing
            - (no type)
            unless a `@var` says it
            - string
            a `@var` above a define() whose value says nothing
            - string
        "#]],
    );
}
