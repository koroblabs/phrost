//! `@return Reseller[]|array` says only `array`, and the element type is gone.
//!
//! Not a style note. `array` covers `Reseller[]`, so the union *is* `array` -- and on the reference
//! project one such tag left sixteen of its three hundred and fifty-two unresolved receivers with
//! nothing to resolve to: every `$reseller->id()` in every callback over what the method returns.
//! Written with the two spellings that occur, `T[]|array` and `array|T[]`, because an author who
//! writes one writes the other.

use expect_test::expect;

use super::diagnostics::check;

#[test]
fn a_bare_array_beside_one_that_states_its_elements_is_reported() {
    check(
        r#"
//- /src/app.php
<?php
namespace App;

class Reseller {}

class Owner
{
    /**
     * The resellers.
     *
     * @return Reseller[]|array
     */
    public function resellers(): array {}

    /**
     * The rows.
     *
     * @return array|array{id: int, name: string}
     */
    public function row(): array {}
}
"#,
        expect![[r#"
            13:21-13:30 warning [doc-redundant-type] @return Reseller[]|array says only array, which covers Reseller[] -- the array adds nothing Reseller[] does not already hold
            20:21-20:24 warning [doc-redundant-type] @return array|array{id: int, name: string} says only array, which covers array{id: int, name: string} -- array{} is the empty case it leaves out
        "#]],
    );
}

/// And a union that says one thing is left alone, however it is spelled.
#[test]
fn a_union_with_nothing_to_swallow_is_not_reported() {
    check(
        r#"
//- /src/app.php
<?php
namespace App;

class Reseller {}

class Owner
{
    /**
     * Elements only.
     *
     * @return Reseller[]
     */
    public function resellers(): array {}

    /**
     * A bare array only.
     *
     * @return array
     */
    public function anything(): array {}

    /**
     * Two array types, neither of which swallows the other.
     *
     * @return array{id: int}|array{name: string}
     */
    public function either(): array {}

    /**
     * A `|` that belongs to the generic rather than to the union.
     *
     * @return array<string, Reseller|null>
     */
    public function keyed(): array {}

    /**
     * An array beside something that is not one.
     *
     * @return array|null
     */
    public function maybe(): ?array {}
}
"#,
        expect![""],
    );
}

/// Which operand the report names for repair, and why the two answers are not interchangeable.
///
/// Reported from a real checkout: told that the `array` covers the shape, the author deleted the
/// `array`. That is the repair for `Reseller[]|array`, where the element type holds `[]` already, and
/// the wrong one for a shape with a required key -- the getters return the full shape or `[]` from
/// an early guard, so dropping the `array` promised keys that are not always there and **54** `?? ''`
/// at the call sites read as redundant to the checker their CI runs. `array{}` is what both wanted,
/// and it is what the message now names.
#[test]
fn the_repair_the_message_names_turns_on_whether_the_operand_holds_an_empty_array() {
    check(
        r#"
//- /src/app.php
<?php
namespace App;

class Reseller {}

class Owner
{
    /** @return array{id: int}|array */
    public function required(): array {}

    /** @return array{id?: int}|array */
    public function optional(): array {}

    /** @return array{int, string}|array */
    public function positional(): array {}

    /** @return array{id: int, ...}|array */
    public function sealedOpen(): array {}

    /** @return array{...}|array */
    public function unsealed(): array {}

    /** @return non-empty-list<Reseller>|array */
    public function nonEmpty(): array {}

    /** @return list<Reseller>|array */
    public function any(): array {}
}
"#,
        expect![[r#"
            9:21-9:29 warning [doc-redundant-type] @return array{id: int}|array says only array, which covers array{id: int} -- array{} is the empty case it leaves out
            12:21-12:29 warning [doc-redundant-type] @return array{id?: int}|array says only array, which covers array{id?: int} -- the array adds nothing array{id?: int} does not already hold
            15:21-15:31 warning [doc-redundant-type] @return array{int, string}|array says only array, which covers array{int, string} -- array{} is the empty case it leaves out
            18:21-18:31 warning [doc-redundant-type] @return array{id: int, ...}|array says only array, which covers array{id: int, ...} -- array{} is the empty case it leaves out
            21:21-21:29 warning [doc-redundant-type] @return array{...}|array says only array, which covers array{...} -- the array adds nothing array{...} does not already hold
            24:21-24:29 warning [doc-redundant-type] @return non-empty-list<Reseller>|array says only array, which covers non-empty-list<Reseller> -- array{} is the empty case it leaves out
            27:21-27:24 warning [doc-redundant-type] @return list<Reseller>|array says only array, which covers list<Reseller> -- the array adds nothing list<Reseller> does not already hold
        "#]],
    );
}
