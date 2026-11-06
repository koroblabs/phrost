//! The namespaces a path being written can go on into.
//!
//! A namespace is declared by nothing of its own, and nothing offered one: `use GuzzleHttp` offered
//! an empty list, `use Psr` two hundred classes whose names merely hold a `p`, an `s` and an `r`,
//! and `use Drupal\Co` nothing at all -- so a path could be completed only by knowing the class at
//! the end of it. What says a namespace exists is a declaration under it.
//!
//! Where it goes in the list is what each place is written with: a `use` line and a docblock write
//! the whole path, so there the namespace leads; code imports, so there the class does.

use expect_test::expect;

const LIBRARY: &str = r#"
//- /vendor/guzzle/src/Client.php
<?php
namespace GuzzleHttp;

class Client {}

//- /vendor/guzzle/src/Events.php
<?php
namespace GuzzleHttp;

class Events {}

//- /vendor/guzzle/src/Exception/GuzzleException.php
<?php
namespace GuzzleHttp\Exception;

class GuzzleException {}

//- /vendor/guzzle/src/Handler/Proxy.php
<?php
namespace GuzzleHttp\Handler;

class Proxy {}
"#;

fn check(client: &str, queries: &[crate::tests::Query], expect: expect_test::Expect) {
    crate::tests::check(&format!("{LIBRARY}\n{client}"), queries, expect);
}

use crate::tests::Query;

#[test]
fn a_top_level_namespace_typed_in_part_is_offered_in_a_use_line() {
    check(
        r#"
//- /src/Client.php
<?php
namespace App;

use Guz@here@
"#,
        &[Query::completion("first segment", "here")],
        expect![[r#"
            first segment
            - GuzzleHttp [namespace]  insert `GuzzleHttp\`
            - GuzzleHttp\Exception\GuzzleException [class]
            - ... and the client must ask again
            - accepting the first leaves `use GuzzleHttp\`
        "#]],
    );
}

#[test]
fn a_namespace_typed_in_full_is_offered_with_its_separator() {
    check(
        r#"
//- /src/Client.php
<?php
namespace App;

use GuzzleHttp@here@
"#,
        &[Query::completion("whole segment", "here")],
        expect![[r#"
            whole segment
            - GuzzleHttp [namespace]  insert `GuzzleHttp\`
            - ... and the client must ask again
            - accepting the first leaves `use GuzzleHttp\`
        "#]],
    );
}

#[test]
fn a_use_line_goes_on_into_the_namespaces_under_a_namespace() {
    check(
        r#"
//- /src/Client.php
<?php
namespace App;

use GuzzleHttp\E@here@
"#,
        &[Query::completion("second segment", "here")],
        expect![[r#"
            second segment
            - GuzzleHttp\Exception [namespace]  insert `GuzzleHttp\Exception\`
            - GuzzleHttp\Events [class]
            - ... and the client must ask again
            - accepting the first leaves `use GuzzleHttp\Exception\`
        "#]],
    );
}

#[test]
fn a_fully_qualified_name_in_code_offers_the_class_before_the_namespace() {
    check(
        r#"
//- /src/Client.php
<?php
namespace App;

function send(): void
{
    throw new \GuzzleHttp\E@here@
}
"#,
        &[Query::completion("in code", "here")],
        expect![[r#"
            in code
            - Events [class]  GuzzleHttp\Events  insert `\GuzzleHttp\Events`
            - Exception [namespace]  GuzzleHttp\Exception  insert `\GuzzleHttp\Exception\`
            - ... and the client must ask again
            - accepting the first leaves `throw new \GuzzleHttp\Events`
        "#]],
    );
}

#[test]
fn a_name_through_an_import_goes_on_from_what_the_import_names() {
    // The anchor stays as written: `Guzzle\` is the import, and what is inserted goes on from it
    // rather than being rewritten into the fully-qualified path.
    check(
        r#"
//- /src/Client.php
<?php
namespace App;

use GuzzleHttp as Guzzle;

function send(): void
{
    throw new Guzzle\H@here@
}
"#,
        &[Query::completion("through an alias", "here")],
        expect![[r#"
            through an alias
            - Handler [namespace]  GuzzleHttp\Handler  insert `Guzzle\Handler\`
            - ... and the client must ask again
            - accepting the first leaves `throw new Guzzle\Handler\`
        "#]],
    );
}

#[test]
fn a_docblock_offers_the_namespace_first() {
    check(
        r#"
//- /src/Client.php
<?php
namespace App;

/**
 * @throws \GuzzleHttp\E@here@
 */
function send(): void {}
"#,
        &[Query::completion("in a docblock", "here")],
        expect![[r#"
            in a docblock
            - Exception [namespace]  GuzzleHttp\Exception  insert `\GuzzleHttp\Exception\`
            - Events [class]  GuzzleHttp\Events  insert `\GuzzleHttp\Events`
            - ... and the client must ask again
            - accepting the first leaves `* @throws \GuzzleHttp\Exception\`
        "#]],
    );
}

#[test]
fn a_bare_word_in_code_is_not_offered_a_top_level_namespace() {
    // Relative to `App`, so `GuzzleHttp\Client` written here means `App\GuzzleHttp\Client`.
    check(
        r#"
//- /src/Client.php
<?php
namespace App;

function send(): void
{
    $client = new Guzz@here@
}
"#,
        &[Query::completion("bare word", "here")],
        expect![[r#"
            bare word
            - GuzzleException [class]  GuzzleHttp\Exception\GuzzleException  + `use GuzzleHttp\Exception\GuzzleException;` at 2:15-2:15
            - ... and the client must ask again
            - accepting the first leaves `$client = new GuzzleException`
        "#]],
    );
}

#[test]
fn the_namespace_typed_in_its_own_case_comes_first() {
    // Drupal's shape: `Drupal\Core` beside a module namespace for every module, all lower case.
    crate::tests::check(
        r#"
//- /core/modules/comment/src/Thing.php
<?php
namespace Drupal\comment;

class Thing {}

//- /core/lib/Drupal/Core/Kernel.php
<?php
namespace Drupal\Core;

class Kernel {}

//- /src/Client.php
<?php
namespace App;

use Drupal\Co@here@
"#,
        &[Query::completion("case", "here")],
        expect![[r#"
            case
            - Drupal\Core [namespace]  insert `Drupal\Core\`
            - Drupal\comment [namespace]  insert `Drupal\comment\`
            - ... and the client must ask again
            - accepting the first leaves `use Drupal\Core\`
        "#]],
    );
}

/// A bare word is relative to the file's namespace, so the namespaces it can begin are the ones
/// under that: `Types\ArrayType::class` written inside `Illuminate\JsonSchema`, six times in one
/// Laravel package. A top-level namespace is not one of them -- written bare inside a namespace it
/// means a path under that namespace -- and a segment the file imports is the import's.
#[test]
fn a_bare_word_begins_the_namespaces_under_the_files_own() {
    crate::tests::check(
        r#"
//- /src/Types/ArrayType.php
<?php
namespace Schema\Types;

class ArrayType {}

//- /src/Typing/Hint.php
<?php
namespace Schema\Typing;

class Hint {}

//- /src/Other/Typing/Hint.php
<?php
namespace Other\Typing;

class Hint {}

//- /vendor/lib/Tyre.php
<?php
namespace Tyre;

class Wheel {}

//- /src/Serializer.php
<?php
namespace Schema;

use Other\Typing;

class Serializer
{
    public function map(): array
    {
        return [Ty@here@];
    }
}
"#,
        &[Query::completion("relative", "here")],
        expect![[r#"
            relative
            - Typing [namespace]  Other\Typing  insert `Typing\`
            - Types [namespace]  Schema\Types  insert `Types\`
            - ... and the client must ask again
            - accepting the first leaves `return [Typing\];`
        "#]],
    );
}
