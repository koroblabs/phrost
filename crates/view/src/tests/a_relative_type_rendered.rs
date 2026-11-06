//! A rendered type never says `self`, `static` or `parent`.
//!
//! Those three are answers about where a type was *written*, and a reader looking at a value has
//! already been told that by the declaration. So a surviving relative word in a rendered type is
//! always wrong, whatever else is right about it -- which makes it worth asserting on its own.
//!
//! The shape that made this necessary is `@return list<static>`, which every backed enum's
//! `cases()` inherits from `UnitEnum`: the word sits inside a generic argument, and substituting
//! only the outermost node left it there as `static[]`.

use expect_test::expect;

use super::{Query, check_view};

#[test]
fn a_relative_type_nested_in_a_generic_is_the_class_it_means() {
    check_view(
        r#"
//- /src/Contracts.php
<?php
namespace App;

interface Listable
{
    /** @return list<static> */
    public static function all(): array;

    public static function first(): static;

    /** @return array<string, ?static> */
    public static function keyed(): array;
}

//- /src/Stage.php
<?php
namespace App;

class Stage implements Listable
{
    public function probe(): void
    {
        $every = Stage::all();
        $ever@every@y;
        $one = Stage::first();
        $o@one@ne;
        $by_key = Stage::keyed();
        $by_@keyed@key;
    }
}
"#,
        &[
            Query::variable_type("list<static>", "every"),
            Query::variable_type("bare static", "one"),
            Query::variable_type("nullable inside a map", "keyed"),
        ],
        expect![[r#"
            list<static>
            - array<int, Stage>
            bare static
            - Stage
            nullable inside a map
            - array<string, ?Stage>
        "#]],
    );
}

/// The word stands as written where nothing can answer for it, rather than taking the rest of the
/// type down with it.
#[test]
fn a_relative_type_with_no_scope_keeps_the_word() {
    check_view(
        r#"
//- /src/free.php
<?php
namespace App;

/** @return list<static> */
function everything(): array
{
    return [];
}

//- /src/Use.php
<?php
namespace App;

class Use_
{
    public function probe(): void
    {
        $all = everything();
        $a@here@ll;
    }
}
"#,
        &[Query::variable_type("no enclosing class to mean", "here")],
        expect![[r#"
            no enclosing class to mean
            - array<int, static>
        "#]],
    );
}
