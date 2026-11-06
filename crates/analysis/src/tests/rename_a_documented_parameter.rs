//! A parameter's name is written in its docblock as well as in its signature and body.
//!
//! `@param string $text` is a claim about the parameter beside it, and goto and hover already read
//! it as that parameter. A rename that edits the signature and the body and leaves the tag spelling
//! the old name hands the project a `doc-param-name` finding it did not have before -- and from the
//! tag itself there was nothing to rename at all.

use std::fmt::Write as _;

use expect_test::{Expect, expect};
use phrost_harness::{OpenScope, World};
use phrost_testkit::FixtureSpec;
use phrost_view::IndexView;

use crate::{references_in, rename_target};

/// Every range a rename from each marked cursor edits, in the cursor's file.
fn check(fixture: &str, markers: &[&str], expect: Expect) {
    let spec = FixtureSpec::parse(fixture);
    let world = World::build(&spec);
    let mut out = String::new();

    for marker in markers {
        let position = spec.markers().position(marker);
        let cursor = world.file_id(&position.path);
        let open = world.open(OpenScope::EveryFile, Some(cursor));
        let view = IndexView::in_file(world.skeleton(), &open, cursor);
        let Some(target) = rename_target(&view, position.offset) else {
            let _ = writeln!(out, "@{marker} -> nothing to rename");
            continue;
        };
        let _ = writeln!(out, "@{marker}");
        for range in references_in(&view, &target) {
            let _ = writeln!(out, "  {}", world.render_range(cursor, range));
        }
    }

    spec.markers().assert_all_used();
    expect.assert_eq(&out);
}

#[test]
fn the_docblock_moves_with_the_parameter() {
    check(
        r#"
//- /src/Translator.php
<?php
namespace App;

class Translator
{
    /**
     * Translates $te@prose@xt.
     *
     * @param string $te@tag@xt
     *   What to translate.
     * @param string $target
     */
    public function translate(string $te@signature@xt, string $target): string
    {
        return $te@body@xt . $target;
    }

    /**
     * @param string $text
     */
    public function other(string $text): void {}
}
"#,
        &["signature", "body", "tag", "prose"],
        expect![[r#"
            @signature
              7:19-7:24
              9:22-9:27
              13:38-13:43
              15:16-15:21
            @body
              7:19-7:24
              9:22-9:27
              13:38-13:43
              15:16-15:21
            @tag
              7:19-7:24
              9:22-9:27
              13:38-13:43
              15:16-15:21
            @prose
              7:19-7:24
              9:22-9:27
              13:38-13:43
              15:16-15:21
        "#]],
    );
}

/// A promoted parameter is a property too, and its rename goes everywhere the property is read --
/// the constructor's `@param` with it, from the tag as from the access.
#[test]
fn a_promoted_parameters_docblock_moves_with_the_property() {
    check(
        r#"
//- /src/Request.php
<?php
namespace App;

class Request
{
    /**
     * @param string $te@tag@xt
     */
    public function __construct(public readonly string $text) {}

    public function length(): int
    {
        return strlen($this->te@access@xt);
    }
}
"#,
        &["tag", "access"],
        expect![[r#"
            @tag
              7:22-7:27
              9:56-9:61
              13:30-13:34
            @access
              7:22-7:27
              9:56-9:61
              13:30-13:34
        "#]],
    );
}
