//! What a `$` typed inside a function's docblock offers: the parameters it can name.
//!
//! Goto and hover already read `$name` there as the parameter; completion offered nothing, because
//! nothing is in scope inside a comment. On a `@param` line a parameter another tag already
//! documents is left out, since a second tag for it is a mistake.

use expect_test::expect;

use crate::tests::{Query, check};

#[test]
fn a_param_tag_offers_the_parameters_not_yet_documented() {
    check(
        r#"
//- /src/Reviewer.php
<?php
namespace App;

class Reviewer
{
    /**
     * @param string $text
     * @param int $@here@
     */
    public function failed(string $text, int $times, ?array $context = NULL): bool {}
}
"#,
        &[Query::completion("second tag", "here")],
        expect![[r#"
            second tag
            - $times [variable]
            - $context [variable]
            - ... and the client must ask again
            - accepting the first leaves `* @param int $times`
        "#]],
    );
}

#[test]
fn a_typed_prefix_narrows_them() {
    check(
        r#"
//- /src/Reviewer.php
<?php
namespace App;

/**
 * @param string $te@here@
 */
function failed(string $text, int $times): bool {}
"#,
        &[Query::completion("prefix", "here")],
        expect![[r#"
            prefix
            - $text [variable]
            - ... and the client must ask again
            - accepting the first leaves `* @param string $text`
        "#]],
    );
}

#[test]
fn prose_may_name_any_of_them() {
    check(
        r#"
//- /src/Reviewer.php
<?php
namespace App;

class Reviewer
{
    /**
     * Fails when $@here@ is empty.
     *
     * @param string $text
     */
    public function failed(string $text, int $times): bool {}
}
"#,
        &[Query::completion("prose", "here")],
        expect![[r#"
            prose
            - $text [variable]
            - $times [variable]
            - ... and the client must ask again
            - accepting the first leaves `* Fails when $text is empty.`
        "#]],
    );
}

#[test]
fn a_docblock_with_no_function_under_it_offers_nothing() {
    check(
        r#"
//- /src/Reviewer.php
<?php
namespace App;

/**
 * About $@here@ things.
 */
class Reviewer {}
"#,
        &[Query::completion("class docblock", "here")],
        expect![[r#"
            class docblock
            - (nothing offered)
        "#]],
    );
}
