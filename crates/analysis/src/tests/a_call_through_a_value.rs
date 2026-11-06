//! `$fn(...)`, PHP's fourth shape of call and the one nothing had added.
//!
//! `phrost_body::site_of` matched a method call, a static call, a function call and a `new`, and
//! ended `_ => return None` -- so `ExprData::IndirectCall`, which lowering has always produced and
//! six other readers already use, was not a call site. Every check about what a call *passes* went
//! silent at the first closure, `$fn(` offered no signature help and no parameter hint, and on the
//! reference project it is what `$digitsOnly($customer->getPhone()['number'])` is.
//!
//! What makes it answerable is that a closure written in the same file carries its parameters in
//! that file's own arena -- so they can never be interned, and the checks have to read an
//! expectation in either spelling. `crate::compat::Expectation` is the seam that already exists for
//! exactly that, and `phrost_view::parameters::Parameter` is where the two meet.
//!
//! Neither count check speaks here, and PHP is why in both directions: measured on 8.5.10, extra
//! arguments to a function, a closure and a method are all silently ignored, so the upper bound was
//! never a runtime claim -- and the lower one would be a claim about a callee identified through a
//! chain of assignments, which is the least certain callee there is.

use expect_test::expect;

use super::diagnostics::{check, check_asked_for};

#[test]
fn a_closure_held_in_a_local_judges_what_it_is_called_with() {
    check(
        r#"
//- /src/app.php
<?php

class Phones
{
    public function number(): ?string
    {
        return NULL;
    }

    public function run(): string
    {
        $digitsOnly = static fn (string $value): string => $value;

        return $digitsOnly($this->number());
    }
}
"#,
        expect![[r#"
            14:28-14:43 warning [possibly-null-argument] $value takes string, and this may be null
        "#]],
    );
}

#[test]
fn an_invokable_object_is_judged_by_what_its_invoke_declares() {
    // PHP's own rule: `$handler($request)` is `$handler->__invoke($request)`. So the parameters are
    // a member lookup like any other -- and they are stored, which is why they arrive in the
    // vocabulary the checks already read rather than needing the written spelling a closure does.
    check(
        r#"
//- /src/app.php
<?php

class Handler
{
    public function __invoke(string $value): string
    {
        return $value;
    }
}

class Phones
{
    public function number(): ?string
    {
        return NULL;
    }

    public function run(Handler $fn): string
    {
        return $fn($this->number());
    }
}
"#,
        expect![[r#"
            20:20-20:35 warning [possibly-null-argument] $value takes string, and this may be null
        "#]],
    );
}

#[test]
fn a_first_class_callable_is_judged_by_the_declaration_it_was_made_from() {
    // `$fn = $this->takes(...)` is a `Closure` made from a declaration, and `Callable` keeps only
    // what invoking it returns -- but the *path* the binding wrote is the call that was never made,
    // so the callee is reachable from there. `$fn = strlen(...)` is the same shape with a free
    // function at the root instead of a method at the last step.
    check(
        r#"
//- /src/app.php
<?php

class Phones
{
    public function number(): ?string
    {
        return NULL;
    }

    public function takes(string $value): string
    {
        return $value;
    }

    public function run(): string
    {
        $fn = $this->takes(...);

        return $fn($this->number());
    }
}
"#,
        expect![[r#"
            19:20-19:35 warning [possibly-null-argument] $value takes string, and this may be null
        "#]],
    );
}

#[test]
fn a_callable_with_no_declaration_behind_it_says_nothing() {
    // The honest remaining limit, and it is a limit of what is knowable rather than of the rule: a
    // parameter typed as a callable is whatever the caller passed, and a guess there is how a wrong
    // parameter list gets into a report.
    check(
        r#"
//- /src/app.php
<?php

class Runner
{
    public function __invoke(callable $fn): string
    {
        return $fn('x');
    }
}
"#,
        expect![""],
    );
}

#[test]
fn neither_count_is_claimed_about_a_call_through_a_value() {
    // Laravel's `Http\Factory` is the shape this is written for: `$response = $callback;` then
    // `while ($response instanceof Closure) { $response = $response($request, $options); }`, where
    // the literal the chain lands on declares no parameters and the call passes two. PHP ignores
    // the extras, so there was never anything to report -- and the type checks degrade to silence
    // on a wrong identification where a count check asserts.
    check_asked_for(
        r#"
//- /src/app.php
<?php

class Calls
{
    public function run(): string
    {
        $none = static fn (): string => 'x';

        return $none('a', 'b');
    }
}
"#,
        &["extra-argument"],
        expect![""],
    );
}

#[test]
fn a_declared_callee_still_has_both_counts_claimed_about_it() {
    // The control for the test above: what was declined is the *invoked* shape, not the rule.
    check_asked_for(
        r#"
//- /src/app.php
<?php

class Calls
{
    public function none(): string
    {
        return 'x';
    }

    public function run(): string
    {
        return $this->none('a', 'b');
    }
}
"#,
        &["extra-argument"],
        expect![[r#"
            12:28-12:36 error [extra-argument] none() takes at most 0 arguments, 2 given
        "#]],
    );
}
