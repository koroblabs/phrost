//! Goto on the class named in an `instanceof`.
//!
//! Not a gap: `class_target` in `phrost-lower` already runs a plain class name, `self`/`static`/
//! `parent`, and a fully-qualified one through the same `name_ref` a `new` or a `::` does (see
//! `relative.rs` for the keyword case), so the position machinery in `phrost-view` already finds
//! it. This locks that in at the feature layer, where nothing had asked the question yet.

use expect_test::expect;

use crate::tests::{Query, check};

#[test]
fn goto_on_a_plain_class_name_reaches_the_class() {
    check(
        r#"
//- /src/Foo.php
<?php
namespace App;

class Foo {}

//- /src/Use.php
<?php
namespace App;

function go($x): void
{
    if ($x instanceof F@plain@oo) {
    }
}
"#,
        &[Query::goto("instanceof Foo", "plain")],
        expect![[r#"
            instanceof Foo
            - class App\Foo @ src/Foo.php 4:7-4:10 (full 4:1-4:13)
        "#]],
    );
}

#[test]
fn goto_on_a_fully_qualified_class_name_reaches_it_across_namespaces() {
    // Multi-segment and fully qualified: the cursor sits on the last segment, and the whole path
    // is one name reference regardless of how many of them it has.
    check(
        r#"
//- /src/BulkOrderApplication.php
<?php
namespace Drupal\app_core\Application\Applications;

class BulkOrderApplication {}

//- /src/Use.php
<?php
namespace App;

function f($application): void {
    if ($application instanceof \Drupal\app_core\Application\Applications\B@fqn@ulkOrderApplication) {
    }
}
"#,
        &[Query::goto("the fully-qualified name", "fqn")],
        expect![[r#"
            the fully-qualified name
            - class Drupal\app_core\Application\Applications\BulkOrderApplication @ src/BulkOrderApplication.php 4:7-4:27 (full 4:1-4:30)
        "#]],
    );
}

#[test]
fn a_qualified_name_that_php_would_not_resolve_finds_nothing() {
    // The same name as above, but written the way PHP would read as a mistake: no leading `\` and
    // no matching `use`, so a *qualified* name gets the current namespace prepended -- here that is
    // `App\Drupal\...`, which nothing declares. `phrost-defmap`'s `resolve_qualified` documents
    // this as deliberate ("No fallback."): trying the name again as if it had been fully qualified
    // would resolve code PHP itself would fail to load. Not a gap -- the answer PHP would give.
    check(
        r#"
//- /src/BulkOrderApplication.php
<?php
namespace Drupal\app_core\Application\Applications;

class BulkOrderApplication {}

//- /src/Use.php
<?php
namespace App;

function f($application): void {
    if ($application instanceof Drupal\app_core\Application\Applications\D@unqualified@ropshipApplication) {
    }
}
"#,
        &[Query::goto("missing the leading backslash", "unqualified")],
        expect![[r#"
            missing the leading backslash
            - (nothing to go to)
        "#]],
    );
}
