//! What a `use` line offers, which is not what the same characters offer in code.
//!
//! Two things separate an import from a name, and both were wrong while the two shared a path.
//!
//! A `use` path is anchored at the root however it is written, so inside `namespace App` a written
//! `Acme\Data\` means that namespace and not `App\Acme\Data\`. Resolved relatively it named nothing
//! and the line offered an empty list — the shape a person reaches for most, since a `use` is
//! written from the root outwards.
//!
//! And the label is the whole path, because the whole path is what the line will contain. A client
//! filters the list against the text an item replaces, which here is everything typed since the
//! keyword, so an item labelled `Node` disappears the moment somebody types a separator.

use expect_test::expect;

use crate::tests::{Query, check};

#[test]
fn a_use_line_offers_the_path_itself() {
    check(
        r#"
//- /vendor/acme/src/Report.php
<?php
namespace Acme;

class Report {}

//- /src/Client.php
<?php
namespace App;

use Rep@here@
"#,
        &[Query::completion("import", "here")],
        expect![[r#"
            import
            - Acme\Report [class]
            - ... and the client must ask again
            - accepting the first leaves `use Acme\Report`
        "#]],
    );
}

/// The reported case, and the one relative resolution got wrong: a namespace typed in full, in a
/// file whose own namespace shares none of it. Read relatively this went looking for
/// `App\Acme\Data\` and answered with nothing.
///
/// A letter of the class stands where a person would leave the cursor on the separator, because the
/// fixture format reads a backslash before a marker as an escape. The corpus fixture this came from
/// asks the bare-separator question, and the anchoring is the same either way.
#[test]
fn a_namespace_in_a_use_line_is_read_from_the_root() {
    check(
        r#"
//- /src/Data/Node.php
<?php
namespace Acme\Data;

class Node {}

//- /src/Data/Nested/Deep.php
<?php
namespace Acme\Data\Nested;

class Deep {}

//- /src/Other/Leaf.php
<?php
namespace Acme\Other;

class Leaf {}

//- /src/Client.php
<?php
namespace App;

use Acme\Data\N@here@
"#,
        &[Query::completion("a namespace typed in full", "here")],
        expect![[r#"
            a namespace typed in full
            - Acme\Data\Nested [namespace]  insert `Acme\Data\Nested\`
            - Acme\Data\Node [class]
            - ... and the client must ask again
            - accepting the first leaves `use Acme\Data\Nested\`
        "#]],
    );
}

/// Written with the separator PHP treats as redundant. It means the same thing, and the answer has
/// to be the same too.
#[test]
fn a_leading_separator_changes_nothing_in_a_use_line() {
    check(
        r#"
//- /src/Data/Node.php
<?php
namespace Acme\Data;

class Node {}

//- /src/Client.php
<?php
namespace App;

use \Acme\Data\N@here@
"#,
        &[Query::completion("written from the root", "here")],
        expect![[r#"
            written from the root
            - Acme\Data\Node [class]
            - ... and the client must ask again
            - accepting the first leaves `use Acme\Data\Node`
        "#]],
    );
}
