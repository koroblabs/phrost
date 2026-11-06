//! Where a docblock's type ends.
//!
//! The rule was one word after the tag, and two shapes break it: a map's value type sits after a
//! space -- `array<string, Transport` -- and `@method static Builder` says `static` before the type.
//! Both read as prose and offered nothing; the second is on every line of Laravel's facades.

use expect_test::expect;

use crate::tests::{Query, check};

const DECLARED: &str = r#"
//- /src/Transport.php
<?php
namespace App;

interface Transport {}
"#;

fn check_declared(client: &str, queries: &[Query], expect: expect_test::Expect) {
    check(&format!("{DECLARED}\n{client}"), queries, expect);
}

#[test]
fn a_map_value_after_the_space() {
    check_declared(
        r#"
//- /src/Transports.php
<?php
namespace App;

class Transports
{
    /** @var array<string, Transp@here@> */
    private array $all = [];
}
"#,
        &[Query::completion("map value", "here")],
        expect![[r#"
            map value
            - Transport [interface]  App\Transport  insert `\App\Transport`
            - Transports [class]  App\Transports  insert `\App\Transports`
            - ... and the client must ask again
            - accepting the first leaves `/** @var array<string, \App\Transport> */`
        "#]],
    );
}

#[test]
fn the_return_type_of_a_static_method_tag() {
    check_declared(
        r#"
//- /src/Facade.php
<?php
namespace App;

/**
 * @method static Transp@here@ make()
 */
class Facade {}
"#,
        &[Query::completion("@method static", "here")],
        expect![[r#"
            @method static
            - Transport [interface]  App\Transport  insert `\App\Transport`
            - ... and the client must ask again
            - accepting the first leaves `* @method static \App\Transport make()`
        "#]],
    );
}

#[test]
fn the_word_after_the_type_is_still_prose() {
    // The method's name, which would offer `Transport` if the type were read as going on.
    check_declared(
        r#"
//- /src/Facade.php
<?php
namespace App;

/**
 * @method static array<int, Transport> Transp@here@()
 */
class Facade {}
"#,
        &[Query::completion("method name", "here")],
        expect![[r#"
            method name
            - (nothing offered)
        "#]],
    );
}

#[test]
fn a_parameter_type_inside_a_method_tag() {
    check_declared(
        r#"
//- /src/Facade.php
<?php
namespace App;

/**
 * @method static void send(array<int, string> $to, Transp@here@ $via)
 */
class Facade {}
"#,
        &[Query::completion("@method parameter", "here")],
        expect![[r#"
            @method parameter
            - Transport [interface]  App\Transport  insert `\App\Transport`
            - ... and the client must ask again
            - accepting the first leaves `* @method static void send(array<int, string> $to, \App\Transport $via)`
        "#]],
    );
}

#[test]
fn a_parameter_name_inside_a_method_tag_is_prose() {
    check_declared(
        r#"
//- /src/Facade.php
<?php
namespace App;

/**
 * @method static void send(Transport Transp@here@)
 */
class Facade {}
"#,
        &[Query::completion("@method parameter name", "here")],
        expect![[r#"
            @method parameter name
            - (nothing offered)
        "#]],
    );
}

/// A shape written over several lines is one type, and every line inside its braces is in it --
/// though none of them carries the tag. The reference project writes `@param array{` with a key
/// per line throughout its forms.
#[test]
fn a_shape_written_over_several_lines() {
    check_declared(
        r#"
//- /src/Report.php
<?php
namespace App;

/**
 * @param array{
 *   transport: Transp@inside@,
 *   name: string
 * } $info
 *   Transp@after@ is described here.
 */
function report(array $info): void {}
"#,
        &[
            Query::completion("inside the braces", "inside"),
            Query::completion("the description after", "after"),
        ],
        expect![[r#"
            inside the braces
            - Transport [interface]  App\Transport  insert `\App\Transport`
            - ... and the client must ask again
            - accepting the first leaves `*   transport: \App\Transport,`
            the description after
            - (nothing offered)
        "#]],
    );
}

/// A template's bound is the type, and its name is a declaration.
///
/// `@template TModel of \Illuminate\Database\Eloquent\Model` writes the name first, so the space
/// after it read as the end of the type and the bound offered nothing -- four sites in one Laravel
/// package, where goto on the same word went to the class. Psalm writes `as` for `of`.
#[test]
fn a_template_bound_after_of_or_as() {
    check_declared(
        r#"
//- /src/Of.php
<?php
namespace App;

/**
 * @template TItem of Transp@of@
 */
class Of {}

//- /src/As.php
<?php
namespace App;

/**
 * @psalm-template TItem as Transp@as@
 */
class As {}

//- /src/Name.php
<?php
namespace App;

/**
 * @template TTrans@name@
 */
class Name {}
"#,
        &[
            Query::completion("after of", "of"),
            Query::completion("after as", "as"),
            Query::completion("the name being declared", "name"),
        ],
        expect![[r#"
            after of
            - Transport [interface]  App\Transport  insert `\App\Transport`
            - ... and the client must ask again
            - accepting the first leaves `* @template TItem of \App\Transport`
            after as
            - Transport [interface]  App\Transport  insert `\App\Transport`
            - ... and the client must ask again
            - accepting the first leaves `* @psalm-template TItem as \App\Transport`
            the name being declared
            - (nothing offered)
        "#]],
    );
}
