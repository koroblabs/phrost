//! The type a returned *variable* names, read from this body's assignments.
//!
//! A factory with more than one line assigns and then returns, and `returned_type` stopped at the
//! variable -- so every receiver downstream of the call was untyped. Symfony writes 492 methods of
//! this shape.
//!
//! Every test below the first is a refusal, because that is what makes the reading safe: a local has
//! a closed set of writers inside its own body, and anything this cannot read gives up the whole
//! answer rather than reasoning from the assignments it can see.

use expect_test::expect;

use super::check_hir;

#[test]
fn a_variable_a_single_new_gave_a_value_is_that_class() {
    check_hir(
        r#"
//- /src/C.php
<?php
class C
{
    public function make()
    {
        $thing = new Bar();
        $thing->configure();

        return $thing;
    }

    public function agreeing(bool $flag)
    {
        $thing = new Bar();
        if ($flag) {
            $thing = new Bar();
        }

        return $thing;
    }

    public function fluent()
    {
        $self = $this;

        return $self;
    }
}
"#,
        expect![[r#"
            class C @ 2:7-2:8
              method make() doc: Bar  public @ 4:21-4:25
              method agreeing(bool $flag) doc: Bar  public @ 12:21-12:29
              method fluent() doc: static  public @ 22:21-22:27
        "#]],
    );
}

#[test]
fn a_variable_something_else_could_have_bound_settles_nothing() {
    check_hir(
        r#"
//- /src/C.php
<?php
class C
{
    public function fromParameter(Bar $thing)
    {
        return $thing;
    }

    public function disagreeing(bool $flag)
    {
        $thing = new Bar();
        if ($flag) {
            $thing = new Baz();
        }

        return $thing;
    }

    public function rebound(array $rows)
    {
        $thing = new Bar();
        foreach ($rows as $thing) {
        }

        return $thing;
    }

    public function caught()
    {
        $thing = new Bar();
        try {
            $thing->run();
        }
        catch (\Throwable $thing) {
        }

        return $thing;
    }

    public function destructured(array $pair)
    {
        $thing = new Bar();
        [$first, $thing] = $pair;

        return $thing;
    }

    public function globalled()
    {
        global $thing;
        $thing = new Bar();

        return $thing;
    }

    public function unreadable()
    {
        $thing = $this->build();

        return $thing;
    }
}
"#,
        expect![[r#"
            class C @ 2:7-2:8
              method fromParameter(Bar $thing)  public @ 4:21-4:34
              method disagreeing(bool $flag)  public @ 9:21-9:32
              method rebound(array $rows)  public @ 19:21-19:28
              method caught()  public @ 28:21-28:27
              method destructured(array $pair)  public @ 40:21-40:33
              method globalled()  public @ 48:21-48:30
              method unreadable()  public @ 56:21-56:31
        "#]],
    );
}

#[test]
fn a_variable_a_closure_writes_is_the_closures_own() {
    // The scope rule every walk in this crate states, asked of an assignment: a `$thing` written
    // inside a closure is a different name, and reading it as this body's would answer about a
    // value this body never held.
    check_hir(
        r#"
//- /src/C.php
<?php
class C
{
    public function make()
    {
        $thing = new Bar();
        $later = function () {
            $thing = new Baz();
        };

        return $thing;
    }
}
"#,
        expect![[r#"
            class C @ 2:7-2:8
              method make() doc: Bar  public @ 4:21-4:25
        "#]],
    );
}
