//! Where a cursor on a string lands when the string is a callable: `[$formatter, 'format']`,
//! `'App\Repo::find'`, `'App\render'` -- and where it does not, which is everything the index does
//! not hold, since the same shapes are written as data.

use expect_test::expect;

use super::{Query, check_view};

#[test]
fn a_string_lands_on_what_it_names_and_only_where_that_exists() {
    check_view(
        r#"
//- /src/app.php
<?php
namespace App;

class Repo
{
    public function format(): string { return ''; }
    public static function find(): void {}
}

function render(): void {}

function run(Repo $repo, int $id): void
{
    array_map([$repo, 'for@pair@mat'], []);
    array_map('App\Repo::fi@static@nd', []);
    array_map('App\ren@function@der', []);
    $order = [$id, 'as@data@c'];
    $label = 'ren@bare@der';
    $none = [$repo, 'miss@missing@ing'];
}
"#,
        &[
            Query::goto("a method of the variable's class", "pair"),
            Query::goto("a static method written with the class", "static"),
            Query::goto("a function written qualified", "function"),
            Query::goto("a pair holding data names nothing", "data"),
            Query::goto("a bare word is a word", "bare"),
            Query::goto("a method the class does not declare", "missing"),
        ],
        expect![[r#"
            a method of the variable's class
            - method App\Repo::format @ src/app.php 6:21-6:27
            a static method written with the class
            - method App\Repo::find @ src/app.php 7:28-7:32
            a function written qualified
            - function App\render @ src/app.php 10:10-10:16
            a pair holding data names nothing
            - (not found)
            a bare word is a word
            - (not found)
            a method the class does not declare
            - (not found)
        "#]],
    );
}
