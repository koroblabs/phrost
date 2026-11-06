//! Chains built out of `static`, `self`, and `$this`.
//!
//! How most builders in PHP are written -- `DataDefinition::create()->setLabel(...)->setRequired(...)`
//! -- and the reason a chain used to die at its first link: a relative return type has left the file
//! that wrote it by the time the index hands it back, so nothing at the cursor says which class it
//! meant.

use expect_test::expect;

use super::{Query, check};

#[test]
fn a_static_return_carries_the_chain() {
    check(
        r#"
//- /src/app.php
<?php

class Definition
{
    public static function create(): static { }
    public function setLabel(string $label): static { }
    public function setRequired(bool $required): static { }
}

class Service
{
    public function run(): void
    {
        Definition::create()->setLabel('x')->@here@
    }
}
"#,
        &[Query::completion("three links in", "here")],
        expect![[r#"
            three links in
            - create [method]  : static  shown last  writes `();`
            - setLabel [method]  : static  writes `(|);`
            - setRequired [method]  : static  writes `(|);`
            - accepting the first leaves `Definition::create()->setLabel('x')->setLabel`
        "#]],
    );
}

#[test]
fn a_docblock_this_is_the_same_claim_as_static() {
    check(
        r#"
//- /src/app.php
<?php

class Query
{
    /** @return $this */
    public function condition(string $field) { }
    public function execute(): array { }
}

class Service
{
    public function run(Query $query): void
    {
        $query->condition('a')->@here@
    }
}
"#,
        &[Query::completion("after a $this return", "here")],
        expect![[r#"
            after a $this return
            - condition [method]  : static  writes `(|);`
            - execute [method]  : array  writes `();`
            - accepting the first leaves `$query->condition('a')->condition`
        "#]],
    );
}

#[test]
fn self_means_the_class_that_wrote_it_and_static_means_the_one_called() {
    // `Base::create(): self` called as `Child::create()` returns a Base, so `onChild` is not there.
    // Returning the called class would be a confident wrong answer, and PHP has two keywords
    // precisely because the difference is visible.
    check(
        r#"
//- /src/app.php
<?php

class Base
{
    public static function make(): self { }
    public static function makeLate(): static { }
    public function onBase(): void { }
}

class Child extends Base
{
    public function onChild(): void { }
}

class Service
{
    public function early(): void
    {
        Child::make()->@early@
    }

    public function late(): void
    {
        Child::makeLate()->@late@
    }
}
"#,
        &[
            Query::completion("self", "early"),
            Query::completion("static", "late"),
        ],
        expect![[r#"
            self
            - make [method]  : self  shown last  writes `();`
            - makeLate [method]  : static  shown last  writes `();`
            - onBase [method]  : void  writes `();`
            - accepting the first leaves `Child::make()->onBase`
            static
            - make [method]  : self  shown last  writes `();`
            - makeLate [method]  : static  shown last  writes `();`
            - onBase [method]  : void  writes `();`
            - onChild [method]  : void  writes `();`
            - accepting the first leaves `Child::makeLate()->onBase`
        "#]],
    );
}
