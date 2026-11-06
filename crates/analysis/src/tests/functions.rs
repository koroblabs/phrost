//! Free functions, which are described from the skeleton rather than from a class.
//!
//! A function has no class to nest it in, so it takes its own branch through hover — and that
//! branch used to have nothing to say beyond the name. Its signature comes from the same stored
//! parameter and type rows a method's does, which is the point: one representation, one rendering.

use expect_test::expect;

use super::{Query, check};

#[test]
fn hover_on_a_function_declaration_shows_its_signature() {
    check(
        r#"
//- /src/helpers.php
<?php
namespace App;

/** Builds a thing. */
function ma@decl@ke(int $count, ?string $label = null): ?Thing {}
"#,
        &[Query::hover("hover", "decl")],
        expect![[r#"
            hover
            - code: <?php
            namespace App;

            function make(int $count, ?string $label = null): ?Thing
            - ---
            - doc: Builds a thing.
        "#]],
    );
}

#[test]
fn a_function_returning_a_reference_says_so() {
    check(
        r#"
//- /src/helpers.php
<?php
function &lat@decl@est(array &$items) {}
"#,
        &[Query::hover("hover", "decl")],
        expect![[r#"
            hover
            - code: <?php
            function &latest(array &$items)
        "#]],
    );
}

#[test]
fn an_imported_function_is_described_from_where_it_was_declared() {
    // Reached through `use function`, so the signature has to come from the other file's skeleton
    // — and its type hint has to have been resolved against *that* file's imports. Both files here
    // have a `Thing` in scope, and they are different classes.
    check(
        r#"
//- /src/One/Thing.php
<?php
namespace One;
class Thing {}

//- /src/Two/Thing.php
<?php
namespace Two;
class Thing {}

//- /src/helpers.php
<?php
namespace App;

use One\Thing;

function build(): Thing {}

//- /src/call.php
<?php
namespace Site;

use Two\Thing;
use function App\bu@import@ild;
"#,
        &[
            Query::goto("goto", "import"),
            Query::hover("hover", "import"),
        ],
        expect![[r#"
            goto
            - function App\build @ src/helpers.php 6:10-6:15 (full 6:1-6:27)
            hover
            - code: <?php
            namespace App;

            function build(): Thing
        "#]],
    );
}

#[test]
fn a_call_navigates_to_the_function_it_names() {
    check(
        r#"
//- /src/helpers.php
<?php
namespace App;

/** Makes one. */
function make(): void {}

//- /src/call.php
<?php
namespace App;

ma@call@ke();
"#,
        &[Query::goto("goto", "call"), Query::hover("hover", "call")],
        expect![[r#"
            goto
            - function App\make @ src/helpers.php 5:10-5:14 (full 5:1-5:25)
            hover
            - code: <?php
            namespace App;

            function make(): void
            - ---
            - doc: Makes one.
        "#]],
    );
}

#[test]
fn a_call_and_a_class_of_the_same_name_are_different_symbols() {
    // PHP keeps classes and functions in separate tables, so both can exist. Resolving a call
    // through the class table finds the class -- and class names are matched case-insensitively, so
    // it finds one even when the spelling differs. Goto would land somewhere unrelated and look
    // deliberate.
    check(
        r#"
//- /src/both.php
<?php
namespace App;

class Render {}

function render(): void {}

//- /src/call.php
<?php
namespace App;

ren@call@der();
new Ren@ctor@der();
"#,
        &[
            Query::goto("the call", "call"),
            Query::goto("the constructor", "ctor"),
        ],
        expect![[r#"
            the call
            - function App\render @ src/both.php 6:10-6:16 (full 6:1-6:27)
            the constructor
            - class App\Render @ src/both.php 4:7-4:13 (full 4:1-4:16)
        "#]],
    );
}

#[test]
fn a_call_at_file_scope_is_reachable_too() {
    // A Drupal `.module` file is almost entirely code at file scope. Without a body for it, nothing
    // written there has a node to click.
    check(
        r#"
//- /src/helpers.php
<?php
function make(): void {}

//- /src/script.php
<?php
if (true) {
    ma@call@ke();
}
"#,
        &[Query::goto("goto", "call")],
        expect![[r#"
            goto
            - function make @ src/helpers.php 2:10-2:14 (full 2:1-2:25)
        "#]],
    );
}

#[test]
fn a_free_functions_docblock_return_is_read() {
    // The skeleton lowered a function's signature and dropped its docblock, so `@return` on a free
    // function said nothing — and a `.module` file is written almost entirely in those.
    //
    // And a template the argument binds: `list<V>` beside `array<K, V> $array` is a list of what
    // the array holds, read the way the stubs' conditional spelling of the same claim is. A
    // rearrangement is not an identity -- the keys are the list's own, see
    // `a_return_spelled_like_a_parameter_is_the_argument`.
    check(
        r#"
//- /app.php
<?php
class Row
{
    public function id(): int {}
}

/** @return Row */
function load() {}

/** @return list<Row> */
function loadAll(): array {}

/**
 * @template K as array-key
 * @template V
 *
 * @param array<K, V> $array
 *
 * @return list<V>
 */
function reindex(array $array): array {}

function run(): void
{
    $row = load();
    $@one@row;
    $rows = loadAll();
    $@many@rows;
    $reindexed = reindex(loadAll());
    $@templated@reindexed;
}
"#,
        &[
            Query::hover("a docblock-only return", "one"),
            Query::hover("a docblock refining an array", "many"),
            Query::hover(
                "a docblock naming a template the argument binds",
                "templated",
            ),
        ],
        expect![[r#"
            a docblock-only return
            - code: Row $row
            - note: assigned
            a docblock refining an array
            - code: array<int, Row> $rows
            - note: assigned
            a docblock naming a template the argument binds
            - code: Row[] $reindexed
            - note: assigned
        "#]],
    );
}
