//! Whatever is written inside an expression stays reachable.
//!
//! One marker per shape that can hold another expression, because this is the project's most
//! repeated bug and its quietest: a shape that models what it is and forgets to descend leaves
//! everything inside it invisible, while the shape itself still resolves and every test still
//! passes. It has happened to an unmodelled node, to a cast, to an array literal, and to `&&`.
//!
//! The walk is exhaustive now, so a new shape cannot be added without deciding. This file is the
//! other half: proof that the decisions taken were the right way round.

use expect_test::expect;

use crate::tests::{Query, check};

#[test]
fn a_call_nested_in_any_shape_is_still_reachable() {
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    public function find(): ?Repo {}
}

//- /src/Holder.php
<?php
namespace App;

class Holder
{
    public function __construct(public ?Repo $repo) {}

    public static function make(?Repo $repo): static {}
}

//- /src/Uses.php
<?php
namespace App;

class Uses
{
    public function __construct(private Repo $repo) {}

    public function run(Repo $other): void
    {
        $and = $this->repo->fi@in_and@nd() && $other->fi@in_or@nd();
        $not = !$this->repo->fi@in_not@nd();
        $cast = (int) $this->repo->fi@in_cast@nd();
        $list = [$this->repo->fi@in_array@nd()];
        $named = new Holder(repo: $this->repo->fi@in_named@nd());
        $made = Holder::make($this->repo->fi@in_static_args@nd());
        $spread = Holder::make(...[$this->repo->fi@in_spread@nd()]);
        $is = $this->repo->fi@in_instanceof@nd() instanceof Repo;
        $picked = match ($this->repo->fi@in_subject@nd()) {
            default => $this->repo->fi@in_arm@nd(),
        };
        $indexed = [$this->repo->fi@in_index@nd()][0];
        $closure = function () use ($ot@in_capture@her) {};
        $class = Holder::class;
        $cl@in_class_target@ass::make(null);
        throw new \RuntimeException((string) $this->repo->fi@in_throw@nd());
    }
}
"#,
        &[
            Query::goto("left of &&", "in_and"),
            Query::goto("right of &&", "in_or"),
            Query::goto("inside !", "in_not"),
            Query::goto("inside a cast", "in_cast"),
            Query::goto("inside an array literal", "in_array"),
            Query::goto("a named argument", "in_named"),
            Query::goto("a static call's argument", "in_static_args"),
            Query::goto("a spread argument", "in_spread"),
            Query::goto("the value of instanceof", "in_instanceof"),
            Query::goto("a match subject", "in_subject"),
            Query::goto("a match arm", "in_arm"),
            Query::goto("inside an array access", "in_index"),
            Query::goto("a closure capture", "in_capture"),
            Query::goto("a class named by a variable", "in_class_target"),
            Query::goto("inside throw", "in_throw"),
        ],
        expect![[r#"
            left of &&
            - method App\Repo::find @ src/Repo.php 6:21-6:25 (full 6:5-6:37)
            right of &&
            - method App\Repo::find @ src/Repo.php 6:21-6:25 (full 6:5-6:37)
            inside !
            - method App\Repo::find @ src/Repo.php 6:21-6:25 (full 6:5-6:37)
            inside a cast
            - method App\Repo::find @ src/Repo.php 6:21-6:25 (full 6:5-6:37)
            inside an array literal
            - method App\Repo::find @ src/Repo.php 6:21-6:25 (full 6:5-6:37)
            a named argument
            - method App\Repo::find @ src/Repo.php 6:21-6:25 (full 6:5-6:37)
            a static call's argument
            - method App\Repo::find @ src/Repo.php 6:21-6:25 (full 6:5-6:37)
            a spread argument
            - method App\Repo::find @ src/Repo.php 6:21-6:25 (full 6:5-6:37)
            the value of instanceof
            - method App\Repo::find @ src/Repo.php 6:21-6:25 (full 6:5-6:37)
            a match subject
            - method App\Repo::find @ src/Repo.php 6:21-6:25 (full 6:5-6:37)
            a match arm
            - method App\Repo::find @ src/Repo.php 6:21-6:25 (full 6:5-6:37)
            inside an array access
            - method App\Repo::find @ src/Repo.php 6:21-6:25 (full 6:5-6:37)
            a closure capture
            - parameter other @ src/Uses.php 8:30-8:36 (full 8:30-8:36)
            a class named by a variable
            - variable class @ src/Uses.php 23:9-23:15 (full 23:9-23:15)
            inside throw
            - method App\Repo::find @ src/Repo.php 6:21-6:25 (full 6:5-6:37)
        "#]],
    );
}
