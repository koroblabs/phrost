//! That completion and goto resolve a receiver to the same class.
//!
//! Two entry points read a receiver, and they have to: at a completion site the code is incomplete,
//! so `$this->` with nothing after it has no expression in the body to look up, and the receiver has
//! to be read from the text instead. What must not differ is the *answer*. Every link of both is
//! resolved by one shared method -- `this_class`, `variable_class`, `member_class` -- and this file
//! is what says so out loud.
//!
//! It exists because the two did diverge once. `$this` gained `instanceof` narrowing on the body
//! path, goto and hover started answering inside a guard, and completion went on offering the base
//! class's members for another hour. Nothing failed. One fixture, both queries, and it would have.

use expect_test::expect;

use super::{Query, check};

/// Every root a receiver can have, each written twice: once with the member typed so goto can be
/// asked, once with the cursor where completion fires.
const EVERY_ROOT: &str = r#"
//- /src/app.php
<?php
namespace App;

class Base
{
    protected Repo $repo;

    public static function make(): static
    {
    }

    public function inherited(): void
    {
    }
}

class Repo
{
    public static function all(): void
    {
    }

    public function find(): void
    {
    }
}

class Special extends Base
{
    public function only(): void
    {
    }
}

class Service extends Base
{
    public function run(Repo $given): void
    {
        $this->inhe@this_complete@rited();
        $this->inherited@this_goto@();

        self::ma@self_complete@ke();
        self::make@self_goto@();

        static::ma@static_complete@ke();
        static::make@static_goto@();

        parent::inhe@parent_complete@rited();
        parent::inherited@parent_goto@();

        $given->fi@local_complete@nd();
        $given->find@local_goto@();

        Repo::al@written_complete@l();
        Repo::all@written_goto@();

        $this->repo->fi@step_complete@nd();
        $this->repo->find@step_goto@();

        if ($this instanceof Special) {
            $this->on@narrowed_complete@ly();
            $this->only@narrowed_goto@();
        }
    }
}
"#;

#[test]
fn both_paths_reach_the_same_member_from_every_kind_of_receiver() {
    check(
        EVERY_ROOT,
        &[
            Query::completion("this, completed", "this_complete"),
            Query::goto("this, goto", "this_goto"),
            Query::completion("self, completed", "self_complete"),
            Query::goto("self, goto", "self_goto"),
            Query::completion("static, completed", "static_complete"),
            Query::goto("static, goto", "static_goto"),
            Query::completion("parent, completed", "parent_complete"),
            Query::goto("parent, goto", "parent_goto"),
            Query::completion("a local, completed", "local_complete"),
            Query::goto("a local, goto", "local_goto"),
            Query::completion("a written class, completed", "written_complete"),
            Query::goto("a written class, goto", "written_goto"),
            Query::completion("a property step, completed", "step_complete"),
            Query::goto("a property step, goto", "step_goto"),
            Query::completion("narrowed by a guard, completed", "narrowed_complete"),
            Query::goto("narrowed by a guard, goto", "narrowed_goto"),
        ],
        expect![[r#"
            this, completed
            - inherited [method]  : void  writes `()`
            - ... and the client must ask again
            - accepting the first leaves `$this->inheritedrited();`
            this, goto
            - method App\Base::inherited @ src/app.php 12:21-12:30 (full 12:5-14:6)
            self, completed
            - make [method]  : static  writes `()`
            - ... and the client must ask again
            - accepting the first leaves `self::makeke();`
            self, goto
            - method App\Base::make @ src/app.php 8:28-8:32 (full 8:5-10:6)
            static, completed
            - make [method]  : static  writes `()`
            - ... and the client must ask again
            - accepting the first leaves `static::makeke();`
            static, goto
            - method App\Base::make @ src/app.php 8:28-8:32 (full 8:5-10:6)
            parent, completed
            - inherited [method]  : void  writes `()`
            - ... and the client must ask again
            - accepting the first leaves `parent::inheritedrited();`
            parent, goto
            - method App\Base::inherited @ src/app.php 12:21-12:30 (full 12:5-14:6)
            a local, completed
            - find [method]  : void  writes `()`
            - ... and the client must ask again
            - accepting the first leaves `$given->findnd();`
            a local, goto
            - method App\Repo::find @ src/app.php 23:21-23:25 (full 23:5-25:6)
            a written class, completed
            - all [method]  : void  writes `()`
            - ... and the client must ask again
            - accepting the first leaves `Repo::alll();`
            a written class, goto
            - method App\Repo::all @ src/app.php 19:28-19:31 (full 19:5-21:6)
            a property step, completed
            - find [method]  : void  writes `()`
            - ... and the client must ask again
            - accepting the first leaves `$this->repo->findnd();`
            a property step, goto
            - method App\Repo::find @ src/app.php 23:21-23:25 (full 23:5-25:6)
            narrowed by a guard, completed
            - only [method]  : void  writes `()`
            - ... and the client must ask again
            - accepting the first leaves `$this->onlyly();`
            narrowed by a guard, goto
            - method App\Special::only @ src/app.php 30:21-30:25 (full 30:5-32:6)
        "#]],
    );
}

#[test]
fn self_is_not_narrowed_by_a_guard_on_this() {
    // The reason `$this` and `self` are separate roots at all. `self::` means the class the code is
    // written in whichever subclass the value turns out to be, so a call to something only the
    // subclass declares is a mistake -- and narrowing `self` would hide it.
    check(
        r#"
//- /src/app.php
<?php
namespace App;

class Base
{
    public function run(): void
    {
        if ($this instanceof Special) {
            $this->on@through_this@ly();
            self::on@through_self@ly();
        }
    }
}

class Special extends Base
{
    public static function only(): void
    {
    }
}
"#,
        &[
            Query::completion("through $this, narrowed", "through_this"),
            Query::completion("through self, not narrowed", "through_self"),
        ],
        expect![[r#"
            through $this, narrowed
            - only [method]  : void  writes `()`
            - ... and the client must ask again
            - accepting the first leaves `$this->onlyly();`
            through self, not narrowed
            - (nothing offered)
        "#]],
    );
}

/// A union operand that names no class, and a call anchored with `namespace\`.
///
/// Two ways the two paths came apart, both found by
/// `tools/completion-offers-what-is-written.py` -- which asks whether completion offers the member
/// the code at that very position already writes, and so cannot be answered by a project's own
/// imprecision.
///
/// `@return ParameterBag|mixed` is how Laravel declares `Request::json()`, and it is an ordinary
/// docblock: one operand names a class and the other names anything. Goto through it has always
/// answered; completion required *every* operand to name a class and so offered nothing, which is
/// what an editor shows when a server is broken. The reading is the one `ReceiverClasses::Any`
/// already describes -- the value is one of these, and only what is named has members to reach.
///
/// The second is a spelling: `namespace\here()` anchors the name to the file's own namespace, and
/// the function half of the receiver walk passed the written name on with the marker still in it
/// while taking the anchor off it. The class half has read both since `\Drupal::`.
#[test]
fn a_union_naming_one_class_and_a_namespace_anchored_call_agree_too() {
    check(
        r#"
//- /src/app.php
<?php
namespace App;

class Row
{
    public function value(): string
    {
    }
}

function make(): Row
{
}

class Service
{
    /**
     * @return Row|mixed
     */
    public function loose()
    {
    }

    /**
     * @return Row|string
     */
    public function scalar()
    {
    }

    public function run(): void
    {
        $this->loose()->val@loose_complete@ue();
        $this->loose()->value@loose_goto@();

        $this->scalar()->val@scalar_complete@ue();
        $this->scalar()->value@scalar_goto@();

        namespace\make()->val@anchored_complete@ue();
        namespace\make()->value@anchored_goto@();
    }
}
"#,
        &[
            Query::completion("a mixed operand, completed", "loose_complete"),
            Query::goto("a mixed operand, goto", "loose_goto"),
            Query::completion("a scalar operand, completed", "scalar_complete"),
            Query::goto("a scalar operand, goto", "scalar_goto"),
            Query::completion("anchored to the namespace, completed", "anchored_complete"),
            Query::goto("anchored to the namespace, goto", "anchored_goto"),
        ],
        expect![[r#"
            a mixed operand, completed
            - value [method]  : string — Row only  writes `()`
            - ... and the client must ask again
            - accepting the first leaves `$this->loose()->valueue();`
            a mixed operand, goto
            - method App\Row::value @ src/app.php 6:21-6:26 (full 6:5-8:6)
            a scalar operand, completed
            - value [method]  : string — Row only  writes `()`
            - ... and the client must ask again
            - accepting the first leaves `$this->scalar()->valueue();`
            a scalar operand, goto
            - method App\Row::value @ src/app.php 6:21-6:26 (full 6:5-8:6)
            anchored to the namespace, completed
            - value [method]  : string  writes `()`
            - ... and the client must ask again
            - accepting the first leaves `namespace\make()->valueue();`
            anchored to the namespace, goto
            - method App\Row::value @ src/app.php 6:21-6:26 (full 6:5-8:6)
        "#]],
    );
}

/// A conditional `@return`, which the two paths read in different places.
///
/// `($key is null ? InputBag : mixed)` is how Laravel declares `Request::json()`, and the branch is
/// settled by what the *call* wrote -- the parameter left out, or a string passed. The path layer
/// has read the contract since it existed; the completion chain never did, so it fell through to
/// the declaration, which such a method does not have, and then to the body. Laravel's returns a
/// property, so `$request->json()->` offered nothing while goto on the very name after it answered.
///
/// The second shape is Mockery's `expects()`, whose branch turns on a *written* argument being a
/// string: `$mock->expects('name')->with(...)` is nine sites in one sample of Laravel's tests, and
/// the scanner reading the quoted argument is the whole of what settles it. A text scan can read a
/// string and nothing else, which is exactly the one thing this test needs it to.
#[test]
fn a_conditional_return_reaches_the_same_branch_from_both_paths() {
    check(
        r#"
//- /src/app.php
<?php
namespace App;

class Bag
{
    public function held(): string
    {
    }
}

class Loud
{
    public function shouted(): string
    {
    }
}

class Service
{
    /**
     * @param string|null $key
     * @param mixed $default
     * @return ($key is null ? Bag : mixed)
     */
    public function read($key = null, $default = null)
    {
        return $default;
    }

    /**
     * @param mixed $what
     * @return ($what is string ? Loud : Bag)
     */
    public function expects($what = null)
    {
    }

    public function run(): void
    {
        $this->read()->he@absent_complete@ld();
        $this->read()->held@absent_goto@();

        $this->expects('x')->sho@written_complete@uted();
        $this->expects('x')->shouted@written_goto@();
    }
}
"#,
        &[
            Query::completion("the argument left out, completed", "absent_complete"),
            Query::goto("the argument left out, goto", "absent_goto"),
            Query::completion("a string written, completed", "written_complete"),
            Query::goto("a string written, goto", "written_goto"),
        ],
        expect![[r#"
            the argument left out, completed
            - held [method]  : string  writes `()`
            - ... and the client must ask again
            - accepting the first leaves `$this->read()->heldld();`
            the argument left out, goto
            - method App\Bag::held @ src/app.php 6:21-6:25 (full 6:5-8:6)
            a string written, completed
            - shouted [method]  : string  writes `()`
            - ... and the client must ask again
            - accepting the first leaves `$this->expects('x')->shouteduted();`
            a string written, goto
            - method App\Loud::shouted @ src/app.php 13:21-13:28 (full 13:5-15:6)
        "#]],
    );
}
