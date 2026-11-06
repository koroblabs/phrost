//! A guard on a collector nobody filled, and what that proves about the values that would have
//! filled it.
//!
//! ```php
//! $errors = [];
//! if ($shipping === NULL) {
//!     $errors[] = $this->dateError('shipping');
//! }
//! if ($errors !== []) {
//!     return $this->validationError($errors);
//! }
//! $application->setDueDate($shipping);
//! ```
//!
//! Read as two unrelated guards this reports a null that cannot arrive, which is what the reference
//! project's own code says in a comment. The step is exact: an append leaves the collector holding
//! something, so a collector that is empty here is one no append ran on, so every condition that
//! would have appended is false.
//!
//! What the rule refuses is most of what is here. Every mention of the collector has to be an
//! append -- a read, a removal or a by-reference argument all mean the emptiness has another
//! possible cause -- and only an append written as a direct statement of an arm names the arm's own
//! condition.

use expect_test::expect;

use super::diagnostics::check;

#[test]
fn a_collector_nothing_filled_rules_out_the_checks_that_would_have_filled_it() {
    check(
        r#"
//- /app.php
<?php

class Repo
{
    public function set(int $shipping, int $billing): void {}

    public function run(?int $shipping, ?int $billing): void
    {
        $errors = [];
        if ($shipping === NULL) {
            $errors[] = 'shipping';
        }
        if ($billing === NULL) {
            $errors[] = 'billing';
        }
        if ($errors !== []) {
            return;
        }

        $this->set($shipping, $billing);
    }
}
"#,
        expect![[r#""#]],
    );
}

#[test]
fn an_append_nested_deeper_loses_its_own_claim_and_keeps_the_others() {
    // The shape the report came with: a third check that only applies once the first two passed,
    // written one level in. What its condition proves is nobody's claim -- an empty collector rules
    // out the *pair* of conditions -- and the two plain checks beside it still hold.
    check(
        r#"
//- /app.php
<?php

class Repo
{
    public function set(int $shipping): void {}

    public function early(int $billing, int $shipping): bool { return TRUE; }

    public function run(?int $shipping, ?int $billing): void
    {
        $errors = [];
        if ($shipping === NULL) {
            $errors[] = 'shipping';
        }
        if ($shipping !== NULL && $billing !== NULL) {
            $limit = $this->early($billing, $shipping);
            if ($limit) {
                $errors[] = 'too early';
            }
        }
        if ($errors !== []) {
            return;
        }

        $this->set($shipping);
    }
}
"#,
        expect![[r#""#]],
    );
}

#[test]
fn every_spelling_of_the_test_says_the_same_thing() {
    check(
        r#"
//- /app.php
<?php

// A fixture has no standard library, and the point of this one is the spelling.
function count(array $held): int
{
    return 0;
}

class Repo
{
    public function set(int $a, int $b, int $c, int $d): void {}

    public function run(?int $a, ?int $b, ?int $c, ?int $d): void
    {
        $one = [];
        $two = [];
        $three = [];
        $four = [];
        if ($a === NULL) {
            $one[] = 'a';
        }
        if ($b === NULL) {
            $two[] = 'b';
        }
        if ($c === NULL) {
            $three[] = 'c';
        }
        if ($d === NULL) {
            $four[] = 'd';
        }
        if (!empty($one)) {
            return;
        }
        if ($two) {
            return;
        }
        if (count($three)) {
            return;
        }
        if ([] !== $four) {
            return;
        }

        $this->set($a, $b, $c, $d);
    }
}
"#,
        expect![[r#""#]],
    );
}

#[test]
fn a_chain_proves_its_first_arm_and_not_the_arms_below_it() {
    // The first arm's condition is claimed because an empty collector means that arm did not run.
    // The second arm's is not: ruling it out needs the arm above it ruled out first, which is a
    // claim the chain does not make. So `$a` is narrowed here and `$b` is left alone.
    check(
        r#"
//- /app.php
<?php

class Repo
{
    public function set(int $a, int $b): void {}

    public function run(?int $a, ?int $b): void
    {
        $errors = [];
        if ($a === NULL) {
            $errors[] = 'a';
        }
        elseif ($b === NULL) {
            $errors[] = 'b';
        }
        if ($errors !== []) {
            return;
        }

        $this->set($a, $b);
    }
}
"#,
        expect![[r#"
            20:24-20:26 warning [possibly-null-argument] $b takes int, and this may be null
        "#]],
    );
}

#[test]
fn a_guard_that_does_not_leave_proves_nothing() {
    // The whole of the inference is that control cannot get here with anything in the collector.
    check(
        r#"
//- /app.php
<?php

class Repo
{
    public function set(int $shipping): void {}

    public function log(string $what): void {}

    public function run(?int $shipping): void
    {
        $errors = [];
        if ($shipping === NULL) {
            $errors[] = 'shipping';
        }
        if ($errors !== []) {
            $this->log('invalid');
        }

        $this->set($shipping);
    }
}
"#,
        expect![[r#"
            19:20-19:29 warning [possibly-null-argument] $shipping takes int, and this may be null
        "#]],
    );
}

#[test]
fn a_collector_something_takes_from_proves_nothing() {
    // `array_pop` is why every mention has to be an append: with one of these in the way, an empty
    // collector no longer says that nothing was put in it.
    check(
        r#"
//- /app.php
<?php

function array_pop(array &$held): mixed
{
    return NULL;
}

class Repo
{
    public function set(int $shipping): void {}

    public function run(?int $shipping): void
    {
        $errors = [];
        if ($shipping === NULL) {
            $errors[] = 'shipping';
        }
        array_pop($errors);
        if ($errors !== []) {
            return;
        }

        $this->set($shipping);
    }
}
"#,
        expect![[r#"
            23:20-23:29 warning [possibly-null-argument] $shipping takes int, and this may be null
        "#]],
    );
}

#[test]
fn a_collector_emptied_after_the_append_forgets_it() {
    check(
        r#"
//- /app.php
<?php

class Repo
{
    public function set(int $shipping): void {}

    public function run(?int $shipping): void
    {
        $errors = [];
        if ($shipping === NULL) {
            $errors[] = 'shipping';
        }
        $errors = [];
        if ($errors !== []) {
            return;
        }

        $this->set($shipping);
    }
}
"#,
        expect![[r#"
            18:20-18:29 warning [possibly-null-argument] $shipping takes int, and this may be null
        "#]],
    );
}

#[test]
fn a_value_written_after_the_check_is_not_the_value_the_check_saw() {
    // What an empty collector rules out is the value the *check* looked at. Anything written
    // between the two is a different value, and the complement says nothing about it.
    check(
        r#"
//- /app.php
<?php

class Repo
{
    public function set(int $shipping): void {}

    public function maybe(): ?int
    {
        return NULL;
    }

    public function run(?int $shipping): void
    {
        $errors = [];
        if ($shipping === NULL) {
            $errors[] = 'shipping';
        }
        $shipping = $this->maybe();
        if ($errors !== []) {
            return;
        }

        $this->set($shipping);
    }
}
"#,
        expect![[r#"
            23:20-23:29 warning [possibly-null-argument] $shipping takes int, and this may be null
        "#]],
    );
}

#[test]
fn a_collector_replaced_forgets_what_is_above_it_and_not_what_is_below() {
    // The reset is about the claims made before it: what it was given may be non-empty, and if it
    // was then the guard leaves and nothing here runs -- so an append written after it still proves
    // its own condition false.
    check(
        r#"
//- /app.php
<?php

class Repo
{
    public function set(int $a, int $b): void {}

    public function held(): array
    {
        return [];
    }

    public function run(?int $a, ?int $b): void
    {
        $errors = [];
        if ($a === NULL) {
            $errors[] = 'a';
        }
        $errors = $this->held();
        if ($b === NULL) {
            $errors[] = 'b';
        }
        if ($errors !== []) {
            return;
        }

        $this->set($a, $b);
    }
}
"#,
        expect![[r#"
            26:20-26:22 warning [possibly-null-argument] $a takes int, and this may be null
        "#]],
    );
}

#[test]
fn a_guard_about_another_collector_says_nothing_about_this_one() {
    check(
        r#"
//- /app.php
<?php

class Repo
{
    public function set(int $shipping): void {}

    public function run(?int $shipping, array $warnings): void
    {
        $errors = [];
        if ($shipping === NULL) {
            $errors[] = 'shipping';
        }
        if ($warnings !== []) {
            return;
        }

        $this->set($shipping);
    }
}
"#,
        expect![[r#"
            9:9-9:16 warning [unused-variable] $errors is assigned and never read
            17:20-17:29 warning [possibly-null-argument] $shipping takes int, and this may be null
        "#]],
    );
}
