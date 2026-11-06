//! A call with fewer arguments than its callee requires.
//!
//! `ArgumentCountError` on every run that reaches the line, and the callee's declaration is in
//! another file -- so this is one of the few checks a per-file linter cannot make, and one of the few
//! that is a fact rather than a preference.
//!
//! Mostly about what it declines. A count is easy to get wrong in five ways, and each of the silences
//! below is one of them.

use expect_test::expect;

use super::diagnostics::check;

#[test]
fn a_call_missing_a_required_argument_is_reported() {
    check(
        r#"
//- /app.php
<?php
class Repo
{
    public function find(int $id, bool $strict): void {}
}

class Caller
{
    public function run(Repo $repo): void
    {
        $repo->find(1);
        $repo->find();
    }
}
"#,
        expect![[r#"
            11:9-11:23 error [missing-argument] find() needs 2 arguments, 1 given -- nothing for $strict
            12:9-12:22 error [missing-argument] find() needs 2 arguments, 0 given -- nothing for $id, $strict
        "#]],
    );
}

/// A parameter with a default is not required, and a variadic takes however many are left.
#[test]
fn a_default_and_a_variadic_are_not_missing() {
    check(
        r#"
//- /app.php
<?php
class Repo
{
    public function some(int $id, bool $strict = false): void {}

    public function many(int $id, string ...$rest): void {}
}

class Caller
{
    public function run(Repo $repo): void
    {
        $repo->some(1);
        $repo->many(1);
    }
}
"#,
        expect![""],
    );
}

/// A spread stands for however many values are in the array, and nothing here knows how many. The
/// difference between this check and a guess.
#[test]
fn a_spread_makes_the_count_unknowable_and_is_declined() {
    check(
        r#"
//- /app.php
<?php
class Repo
{
    public function find(int $id, bool $strict): void {}
}

class Caller
{
    public function run(Repo $repo, array $args): void
    {
        $repo->find(...$args);
    }
}
"#,
        expect![""],
    );
}

/// A named argument fills the parameter it names, wherever that is in the list.
#[test]
fn a_named_argument_fills_the_parameter_it_names() {
    check(
        r#"
//- /app.php
<?php
class Repo
{
    public function find(int $id, bool $strict): void {}
}

class Caller
{
    public function run(Repo $repo): void
    {
        $repo->find(strict: TRUE);
        $repo->find(id: 1, strict: TRUE);
    }
}
"#,
        expect![[r#"
            11:9-11:34 error [missing-argument] find() needs 2 arguments, 1 given -- nothing for $id
        "#]],
    );
}

/// A callee nothing declares is not a call with too few arguments. Reporting a count against a
/// signature that was never found is how a checker earns its reputation for noise.
#[test]
fn a_call_whose_callee_is_unknown_says_nothing() {
    check(
        r#"
//- /app.php
<?php
class Caller
{
    public function run($whatever): void
    {
        $whatever->find();
    }
}
"#,
        expect![""],
    );
}

/// A constructor is counted like anything else, promoted parameters included.
#[test]
fn a_constructor_is_counted_too() {
    check(
        r#"
//- /app.php
<?php
class Repo
{
    public function __construct(private int $id, public string $label) {}
}

class Caller
{
    public function run(): void
    {
        new Repo(1);
    }
}
"#,
        expect![[r#"
            11:9-11:20 error [missing-argument] the constructor needs 2 arguments, 1 given -- nothing for $label
        "#]],
    );
}

/// A nullable value where the parameter does not admit one. `TypeError` at runtime, from the other
/// side of the same claim a null receiver makes.
#[test]
fn a_nullable_argument_is_reported() {
    check(
        r#"
//- /app.php
<?php
class Repo
{
    public function take(string $name): void {}
}

class Caller
{
    public function run(Repo $repo, ?string $maybe): void
    {
        $repo->take($maybe);
        $repo->take(NULL);
    }
}
"#,
        expect![[r#"
            11:21-11:27 warning [possibly-null-argument] $name takes string, and this may be null
            12:21-12:25 error [null-argument] Cannot pass null as $name, which takes string
        "#]],
    );
}

/// A guard is what makes this liveable, and it is the layer below that applies it.
#[test]
fn a_guarded_value_is_not_reported() {
    check(
        r#"
//- /app.php
<?php
class Repo
{
    public function take(string $name): void {}
}

class Caller
{
    public function run(Repo $repo, ?string $maybe): void
    {
        if ($maybe !== NULL) {
            $repo->take($maybe);
        }
    }
}
"#,
        expect![""],
    );
}

/// The parameters that accept anything, and the one whose *default* says it accepts null.
#[test]
fn a_parameter_that_admits_null_or_says_nothing_is_silent() {
    check(
        r#"
//- /app.php
<?php
class Repo
{
    public function nullable(?string $a): void {}

    public function untyped($a): void {}

    public function anything(mixed $a): void {}

    /// PHP makes a parameter that defaults to `null` implicitly nullable whatever its hint says --
    /// deprecated on 8.4 and still in force. A default that is anything else says nothing about
    /// what the parameter takes, and used to end the pairing for the whole call.
    public function defaultsToNull(string $a = null): void {}

    public function defaultsToSomething(string $a = 'x'): void {}
}

class Caller
{
    public function run(Repo $repo, ?string $maybe): void
    {
        $repo->nullable($maybe);
        $repo->untyped($maybe);
        $repo->anything($maybe);
        $repo->defaultsToNull($maybe);
        $repo->defaultsToSomething($maybe);
    }
}
"#,
        expect![[r#"
            26:36-26:42 warning [possibly-null-argument] $a takes string, and this may be null
        "#]],
    );
}

/// A spread stops the pairing, because an array's length is not knowable here.
///
/// A named argument used to stop it too, and that was a gap rather than a rule -- PHP fills the
/// parameter the name names. See `a_named_argument_against_its_parameter` for that half; here the
/// named call is the control that proves the spread is what this test is about.
#[test]
fn a_spread_stops_the_pairing_and_a_named_argument_does_not() {
    check(
        r#"
//- /app.php
<?php
class Repo
{
    public function two(string $first, string $second): void {}
}

class Caller
{
    public function run(Repo $repo, ?string $maybe, array $args): void
    {
        $repo->two(...$args);
        $repo->two(second: $maybe, first: 'x');
    }
}
"#,
        expect![[r#"
            12:20-12:34 warning [possibly-null-argument] $second takes string, and this may be null
        "#]],
    );
}

/// A `new` on something PHP will not instantiate. Fatal, and only the index knows: whether a name is
/// abstract is decided by a declaration in another file.
#[test]
fn an_abstract_class_an_interface_and_a_trait_cannot_be_instantiated() {
    check(
        r#"
//- /app.php
<?php
abstract class Shape {}

interface Drawable {}

trait Helps {}

class Circle extends Shape {}

class Factory
{
    public function run(): void
    {
        new Circle();
        new Shape();
        new Drawable();
        new Helps();
    }
}
"#,
        expect![[r#"
            15:9-15:20 error [uninstantiable-class] Cannot instantiate abstract class Shape
            16:9-16:23 error [uninstantiable-class] Cannot instantiate interface Drawable
            17:9-17:20 error [uninstantiable-class] Cannot instantiate trait Helps
        "#]],
    );
}

/// The exemption that matters, and the distinction the first version of this check had backwards.
///
/// `new static()` inside an abstract class is legal and is the standard factory idiom: `static` is
/// the class the call was made on, which by definition was instantiable. `new self()` is the class
/// the code is written in, so there it is a real error.
#[test]
fn new_static_is_the_factory_idiom_and_new_self_is_not() {
    check(
        r#"
//- /app.php
<?php
abstract class Maker
{
    public static function create(): static
    {
        return new static();
    }

    public static function wrong(): self
    {
        return new self();
    }
}
"#,
        expect![[r#"
            11:16-11:26 error [uninstantiable-class] Cannot instantiate abstract class Maker
        "#]],
    );
}
