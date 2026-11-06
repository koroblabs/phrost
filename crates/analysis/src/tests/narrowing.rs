//! What an `instanceof` guard proves.
//!
//! The Drupal shape: a parameter typed as an interface, a guard for the concrete one, and the
//! concrete members inside the branch. The declaration says what the value may be; the guard says
//! what it is here.

use expect_test::expect;

use super::{Query, check};

#[test]
fn a_guard_offers_the_class_it_proved() {
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

interface Thing
{
    public function shared(): void;
}

class Node implements Thing
{
    public function shared(): void {}

    public function nodeOnly(): void {}
}

class Service
{
    public function run(Thing $thing): void
    {
        if ($thing instanceof Node) {
            $thing->@inside@
        }
    }
}
"#,
        &[Query::completion("inside the guard", "inside")],
        expect![[r#"
            inside the guard
            - nodeOnly [method]  : void  writes `();`
            - shared [method]  : void  writes `();`
            - accepting the first leaves `$thing->nodeOnly`
        "#]],
    );
}

#[test]
fn outside_the_branch_the_declaration_is_what_holds() {
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

interface Thing
{
    public function shared(): void;
}

class Node implements Thing
{
    public function nodeOnly(): void {}
}

class Service
{
    public function run(Thing $thing): void
    {
        if ($thing instanceof Node) {
        }
        $thing->@outside@
    }
}
"#,
        &[Query::completion("after the branch", "outside")],
        expect![[r#"
            after the branch
            - shared [method]  : void  writes `();`
            - accepting the first leaves `$thing->shared`
        "#]],
    );
}

#[test]
fn a_guard_through_an_or_offers_only_what_the_declaration_says() {
    // `$thing instanceof Node || $other` is true whenever `$other` is, so inside the branch the
    // value may still be anything the declaration allows. Offering `Node`'s members would suggest a
    // call that fails at runtime.
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

interface Thing
{
    public function shared(): void;
}

class Node implements Thing
{
    public function nodeOnly(): void {}
}

class Service
{
    public function run(Thing $thing, $other): void
    {
        if ($thing instanceof Node || $other) {
            $thing->@inside@
        }
    }
}
"#,
        &[Query::completion("through an or", "inside")],
        expect![[r#"
            through an or
            - shared [method]  : void  writes `();`
            - accepting the first leaves `$thing->shared`
        "#]],
    );
}

#[test]
fn a_guard_narrows_a_property_read_into_a_local() {
    // The common shape: read something untyped, prove what it is, use it.
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Node
{
    public function nodeOnly(): void {}
}

class Service
{
    private $entity;

    public function run(): void
    {
        $value = $this->entity;
        if ($value instanceof Node) {
            $value->@inside@
        }
    }
}
"#,
        &[Query::completion("narrowed local", "inside")],
        expect![[r#"
            narrowed local
            - nodeOnly [method]  : void  writes `();`
            - accepting the first leaves `$value->nodeOnly`
        "#]],
    );
}

#[test]
fn an_early_return_guard_offers_the_class_for_the_rest_of_the_method() {
    // The shape a great deal of Drupal is written in: refuse what you cannot handle, then work with
    // what is left. Everything after the guard is the concrete class.
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

interface Thing
{
    public function shared(): void;
}

class Node implements Thing
{
    public function nodeOnly(): void {}
}

class Service
{
    public function run(Thing $thing): void
    {
        if (!($thing instanceof Node)) {
            return;
        }

        $thing->@after@
    }
}
"#,
        &[Query::completion("after the guard", "after")],
        expect![[r#"
            after the guard
            - nodeOnly [method]  : void  writes `();`
            - shared [method]  : void  writes `();`
            - accepting the first leaves `$thing->nodeOnly`
        "#]],
    );
}

#[test]
fn an_assert_offers_the_class_it_asserts() {
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

interface Thing
{
    public function shared(): void;
}

class Node implements Thing
{
    public function nodeOnly(): void {}
}

class Service
{
    public function run(Thing $thing): void
    {
        assert($thing instanceof Node);
        $thing->@after@
    }
}
"#,
        &[Query::completion("after the assert", "after")],
        expect![[r#"
            after the assert
            - nodeOnly [method]  : void  writes `();`
            - shared [method]  : void  writes `();`
            - accepting the first leaves `$thing->nodeOnly`
        "#]],
    );
}

#[test]
fn a_guard_that_falls_through_offers_only_the_declaration() {
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

interface Thing
{
    public function shared(): void;
}

class Node implements Thing
{
    public function nodeOnly(): void {}
}

class Service
{
    public function run(Thing $thing): void
    {
        if (!($thing instanceof Node)) {
            $thing->shared();
        }

        $thing->@after@
    }
}
"#,
        &[Query::completion("no early exit", "after")],
        expect![[r#"
            no early exit
            - shared [method]  : void  writes `();`
            - accepting the first leaves `$thing->shared`
        "#]],
    );
}

#[test]
fn a_guard_narrows_the_expression_it_names_and_not_only_a_local() {
    // The shape a keyed cache of entities is always written in, and the guard is the only claim in
    // the file: `array $users` says nothing about what is in it.
    check(
        r#"
//- /src/User.php
<?php
namespace App;

class User
{
    public function hasPermission(string $permission): bool {}
}

//- /src/Proxy.php
<?php
namespace App;

class Proxy
{
    protected array $users = [];

    public function allowed(int $uid, string $permission): bool
    {
        if ($this->users[$uid] instanceof User) {
            return $this->users[$uid]->has@here@Permission($permission);
        }
        return false;
    }
}
"#,
        &[Query::goto("through the guard", "here")],
        expect![[r#"
            through the guard
            - method App\User::hasPermission @ src/User.php 6:21-6:34 (full 6:5-6:63)
        "#]],
    );
}

#[test]
fn a_guard_about_one_slot_says_nothing_about_another() {
    // The reason an element step carries the key it was written with. Both reads have the same
    // shape, and calling them the same slot would let a guard about one describe the other -- the
    // sort of nearly-right answer that is worse than none.
    check(
        r#"
//- /src/User.php
<?php
namespace App;

class User
{
    public function hasPermission(string $permission): bool {}
}

//- /src/Proxy.php
<?php
namespace App;

class Proxy
{
    protected array $users = [];

    public function allowed(int $one, int $other): bool
    {
        if ($this->users[$one] instanceof User) {
            return $this->users[$other]->has@here@Permission('x');
        }
        return false;
    }
}
"#,
        &[Query::goto("a different slot", "here")],
        expect![[r#"
            a different slot
            - (nothing to go to)
        "#]],
    );
}

#[test]
fn an_early_return_guard_narrows_an_expression_for_the_rest_of_the_method() {
    check(
        r#"
//- /src/User.php
<?php
namespace App;

class User
{
    public function hasPermission(string $permission): bool {}
}

//- /src/Proxy.php
<?php
namespace App;

class Proxy
{
    protected array $users = [];

    public function allowed(int $uid): bool
    {
        if (!($this->users[$uid] instanceof User)) {
            return false;
        }
        return $this->users[$uid]->has@here@Permission('x');
    }
}
"#,
        &[Query::goto("after the guard", "here")],
        expect![[r#"
            after the guard
            - method App\User::hasPermission @ src/User.php 6:21-6:34 (full 6:5-6:63)
        "#]],
    );
}

#[test]
fn a_guard_narrows_this_to_the_subclass_it_names() {
    // How PHP writes what other languages write with pattern matching: a base class asks what it
    // itself is and then calls something only the subclass has. Without the guard the call reads as
    // a method that does not exist, and the diagnostic says so -- which is the report that found
    // this.
    check(
        r#"
//- /src/app.php
<?php
namespace App;

class Base
{
    public function total(): void
    {
        if ($this instanceof Special) {
            $th@inside@is;
            $this->@member@
        }
        $th@outside@is;
    }
}

class Special extends Base
{
    public function bonus(): int
    {
        return 1;
    }
}
"#,
        &[
            Query::hover("inside the guard", "inside"),
            Query::completion("what the guard offers", "member"),
            Query::hover("after the guard", "outside"),
        ],
        expect![[r#"
            inside the guard
            - code: Special $this
            - note: narrowed by a guard
            what the guard offers
            - bonus [method]  : int  writes `();`
            - total [method]  : void  writes `();`
            - accepting the first leaves `$this->bonus`
            after the guard
            - code: Base $this
        "#]],
    );
}

#[test]
fn an_elseif_narrows_this_the_same_way_as_an_if() {
    check(
        r#"
//- /src/app.php
<?php
namespace App;

class Base
{
    public function total(): void
    {
        if ($this instanceof First) {
            $this->@first@
        }
        elseif ($this instanceof Second) {
            $this->@second@
        }
    }
}

class First extends Base
{
    public function one(): int
    {
        return 1;
    }
}

class Second extends Base
{
    public function two(): int
    {
        return 2;
    }
}
"#,
        &[
            Query::completion("the if branch", "first"),
            Query::completion("the elseif branch", "second"),
        ],
        expect![[r#"
            the if branch
            - one [method]  : int  writes `();`
            - total [method]  : void  writes `();`
            - accepting the first leaves `$this->one`
            the elseif branch
            - total [method]  : void  writes `();`
            - two [method]  : int  writes `();`
            - accepting the first leaves `$this->total`
        "#]],
    );
}

#[test]
fn a_guard_narrows_the_rest_of_the_condition_it_is_written_in() {
    // How the condition that needs a narrowing is actually written. Guards were only looked for in
    // the statements *around* a use, and this one is beside it -- on the other side of an `&&` in
    // the same expression.
    check(
        r#"
//- /src/app.php
<?php
namespace App;

class Base {}

class Special extends Base
{
    public function balance(): int
    {
        return 0;
    }
}

class Service
{
    public function run(Base $value, bool $ready): void
    {
        if ($value instanceof Special && $value->@and_right@balance() > 0) {
        }
        if ($ready && $value instanceof Special && $value->@and_chain@balance() > 0) {
        }
        if (!($value instanceof Special) || $value->@or_right@balance() > 0) {
        }
        if ($value instanceof Special || $value->@or_unproven@balance() > 0) {
        }
        if ($value->@left_operand@balance() && $value instanceof Special) {
        }
    }
}
"#,
        &[
            Query::completion("right of an and", "and_right"),
            Query::completion("right of a chain of ands", "and_chain"),
            Query::completion("right of an or, guarded by a negation", "or_right"),
            Query::completion("right of an or, proving nothing", "or_unproven"),
            Query::completion("left of the and that proves it", "left_operand"),
        ],
        expect![[r#"
            right of an and
            - balance [method]  : int  writes `()`
            - accepting the first leaves `if ($value instanceof Special && $value->balancebalance() > 0) {`
            right of a chain of ands
            - balance [method]  : int  writes `()`
            - accepting the first leaves `if ($ready && $value instanceof Special && $value->balancebalance() > 0) {`
            right of an or, guarded by a negation
            - balance [method]  : int  writes `()`
            - accepting the first leaves `if (!($value instanceof Special) || $value->balancebalance() > 0) {`
            right of an or, proving nothing
            - (nothing offered)
            left of the and that proves it
            - (nothing offered)
        "#]],
    );
}

#[test]
fn a_boolean_written_down_narrows_where_it_is_tested() {
    // How a condition tested in several places gets written. A report generator is nothing but this
    // shape -- one test at the top and a dozen uses below -- and until now every one of those uses
    // read as a method the base class does not have.
    check(
        r#"
//- /src/app.php
<?php
namespace App;

class Base {}

class Special extends Base
{
    public function balance(): int
    {
        return 0;
    }
}

class Service
{
    public function run(Base $value): void
    {
        $isSpecial = $value instanceof Special;
        if ($isSpecial) {
            $value->@flag@balance();
        }
        if (!$isSpecial) {
            $value->@negated@balance();
        }

        $indirect = $isSpecial;
        if ($indirect) {
            $value->@through_two@balance();
        }
    }
}
"#,
        &[
            Query::completion("tested through a flag", "flag"),
            Query::completion("the negated branch", "negated"),
            Query::completion("through two flags", "through_two"),
        ],
        expect![[r#"
            tested through a flag
            - balance [method]  : int  writes `()`
            - accepting the first leaves `$value->balancebalance();`
            the negated branch
            - (nothing offered)
            through two flags
            - balance [method]  : int  writes `()`
            - accepting the first leaves `$value->balancebalance();`
        "#]],
    );
}

#[test]
fn a_flag_reassigned_before_the_test_is_the_one_that_counts() {
    // The flag is read at the guard, so the assignment that reaches it is the last one before it --
    // the same rule every other binding here follows.
    check(
        r#"
//- /src/app.php
<?php
namespace App;

class Base {}

class First extends Base
{
    public function one(): int
    {
        return 1;
    }
}

class Second extends Base
{
    public function two(): int
    {
        return 2;
    }
}

class Service
{
    public function run(Base $value): void
    {
        $matches = $value instanceof First;
        $matches = $value instanceof Second;
        if ($matches) {
            $value->@latest@two();
        }
    }
}
"#,
        &[Query::completion("the last assignment wins", "latest")],
        expect![[r#"
            the last assignment wins
            - two [method]  : int  writes `()`
            - accepting the first leaves `$value->twotwo();`
        "#]],
    );
}

#[test]
fn a_ternary_narrows_the_arm_its_condition_selected() {
    // Where half the calls in a report generator are: `$isBulk ? round($app->fee(), 2) : ''`.
    // A guard was read from statements and from `&&`; a ternary is neither, and it is the shape a
    // row of a table gets written in.
    check(
        r#"
//- /src/app.php
<?php
namespace App;

class Base {}

class Special extends Base
{
    public function balance(): int
    {
        return 0;
    }
}

class Service
{
    public function run(Base $value): void
    {
        $rows = [
            $value instanceof Special ? $value->@direct@balance() : 0,
            !($value instanceof Special) ? 0 : $value->@negated_else@balance(),
            $value instanceof Special ? 0 : $value->@wrong_arm@balance(),
        ];

        $isSpecial = $value instanceof Special;
        $one = $isSpecial ? $value->@through_flag@balance() : 0;
    }
}
"#,
        &[
            Query::completion("the then arm", "direct"),
            Query::completion("the else arm of a negated condition", "negated_else"),
            Query::completion("the arm the condition did not select", "wrong_arm"),
            Query::completion("through a flag", "through_flag"),
        ],
        expect![[r#"
            the then arm
            - balance [method]  : int  writes `()`
            - accepting the first leaves `$value instanceof Special ? $value->balancebalance() : 0,`
            the else arm of a negated condition
            - balance [method]  : int  writes `()`
            - accepting the first leaves `!($value instanceof Special) ? 0 : $value->balancebalance(),`
            the arm the condition did not select
            - (nothing offered)
            through a flag
            - balance [method]  : int  writes `()`
            - accepting the first leaves `$one = $isSpecial ? $value->balancebalance() : 0;`
        "#]],
    );
}

/// A nullable local, with one marker filled in per test.
fn maybe(body: &str) -> String {
    format!(
        r#"
//- /src/app.php
<?php
namespace App;

class Repo
{{
    public function find(): void {{}}
}}

class Service
{{
    public function load(): ?Repo
    {{
    }}

    public function count(): ?int
    {{
    }}

    public function run(): void
    {{
        {body}
    }}
}}
"#
    )
}

#[test]
fn a_truthiness_guard_takes_the_null_off_a_nullable() {
    // The whole point of writing `if ($repo)`. Before this the hover inside the branch still said
    // `?Repo`, which is the declaration's answer rather than the branch's.
    check(
        &maybe(
            "$repo = $this->load();\n        if ($repo) {\n            $re@inside@po;\n        }",
        ),
        &[Query::hover("inside the guard", "inside")],
        expect![[r#"
            inside the guard
            - code: Repo $repo
            - note: narrowed by a guard
        "#]],
    );
}

#[test]
fn the_null_comes_back_after_the_branch() {
    check(
        &maybe("$repo = $this->load();\n        if ($repo) {\n        }\n        $re@after@po;"),
        &[Query::hover("after the branch", "after")],
        expect![[r#"
            after the branch
            - code: ?Repo $repo
            - note: assigned
        "#]],
    );
}

#[test]
fn a_negated_guard_on_a_nullable_object_proves_null() {
    // An object is always truthy in PHP, so null is the only other thing a `?Repo` holds.
    check(
        &maybe(
            "$repo = $this->load();\n        if (!$repo) {\n            $re@inside@po;\n        }",
        ),
        &[Query::hover("inside the negated guard", "inside")],
        expect![[r#"
            inside the negated guard
            - code: null $repo
            - note: narrowed by a guard
        "#]],
    );
}

#[test]
fn a_negated_guard_on_a_nullable_scalar_proves_nothing() {
    // Zero is falsy too, so `if (!$count)` on a `?int` says nothing about which of the two it is.
    // Answering `null` here would be a value a reader then acts on; what it still is, is the `?int`.
    check(
        &maybe(
            "$count = $this->count();\n        if (!$count) {\n            $co@inside@unt;\n        }",
        ),
        &[Query::hover("inside the negated guard", "inside")],
        expect![[r#"
            inside the negated guard
            - code: ?int $count
            - note: narrowed by a guard
        "#]],
    );
}

#[test]
fn an_early_return_takes_the_null_off_for_the_rest_of_the_body() {
    // How Drupal writes it: `if (!$item) { return NULL; }` and then the whole method assumes it.
    check(
        &maybe(
            "$repo = $this->load();\n        if (!$repo) {\n            return;\n        }\n        $re@after@po;",
        ),
        &[Query::hover("after the early return", "after")],
        expect![[r#"
            after the early return
            - code: Repo $repo
            - note: narrowed by a guard
        "#]],
    );
}

#[test]
fn the_right_of_an_and_is_inside_the_guard_too() {
    check(
        &maybe("$repo = $this->load();\n        $ok = $repo && $re@right@po;"),
        &[Query::hover("right of the and", "right")],
        expect![[r#"
            right of the and
            - code: Repo $repo
            - note: narrowed by a guard
        "#]],
    );
}

#[test]
fn the_else_branch_is_guarded_by_the_negation_of_the_condition() {
    // Half of every written-out guard. After `if (!$repo) { } else { ... }` the value is not null in
    // there, and only the then branch used to be offered anything.
    check(
        &maybe(
            "$repo = $this->load();\n        if (!$repo) {\n        }\n        else {\n            $re@in_else@po;\n        }",
        ),
        &[Query::hover("in the else of a negated guard", "in_else")],
        expect![[r#"
            in the else of a negated guard
            - code: Repo $repo
            - note: narrowed by a guard
        "#]],
    );
}

#[test]
fn the_else_of_a_positive_guard_proves_the_negation() {
    check(
        &maybe(
            "$repo = $this->load();\n        if ($repo) {\n        }\n        else {\n            $re@in_else@po;\n        }",
        ),
        &[Query::hover("in the else of a positive guard", "in_else")],
        expect![[r#"
            in the else of a positive guard
            - code: null $repo
            - note: narrowed by a guard
        "#]],
    );
}

#[test]
fn the_else_of_an_instanceof_proves_nothing() {
    // `$x instanceof T` being false names no class, so the else branch keeps what the declaration
    // said. Nothing to prove is not the same as a proof of nothing -- which is why `Base` has a
    // member here: without one the test could not tell those two apart.
    check(
        r#"
//- /src/app.php
<?php
namespace App;

class Base
{
    public function shared(): void {}
}

class Special extends Base
{
    public function only(): void {}
}

class Service
{
    public function run(Base $value): void
    {
        if ($value instanceof Special) {
        }
        else {
            $value->@in_else@
        }
    }
}
"#,
        &[Query::completion("in the else of an instanceof", "in_else")],
        expect![[r#"
            in the else of an instanceof
            - shared [method]  : void  writes `();`
            - accepting the first leaves `$value->shared`
        "#]],
    );
}

#[test]
fn a_method_that_asserts_its_own_subtype_narrows_the_guard() {
    // How a base class dispatches on its own subtype without an `instanceof` at every call site.
    // The tag is the only place the answer is written: the method body says
    // `return $this instanceof X`, and bodies are not read across files.
    check(
        r#"
//- /src/app.php
<?php
namespace App;

class Base
{
    /**
     * @phpstan-assert-if-true \App\Special $this
     */
    public function isSpecial(): bool
    {
        return $this instanceof Special;
    }

    public function run(Base $value): void
    {
        if ($value->isSpecial()) {
            $value->@here@
        }
    }
}

class Special extends Base
{
    public function only(): int
    {
        return 1;
    }
}
"#,
        &[Query::completion("inside the asserted guard", "here")],
        expect![[r#"
            inside the asserted guard
            - isSpecial [method]  : bool  writes `();`
            - only [method]  : int  writes `();`
            - run [method]  : void  writes `(|);`
            - accepting the first leaves `$value->isSpecial`
        "#]],
    );
}

#[test]
fn the_assertion_holds_for_this_as_well() {
    check(
        r#"
//- /src/app.php
<?php
namespace App;

class Base
{
    /**
     * @assert-if-true \App\Special $this
     */
    public function isSpecial(): bool
    {
        return $this instanceof Special;
    }

    public function total(): void
    {
        if ($this->isSpecial()) {
            $this->@here@
        }
    }
}

class Special extends Base
{
    public function only(): int
    {
        return 1;
    }
}
"#,
        &[Query::completion("through $this", "here")],
        expect![[r#"
            through $this
            - isSpecial [method]  : bool  writes `();`
            - only [method]  : int  writes `();`
            - total [method]  : void  writes `();`
            - accepting the first leaves `$this->isSpecial`
        "#]],
    );
}

#[test]
fn an_if_true_tag_proves_nothing_in_the_else_branch() {
    // The tag says what *one* return value proves. The other branch does not prove the negation,
    // which names no class anyway.
    check(
        r#"
//- /src/app.php
<?php
namespace App;

class Base
{
    public function shared(): void {}

    /**
     * @assert-if-true \App\Special $this
     */
    public function isSpecial(): bool
    {
        return $this instanceof Special;
    }

    public function run(Base $value): void
    {
        if ($value->isSpecial()) {
        }
        else {
            $value->@here@
        }
    }
}

class Special extends Base
{
    public function only(): int
    {
        return 1;
    }
}
"#,
        &[Query::completion("in the else branch", "here")],
        expect![[r#"
            in the else branch
            - isSpecial [method]  : bool  writes `();`
            - run [method]  : void  writes `(|);`
            - shared [method]  : void  writes `();`
            - accepting the first leaves `$value->isSpecial`
        "#]],
    );
}

#[test]
fn an_if_false_tag_narrows_the_negated_branch() {
    check(
        r#"
//- /src/app.php
<?php
namespace App;

class Base
{
    /**
     * @assert-if-false \App\Special $this
     */
    public function isOrdinary(): bool
    {
        return !($this instanceof Special);
    }

    public function run(Base $value): void
    {
        if (!$value->isOrdinary()) {
            $value->@here@
        }
    }
}

class Special extends Base
{
    public function only(): int
    {
        return 1;
    }
}
"#,
        &[Query::completion("where the call returned false", "here")],
        expect![[r#"
            where the call returned false
            - isOrdinary [method]  : bool  writes `();`
            - only [method]  : int  writes `();`
            - run [method]  : void  writes `(|);`
            - accepting the first leaves `$value->isOrdinary`
        "#]],
    );
}

/// A guard is not always a replacement. A value the signature typed as an unrelated *interface* is
/// both once the guard names a class, which is what every test double is -- and PHP has a type for
/// it.
#[test]
fn a_guard_on_an_interface_composes_rather_than_replaces() {
    check(
        r#"
//- /src/Mock.php
<?php
namespace App;

interface MockInterface
{
    public function shouldReceive(string $name): self;
}

class Payment
{
    public function canBeRefunded(): bool {}
}

class Test
{
    public function mock(string $class): MockInterface {}

    public function run(): void
    {
        $payment = $this->mock(Payment::class);
        assert($payment instanceof Payment);
        $payment->@both@
    }
}
"#,
        &[Query::completion("both halves of the double", "both")],
        expect![[r#"
            both halves of the double
            - shouldReceive [method]  : self  writes `(|);`
            - canBeRefunded [method]  : bool  writes `();`
            - accepting the first leaves `$payment->shouldReceive`
        "#]],
    );
}

/// Two unrelated classes cannot both be true, so there the guard is the answer: PHP has single
/// inheritance, and an intersection of two classes is a type no object can have.
#[test]
fn a_guard_on_a_sibling_class_replaces_rather_than_composes() {
    check(
        r#"
//- /src/Siblings.php
<?php
namespace App;

class Base
{
    public function baseMethod(): void {}
}

class Special extends Base
{
    public function onlyMethod(): void {}
}

class Service extends Base
{
    public function serviceMethod(): void {}

    public function run(Base $thing): void
    {
        assert($thing instanceof Special);
        $thing->@narrowed@
    }
}
"#,
        &[Query::completion("narrowed to the sibling", "narrowed")],
        expect![[r#"
            narrowed to the sibling
            - baseMethod [method]  : void  writes `();`
            - onlyMethod [method]  : void  writes `();`
            - accepting the first leaves `$thing->baseMethod`
        "#]],
    );
}

/// An `elseif` whose branch also leaves proves its own negation, and the code after the whole chain
/// is past both: `if (A) return; elseif (B) die();` leaves neither an A nor a B.
#[test]
fn an_elseif_that_leaves_narrows_as_well_as_the_if() {
    check(
        r#"
//- /src/Chain.php
<?php
namespace App;

class Foo
{
    public function fooMethod(): void {}
}

class Bar
{
    public function barMethod(): void {}
}

class Runner
{
    public function run(Foo|Bar|string $x): void
    {
        if (is_string($x)) {
            return;
        } elseif ($x instanceof Bar) {
            return;
        }

        $x->@member@
    }
}
"#,
        &[Query::completion("past both branches", "member")],
        expect![[r#"
            past both branches
            - fooMethod [method]  : void  writes `();`
            - accepting the first leaves `$x->fooMethod`
        "#]],
    );
}

#[test]
fn a_guard_on_an_assignment_narrows_the_name_it_wrote() {
    // `if (!($user = $this->load($id)) instanceof User) { return; }` is one statement doing two
    // things, and what the code after it reads is `$user`. The claim used to be recorded about the
    // call instead, so the variable kept the `?User` its declaration gave it.
    check(
        r#"
//- /app.php
<?php
class User
{
    public function id(): int {}
}

class Resource
{
    public function load(int $id): ?User {}

    public function run(int $id): void
    {
        if (!($user = $this->load($id)) instanceof User) {
            return;
        }
        $@here@user;
        $user->$0
    }
}
"#,
        &[
            Query::hover("after the guard", "here"),
            Query::completion("what it is now", "0"),
        ],
        expect![[r#"
            after the guard
            - code: User $user
            - note: narrowed by a guard
            what it is now
            - id [method]  : int  writes `();`
            - accepting the first leaves `$user->id`
        "#]],
    );
}

#[test]
fn a_null_test_on_an_assignment_narrows_the_name_it_wrote() {
    check(
        r#"
//- /app.php
<?php
class User
{
    public function id(): int {}
}

class Resource
{
    public function load(int $id): ?User {}

    public function run(int $id): void
    {
        if (($user = $this->load($id)) === NULL) {
            return;
        }
        $@here@user;
    }
}
"#,
        &[Query::hover("after the null test", "here")],
        expect![[r#"
            after the null test
            - code: User $user
            - note: narrowed by a guard
        "#]],
    );
}
