//! What a class header and an attribute offer, which is what PHP lets stand there.
//!
//! `implements` takes an interface, an `interface`'s `extends` takes interfaces, a class's takes a
//! class, and an attribute is a class. Each of those was offered the whole catalog: `implements
//! Rep|` listed the class, the trait, the enum and the free function beside the one interface that
//! can actually go there -- which is the first completion anybody triggers when declaring a class.

use expect_test::expect;

use crate::tests::{Query, check};

/// One declaration of each kind, all answering to the same prefix, so a snapshot says which of them
/// survive a position rather than only that something was offered.
const KIT: &str = r#"
//- /src/kit.php
<?php
namespace App;

class Repo {}

interface Repository {}

trait Repeatable {}

enum Repeated {}

function repay(): void {}

const REPAID = 1;
"#;

fn asking(question: &str) -> String {
    format!("{KIT}{question}")
}

#[test]
fn only_an_interface_is_offered_after_implements() {
    check(
        &asking(
            r#"
//- /src/Client.php
<?php
namespace App;

class Client implements Rep@here@
"#,
        ),
        &[Query::completion("after implements", "here")],
        expect![[r#"
            after implements
            - Repository [interface]  App\Repository
            - ... and the client must ask again
            - accepting the first leaves `class Client implements Repository`
        "#]],
    );
}

#[test]
fn a_class_extends_a_class_and_an_interface_extends_interfaces() {
    check(
        &asking(
            r#"
//- /src/Client.php
<?php
namespace App;

class Client extends Rep@class@

//- /src/Contract.php
<?php
namespace App;

interface Contract extends Rep@interface@
"#,
        ),
        &[
            Query::completion("a class extends a class", "class"),
            Query::completion("an interface extends interfaces", "interface"),
        ],
        expect![[r#"
            a class extends a class
            - Repo [class]  App\Repo
            - ... and the client must ask again
            - accepting the first leaves `class Client extends Repo`
            an interface extends interfaces
            - Repository [interface]  App\Repository
            - ... and the client must ask again
            - accepting the first leaves `interface Contract extends Repository`
        "#]],
    );
}

/// An attribute is an ordinary class, and PHP constructs one to read it.
#[test]
fn only_a_class_is_offered_inside_an_attribute() {
    check(
        &asking(
            r#"
//- /src/Client.php
<?php
namespace App;

#[Rep@here@
class Client {}
"#,
        ),
        &[Query::completion("inside an attribute", "here")],
        expect![[r#"
            inside an attribute
            - Repo [class]  App\Repo
            - ... and the client must ask again
            - accepting the first leaves `#[Repo`
        "#]],
    );
}

/// And among the classes, the ones that are attributes come first.
///
/// The end of the seam the projection's own tests cover: what makes this reach them is the place's
/// kind, and `#[` is the only position that names it. Written with an attribute that sorts last
/// alphabetically, so that the order can only have come from the tier.
#[test]
fn the_classes_that_are_attributes_are_offered_first() {
    check(
        &asking(
            r#"
//- /src/Zone.php
<?php
namespace App;

#[\Attribute]
final class Zone {}

//- /src/Client.php
<?php
namespace App;

#[@here@
class Client {}
"#,
        ),
        &[Query::completion("at a bare attribute", "here")],
        expect![[r#"
            at a bare attribute
            - Zone [class]  App\Zone
            - Repo [class]  App\Repo
            - accepting the first leaves `#[Zone`
        "#]],
    );
}

/// And the arguments of an attribute are an argument position like any other, which is what the
/// bracket test must not swallow.
#[test]
fn an_attributes_arguments_are_not_a_name_position() {
    check(
        r#"
//- /src/Route.php
<?php
namespace App;

#[\Attribute]
class Route
{
    public function __construct(public string $path) {}
}

//- /src/Client.php
<?php
namespace App;

#[Route(pa@here@)]
class Client {}
"#,
        &[Query::completion("inside the arguments", "here")],
        expect![[r#"
            inside the arguments
            - path: [variable]  string  insert `path: `
            - ... and the client must ask again
            - accepting the first leaves `#[Route(path: )]`
        "#]],
    );
}
