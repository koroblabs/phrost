//! `{@inheritdoc}` over a signature that states no type, which is how most of Drupal is written.
//!
//! An override saying nothing about its return is not saying the return is unknown -- it is saying
//! it has not changed. Resolution reads it that way already, so go-to-type-definition at
//! `$entity->get(...)` lands on `FieldItemListInterface` while the card said
//! `public function get($field_name)`: everything the reader could already see, and nothing they
//! came for. One declaration described two ways.
//!
//! The climb stops at whatever declares a type, and a declaration that states its own is never
//! second-guessed. Nor does a type read off the *body* stop it: Drupal's `SelectInterface::execute()`
//! declares `@return StatementInterface|null` and the concrete class overrides it with no docblock,
//! so the non-nullable thing the body happens to hand back is the one answer a reader must not be
//! shown -- and a card that showed it disagreed with every check about the same call.

use expect_test::expect;

use super::{Query, check};

const FIXTURE: &str = r#"
//- /src/app.php
<?php

class Field {}

interface Holder
{
    /**
     * @return Field
     */
    public function get(string $name);

    /**
     * @return Field
     */
    public function first();

    /**
     * @return ?Field
     */
    public function execute();
}

abstract class Base implements Holder
{
    /**
     * {@inheritdoc}
     */
    public function get(string $name)
    {
        return new Field();
    }

    /**
     * {@inheritdoc}
     */
    public function first(): ?Field
    {
        return null;
    }

    public function execute()
    {
        return new Field();
    }
}

class Entity extends Base
{
    public function own(): string
    {
        return '';
    }
}

class Caller
{
    public function run(Entity $entity): void
    {
        $entity->g@inherited@et('body');
        $entity->fi@declared@rst();
        $entity->o@own@wn();
        $entity->exec@body@ute();
    }
}
"#;

#[test]
fn a_declaration_with_no_type_of_its_own_shows_the_one_it_inherits() {
    check(
        FIXTURE,
        &[
            Query::hover("two levels up, through an interface", "inherited"),
            Query::hover("a declaration of its own wins", "declared"),
            Query::hover("nothing to inherit", "own"),
            Query::hover("a declaration outranks the body", "body"),
        ],
        expect![[r#"
            two levels up, through an interface
            - code: <?php
            class Base
            {
                public function get(string $name): Field
            }
            - note: inherited Base
            - ---
            - tag: Returns Field 
            a declaration of its own wins
            - code: <?php
            class Base
            {
                public function first(): ?Field
            }
            - note: inherited Base
            - ---
            - tag: Returns Field 
            nothing to inherit
            - code: <?php
            class Entity
            {
                public function own(): string
            }
            a declaration outranks the body
            - code: <?php
            class Base
            {
                public function execute(): ?Field
            }
            - note: inherited Base
        "#]],
    );
}

#[test]
fn a_relative_return_stays_the_keyword_on_a_card_about_the_declaration() {
    // `static` means the class the call was made *on*, and a `SymbolLocation` does not carry it --
    // its container is the class that *declared* the member. Reading that as the called class put
    // `?EntityBase` on the card for `User::load($id)`, a class that is abstract and so one `static`
    // can never be, while go-to-type-definition at the same site landed on `User`. Two answers
    // about one value, disagreeing.
    //
    // The keyword is the honest rendering here: the card is describing `Base::load`, whose return
    // really is `static`. Completion holds a receiver and gets the class instead -- see
    // `IndexView::resolved_signature`, whose `called_on` is what separates the two.
    check(
        r#"
//- /src/app.php
<?php

interface Loads
{
    /**
     * @return static|null
     */
    public static function load(int $id);
}

abstract class Base implements Loads
{
    /**
     * {@inheritdoc}
     */
    public static function load(int $id)
    {
        return NULL;
    }
}

final class User extends Base {}

function run(): void
{
    User::l@subclass@oad(1);
}
"#,
        &[Query::hover(
            "a relative return, through a subclass",
            "subclass",
        )],
        expect![[r#"
            a relative return, through a subclass
            - code: <?php
            class Base
            {
                public static function load(int $id): ?static
            }
            - note: returns ?User here
            - note: inherited Base
            - ---
            - tag: Returns static|null
        "#]],
    );
}
