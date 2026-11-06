//! A key an attribute writes without naming the argument.
//!
//! `#[Element('textfield')]` is the same fact as `#[Plugin(id: 'x')]` written the shorter way, and
//! until lowering was told to keep a positional literal it reached nothing at all: the catalog was
//! simply empty, with no error anywhere and a feature that read as unbuilt.
//!
//! Kept apart from the named-argument tests because the failure is different in kind. A named rule
//! that does not match answers nothing; a *positional* one that matches too eagerly answers with
//! whatever string happened to be written first, which is a wrong answer rather than a missing one.

use std::path::PathBuf;

use expect_test::expect;
use phrost_model::{FileId, Interner, SourceOrigin};

use super::{Step, check_incremental, check_index};
use crate::{KeyedAttributes, OpenFile, ProjectSkeleton};

#[test]
fn a_positional_id_claims_its_key() {
    check_index(
        r#"
//- /src/Textfield.php
<?php
namespace App\Element;

#[Element('textfield')]
class Textfield {}
"#,
        expect![[r#"
            state ready
            file src/Textfield.php [User]
              class App\Element\Textfield @ 5:7-5:16
              App\Element\Textfield is class
            catalog
              [class] app\element\textfield -> src/Textfield.php (App\Element\Textfield)
            namespaces
              app -> App x1
              app\element -> App\Element x1
            declared keys
              [element] textfield -> App\Element\Textfield
            dispatched keys
        "#]],
    );
}

#[test]
fn a_written_argument_name_is_not_the_position_the_rule_asked_for() {
    // The rule names argument zero. An attribute writing `id:` has written no argument zero, so it
    // claims nothing -- which is the safe direction, and the one a rule matching "the first literal
    // however it was written" would get wrong.
    check_index(
        r#"
//- /src/Textfield.php
<?php
namespace App\Element;

#[Element(id: 'textfield')]
class Textfield {}
"#,
        expect![[r#"
            state ready
            file src/Textfield.php [User]
              class App\Element\Textfield @ 5:7-5:16
              App\Element\Textfield is class
            catalog
              [class] app\element\textfield -> src/Textfield.php (App\Element\Textfield)
            namespaces
              app -> App x1
              app\element -> App\Element x1
            declared keys
            dispatched keys
        "#]],
    );
}

#[test]
fn an_attribute_nobody_asked_about_claims_nothing_however_it_is_written() {
    // The whole point of the gate. A project writes thousands of leading positional literals --
    // test groups, data providers, hook names -- and none of them is a key.
    check_index(
        r#"
//- /src/SomeTest.php
<?php
namespace App\Tests;

#[Group('functional')]
class SomeTest {}
"#,
        expect![[r#"
            state ready
            file src/SomeTest.php [User]
              class App\Tests\SomeTest @ 5:7-5:15
              App\Tests\SomeTest is class
            catalog
              [class] app\tests\sometest -> src/SomeTest.php (App\Tests\SomeTest)
            namespaces
              app -> App x1
              app\tests -> App\Tests x1
            declared keys
            dispatched keys
        "#]],
    );
}

#[test]
fn renaming_a_positional_id_moves_what_it_claims() {
    // The same invalidation the named spelling rests on, and it has to hold for this one too: a
    // claim derived one way on the way in and another on the way out leaves the old id resolving to
    // a class that no longer declares it.
    check_incremental(
        r#"
//- /src/Textfield.php
<?php
namespace App\Element;
#[Element('old_name')]
class Textfield {}
"#,
        &[Step::write(
            "src/Textfield.php",
            "<?php\nnamespace App\\Element;\n#[Element('new_name')]\nclass Textfield {}\n",
        )],
        expect![[r#"
            state ready
            file src/Textfield.php [User]
              class App\Element\Textfield @ 4:7-4:16
              App\Element\Textfield is class
            catalog
              [class] app\element\textfield -> src/Textfield.php (App\Element\Textfield)
            namespaces
              app -> App x1
              app\element -> App\Element x1
            declared keys
              [element] new_name -> App\Element\Textfield
            dispatched keys
        "#]],
    );
}

/// The invariant this whole mechanism turns on.
///
/// Two things lower a file -- the skeleton for a closed one and the open set for a buffer -- and the
/// list of attributes to keep a positional literal for is installed on both. If it ever reaches only
/// one of them, *opening* a file withdraws the keys it declares and closing it puts them back, which
/// reads as a catalog that empties itself while somebody works.
#[test]
fn an_open_buffer_claims_exactly_what_the_closed_file_did() {
    const SOURCE: &str = r#"<?php
namespace App\Element;

#[Element('textfield')]
class Textfield {}
"#;

    let closed = claimed(SOURCE, Ingest::Closed);
    let open = claimed(SOURCE, Ingest::Open);

    assert_eq!(
        closed, open,
        "the open and closed readings of one file claimed different keys"
    );
    assert_eq!(closed, ["element textfield -> App\\Element\\Textfield"]);
}

enum Ingest {
    /// Lowered by the skeleton, as a file nobody has open.
    Closed,
    /// Lowered by the open set and handed over, as a buffer the editor sent.
    Open,
}

/// The catalog keys one file claims, reached the way `how` says.
fn claimed(source: &str, how: Ingest) -> Vec<String> {
    let mut skeleton = ProjectSkeleton::new();
    skeleton.set_root("/fixture");
    skeleton
        .returns_mut()
        .position_keys_with("Element", 0, "element");
    skeleton.set_keyed_attributes(KeyedAttributes::of(["Element"]));

    let file = FileId::from_raw(0);
    let path = PathBuf::from("/fixture/src/Textfield.php");
    match how {
        Ingest::Closed => skeleton.ingest(file, &path, SourceOrigin::User, source),
        Ingest::Open => {
            let mut interner = Interner::new();
            // Through the same accessor the server hands the open set, which is the thing being
            // checked: a buffer lowered with a different list is a buffer that declares nothing.
            let analyzed =
                OpenFile::analyze(&mut interner, file, source, skeleton.keyed_attributes());
            skeleton.ingest_lowered(
                file,
                &path,
                SourceOrigin::User,
                source,
                phrost_vfs::FileStamp::of(source.as_bytes()),
                analyzed.hir(),
            );
        }
    }

    skeleton
        .returns()
        .declared_keys()
        .into_iter()
        .map(|(catalog, key, classes)| format!("{catalog} {key} -> {}", classes.join(", ")))
        .collect()
}
