//! Which declaration wins when a member has both a native hint and a docblock.
//!
//! Three answers, and all three are written in real framework code beside each other: the docblock
//! supplies type arguments PHP has no syntax for, it names a subclass of the hint, or it restates
//! the hint. Only the third is noise, and telling them apart takes a resolved type — which is why
//! the decision is here and not in lowering.

use expect_test::expect;

use super::{Query, check};

#[test]
fn a_docblock_naming_a_subclass_wins() {
    // The hint is the interface the framework hands out; the docblock is the author saying which
    // class the value really is. Reading the hint instead reports every call to the subclass's own
    // API as undefined, which on one real project was ten findings from this one property.
    check(
        r#"
//- /app.php
<?php
interface UserInterface
{
    public function id(): int;
}

class User implements UserInterface
{
    public function id(): int {}
    public function getResellers(): array {}
}

class Form
{
    /**
     * @var \User|null
     */
    protected ?UserInterface $user = NULL;

    public function run(): void
    {
        $this->user->getResel$0lers();
    }
}
"#,
        &[Query::goto("a narrowing docblock", "0")],
        expect![[r#"
            a narrowing docblock
            - method User::getResellers @ app.php 10:21-10:33 (full 10:5-10:45)
        "#]],
    );
}

#[test]
fn a_docblock_naming_something_unrelated_loses() {
    // A disagreement rather than extra information, and the hint is what the runtime enforces.
    check(
        r#"
//- /app.php
<?php
interface UserInterface
{
    public function id(): int;
}

class Unrelated
{
    public function getResellers(): array {}
}

class Form
{
    /**
     * @var \Unrelated
     */
    protected ?UserInterface $user = NULL;

    public function run(): void
    {
        $this->user->getResel$0lers();
    }
}
"#,
        &[Query::goto("a disagreeing docblock", "0")],
        expect![[r#"
            a disagreeing docblock
            - (nothing to go to)
        "#]],
    );
}

#[test]
fn a_relative_type_on_either_side_keeps_the_hint() {
    // `: self` with `@return $this` beside it is how every fluent builder is written. Neither side
    // names a class this can look up, so the rule declines -- and declining has to keep what the
    // declaration already gave. Answering "no type" here sends the caller climbing to the interface
    // above, where `: self` means the interface, and a whole chain lands one level too high.
    check(
        r#"
//- /app.php
<?php
interface Match_
{
    public function with(): self;
}

class Builder implements Match_
{
    /**
     * @return $this
     */
    public function with(): self {}

    public function willReturn(): void {}
}

class Test
{
    public function run(Builder $builder): void
    {
        $builder->with()->willRet$0urn();
    }
}
"#,
        &[Query::goto("a fluent chain", "0")],
        expect![[r#"
            a fluent chain
            - method Builder::willReturn @ app.php 14:21-14:31 (full 14:5-14:42)
        "#]],
    );
}
