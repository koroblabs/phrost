//! `actions::annotation`: a docblock annotation written as a PHP attribute instead.
//!
//! The *file the edits produce* is snapshotted, not the text of any one of them. Three separate
//! things have to land: the attribute above the declaration, the docblock left holding only its
//! prose, and the `use` lines for classes that were never imported because an annotation does not
//! need one. An assertion on the attribute alone would pass with the annotation still in the
//! comment beneath it, which is a file that declares the plugin twice.
//!
//! Neutral names throughout, as everywhere below the framework seam: what the test states is a
//! namespace segment and a name, and nothing here knows whose convention that is.

use expect_test::{Expect, expect};
use phrost_harness::{World, with_open_scopes};
use phrost_testkit::FixtureSpec;
use phrost_view::IndexView;

use crate::code_actions;

/// The whole of the fixture's last file after every offered conversion is applied.
fn check(fixture: &str, expect: Expect) {
    check_with(
        fixture,
        |world| {
            world.contracts_mut().rewrite_annotations_in("Attribute");
            world
                .contracts_mut()
                .construct_for_annotation("Translation", "App\\Text\\Markup");
        },
        expect,
    );
}

/// The same, for a project where the seam said nothing.
fn check_with(fixture: &str, state: impl FnOnce(&mut World), expect: Expect) {
    let spec = FixtureSpec::parse(fixture);
    let world = World::build_with(&spec, state);
    let position = spec.markers().position("at");
    let file = world.file_id(&position.path);

    let rendered = with_open_scopes(&world, Some(file), |open| {
        let view = IndexView::in_file(world.skeleton(), open, file);
        let request = crate::ActionRequest {
            view: &view,
            at: phrost_model::TextRange::empty(position.offset),
            only: &[],
            asked: Default::default(),
        };
        let offered: Vec<_> = code_actions(&request)
            .into_iter()
            .filter(|action| action.title.starts_with("Convert @"))
            .collect();
        let Some(action) = offered.first() else {
            return "(nothing offered)\n".to_owned();
        };
        format!(
            "{}\n----\n{}",
            action.title,
            applied(world.text_of(file), &action.edits)
        )
    });

    spec.markers().assert_all_used();
    expect.assert_eq(&rendered);
}

/// The file with every edit applied, back to front so an earlier offset stays where it was measured.
fn applied(text: &str, edits: &[crate::TextEdit]) -> String {
    let mut text = text.to_owned();
    let mut edits = edits.to_vec();
    edits.sort_by_key(|edit| std::cmp::Reverse(edit.range.start()));
    for edit in edits {
        let range = usize::from(edit.range.start())..usize::from(edit.range.end());
        text.replace_range(range, &edit.text);
    }
    text
}

/// The attribute classes a project has beside its annotations, and the object one annotation stands
/// for. Written once because every fixture below needs all three.
const CLASSES: &str = r#"
//- /src/Plugin/Attribute/Resource.php
<?php
namespace App\Plugin\Attribute;

class Resource {}

//- /src/Text/Markup.php
<?php
namespace App\Text;

class Markup {}

//- /src/Base.php
<?php
namespace App;

class Base {}
"#;

#[test]
fn every_part_of_an_annotation_is_repunctuated_and_its_classes_imported() {
    // The shape a real plugin is written in: a named argument, a nested annotation standing for an
    // object, and a map of paths. All three change punctuation, and two of them need an import the
    // file never had, because a docblock resolves nothing.
    check(
        &[
            CLASSES,
            r#"
//- /src/Plugin/resource/RegionNotice.php
<?php

namespace App\Plugin\resource;

use App\Base;

/**
 * Provides a resource for the region notice.
 *
 * @Resource(
 *   id = "region_notice",
 *   label = @Translation("Region Notice."),
 *   uri_paths = {
 *     "canonical" = "/api/v1/region-notice",
 *     "create" = "/api/v1/region-notice"
 *   }
 * )
 */
class Region@at@Notice extends Base {}
"#,
        ]
        .concat(),
        expect![[r#"
            Convert @Resource to an attribute
            ----
            <?php

            namespace App\Plugin\resource;

            use App\Base;
            use App\Plugin\Attribute\Resource;
            use App\Text\Markup;

            /**
             * Provides a resource for the region notice.
             */
            #[Resource(
              id: "region_notice",
              label: new Markup("Region Notice."),
              uri_paths: [
                "canonical" => "/api/v1/region-notice",
                "create" => "/api/v1/region-notice"
              ]
            )]
            class RegionNotice extends Base {}
        "#]],
    );
}

#[test]
fn a_docblock_that_said_nothing_else_goes_with_the_annotation() {
    // An empty comment above a declaration is worse than none, so the whole of it goes. Written
    // inside a braced namespace, which is the one shape that indents a class -- and the attribute
    // has to arrive at the declaration's own indentation rather than at column zero.
    //
    // The `use` line does not, and that is `import_for`'s own answer rather than this offer's: it
    // anchors after the namespace declaration and writes at column zero, which is right for the
    // semicolon form every project file uses and wrong inside braces. Snapshotted as it is rather
    // than worked around here, since completion writes the same line in the same place.
    check(
        &[
            CLASSES,
            r#"
//- /src/Plugin/resource/Bare.php
<?php

namespace App\Plugin\resource {
    /**
     * @Resource(id = "bare")
     */
    class Ba@at@re {}
}
"#,
        ]
        .concat(),
        expect![[r#"
            Convert @Resource to an attribute
            ----
            <?php

            namespace App\Plugin\resource {
            use App\Plugin\Attribute\Resource;
                #[Resource(id: "bare")]
                class Bare {}
            }
        "#]],
    );
}

#[test]
fn a_project_whose_framework_named_no_segment_is_offered_nothing() {
    // The gate. Without a segment there is no way to know what `@Resource` would become, and the
    // class beside it in the index is an ordinary class that happens to share the name.
    check_with(
        &[
            CLASSES,
            r#"
//- /src/Plugin/resource/Plain.php
<?php

namespace App\Plugin\resource;

/**
 * Provides a resource.
 *
 * @Resource(id = "plain")
 */
class Pl@at@ain {}
"#,
        ]
        .concat(),
        |_| {},
        expect!["(nothing offered)\n"],
    );
}

#[test]
fn an_annotation_carrying_one_nothing_can_resolve_is_refused_whole() {
    // Half a conversion is a file naming a class that is not there. Refusing costs a menu entry.
    check(
        &[
            CLASSES,
            r#"
//- /src/Plugin/resource/Nested.php
<?php

namespace App\Plugin\resource;

/**
 * Provides a resource.
 *
 * @Resource(
 *   id = "nested",
 *   label = @Unknowable("What")
 * )
 */
class Nes@at@ted {}
"#,
        ]
        .concat(),
        expect!["(nothing offered)\n"],
    );
}

#[test]
fn a_docblock_of_ordinary_tags_is_not_an_annotation() {
    // Every tag here is written with a space and a value, which is what tells a PHPDoc tag from an
    // annotation -- and `@see App\Base` names a class the search would happily have found.
    check(
        &[
            CLASSES,
            r#"
//- /src/Plugin/resource/Tags.php
<?php

namespace App\Plugin\resource;

/**
 * Provides a resource.
 *
 * @see App\Base
 * @internal
 */
class Ta@at@gs {}
"#,
        ]
        .concat(),
        expect!["(nothing offered)\n"],
    );
}
