//! A key declared the older way, in a docblock annotation rather than an attribute.
//!
//! `@ContentEntityType(id = "node")` is the same declaration as `#[ContentEntityType(id: 'node')]`,
//! and a framework mid-migration carries both for years: one real Drupal 10 checkout writes 107
//! entity type ids as attributes and 11 as annotations, overlapping in 7. Reading only the attribute
//! left four ids in code that runs and in no catalog, which is the difference between closing that
//! catalog and calling four correct declarations undeclared.
//!
//! Kept apart from the attribute tests because what can go wrong is different. An attribute is a
//! node in the tree; an annotation is prose that happens to look like one, so the two failures worth
//! catching are reading a *documented example* as a declaration, and letting one class's two
//! spellings look like two classes claiming one key.

use std::path::PathBuf;

use expect_test::expect;
use phrost_model::{FileId, Interner, SourceOrigin};

use super::{Step, check_incremental, check_index};
use crate::{KeyedAttributes, OpenFile, ProjectSkeleton};

#[test]
fn an_annotated_id_claims_its_key() {
    check_index(
        r#"
//- /src/BehaviorSettings.php
<?php
namespace App\Entity;

/**
 * Defines the Behavior settings entity.
 *
 * @Plugin(
 *   id = "behavior_settings",
 *   label = @Translation("Rabbit hole settings"),
 *   entity_keys = {
 *     "id" = "id"
 *   }
 * )
 */
class BehaviorSettings {}
"#,
        // One key and not two: the `entity_keys` map is values rather than arguments, and reading
        // it would have every entity type in a project claim the key `id`.
        expect![[r#"
            state ready
            file src/BehaviorSettings.php [User]
              class App\Entity\BehaviorSettings @ 15:7-15:23
              App\Entity\BehaviorSettings is class
            catalog
              [class] app\entity\behaviorsettings -> src/BehaviorSettings.php (App\Entity\BehaviorSettings)
            namespaces
              app -> App x1
              app\entity -> App\Entity x1
            declared keys
              [plugin] behavior_settings -> App\Entity\BehaviorSettings
            dispatched keys
        "#]],
    );
}

#[test]
fn the_shorthand_annotation_matches_a_rule_written_for_the_position() {
    // `@Element("textfield")` is how a single-key annotation is written, and it has to reach the
    // same rule `#[Element('textfield')]` does -- one rule for a declaration, not one per spelling.
    check_index(
        r#"
//- /src/Textfield.php
<?php
namespace App\Element;

/**
 * Provides a textfield.
 *
 * @Element("textfield")
 */
class Textfield {}
"#,
        expect![[r#"
            state ready
            file src/Textfield.php [User]
              class App\Element\Textfield @ 9:7-9:16
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
fn one_class_written_both_ways_claims_the_key_once() {
    // What a module supporting two Drupal versions writes. Two claims on one key from two different
    // classes answer nothing at all -- `catalog_class` refuses to rank them -- so a declaration
    // read twice must not look like two declarations.
    check_index(
        r#"
//- /src/Paragraph.php
<?php
namespace App\Entity;

/**
 * @Plugin(
 *   id = "paragraph"
 * )
 */
#[Plugin(id: 'paragraph')]
class Paragraph {}
"#,
        expect![[r#"
            state ready
            file src/Paragraph.php [User]
              class App\Entity\Paragraph @ 10:7-10:16
              App\Entity\Paragraph is class
            catalog
              [class] app\entity\paragraph -> src/Paragraph.php (App\Entity\Paragraph)
            namespaces
              app -> App x1
              app\entity -> App\Entity x1
            declared keys
              [plugin] paragraph -> App\Entity\Paragraph
            dispatched keys
        "#]],
    );
}

#[test]
fn a_documented_example_declares_nothing() {
    // Drupal's own `core.api.php` teaches the annotation syntax with a real entity type id in it,
    // inside an `@code` block. Read as a declaration it would put a second claim on that id from a
    // file declaring no such class, and the id would stop resolving rather than start.
    check_index(
        r#"
//- /src/Documentation.php
<?php
namespace App\Docs;

/**
 * How to annotate a plugin.
 *
 * @code
 * * @Plugin(
 * *   id = "textfield"
 * * )
 * @endcode
 */
class Documentation {}
"#,
        expect![[r#"
            state ready
            file src/Documentation.php [User]
              class App\Docs\Documentation @ 13:7-13:20
              App\Docs\Documentation is class
            catalog
              [class] app\docs\documentation -> src/Documentation.php (App\Docs\Documentation)
            namespaces
              app -> App x1
              app\docs -> App\Docs x1
            declared keys
            dispatched keys
        "#]],
    );
}

#[test]
fn editing_an_annotated_id_moves_what_it_claims() {
    // The same invalidation the attribute spelling rests on. A claim derived one way going in and
    // another coming out leaves the old id resolving to a class that no longer declares it.
    check_incremental(
        r#"
//- /src/Textfield.php
<?php
namespace App\Element;
/**
 * @Plugin(id = "old_name")
 */
class Textfield {}
"#,
        &[Step::write(
            "src/Textfield.php",
            "<?php\nnamespace App\\Element;\n/**\n * @Plugin(id = \"new_name\")\n */\nclass Textfield {}\n",
        )],
        expect![[r#"
            state ready
            file src/Textfield.php [User]
              class App\Element\Textfield @ 6:7-6:16
              App\Element\Textfield is class
            catalog
              [class] app\element\textfield -> src/Textfield.php (App\Element\Textfield)
            namespaces
              app -> App x1
              app\element -> App\Element x1
            declared keys
              [plugin] new_name -> App\Element\Textfield
            dispatched keys
        "#]],
    );
}

/// The invariant this whole mechanism turns on, and the one the positional gate already pays for.
///
/// Two things lower a file -- the skeleton for a closed one and the open set for a buffer -- and the
/// list of attribute names to read an annotation for is installed on both. If it ever reaches only
/// one of them, *opening* a file withdraws the keys it declares and closing it puts them back, which
/// reads as a catalog that empties itself while somebody works.
#[test]
fn an_open_buffer_claims_exactly_what_the_closed_file_did() {
    const SOURCE: &str = r#"<?php
namespace App\Element;

/**
 * @Element(id = "textfield")
 */
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
        .name_keys_with("Element", "id", "element");
    skeleton.set_keyed_attributes(KeyedAttributes::of(["Element"]));

    let file = FileId::from_raw(0);
    let path = PathBuf::from("/fixture/src/Textfield.php");
    match how {
        Ingest::Closed => skeleton.ingest(file, &path, SourceOrigin::User, source),
        Ingest::Open => {
            let mut interner = Interner::new();
            // Through the same accessor the server hands the open set, which is the thing being
            // checked: a buffer lowered with a different list declares nothing.
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
