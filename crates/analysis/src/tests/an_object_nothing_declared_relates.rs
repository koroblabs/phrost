//! An object handed where a class or interface it has no declared relation to is expected.
//!
//! `relate` used to answer `Unknown` whenever either side was an interface, on the grounds that some
//! class somewhere could implement both. PHPStan, at level 5, and PHP's own `TypeError` go by what is
//! declared, and so does this now: neither below the other is a rejection. What stays silent is a
//! downcast -- a parent where a child is asked for, which may be one at runtime -- and an ancestry
//! the index cannot see all of.

use expect_test::expect;

use super::diagnostics::check;

#[test]
fn an_unrelated_interface_is_reported() {
    check(
        r#"
//- /app.php
<?php
interface Reader {}
interface Writer {}

function wants(Writer $w): void {}

function run(Reader $r): void
{
    wants($r);
}
"#,
        expect![[r#"
            9:11-9:13 error [incompatible-argument] $w takes \Writer, and this is Reader
        "#]],
    );
}

#[test]
fn a_class_that_does_not_implement_the_interface_is_reported() {
    check(
        r#"
//- /app.php
<?php
interface Writer {}
class File {}

function wants(Writer $w): void {}

function run(File $f): void
{
    wants($f);
}
"#,
        expect![[r#"
            9:11-9:13 error [incompatible-argument] $w takes \Writer, and this is File
        "#]],
    );
}

/// An interface declaring `__toString()` extends `Stringable` without saying so -- PHP 8 adds it, and
/// `is_subclass_of('CustomStringable', 'Stringable')` is true. Silent, because it is one.
#[test]
fn an_interface_declaring_to_string_is_stringable() {
    check(
        r#"
//- /stubs.php
<?php
interface Stringable
{
    public function __toString(): string;
}

//- /app.php
<?php
interface CustomStringable
{
    public function __toString(): string;
}

function wants(\Stringable $s): void {}

function run(CustomStringable $value): void
{
    wants($value);
}
"#,
        expect![""],
    );
}

/// And a class whose `__toString()` comes from a trait, which PHP makes `Stringable` the same way.
/// Silent, because it is one.
#[test]
fn a_class_whose_trait_declares_to_string_is_stringable() {
    check(
        r#"
//- /stubs.php
<?php
interface Stringable
{
    public function __toString(): string;
}

//- /app.php
<?php
trait Printable
{
    public function __toString(): string { return ''; }
}

class Invoice
{
    use Printable;
}

function wants(\Stringable $s): void {}

function run(Invoice $invoice): void
{
    wants($invoice);
}
"#,
        expect![""],
    );
}

#[test]
fn an_interface_without_to_string_is_not_stringable() {
    check(
        r#"
//- /stubs.php
<?php
interface Stringable
{
    public function __toString(): string;
}

//- /app.php
<?php
interface Named
{
    public function name(): string;
}

function wants(\Stringable $s): void {}

function run(Named $value): void
{
    wants($value);
}
"#,
        expect![[r#"
            11:11-11:17 error [incompatible-argument] $s takes \Stringable, and this is Named
        "#]],
    );
}

/// A parent where a child is asked for may be the child at runtime. The one leniency kept, and
/// silent for that reason.
#[test]
fn a_downcast_is_left_alone() {
    check(
        r#"
//- /app.php
<?php
interface Shape {}
final class Circle implements Shape {}

function wants(Circle $c): void {}

function run(Shape $s): void
{
    wants($s);
}
"#,
        expect![""],
    );
}

/// `Base` extends a class nothing declares, which may well be the one implementing `Writer`. The
/// missing class is reported for what it is and the call is not, because the declared ancestry is
/// not all of it.
#[test]
fn an_ancestry_the_index_cannot_see_is_left_alone() {
    check(
        r#"
//- /app.php
<?php
interface Writer {}
class Base extends \Vendor\NotInstalled {}

function wants(Writer $w): void {}

function run(Base $b): void
{
    wants($b);
}
"#,
        expect![[r#"
            3:20-3:40 error [undefined-type] Undefined type Vendor\NotInstalled
        "#]],
    );
}
