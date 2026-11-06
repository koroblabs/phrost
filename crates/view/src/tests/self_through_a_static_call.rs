//! What `self` means in a return type reached through `Foo::m()` or `parent::m()`.
//!
//! `self` is the class the declaration was *written* in, always -- unlike `static`, which is the
//! class the call was made on. The two part company for exactly one spelling: a static call looks
//! the member up on one class while late static binding still points at another, and answering
//! `static`'s question for `self` offers a subclass's whole API on a value that is a base.
//!
//! Its own file beside `late_static_binding`, which pins the other half of the same pair: there the
//! forwarding is the rule and here it is the thing that must not happen.

use expect_test::expect;

use super::{Query, check_view};

/// The contrast is the whole test. Both calls reach one declaration, `parent::` through the parent
/// and `$this->` through the child, and `self` has to answer `Base` either way.
#[test]
fn self_through_parent_is_the_class_that_declared_it() {
    check_view(
        r#"
//- /src/Base.php
<?php
namespace App;

class Base
{
    public function itself(): self
    {
        return $this;
    }
}

//- /src/Child.php
<?php
namespace App;

class Child extends Base
{
    public function probe(): void
    {
        $through_parent = parent::itself();
        $throu@parent@gh_parent;
    }

    public function beside(): void
    {
        $through_this = $this->itself();
        $throu@this@gh_this;
    }
}
"#,
        &[
            Query::variable_class("parent::itself()", "parent", "through_parent"),
            Query::variable_class("$this->itself()", "this", "through_this"),
        ],
        expect![[r#"
            parent::itself()
            - App\Base
            $this->itself()
            - App\Base
        "#]],
    );
}

/// `self` in a grandparent is the grandparent, however many classes the lookup passed on the way.
#[test]
fn self_in_a_grandparent_is_the_grandparent() {
    check_view(
        r#"
//- /src/Root.php
<?php
namespace App;

class Root
{
    public function itself(): self
    {
        return $this;
    }
}

//- /src/Middle.php
<?php
namespace App;

class Middle extends Root {}

//- /src/Leaf.php
<?php
namespace App;

class Leaf extends Middle
{
    public function probe(): void
    {
        $found = parent::itself();
        $fou@here@nd;
    }
}
"#,
        &[Query::variable_class("three deep", "here", "found")],
        expect![[r#"
            three deep
            - App\Root
        "#]],
    );
}

/// The half that keeps the rule from being "always the parent": `static` through a forwarding call
/// is still the calling class, and `parent` in a return type is still the declarer's parent.
#[test]
fn static_and_parent_keep_their_own_answers_through_the_same_call() {
    check_view(
        r#"
//- /src/Root.php
<?php
namespace App;

class Root {}

//- /src/Base.php
<?php
namespace App;

class Base extends Root
{
    public static function make(): static
    {
        return new static();
    }

    public function up(): parent
    {
        return $this;
    }
}

//- /src/Child.php
<?php
namespace App;

class Child extends Base
{
    public function probe(): void
    {
        $late = parent::make();
        $la@late@te;
        $above = parent::up();
        $abo@above@ve;
    }
}
"#,
        &[
            Query::variable_class("static forwards", "late", "late"),
            Query::variable_class("parent is the declarer's", "above", "above"),
        ],
        expect![[r#"
            static forwards
            - App\Child
            parent is the declarer's
            - App\Root
        "#]],
    );
}

/// An explicit class name is not a forwarding call, so nothing about it depends on the caller.
#[test]
fn self_through_a_named_class_is_that_class() {
    check_view(
        r#"
//- /src/Base.php
<?php
namespace App;

class Base
{
    public function itself(): self
    {
        return $this;
    }
}

//- /src/Child.php
<?php
namespace App;

class Child extends Base
{
    public function probe(): void
    {
        $named = Base::itself();
        $nam@here@ed;
    }
}
"#,
        &[Query::variable_class("named outright", "here", "named")],
        expect![[r#"
            named outright
            - App\Base
        "#]],
    );
}
