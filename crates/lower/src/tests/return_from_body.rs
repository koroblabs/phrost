//! The return type a body states when the signature does not.
//!
//! `public function create() { return new Bar(); }` says what it hands back as plainly as a hint
//! would. Read here, where the body is still in hand: it belongs to whichever file declares the
//! method, and a body is lowered only for an open buffer -- so a reader asking about a call into
//! another file has nothing to walk.

use expect_test::expect;

use super::check_hir;

#[test]
fn one_new_is_the_type_and_two_are_a_union() {
    check_hir(
        r#"
//- /src/C.php
<?php
class C
{
    public function one()
    {
        return new Bar();
    }

    public function branching(bool $flag)
    {
        if ($flag) {
            return new Bar();
        }
        return new Baz();
    }

    public function fluent()
    {
        return $this;
    }

    public function rows()
    {
        return [new Bar()];
    }

    public function nullable(bool $flag)
    {
        if ($flag) {
            return new Bar();
        }
        return NULL;
    }
}
"#,
        expect![[r#"
            class C @ 2:7-2:8
              method one() doc: Bar  public @ 4:21-4:24
              method branching(bool $flag) doc: Bar|Baz  public @ 9:21-9:30
              method fluent() doc: static  public @ 17:21-17:27
              method rows() doc: array<int, Bar>  public @ 22:21-22:25
              method nullable(bool $flag) doc: Bar|null  public @ 27:21-27:29
        "#]],
    );
}

/// What is written wins, and nothing is computed for it.
#[test]
fn a_declared_return_and_a_documented_one_are_left_alone() {
    check_hir(
        r#"
//- /src/C.php
<?php
class C
{
    public function declared(): Baz
    {
        return new Bar();
    }

    /** @return Baz */
    public function documented()
    {
        return new Bar();
    }
}
"#,
        expect![[r#"
            class C @ 2:7-2:8
              method declared(): Baz  public @ 4:21-4:29
              method documented() doc: Baz  public @ 10:21-10:31  [doc]
        "#]],
    );
}

/// A body this cannot read whole settles nothing: a union of the returns it *can* read is a claim
/// narrower than the code, and narrower is what a false positive is made of.
#[test]
fn one_unreadable_return_gives_up_the_whole_body() {
    check_hir(
        r#"
//- /src/C.php
<?php
class C
{
    public function mixedBag(bool $flag)
    {
        if ($flag) {
            return new Bar();
        }
        return $this->make();
    }

    public function nothingAtAll()
    {
        $this->run();
    }

    public function bare(bool $flag)
    {
        if ($flag) {
            return;
        }
        return new Bar();
    }
}
"#,
        expect![[r#"
            class C @ 2:7-2:8
              method mixedBag(bool $flag)  public @ 4:21-4:29
              method nothingAtAll()  public @ 12:21-12:33
              method bare(bool $flag)  public @ 17:21-17:25
        "#]],
    );
}

/// A closure returns from itself. Reading its `return` as the enclosing method's is a claim about a
/// value that method never produces.
#[test]
fn a_return_inside_a_closure_belongs_to_the_closure() {
    check_hir(
        r#"
//- /src/C.php
<?php
class C
{
    public function outer()
    {
        $fn = function () {
            return new Inner();
        };
        return new Outer();
    }

    public function onlyTheClosure()
    {
        $fn = fn () => new Inner();
    }
}
"#,
        expect![[r#"
            class C @ 2:7-2:8
              method outer() doc: Outer  public @ 4:21-4:26
              method onlyTheClosure()  public @ 12:21-12:35
        "#]],
    );
}

#[test]
fn two_returns_of_one_class_are_that_class_and_not_a_union_of_two_copies() {
    // Each `new Row()` allocates its own type reference and its own name reference, so the
    // agreement between them is about what was written rather than about which id it landed on.
    // Compared by id, the guard clause below reads as a method returning `Row|Row` and the literal
    // beside it as elements that disagree.
    check_hir(
        r#"
//- /src/C.php
<?php
class C
{
    public function either(bool $flag)
    {
        if ($flag) {
            return new Row();
        }
        return new Row();
    }

    public function pair()
    {
        return [new Row(), new Row()];
    }
}
"#,
        expect![[r#"
            class C @ 2:7-2:8
              method either(bool $flag) doc: Row  public @ 4:21-4:27
              method pair() doc: array<int, Row>  public @ 12:21-12:25
        "#]],
    );
}
