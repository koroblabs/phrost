//! A card over a call says what that call gives back where it is written.
//!
//! `get(): T` is the declaration and is true of every call; `$box->get()` on a `Box<User>` is a
//! `User`, which goto and completion already knew and the card did not say. Said only where the
//! two differ, so a declaration that already names the type is not repeated.

use expect_test::expect;

use crate::tests::{Query, check};

#[test]
fn a_card_over_a_call_says_what_it_gives_back_here() {
    check(
        r#"
//- /src/Box.php
<?php
namespace App;

/**
 * @template T
 */
class Box
{
    /** @var T */
    public mixed $value;

    /** @return T */
    public function get(): mixed
    {
        return $this->value;
    }

    public function count(): int
    {
        return 1;
    }
}

class User {}

/**
 * @template V
 * @param V $value
 * @return V
 */
function same(mixed $value): mixed
{
    return $value;
}

class Reader
{
    /** @param Box<User> $box */
    public function read(Box $box): void
    {
        $box->g@method@et();
        $box->val@property@ue;
        $box->cou@plain@nt();
        sa@function@me(new User());
    }
}
"#,
        &[
            Query::hover("a template's method", "method"),
            Query::hover("a template's property", "property"),
            Query::hover("a declaration already naming it", "plain"),
            Query::hover("a function binding its template", "function"),
        ],
        expect![[r#"
            a template's method
            - code: <?php
            namespace App;

            class Box
            {
                public function get(): T
            }
            - note: returns User here
            - ---
            - tag: Returns T 
            a template's property
            - code: <?php
            namespace App;

            class Box
            {
                public T $value
            }
            - note: holds User here
            - ---
            - tag: Type T 
            a declaration already naming it
            - code: <?php
            namespace App;

            class Box
            {
                public function count(): int
            }
            a function binding its template
            - code: <?php
            namespace App;

            function same(mixed $value): mixed
            - note: returns User here
            - ---
            - tag: Parameter $value V 
            - tag: Returns V 
            - tag: @template V
        "#]],
    );
}
