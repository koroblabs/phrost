//! A call to a method the calling scope may not reach, on a class with `__call` or `__callStatic`.
//!
//! PHP runs the magic method and never the declaration, so the declaration's parameters say nothing
//! about the arguments -- run against 8.5.10, `A::hidden()` on a `protected function hidden($x)`
//! beside a `__callStatic` returns what `__callStatic` does, with no `ArgumentCountError`. Laravel's
//! `#[Scope] protected function popular($query)` is called exactly that way. Inside the class the
//! declaration is what runs, and its parameters are checked as ever.

use expect_test::expect;

use super::diagnostics::check;

#[test]
fn only_a_call_the_declaration_answers_is_held_to_its_parameters() {
    check(
        r#"
//- /src/Calls.php
<?php
namespace App;

class Magic
{
    protected function hidden(int $x): void {}

    public function __call(string $name, array $arguments): mixed {}

    public static function __callStatic(string $name, array $arguments): mixed {}

    public function inside(): void
    {
        $this->hidden();
    }
}

class Plain
{
    public function shown(int $x): void {}
}

function run(Magic $magic, Plain $plain): void
{
    $magic->hidden();
    Magic::hidden();
    $plain->shown();
}
"#,
        expect![[r#"
            14:9-14:24 error [missing-argument] hidden() needs 1 argument, 0 given -- nothing for $x
            27:5-27:20 error [missing-argument] shown() needs 1 argument, 0 given -- nothing for $x
        "#]],
    );
}
