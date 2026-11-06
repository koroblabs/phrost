//! A function or a constant written as a path: `\sprintf`, `\E_ALL`, `use function App\render`.
//!
//! A path was matched against the class table alone, so all three offered nothing -- and Symfony
//! writes every global function from the root, so after the separator its code got no completion
//! at all. What was typed is kept: the separator is the project's style, not a slip to correct.

use expect_test::expect;

use crate::tests::{Query, check};

const DECLARED: &str = r#"
//- /vendor/php/standard.php
<?php
function sprintf(string $format, mixed ...$values): string {}
function sprintf_like(): void {}
const E_ALL = 32767;

//- /src/Support/render.php
<?php
namespace App\Support;

function render(): string {}
"#;

fn check_declared(client: &str, queries: &[Query], expect: expect_test::Expect) {
    check(&format!("{DECLARED}\n{client}"), queries, expect);
}

#[test]
fn a_global_function_written_from_the_root() {
    check_declared(
        r#"
//- /src/Client.php
<?php
namespace App;

function run(): string
{
    return \sprin@here@
}
"#,
        &[Query::completion("function", "here")],
        expect![[r#"
            function
            - sprintf [function]  sprintf  insert `\sprintf`
            - sprintf_like [function]  sprintf_like  insert `\sprintf_like`
            - ... and the client must ask again
            - accepting the first leaves `return \sprintf`
        "#]],
    );
}

#[test]
fn a_global_constant_written_from_the_root() {
    check_declared(
        r#"
//- /src/Client.php
<?php
namespace App;

function run(): int
{
    return \E_A@here@
}
"#,
        &[Query::completion("constant", "here")],
        expect![[r#"
            constant
            - E_ALL [constant]  E_ALL  insert `\E_ALL`
            - ... and the client must ask again
            - accepting the first leaves `return \E_ALL`
        "#]],
    );
}

#[test]
fn a_namespaced_function_in_a_use_line() {
    check_declared(
        r#"
//- /src/Client.php
<?php
namespace App;

use function App\Support\ren@here@
"#,
        &[Query::completion("use function", "here")],
        expect![[r#"
            use function
            - App\Support\render [function]
            - ... and the client must ask again
            - accepting the first leaves `use function App\Support\render`
        "#]],
    );
}

/// `namespace\q_decode()` -- the third way to spell a name, relative to the file's own namespace and
/// to nothing it imports. Read as an ordinary relative name it looked for a namespace called
/// `namespace` and offered nothing.
#[test]
fn a_function_of_the_files_own_namespace_written_with_the_keyword() {
    check_declared(
        r#"
//- /src/Support/decode.php
<?php
namespace App\Support;

function q_decode(string $text): string {}

function decode(string $text): string
{
    return namespace\q_dec@here@
}
"#,
        &[Query::completion("namespace\\", "here")],
        expect![[r#"
            namespace\
            - q_decode [function]  App\Support\q_decode  insert `namespace\q_decode`
            - ... and the client must ask again
            - accepting the first leaves `return namespace\q_decode`
        "#]],
    );
}

/// A function the file imports by name is offered by that name, beside the classes it shares a
/// short name with.
///
/// `use function Termwind\terminal;` then `min(terminal()->width(), 150)`, in Laravel's console
/// components: completion offered five classes called `Terminal` and not the function the line
/// calls, while goto on the same word went to it. The function shares its namespace and its name,
/// case aside, with the class `Termwind\Terminal` -- and the list kept one row per name without
/// asking which of PHP's tables each was in.
#[test]
fn a_function_imported_by_name_is_offered_by_that_name() {
    check_declared(
        r#"
//- /src/Terminal.php
<?php
namespace Other;

class Terminal {}

//- /src/Task.php
<?php
namespace App;

use function App\Support\render;

function run(): string
{
    return rend@here@
}

//- /src/Width.php
<?php
namespace App;

use function App\Support\render;

function width(): int
{
    return min(rend@call@(), 150);
}

//- /vendor/termwind/Functions.php
<?php
namespace Termwind;

class Terminal {}

if (! function_exists('Termwind\terminal')) {
    function terminal(): Terminal {}
}

//- /src/Size.php
<?php
namespace App;

use function Termwind\terminal;

function size(): int
{
    return min(termin@guarded@(), 150);
}
"#,
        &[
            Query::completion("an imported function", "here"),
            Query::completion("called, inside another call", "call"),
            Query::completion("declared under a guard", "guarded"),
        ],
        expect![[r#"
            an imported function
            - render [function]  App\Support\render
            - ... and the client must ask again
            - accepting the first leaves `return render`
            called, inside another call
            - render [function]  App\Support\render
            - ... and the client must ask again
            - accepting the first leaves `return min(render(), 150);`
            declared under a guard
            - Terminal [class]  Other\Terminal  + `use Other\Terminal;` at 4:32-4:32
            - Terminal [class]  Termwind\Terminal  + `use Termwind\Terminal;` at 4:32-4:32
            - terminal [function]  Termwind\terminal
            - ... and the client must ask again
            - accepting the first leaves `return min(Terminal(), 150);`
        "#]],
    );
}
