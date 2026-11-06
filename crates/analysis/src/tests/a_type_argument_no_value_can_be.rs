//! A generic value whose type argument no value of the declared one can be.
//!
//! `Box<string>` where `Box<int>` is declared, and `IntBox extends Box<int>` where `Box<string>` is.
//! What variance would decide is left alone -- a `Box<Cat>` handed to a `Box<Animal>` or a `Box<Pet>`
//! it implements, an `int` where a `float` is asked -- because most libraries mean covariance and
//! declare nothing, and read strictly every `Collection<User>` for a `Collection<Model>` would be
//! reported. A `Box<Rock>` for a `Box<Pet>` is not variance: nothing declared makes a `Rock` a `Pet`.

use expect_test::expect;

use super::diagnostics::check;

#[test]
fn only_arguments_that_share_no_value_are_reported() {
    check(
        r#"
//- /src/Box.php
<?php
namespace App;

/** @template T */
class Box
{
    /** @param T $value */
    public function __construct(public mixed $value) {}
}

/** @extends Box<int> */
final class IntBox extends Box {}

interface Pet {}
class Animal {}
final class Cat extends Animal implements Pet {}
final class Rock {}

/** @param Box<int> $box */
function ints(Box $box): void {}

/** @param Box<string> $box */
function strings(Box $box): void {}

/** @param Box<float> $box */
function floats(Box $box): void {}

/** @param Box<Animal> $box */
function animals(Box $box): void {}

/** @param Box<Pet> $box */
function pets(Box $box): void {}

/**
 * @template U
 * @param U $value
 */
function passAlong(mixed $value): void
{
    /** @var Box<U> $box */
    $box = new Box($value);
    ints($box);
}

function go(): void
{
    ints(new Box(1));
    ints(new Box('x'));
    strings(new IntBox(1));
    floats(new Box(1));
    animals(new Box(new Cat()));
    animals(new Box(new Rock()));
    pets(new Box(new Rock()));
    /** @var Box<string> $written */
    $written = new Box('y');
    ints($written);
}
"#,
        expect![[r#"
            48:10-48:22 error [incompatible-argument] $box takes \App\Box<int>, and this is Box<non-falsy-string>
            49:13-49:26 error [incompatible-argument] $box takes \App\Box<string>, and this is IntBox
            52:13-52:32 error [incompatible-argument] $box takes \App\Box<\App\Animal>, and this is Box<Rock>
            53:10-53:29 error [incompatible-argument] $box takes \App\Box<\App\Pet>, and this is Box<Rock>
            56:10-56:18 error [incompatible-argument] $box takes \App\Box<int>, and this is Box<string>
        "#]],
    );
}
