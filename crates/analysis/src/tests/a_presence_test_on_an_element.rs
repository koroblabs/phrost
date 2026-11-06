//! `isset($row['vin'])` says `$row` is there, not only that the element is.
//!
//! `isset` answers for a whole path rather than reading it -- a null anywhere along the way makes it
//! false instead of a warning -- so the guard holding rules out the container being null as surely
//! as it rules out the element. Without that, `isset($row['vin']) ? f($row['vin']) : NULL` reported
//! `possibly-null-subscript` on the then-branch: a guard the author wrote precisely to make the read
//! safe, and a finding that reads as the server not understanding `isset`.
//!
//! `??` is the same construct wearing another spelling. `is_array($row['vin'] ?? NULL)` cannot be
//! true unless the value came from the left operand, because the right one is the null -- so any
//! guard that rules a null out of the result has ruled out the container being null too.
//!
//! `empty` is the same construct asked the other way round, and both say the container is not the
//! `false` sentinel either: PHP answers `isset($false['k'])` with false and `empty($false['k'])` with
//! true, without a warning, so a guard that holds has ruled the `false` out as well as the null.
//!
//! Its neighbour `a_subscript_of_a_nullable` owns the other half: where there is no guard, the read
//! is a site and stays one.

use expect_test::expect;

use super::diagnostics::check;

#[test]
fn an_isset_on_an_element_guards_a_read_of_the_container() {
    check(
        r#"
//- /src/Rows.php
<?php
namespace App;

class Rows
{
    /** @return array{vin?: string}|null */
    public function row(): ?array
    {
        return null;
    }

    public function read(): ?string
    {
        $row = $this->row();

        return isset($row['vin']) ? $row['vin'] : NULL;
    }
}
"#,
        expect![[r#""#]],
    );
}

#[test]
fn a_coalesced_element_ruled_non_null_guards_the_container() {
    check(
        r#"
//- /src/functions.php
<?php
// The diagnostics fixture carries no standard library, and `type_predicate` matches on the name --
// so a declaration here is enough to make the guard the one PHP would run.
function is_array(mixed $value): bool { return TRUE; }

//- /src/Rows.php
<?php
namespace App;

class Rows
{
    /** @return array<string, mixed>|null */
    public function row(): ?array
    {
        return null;
    }

    public function read(): ?array
    {
        $row = $this->row();

        return is_array($row['parts'] ?? NULL) ? $row['parts'] : NULL;
    }
}
"#,
        expect![[r#""#]],
    );
}

/// The guard failing says nothing, which is the half that must not be claimed.
///
/// `isset($row['vin'])` being false leaves three states -- `$row` null, the key absent, the value
/// null -- and they are not one answer. So the else-branch reads the container at its own risk and
/// the site stands.
#[test]
fn the_failing_side_of_the_guard_proves_nothing() {
    check(
        r#"
//- /src/Rows.php
<?php
namespace App;

class Rows
{
    /** @return array{vin?: string}|null */
    public function row(): ?array
    {
        return null;
    }

    public function read(): ?string
    {
        $row = $this->row();
        if (isset($row['vin'])) {
            return NULL;
        }

        return $row['make'];
    }
}
"#,
        expect![[r#"
            19:16-19:20 warning [possibly-null-subscript] Cannot read element 'make' of ?array{vin?: string}
        "#]],
    );
}

/// `$this` is never null, so a guard over `$this->rows['a']` must not claim it is.
///
/// Not a precision question -- the claim is worthless either way -- but a termination one: resolving
/// a claim about `$this` asks what `$this` was narrowed to, which asks the guards again. Left
/// unguarded, a second such test in one condition took the check thread's stack down, which is an
/// abort rather than a panic and leaves no server to restart from.
#[test]
fn a_guard_over_a_property_does_not_claim_anything_about_this() {
    check(
        r#"
//- /src/Rows.php
<?php
namespace App;

class Rows
{
    private ?array $rows = NULL;

    public function read(): ?string
    {
        if (isset($this->rows['lat'], $this->rows['lon'])) {
            return (string) $this->rows['lat'];
        }

        return NULL;
    }
}
"#,
        expect![[r#""#]],
    );
}

/// `parse_url()` hands back `false` for a URL it cannot read, and `isset` on one of its keys is how
/// that is checked.
#[test]
fn an_isset_on_an_element_rules_out_a_false_container() {
    check(
        r#"
//- /src/Urls.php
<?php
namespace App;

class Urls
{
    /** @return array{host?: string}|false */
    public function parsed(): array|false
    {
        return false;
    }

    public function host(): ?string
    {
        $url = $this->parsed();

        return isset($url['host']) ? $url['host'] : NULL;
    }
}
"#,
        expect![[r#""#]],
    );
}

/// `if (empty($comment->comment_ID)) { return; }` -- the early exit WordPress writes under
/// everything that may not have been found -- leaves a container that is there, whether it was
/// missing as a null or as a `false`.
#[test]
fn an_empty_element_that_leaves_rules_out_null_and_false() {
    check(
        r#"
//- /src/Urls.php
<?php
namespace App;

class Urls
{
    /** @return array{host?: string}|false|null */
    public function parsed(): array|false|null
    {
        return null;
    }

    public function host(): string
    {
        $url = $this->parsed();
        if (empty($url['host'])) {
            return '';
        }

        return $url['host'];
    }
}
"#,
        expect![[r#""#]],
    );
}

/// `empty` holding says what `isset` failing says: nothing one answer covers.
#[test]
fn an_empty_element_that_holds_proves_nothing() {
    check(
        r#"
//- /src/Urls.php
<?php
namespace App;

class Urls
{
    /** @return array{host?: string, path?: string}|false */
    public function parsed(): array|false
    {
        return false;
    }

    public function path(): ?string
    {
        $url = $this->parsed();
        if (empty($url['host'])) {
            return $url['path'];
        }

        return NULL;
    }
}
"#,
        expect![[r#"
            16:20-16:24 warning [possibly-false-subscript] Cannot read element 'path' of array{host?: string, path?: string}|false
        "#]],
    );
}
