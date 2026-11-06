//! Goto and hover on a member where it is *used*, not where it is declared.
//!
//! The most-used navigation there is, and the one that needs everything under it: a body to find the
//! site in, a receiver whose class can be worked out, and a flattened member table to look the name
//! up in. A miss anywhere gives nothing rather than a guess.

use expect_test::expect;

use super::{Query, check};

#[test]
fn a_method_call_on_this_navigates_to_the_method() {
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    /** Finds one. */
    public function find(int $id): void {}

    public function run(): void
    {
        $this->fi@call@nd(1);
    }
}
"#,
        &[Query::goto("goto", "call"), Query::hover("hover", "call")],
        expect![[r#"
            goto
            - method App\Repo::find @ src/Repo.php 7:21-7:25 (full 7:5-7:43)
            hover
            - code: <?php
            namespace App;

            class Repo
            {
                public function find(int $id): void
            }
            - ---
            - doc: Finds one.
        "#]],
    );
}

#[test]
fn a_call_through_a_property_chain_navigates_to_the_right_class() {
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    public function find(): void {}
}

class Service
{
    private Repo $backend;

    public function run(): void
    {
        $this->backend->fi@call@nd();
    }
}
"#,
        &[Query::goto("goto", "call")],
        expect![[r#"
            goto
            - method App\Repo::find @ src/Repo.php 6:21-6:25 (full 6:5-6:36)
        "#]],
    );
}

#[test]
fn a_property_read_navigates_to_its_declaration() {
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    public string $label = '';

    public function run(): void
    {
        $this->la@read@bel;
    }
}
"#,
        &[Query::goto("goto", "read")],
        expect![[r#"
            goto
            - property App\Repo::label @ src/Repo.php 6:19-6:25 (full 6:5-6:31)
        "#]],
    );
}

#[test]
fn a_call_on_an_inherited_method_lands_where_it_was_written() {
    // The point of a flattened table: the receiver's class does not declare it, and the answer is
    // the file that does.
    check(
        r#"
//- /src/Base.php
<?php
namespace App;

abstract class Base
{
    public function shared(): void {}
}

//- /src/Repo.php
<?php
namespace App;

class Repo extends Base
{
    public function run(): void
    {
        $this->sh@call@ared();
    }
}
"#,
        &[Query::goto("goto", "call")],
        expect![[r#"
            goto
            - method App\Base::shared @ src/Base.php 6:21-6:27 (full 6:5-6:38)
        "#]],
    );
}

#[test]
fn a_static_call_and_a_class_constant_are_reachable() {
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    public const string VERSION = '1';

    public static function make(): void {}

    public function run(): void
    {
        Repo::ma@call@ke();
        self::VER@constant@SION;
    }
}
"#,
        &[
            Query::goto("static call", "call"),
            Query::goto("class constant", "constant"),
        ],
        expect![[r#"
            static call
            - method App\Repo::make @ src/Repo.php 8:28-8:32 (full 8:5-8:43)
            class constant
            - const App\Repo::VERSION @ src/Repo.php 6:25-6:32 (full 6:5-6:39)
        "#]],
    );
}

#[test]
fn a_call_inside_a_closure_uses_the_closures_own_receiver() {
    check(
        r#"
//- /src/Two.php
<?php
namespace App;

class Alpha
{
    public function alphaOnly(): void {}
}

class Beta
{
    public function betaOnly(): void {}
}

class Two
{
    public function run(Alpha $item): void
    {
        $callback = function (Beta $item) {
            $item->beta@call@Only();
        };
    }
}
"#,
        &[Query::goto("goto", "call")],
        expect![[r#"
            goto
            - method App\Beta::betaOnly @ src/Two.php 11:21-11:29 (full 11:5-11:40)
        "#]],
    );
}

#[test]
fn a_call_wrapped_in_a_shape_that_is_not_modelled_is_still_reachable() {
    // `$this->attachmentTypes()[$id]` is an array access, which is not modelled -- and an unmodelled
    // node used to swallow its subtree, so the call inside had no node and goto found nothing.
    // Reported from real code, where this shape is everywhere: an index, a concatenation, a
    // ternary, a `match` arm.
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    public function attachmentTypes(): array {}

    public function run(string $id): void
    {
        $byIndex = $this->attachmentTy@index@pes()[$id];
        $byConcat = 'x' . $this->attachmentTy@concat@pes();
        $byTernary = $id ? $this->attachmentTy@ternary@pes() : [];
        $byMatch = match ($id) {
            default => $this->attachmentTy@arm@pes(),
        };
    }
}
"#,
        &[
            Query::goto("inside an index", "index"),
            Query::goto("inside a concatenation", "concat"),
            Query::goto("inside a ternary", "ternary"),
            Query::goto("inside a match arm", "arm"),
        ],
        expect![[r#"
            inside an index
            - method App\Repo::attachmentTypes @ src/Repo.php 6:21-6:36 (full 6:5-6:48)
            inside a concatenation
            - method App\Repo::attachmentTypes @ src/Repo.php 6:21-6:36 (full 6:5-6:48)
            inside a ternary
            - method App\Repo::attachmentTypes @ src/Repo.php 6:21-6:36 (full 6:5-6:48)
            inside a match arm
            - method App\Repo::attachmentTypes @ src/Repo.php 6:21-6:36 (full 6:5-6:48)
        "#]],
    );
}

#[test]
fn a_variable_assigned_inside_a_shape_that_is_not_modelled_is_still_bound() {
    // Same reason: an assignment inside a `switch` case is written in a statement nobody models, and
    // the local it binds still has to be offered and still has to have a type.
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    public function find(): void {}

    public function run(string $id): void
    {
        switch ($id) {
            default:
                $repo = new Repo();
                $repo->fi@call@nd();
        }
    }
}
"#,
        &[Query::goto("goto", "call")],
        expect![[r#"
            goto
            - method App\Repo::find @ src/Repo.php 6:21-6:25 (full 6:5-6:36)
        "#]],
    );
}

#[test]
fn a_call_on_a_receiver_of_unknown_type_goes_nowhere() {
    // Nothing says what `$thing` is. Landing on a same-named method of some other class would be a
    // navigation that looks deliberate and is not.
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    public function find(): void {}

    public function run($thing): void
    {
        $thing->fi@call@nd();
    }
}
"#,
        &[Query::goto("goto", "call")],
        expect![[r#"
            goto
            - (nothing to go to)
        "#]],
    );
}
