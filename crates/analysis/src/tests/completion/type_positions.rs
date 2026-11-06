//! What a position that takes a type offers, which is not what an expression offers.
//!
//! `function run(): st|` answered `Stack`, `stat` and eighteen `stats_cdf_*` functions and did not
//! answer `string`. Two halves of one gap: the words PHP writes a type with were in no list at all,
//! and the list it did offer was the whole catalog, where a function or a constant can never stand.
//!
//! Which keywords are legal is the language's business rather than a ranking. `void`, `never` and
//! `static` describe a return and nothing else; an enum's backing type is `int` or `string` and not
//! one name a project can declare; a `catch` takes a class and no keyword at all; and a docblock has
//! a vocabulary PHP has no syntax for.

use expect_test::expect;

use crate::tests::{Query, check};

/// One class, so the lists below are short enough to read whole.
const PROJECT: &str = r#"
//- /src/Stage.php
<?php
namespace App;

class Stage {}
"#;

#[test]
fn a_return_type_offers_the_words_only_a_return_can_say() {
    check(
        &format!(
            r#"{PROJECT}
//- /src/Repo.php
<?php
namespace App;

class Repo
{{
    public function run(): st@here@
}}
"#
        ),
        &[Query::completion("a return type", "here")],
        expect![[r#"
            a return type
            - static [keyword]
            - string [keyword]
            - Stage [class]  App\Stage
            - ... and the client must ask again
            - accepting the first leaves `public function run(): static`
        "#]],
    );
}

#[test]
fn a_parameter_type_leaves_out_what_only_a_return_may_be() {
    check(
        &format!(
            r#"{PROJECT}
//- /src/Repo.php
<?php
namespace App;

class Repo
{{
    public function run(st@here@
}}
"#
        ),
        &[Query::completion("a parameter type", "here")],
        expect![[r#"
            a parameter type
            - string [keyword]
            - Stage [class]  App\Stage
            - ... and the client must ask again
            - accepting the first leaves `public function run(string`
        "#]],
    );
}

#[test]
fn an_enum_is_backed_by_two_words_and_by_nothing_a_project_declares() {
    check(
        &format!(
            r#"{PROJECT}
//- /src/Status.php
<?php
namespace App;

enum Status: @here@
"#
        ),
        &[Query::completion("a backing type", "here")],
        expect![[r#"
            a backing type
            - int [keyword]
            - string [keyword]
            - accepting the first leaves `enum Status: int`
        "#]],
    );
}

#[test]
fn a_catch_takes_a_class_and_no_keyword_at_all() {
    check(
        &format!(
            r#"{PROJECT}
//- /src/Repo.php
<?php
namespace App;

class Repo
{{
    public function run(): void
    {{
        try {{
            echo 1;
        }} catch (Sta@here@
    }}
}}
"#
        ),
        &[Query::completion("a caught class", "here")],
        expect![[r#"
            a caught class
            - Stage [class]  App\Stage
            - ... and the client must ask again
            - accepting the first leaves `} catch (Stage`
        "#]],
    );
}

/// The one position with a vocabulary of its own: `list`, `class-string` and their neighbours are
/// what the tag language says and PHP has no word for.
#[test]
fn a_docblock_tag_offers_what_only_a_docblock_can_say() {
    check(
        &format!(
            r#"{PROJECT}
//- /src/Repo.php
<?php
namespace App;

class Repo
{{
    /** @return non-@here@ */
    public function run() {{}}
}}
"#
        ),
        &[Query::completion("a docblock type", "here")],
        expect![[r#"
            a docblock type
            - non-empty-array [keyword]
            - non-empty-list [keyword]
            - non-empty-string [keyword]
            - non-negative-int [keyword]
            - ... and the client must ask again
            - accepting the first leaves `/** @return non-empty-array */`
        "#]],
    );
}

/// The negative half, and the load-bearing one. Three of these end in the same character a type
/// position does and are not one; offering `void` in a ternary is the failure this would have.
#[test]
fn a_colon_that_introduces_no_type_still_offers_an_expression() {
    check(
        &format!(
            r#"{PROJECT}
//- /src/Repo.php
<?php
namespace App;

class Repo
{{
    public function run(bool $flag): int
    {{
        return $flag ? 1 : st@here@;
    }}
}}
"#
        ),
        &[Query::completion("a ternary's else", "here")],
        expect![[r#"
            a ternary's else
            - Stage [class]  App\Stage
            - ... and the client must ask again
            - accepting the first leaves `return $flag ? 1 : Stage;`
        "#]],
    );
}
