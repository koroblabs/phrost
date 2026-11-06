//! Goto from the local name an import binds.
//!
//! `use App\Repo as Store;` writes two names for one thing, and both answer the same question. The
//! alias needs its own range to be found at all: a grouped import shares one `full_range` across
//! every item in its braces, so that range says nothing about where any single alias is.

use expect_test::expect;

use super::{Query, check_view};

#[test]
fn goto_on_an_alias_lands_where_goto_on_the_path_does() {
    check_view(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repo
{
}

//- /src/Uses.php
<?php
namespace App\Other;

use App\Repo as St@target@ore;
"#,
        &[
            Query::cursor("cursor", "target"),
            Query::goto("goto", "target"),
        ],
        expect![[r#"
            cursor
            - import App\Repo [class]
            goto
            - class App\Repo @ src/Repo.php 4:7-4:11
        "#]],
    );
}

#[test]
fn an_alias_inside_a_group_is_found_too() {
    check_view(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repo
{
}

//- /src/Uses.php
<?php
namespace App\Other;

use App\{Repo as St@target@ore};
"#,
        &[
            Query::cursor("cursor", "target"),
            Query::goto("goto", "target"),
        ],
        expect![[r#"
            cursor
            - import App\Repo [class]
            goto
            - class App\Repo @ src/Repo.php 4:7-4:11
        "#]],
    );
}

#[test]
fn an_implicit_local_name_has_no_alias_to_stand_on() {
    check_view(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repo
{
}

//- /src/Uses.php
<?php
namespace App\Other;

use App\R@target@epo;
"#,
        &[
            Query::cursor("cursor", "target"),
            Query::goto("goto", "target"),
        ],
        expect![[r#"
            cursor
            - import App\Repo [class]
            goto
            - class App\Repo @ src/Repo.php 4:7-4:11
        "#]],
    );
}
