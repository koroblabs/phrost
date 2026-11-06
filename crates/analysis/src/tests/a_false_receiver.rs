//! Reaching a member through a value that can be `false`.
//!
//! `reset($rows)` on an array nothing proved non-empty hands back `false`, and `false->label()` is
//! a fatal `Call to a member function label() on bool` exactly as a null receiver is. The check
//! exists because the type stopped lying about which sentinel it holds -- while `V|false` was folded
//! into `?V` this was reported as a possible *null*, which is the wrong guard to send anybody to
//! write.
//!
//! Mostly about the silence, for the same reason the nullable one is: every way PHP has of ruling
//! `false` out has to be understood one layer down, and a fixture per way is what says so.
//!
//! The *certain* case is not here. `non-object-receiver` already reports `false->label()` as the
//! error it is, because there PHP stops on every run.

use expect_test::expect;

use super::diagnostics::check;

/// A stub binding a template to an array's elements, with the `false` its else branch admits.
const RESET: &str = r#"
//- /stubs/standard.php  stub
<?php
/**
 * @template K
 * @template V
 * @param array<K, V> $array
 * @return ($array is non-empty-array ? V : V|false)
 */
function reset(object|array &$array): mixed {}
"#;

#[test]
fn an_unguarded_false_receiver_is_reported() {
    check(
        &[
            RESET,
            r#"
//- /app.php
<?php
class Role
{
    public string $name = '';
    public function label(): string
    {
        return '';
    }
}

class Listing
{
    /** @param Role[] $roles */
    public function run(array $roles): void
    {
        $role = reset($roles);
        echo $role->label();
        echo $role->name;
    }
}
"#,
        ]
        .concat(),
        expect![[r#"
            17:14-17:19 warning [possibly-false-receiver] Cannot call method label() on Role|false
            18:14-18:19 warning [possibly-false-receiver] Cannot access property $name on Role|false
        "#]],
    );
}

/// `?->` handles a null and does *not* handle a `false`: measured on PHP 8.5.10, `$v?->label()` on
/// a `false` is a fatal `Call to a member function label() on false`. So this is the one shape where
/// the two sentinels part company, and the only place the finding can come from.
#[test]
fn a_nullsafe_access_does_not_handle_a_false() {
    check(
        &[
            RESET,
            r#"
//- /app.php
<?php
class Role
{
    public function label(): string
    {
        return '';
    }
}

class Listing
{
    /** @param Role[] $roles */
    public function run(array $roles): void
    {
        $role = reset($roles);
        echo $role?->label();
    }
}
"#,
        ]
        .concat(),
        expect![[r#"
            16:14-16:19 warning [possibly-false-receiver] Cannot call method label() on Role|false
        "#]],
    );
}

/// Every way of ruling the `false` out, in one file, so a regression in any of them shows here.
///
/// The `instanceof` is the one that had to be earned: narrowing a union was written as a filter over
/// the alternatives, so a guard naming a *subtype* of what the operand held kept nothing and left
/// the union whole -- which is how a guard reads as ignored.
#[test]
fn every_guard_that_rules_the_false_out_is_silent() {
    check(
        &[
            RESET,
            r#"
//- /app.php
<?php
interface Named
{
    public function label(): string;
}

class Role implements Named
{
    public function label(): string
    {
        return '';
    }
}

class Listing
{
    /** @param Named[] $roles */
    public function strict(array $roles): void
    {
        $role = reset($roles);
        if ($role === FALSE) {
            return;
        }
        echo $role->label();
    }

    /** @param Named[] $roles */
    public function notFalse(array $roles): void
    {
        $role = reset($roles);
        if ($role !== FALSE) {
            echo $role->label();
        }
    }

    /** @param Named[] $roles */
    public function truthy(array $roles): void
    {
        $role = reset($roles);
        if ($role) {
            echo $role->label();
        }
    }

    /** @param Named[] $roles */
    public function negated(array $roles): void
    {
        $role = reset($roles);
        if (!$role) {
            return;
        }
        echo $role->label();
    }

    /** @param Named[] $roles */
    public function guardNamesASubtype(array $roles): void
    {
        $role = reset($roles);
        if ($role instanceof Role) {
            echo $role->label();
        }
    }

    /** @param Named[] $roles */
    public function nonEmptyIsProved(array $roles): void
    {
        if (empty($roles)) {
            return;
        }
        $role = reset($roles);
        echo $role->label();
    }
}
"#,
        ]
        .concat(),
        expect![""],
    );
}
