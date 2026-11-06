//! The kinds one file cannot settle and the index can.
//!
//! An interface, a trait and an enum are written exactly like a class, and `Foo::BAR` is a class
//! constant or an enum case -- the declaration is the only thing that says which, and it is in
//! another file. Rendered whole rather than as the refined rows alone, because what has to hold is
//! that nothing *else* moved.

use expect_test::{Expect, expect};
use phrost_harness::{World, with_open_scopes};
use phrost_testkit::FixtureSpec;
use phrost_view::IndexView;

use crate::semantic_tokens;

/// Every token of the fixture's last file, as its position, its text and its kind.
fn check(fixture: &str, expected: Expect) {
    let spec = FixtureSpec::parse(fixture);
    let world = World::build(&spec);
    let last = spec.files().last().expect("a fixture has a file");
    // No leading slash: the header writes one and `FixtureSpec` strips it, so a lookup that adds
    // it back finds nothing -- see the note this trap already has.
    let file = world.file_id(last.path());
    let text = last.text().to_owned();

    let rendered = with_open_scopes(&world, Some(file), |open| {
        let view = IndexView::in_file(world.skeleton(), open, file);
        semantic_tokens(&view, &text)
            .into_iter()
            .map(|token| {
                let at = usize::from(token.range.start());
                let to = usize::from(token.range.end());
                format!("{:<18} {:?}\n", &text[at..to], token.kind)
            })
            .collect::<String>()
    });

    expected.assert_eq(&rendered);
}

#[test]
fn an_interface_and_an_enum_in_use_are_not_a_class() {
    check(
        r#"
//- /src/Reads.php
<?php
namespace App;
interface Reads {}

//- /src/Status.php
<?php
namespace App;
enum Status: string { case Draft = 'draft'; }

//- /src/Plain.php
<?php
namespace App;
class Plain {}

//- /src/Use.php
<?php
namespace App;

function run(Reads $reads, Status $status, Plain $plain): void {}
"#,
        expect![[r#"
            <?php              Keyword
            namespace          Keyword
            App                Namespace
            function           Keyword
            run                Function
            Reads              Interface
            $reads             Parameter
            Status             Enum
            $status            Parameter
            Plain              Class
            $plain             Parameter
            void               Type
        "#]],
    );
}

/// The case, and the constant beside it that must stay one.
#[test]
fn a_constant_read_off_an_enum_is_a_case_and_one_off_a_class_is_not() {
    check(
        r#"
//- /src/Status.php
<?php
namespace App;
enum Status: string { case Draft = 'draft'; }

//- /src/Limits.php
<?php
namespace App;
class Limits { public const int MAX = 10; }

//- /src/Use.php
<?php
namespace App;

function run(): void
{
    $a = Status::Draft;
    $b = Limits::MAX;
}
"#,
        expect![[r#"
            <?php              Keyword
            namespace          Keyword
            App                Namespace
            function           Keyword
            run                Function
            void               Type
            $a                 Variable
            Status             Enum
            Draft              EnumMember
            $b                 Variable
            Limits             Class
            MAX                Property
        "#]],
    );
}

/// An enum names its own cases through `self`, where no written name reaches.
#[test]
fn self_inside_an_enum_reaches_its_own_cases() {
    check(
        r#"
//- /src/Status.php
<?php
namespace App;

enum Status: string
{
    case Draft = 'draft';

    public function letter(): string
    {
        return self::Draft->value;
    }
}
"#,
        expect![[r#"
            <?php              Keyword
            namespace          Keyword
            App                Namespace
            enum               Keyword
            Status             Enum
            string             Type
            case               Keyword
            Draft              EnumMember
            'draft'            Str
            public             Keyword
            function           Keyword
            letter             Method
            string             Type
            return             Keyword
            self               Keyword
            Draft              EnumMember
            value              Property
        "#]],
    );
}

/// The property the refinement protects: a name the index cannot resolve keeps what the tree
/// said, so a buffer opened before the project is read is coloured rather than grey.
#[test]
fn a_name_that_resolves_to_nothing_keeps_the_colour_the_tree_gave_it() {
    check(
        r#"
//- /src/Use.php
<?php
namespace App;

function run(Missing $gone): void {}
"#,
        expect![[r#"
            <?php              Keyword
            namespace          Keyword
            App                Namespace
            function           Keyword
            run                Function
            Missing            Class
            $gone              Parameter
            void               Type
        "#]],
    );
}
