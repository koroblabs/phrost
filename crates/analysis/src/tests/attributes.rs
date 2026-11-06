//! An attribute names a class, and every position PHP lets one appear.
//!
//! A file of its own, and one marker per position, because the name is recorded during lowering at
//! each declaration that can carry attributes -- and PHP lets nearly all of them. A position left
//! out of that list is silent otherwise: the attribute still parses, the declaration still lowers,
//! and only goto on the name goes quiet.

use expect_test::expect;

use crate::tests::{Query, check};

#[test]
fn goto_reaches_the_class_an_attribute_names_wherever_it_is_written() {
    check(
        r#"
//- /src/Marks.php
<?php
namespace App;

#[\Attribute]
class Mark {}

//- /src/Uses.php
<?php
namespace App;

#[Ma@on_class@rk]
class Holder
{
    #[Ma@on_constant@rk]
    public const string VERSION = '1';

    #[Ma@on_property@rk]
    private string $label = '';

    public function __construct(
        #[Ma@on_promoted@rk] private int $count,
        #[Ma@on_param@rk] string $note,
    ) {}

    #[Ma@on_method@rk]
    public function run(): void {}
}

#[Ma@on_interface@rk]
interface Contract {}

#[Ma@on_trait@rk]
trait Helper {}

#[Ma@on_enum@rk]
enum Stage: string
{
    #[Ma@on_case@rk]
    case Draft = 'draft';
}

#[Ma@on_function@rk]
function helper(): void {}
"#,
        &[
            Query::goto("on a class", "on_class"),
            Query::goto("on a class constant", "on_constant"),
            Query::goto("on a property", "on_property"),
            Query::goto("on a promoted property", "on_promoted"),
            Query::goto("on a plain parameter", "on_param"),
            Query::goto("on a method", "on_method"),
            Query::goto("on an interface", "on_interface"),
            Query::goto("on a trait", "on_trait"),
            Query::goto("on an enum", "on_enum"),
            Query::goto("on an enum case", "on_case"),
            Query::goto("on a function", "on_function"),
        ],
        expect![[r#"
            on a class
            - class App\Mark @ src/Marks.php 5:7-5:11 (full 4:1-5:14)
            on a class constant
            - class App\Mark @ src/Marks.php 5:7-5:11 (full 4:1-5:14)
            on a property
            - class App\Mark @ src/Marks.php 5:7-5:11 (full 4:1-5:14)
            on a promoted property
            - class App\Mark @ src/Marks.php 5:7-5:11 (full 4:1-5:14)
            on a plain parameter
            - class App\Mark @ src/Marks.php 5:7-5:11 (full 4:1-5:14)
            on a method
            - class App\Mark @ src/Marks.php 5:7-5:11 (full 4:1-5:14)
            on an interface
            - class App\Mark @ src/Marks.php 5:7-5:11 (full 4:1-5:14)
            on a trait
            - class App\Mark @ src/Marks.php 5:7-5:11 (full 4:1-5:14)
            on an enum
            - class App\Mark @ src/Marks.php 5:7-5:11 (full 4:1-5:14)
            on an enum case
            - class App\Mark @ src/Marks.php 5:7-5:11 (full 4:1-5:14)
            on a function
            - class App\Mark @ src/Marks.php 5:7-5:11 (full 4:1-5:14)
        "#]],
    );
}

#[test]
fn hover_on_an_attribute_describes_the_class_it_names() {
    check(
        r#"
//- /src/Marks.php
<?php
namespace App;

/** Marks a thing worth marking. */
#[\Attribute]
final class Mark
{
    public function __construct(public string $why) {}
}

//- /src/Uses.php
<?php
namespace App;

#[Ma@here@rk(why: 'because')]
class Holder {}
"#,
        &[Query::hover("attribute", "here")],
        expect![[r#"
            attribute
            - code: <?php
            namespace App;

            final class Mark
            - ---
            - doc: Marks a thing worth marking.
        "#]],
    );
}
