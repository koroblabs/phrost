//! A member reached on a value declared `object`, asked for as `member-on-object`.
//!
//! `object $supplier` and then `$supplier->getAddress()`: nothing declares the method, and
//! PHPStan reports the call as `method.notFound` on `object`. Off unless asked, since a project
//! typing its parameters `object` does it everywhere; and quiet where the code already asked --
//! `method_exists`, `isset`, a read under its own `isset` -- and where an `object{...}` shape names
//! the property. An `isset` of one name vouches for that name and no other.

use expect_test::expect;

use super::diagnostics::{check, check_asked_for};

const FIXTURE: &str = r#"
//- /src/stubs.php
<?php
function method_exists(object|string $object_or_class, string $method): bool {}

//- /src/app.php
<?php
namespace App;

class Row
{
    public function id(): int { return 1; }
}

/** @return object{cid: string} */
function cached(): object {}

function format(object $supplier, Row $row): void
{
    $supplier->getAddress('city');
    echo $supplier->name;
    if (method_exists($supplier, 'getTitle')) {
        $supplier->getTitle();
    }
    if (isset($supplier->label)) {
        echo 'set';
    }
    echo isset($supplier->label) ? $supplier->label : '';
    echo isset($supplier->label) ? $supplier->title : '';
    echo cached()->cid;
    $row->id();
}
"#;

const ASKED: &[&str] = &["member-on-object"];

#[test]
fn a_member_on_object_is_reported_when_asked_for() {
    check_asked_for(
        FIXTURE,
        ASKED,
        expect![[r#"
            14:16-14:26 warning [member-on-object] Nothing declares getAddress() on object, which names no class
            15:21-15:25 warning [member-on-object] Nothing declares $name on object, which names no class
            23:47-23:52 warning [member-on-object] Nothing declares $title on object, which names no class
        "#]],
    );
}

#[test]
fn a_member_on_object_is_silent_unless_asked_for() {
    check(FIXTURE, expect![""]);
}
