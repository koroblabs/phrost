//! Goto-type-definition, asked from every site a local can be written at.
//!
//! The three sites are typed three different ways -- a parameter by its hint, an assignment target
//! by its own right-hand side, a use by searching backwards for what bound it -- and `variable_at`
//! is the one function that knows which is which. Goto-type-definition did not ask it: it took the
//! *name* and searched backwards, which finds nothing at an assignment target because the only
//! thing that binds it is on the same line. Hover, one key away on the same offset, named the class.
//!
//! §8.1's oldest form, and the repair it prescribes is the seam: the class travels beside the
//! rendered type, out of the function that already worked out which site this is.

use expect_test::expect;

use crate::tests::{Query, check};

#[test]
fn every_site_a_local_is_written_at_jumps_to_its_type() {
    check(
        r#"
//- /src/app.php
<?php
namespace App;

class Result {}

class Handler
{
    public function handle(): Result
    {
        return new Result();
    }
}

class Caller
{
    public function run(Handler $h, Result $gi@parameter@ven): void
    {
        $assi@assignment@gned = $h->handle();
        echo $ass@use@igned->x;
        echo $gi@second@ven->x;
    }
}
"#,
        &[
            Query::goto_type("at a parameter's own name", "parameter"),
            Query::goto_type("at an assignment target", "assignment"),
            Query::goto_type("at a use of the assigned name", "use"),
            Query::goto_type("at a use of the parameter", "second"),
        ],
        expect![[r#"
            at a parameter's own name
            - class App\Result @ src/app.php 4:7-4:13 (full 4:1-4:16)
            at an assignment target
            - class App\Result @ src/app.php 4:7-4:13 (full 4:1-4:16)
            at a use of the assigned name
            - class App\Result @ src/app.php 4:7-4:13 (full 4:1-4:16)
            at a use of the parameter
            - class App\Result @ src/app.php 4:7-4:13 (full 4:1-4:16)
        "#]],
    );
}

/// An intersection is declared in each of its parts, as a union is in each of its operands.
///
/// `$urlGenerator = $this->createMock(UrlGeneratorInterface::class)` holds
/// `MockObject&UrlGeneratorInterface`; hover said so and the jump answered nothing, because the list
/// of classes a type names was filled for a union only and the single class an intersection reduces
/// to does not exist. Found by the capability audit on the reference project.
#[test]
fn an_intersection_jumps_to_each_of_its_parts() {
    check(
        r#"
//- /src/Mock.php
<?php
namespace Dep;

interface Mock {}

//- /src/Generator.php
<?php
namespace App;

interface Generator {}

//- /src/Test.php
<?php
namespace App;

use Dep\Mock;

class Test
{
    public function run(Mock&Generator $generator): void
    {
        $gen@here@erator;
    }
}
"#,
        &[Query::goto_type("an intersection", "here")],
        expect![[r#"
            an intersection
            - interface Dep\Mock @ src/Mock.php 4:11-4:15 (full 4:1-4:18)
            - interface App\Generator @ src/Generator.php 4:11-4:20 (full 4:1-4:23)
        "#]],
    );
}
