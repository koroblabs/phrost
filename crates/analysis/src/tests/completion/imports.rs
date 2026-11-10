//! The `use` line an accepted name needs.
//!
//! Completion used to insert `\App\Nested\Repo` in the middle of a statement: always correct, never
//! what anybody types. Offering the short name instead means writing an import, and an import
//! changes what that name means for the whole file — so the interesting cases here are the ones
//! where it is refused.

use expect_test::expect;

use crate::tests::{Query, check};

#[test]
fn a_class_from_another_namespace_brings_its_use_line_with_it() {
    check(
        r#"
//- /src/Data/Node.php
<?php
namespace App\Data;

class Node {}

//- /src/Uses.php
<?php
namespace App;

class Uses
{
    public function run(): void
    {
        $x = new No@here@
    }
}
"#,
        &[Query::completion("needs an import", "here")],
        expect![[r#"
            needs an import
            - Node [class]  App\Data\Node  + `use App\Data\Node;` at 2:15-2:15
            - ... and the client must ask again
            - accepting the first leaves `$x = new Node`
        "#]],
    );
}

#[test]
fn the_use_line_lands_after_the_imports_already_there() {
    check(
        r#"
//- /src/Data/Node.php
<?php
namespace App\Data;

class Node {}

//- /src/Data/Leaf.php
<?php
namespace App\Data;

class Leaf {}

//- /src/Uses.php
<?php
namespace App;

use App\Data\Leaf;

class Uses
{
    public function run(Leaf $leaf): void
    {
        $x = new No@here@
    }
}
"#,
        &[Query::completion("after the last import", "here")],
        expect![[r#"
            after the last import
            - Node [class]  App\Data\Node  + `use App\Data\Node;` at 4:19-4:19
            - ... and the client must ask again
            - accepting the first leaves `$x = new Node`
        "#]],
    );
}

#[test]
fn a_name_already_imported_is_offered_short_with_no_second_import() {
    check(
        r#"
//- /src/Data/Node.php
<?php
namespace App\Data;

class Node {}

//- /src/Uses.php
<?php
namespace App;

use App\Data\Node;

class Uses
{
    public function run(): void
    {
        $x = new No@here@
    }
}
"#,
        &[Query::completion("already imported", "here")],
        expect![[r#"
            already imported
            - Node [class]  App\Data\Node
            - ... and the client must ask again
            - accepting the first leaves `$x = new Node`
        "#]],
    );
}

#[test]
fn a_short_name_that_already_means_something_else_is_written_out_in_full() {
    // The case an import must not be offered for. `use App\Data\Node` here would silently change
    // what every existing `Node` in the file refers to, so the qualified spelling is the answer --
    // which is what it is still there for.
    check(
        r#"
//- /src/Data/Node.php
<?php
namespace App\Data;

class Node {}

//- /src/Node.php
<?php
namespace App;

class Node {}

//- /src/Uses.php
<?php
namespace App;

class Uses
{
    public function run(): void
    {
        $x = new \App\Data\No@here@
    }
}
"#,
        &[Query::completion("the name is taken", "here")],
        expect![[r#"
            the name is taken
            - Node [class]  App\Data\Node  insert `\App\Data\Node`
            - ... and the client must ask again
            - accepting the first leaves `$x = new \App\Data\Node`
        "#]],
    );
}

#[test]
fn a_class_in_the_same_namespace_needs_nothing() {
    check(
        r#"
//- /src/Node.php
<?php
namespace App;

class Node {}

//- /src/Uses.php
<?php
namespace App;

class Uses
{
    public function run(): void
    {
        $x = new No@here@
    }
}
"#,
        &[Query::completion("same namespace", "here")],
        expect![[r#"
            same namespace
            - Node [class]  App\Node
            - ... and the client must ask again
            - accepting the first leaves `$x = new Node`
        "#]],
    );
}

#[test]
fn a_function_is_imported_into_the_function_table() {
    // `use App\Helpers\slugify;` would import a class nobody declared: PHP keeps three separate
    // tables and the `use` line has to say which one.
    check(
        r#"
//- /src/Helpers/text.php
<?php
namespace App\Helpers;

function slugify(string $value): string {}

//- /src/Uses.php
<?php
namespace App;

class Uses
{
    public function run(): void
    {
        $x = slug@here@
    }
}
"#,
        &[Query::completion("a function", "here")],
        expect![[r#"
            a function
            - slugify [function]  App\Helpers\slugify  + `use function App\Helpers\slugify;` at 2:15-2:15
            - ... and the client must ask again
            - accepting the first leaves `$x = slugify`
        "#]],
    );
}

#[test]
fn a_use_statement_still_takes_the_path_outright() {
    check(
        r#"
//- /src/Data/Node.php
<?php
namespace App\Data;

class Node {}

//- /src/Uses.php
<?php
namespace App;

use No@here@
"#,
        &[Query::completion("inside a use line", "here")],
        expect![[r#"
            inside a use line
            - App\Data\Node [class]
            - ... and the client must ask again
            - accepting the first leaves `use App\Data\Node`
        "#]],
    );
}

#[test]
fn a_comment_offers_nothing_until_a_tag_asks_for_a_type() {
    check(
        r#"
//- /src/app.php
<?php
namespace App;

class Repo {}

class Service
{
    // Re@line@
    # Re@hash@
    /* Re@block@ */
    /**
     * Re@prose@
     *
     * @var Re@var_tag@
     * @param Re@param_tag@ $x
     * @return Re@return_tag@
     */
    public function run(): void
    {
        Re@code@;
    }
}
"#,
        &[
            Query::completion("a line comment", "line"),
            Query::completion("a hash comment", "hash"),
            Query::completion("a block comment", "block"),
            Query::completion("prose in a docblock", "prose"),
            Query::completion("after @var", "var_tag"),
            Query::completion("after @param", "param_tag"),
            Query::completion("after @return", "return_tag"),
            Query::completion("in code, for comparison", "code"),
        ],
        expect![[r#"
            a line comment
            - (nothing offered)
            a hash comment
            - (nothing offered)
            a block comment
            - (nothing offered)
            prose in a docblock
            - (nothing offered)
            after @var
            - Repo [class]  App\Repo  insert `\App\Repo`
            - ... and the client must ask again
            - accepting the first leaves `* @var \App\Repo`
            after @param
            - Repo [class]  App\Repo  insert `\App\Repo`
            - ... and the client must ask again
            - accepting the first leaves `* @param \App\Repo $x`
            after @return
            - Repo [class]  App\Repo  insert `\App\Repo`
            - ... and the client must ask again
            - accepting the first leaves `* @return \App\Repo`
            in code, for comparison
            - readonly [keyword]
            - require [keyword]
            - return [keyword]
            - Repo [class]  App\Repo
            - ... and the client must ask again
            - accepting the first leaves `readonly;`
        "#]],
    );
}

#[test]
fn a_comment_marker_inside_a_string_is_not_a_comment() {
    // Suppressing completion after every URL in the file would be worse than the bug that made
    // this necessary.
    check(
        r#"
//- /src/app.php
<?php
namespace App;

class Repo {}

class Service
{
    #[Attr]
    public function run(): void
    {
        $url = 'http://example.test'; Re@after_string@;
        Re@after_attribute@;
        /* aside */ Re@after_closed_block@;
    }
}
"#,
        &[
            Query::completion("after a string holding a slash pair", "after_string"),
            Query::completion("on a line with an attribute", "after_attribute"),
            Query::completion("after a block comment that closed", "after_closed_block"),
        ],
        expect![[r#"
            after a string holding a slash pair
            - readonly [keyword]
            - require [keyword]
            - return [keyword]
            - Repo [class]  App\Repo
            - ... and the client must ask again
            - accepting the first leaves `$url = 'http://example.test'; readonly;`
            on a line with an attribute
            - readonly [keyword]
            - require [keyword]
            - return [keyword]
            - Repo [class]  App\Repo
            - ... and the client must ask again
            - accepting the first leaves `readonly;`
            after a block comment that closed
            - readonly [keyword]
            - require [keyword]
            - return [keyword]
            - Repo [class]  App\Repo
            - ... and the client must ask again
            - accepting the first leaves `/* aside */ readonly;`
        "#]],
    );
}

#[test]
fn a_tag_stops_asking_for_a_type_once_one_has_been_written() {
    // `@param Repo $x` puts the type first. Offering classes for the parameter name as well would
    // be the same interruption one word later.
    check(
        r#"
//- /src/app.php
<?php
namespace App;

class Repo {}

class Service
{
    /**
     * @param Repo $re@name_after_type@
     * @return Re@in_the_type@
     * @deprecated Re@prose_tag@
     */
    public function run(): void
    {
    }
}
"#,
        &[
            Query::completion("the parameter name after the type", "name_after_type"),
            Query::completion("the type itself", "in_the_type"),
            Query::completion("a tag that takes no type", "prose_tag"),
        ],
        expect![[r#"
            the parameter name after the type
            - (nothing offered)
            the type itself
            - Repo [class]  App\Repo  insert `\App\Repo`
            - ... and the client must ask again
            - accepting the first leaves `* @return \App\Repo`
            a tag that takes no type
            - (nothing offered)
        "#]],
    );
}

#[test]
fn an_attribute_position_offers_class_names() {
    // `#[` is not a comment, and an attribute name is a class name. The one place the two collide:
    // `#` alone starts a line comment, so reading the marker without its bracket would suppress
    // completion on every attribute in a project.
    check(
        r#"
//- /src/app.php
<?php
namespace App;

#[\Attribute]
class Marker {}

class Service
{
    #[Mar@here@]
    public function run(): void {}
}
"#,
        &[Query::completion("on an attribute name", "here")],
        expect![[r#"
            on an attribute name
            - Marker [class]  App\Marker
            - ... and the client must ask again
            - accepting the first leaves `#[Marker]`
        "#]],
    );
}

/// Two classes of one short name, and the file has said which it means. The row for the one it
/// imports leads and writes the short name; the other is still there, by its full path.
///
/// They used to stand in the order of the files that declare them -- PHP's `RuntimeException` is
/// in `spl.php` and the MongoDB extension's in `mongodb.php` -- so `new RuntimeE` in a file with
/// `use RuntimeException;` put the driver's class first and accepting it wrote
/// `\MongoDB\Driver\Exception\RuntimeException`.
#[test]
fn of_two_classes_with_one_name_the_imported_one_leads() {
    check(
        r#"
//- /src/Alpha/Failure.php
<?php
namespace App\Alpha;

class Failure {}

//- /src/Omega/Failure.php
<?php
namespace App\Omega;

class Failure {}

//- /src/Job.php
<?php
namespace App;

use App\Omega\Failure;

function run(): void
{
    throw new Fail@here@;
}
"#,
        &[Query::completion("a name two classes share", "here")],
        expect![[r#"
            a name two classes share
            - Failure [class]  App\Omega\Failure
            - Failure [class]  App\Alpha\Failure  insert `\App\Alpha\Failure`
            - ... and the client must ask again
            - accepting the first leaves `throw new Failure;`
        "#]],
    );
}

/// The same rule without an import: a file in no namespace means the global class by the bare
/// name, and a project's own class of that name in a namespace is the one that has to be written
/// out. The project's row used to come first for being the project's.
#[test]
fn of_two_classes_with_one_name_the_one_the_namespace_means_leads() {
    check(
        r#"
//- /src/Alpha/Failure.php
<?php
namespace App\Alpha;

class Failure {}

//- /src/Zeta/Failure.php
<?php

class Failure {}

//- /script.php
<?php

throw new Fail@here@;
"#,
        &[Query::completion(
            "a name the file's namespace settles",
            "here",
        )],
        expect![[r#"
            a name the file's namespace settles
            - Failure [class]  Failure
            - Failure [class]  App\Alpha\Failure  insert `\App\Alpha\Failure`
            - ... and the client must ask again
            - accepting the first leaves `throw new Failure;`
        "#]],
    );
}
