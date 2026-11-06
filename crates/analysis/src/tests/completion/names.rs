//! Offering names rather than members.
//!
//! A bare prefix, a qualified prefix — and a string, where a name is not what anyone is reaching
//! for. What a `use` line offers is its own question, and is in `use_paths`.

use expect_test::expect;

use crate::tests::{Query, check};

#[test]
fn a_bare_prefix_offers_names_and_says_how_to_write_them() {
    // An imported name is inserted short; anything else fully qualified, which is verbose and always
    // correct. Offering the short name and leaving the file broken is not a trade worth making.
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repo {}

//- /vendor/acme/src/Report.php
<?php
namespace Acme;

class Report {}

//- /src/Client.php
<?php
namespace App;

use Acme\Report;

class Client
{
    public function run(): void
    {
        new Rep@here@
    }
}
"#,
        &[Query::completion("names", "here")],
        expect![[r#"
            names
            - Repo [class]  App\Repo
            - Report [class]  Acme\Report
            - ... and the client must ask again
            - accepting the first leaves `new Repo`
        "#]],
    );
}

#[test]
fn a_qualified_prefix_narrows_by_the_namespace_already_typed() {
    check(
        r#"
//- /src/A.php
<?php
namespace App\Data;

class Node {}

//- /src/B.php
<?php
namespace Other\Tree;

class Node {}

//- /src/Client.php
<?php
namespace App;

class Client
{
    public function run(): void
    {
        new Data\No@here@
    }
}
"#,
        &[Query::completion("qualified", "here")],
        expect![[r#"
            qualified
            - Node [class]  App\Data\Node  insert `Data\Node`
            - ... and the client must ask again
            - accepting the first leaves `new Data\Node`
        "#]],
    );
}

#[test]
fn a_qualified_prefix_names_a_namespace_and_a_class_within_it() {
    // `\Drupal` means a name that *begins* that way, and its last segment is the class. Matching
    // short names offers every class whose last segment starts with `Drupal`; matching the whole
    // prefix without the one-level rule offers every class in the framework. Both are long,
    // confident and wrong -- on the real project the first offered 40 and the second 200.
    check(
        r#"
//- /src/Drupal.php
<?php

class Drupal {}

//- /src/Entity.php
<?php
namespace Drupal\Core;

class Entity {}

//- /src/Node.php
<?php
namespace Drupal\Core\Entity;

class Node {}

//- /src/Unrelated.php
<?php
namespace App;

class DrupalBoot {}

//- /src/script.php
<?php

$a = \Drupal@root@
$b = \Drupal\Core\Ent@nested@
"#,
        &[
            Query::completion("at the root", "root"),
            Query::completion("inside a namespace", "nested"),
        ],
        expect![[r#"
            at the root
            - Drupal [class]  Drupal  insert `\Drupal`
            - Drupal [namespace]  Drupal  insert `\Drupal\`
            - ... and the client must ask again
            - accepting the first leaves `$a = \Drupal`
            inside a namespace
            - Entity [class]  Drupal\Core\Entity  insert `\Drupal\Core\Entity`
            - Entity [namespace]  Drupal\Core\Entity  insert `\Drupal\Core\Entity\`
            - ... and the client must ask again
            - accepting the first leaves `$b = \Drupal\Core\Entity`
        "#]],
    );
}

#[test]
fn nothing_is_offered_inside_a_string() {
    // A service id, a route name, a translation key: none of them is a class, and a list of classes
    // over one hides whatever the editor would have suggested from its own history.
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Container
{
    public static function service(string $name) {}
}

/// Named so the prefix would match it. A fixture that declares nothing the prefix could find
/// would pass with the guard removed, which is no test at all.
class OrderManager {}

class Repo
{
    public function run(): void
    {
        Container::service('appl@here@');
    }
}
"#,
        &[Query::completion("in a string", "here")],
        expect![[r#"
            in a string
            - (nothing offered)
        "#]],
    );
}

#[test]
fn nothing_is_offered_inside_a_string_that_is_still_being_typed() {
    // The case that matters most, and the one a tree cannot answer: an unclosed quote does not parse,
    // so the recovered tree has no literal for it -- and that is exactly the keystroke at which a
    // list of every global name appears over a service id. Accepting one by reflex writes
    // `\array_diff` into the string.
    check(
        r#"
//- /src/app.php
<?php

// Named so the prefix would match it: a test whose fixture declares nothing the prefix could
// find would pass with the guard removed, which is no test at all.
class ArrayHelper {}

date('arr@here@
"#,
        &[Query::completion("mid-string", "here")],
        expect![[r#"
            mid-string
            - (nothing offered)
        "#]],
    );
}

#[test]
fn an_interpolated_variable_is_not_the_string_around_it() {
    // `"text $name"` is string, variable, string. Someone typing at the variable is naming one and
    // should be offered one; suppressing the whole quoted region would take that away.
    check(
        r#"
//- /src/app.php
<?php

class Service
{
    public function run(string $appliance, string $other): void
    {
        echo "value: $appl@here@";
    }
}
"#,
        &[Query::completion("at the variable", "here")],
        expect![[r#"
            at the variable
            - $appliance [variable]
            - ... and the client must ask again
            - accepting the first leaves `echo "value: $appliance";`
        "#]],
    );
}

#[test]
fn an_interpolated_variable_followed_by_more_text_is_still_a_variable() {
    // The lexer hands back the text after the variable as a string part of its own, starting where
    // the variable ends -- so a cursor at the end of `$appl` sat at the start of a string and was
    // offered nothing. The closing quote right after it, in the test above, is not a string part,
    // which is why that one passed. Both shapes are from a real project.
    check(
        r#"
//- /src/app.php
<?php

class Service
{
    public function run(string $appliance, int $code): void
    {
        echo "value: $appl@line@\n";
        echo "response code `$co@tick@` received";
    }
}
"#,
        &[
            Query::completion("text after it", "line"),
            Query::completion("a backtick after it", "tick"),
        ],
        expect![[r#"
            text after it
            - $appliance [variable]
            - ... and the client must ask again
            - accepting the first leaves `echo "value: $appliance\n";`
            a backtick after it
            - $code [variable]
            - ... and the client must ask again
            - accepting the first leaves `echo "response code `$code` received";`
        "#]],
    );
}

#[test]
fn a_global_function_or_constant_is_offered_bare_and_a_global_class_is_not() {
    // PHP falls back to the global namespace for functions and constants and *not* for classes, so
    // `array_map` needs nothing and `\Exception` needs its separator. Resolving all three against
    // the class table -- which is what used to happen -- found none of the first two, so every one
    // of them was offered fully qualified: `\array_map`, `\JSON_THROW_ON_ERROR`.
    //
    // Three markers in one file, which usually loses everything after the first parse break -- here
    // it does not, because completion reads the text backwards for context and the file's namespace
    // for resolution, and neither needs the statements to have survived. The marker-consumption
    // check is what would say so if that ever changed.
    check(
        r#"
//- /stubs.php
<?php
function array_map(callable $callback, array $array): array {}
const JSON_THROW_ON_ERROR = 4194304;
class Exception {}
//- /src/app.php
<?php
namespace App;

class Service
{
    public function run(): void
    {
        array_m@function@
        JSON_TH@constant@
        new Excep@class@
    }
}
"#,
        &[
            Query::completion("a global function", "function"),
            Query::completion("a global constant", "constant"),
            Query::completion("a global class", "class"),
        ],
        expect![[r#"
            a global function
            - array_map [function]  array_map
            - ... and the client must ask again
            - accepting the first leaves `array_map`
            a global constant
            - JSON_THROW_ON_ERROR [constant]  JSON_THROW_ON_ERROR
            - ... and the client must ask again
            - accepting the first leaves `JSON_THROW_ON_ERROR`
            a global class
            - Exception [class]  Exception  insert `\Exception`
            - ... and the client must ask again
            - accepting the first leaves `new \Exception`
        "#]],
    );
}

#[test]
fn a_stub_internal_marker_interface_is_never_offered_by_name() {
    // `__INTERNAL_DO_NOT_USE__StringBackedEnum` is a real declaration -- the standard library
    // ships it on purpose, to narrow a backed enum's `$value` for the type checker -- but its
    // whole point is that application code never writes it. Offering it here would be the same
    // leak completion already refuses for `implements` and `goto implementation`.
    check(
        r#"
//- /stubs.php
<?php
interface __INTERNAL_DO_NOT_USE__StringBackedEnum {}
interface __INTERNAL_DO_NOT_USE__IntBackedEnum {}
//- /src/app.php
<?php
namespace App;

class Service
{
    public function run(): void
    {
        new __INTERNAL@here@
    }
}
"#,
        &[Query::completion("names", "here")],
        expect![[r#"
            names
            - (nothing offered)
        "#]],
    );
}
