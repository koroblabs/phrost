//! A member the author already checked for at runtime.
//!
//! `method_exists` is PHP's only way of writing "this may be on a subclass", so a report inside the
//! guard is a report about the guard. Mostly about the boundary: a check that vouches for the wrong
//! value, or the wrong name, must not silence anything -- that would turn one written guard into a
//! licence for the whole file.

use expect_test::expect;

use super::diagnostics::check;

/// The check and the call in the branch it guards.
#[test]
fn a_guarded_member_is_not_reported() {
    check(
        r#"
//- /stubs/builtins.php  stub
<?php
function method_exists(object|string $object_or_class, string $method): bool {}
function property_exists(object|string $object_or_class, string $property): bool {}

//- /app.php
<?php
class Offer
{
    public function data(): array
    {
        $values = [];
        if (method_exists($this, 'getWarranty')) {
            $values['coverage'] = $this->getWarranty();
        }
        return $values;
    }
}
"#,
        expect![""],
    );
}

/// The check and the call in the same `&&`, which is how core PHP libraries are used:
/// `method_exists($e, 'getResponse') && $e->getResponse()`.
#[test]
fn a_check_beside_the_call_in_one_condition_counts() {
    check(
        r#"
//- /stubs/builtins.php  stub
<?php
function method_exists(object|string $object_or_class, string $method): bool {}
function property_exists(object|string $object_or_class, string $property): bool {}

//- /app.php
<?php
class Handler
{
    public function run(\Throwable $error): int
    {
        if (method_exists($error, 'getResponse') && $error->getResponse()) {
            return 1;
        }
        return 0;
    }
}
"#,
        expect![[r#"
            4:25-4:35 error [undefined-type] Undefined type Throwable
        "#]],
    );
}

/// One term of several, because a check is rarely written alone.
#[test]
fn a_check_among_other_conditions_counts() {
    check(
        r#"
//- /stubs/builtins.php  stub
<?php
function method_exists(object|string $object_or_class, string $method): bool {}
function property_exists(object|string $object_or_class, string $property): bool {}

//- /app.php
<?php
class Walker
{
    public function run(Field $field, bool $flag): bool
    {
        if ($flag && method_exists($field, 'id') && $field->id() !== '') {
            return TRUE;
        }
        return FALSE;
    }
}

class Field {}
"#,
        expect![""],
    );
}

/// `property_exists`, the same idiom for the other kind of member.
/// A check on an assignment is about the name it wrote: Laravel's `AuthServiceProvider` writes
/// `method_exists($guard = $app['auth']->guard(), 'setDispatcher')` and calls it on `$guard`. Only
/// that name -- a check written on another assignment vouches for nothing here.
#[test]
fn a_check_on_an_assignment_vouches_for_the_name_it_wrote() {
    check(
        r#"
//- /stubs/builtins.php  stub
<?php
function method_exists(object|string $object_or_class, string $method): bool {}

//- /app.php
<?php
interface Guard
{
    public function check(): bool;
}

interface Auth
{
    public function guard(): Guard;
}

class Provider
{
    public function run(Auth $auth, Guard $guard): void
    {
        if (method_exists($guard = $auth->guard(), 'setDispatcher')) {
            $guard->setDispatcher();
        }
        if (method_exists($other = $auth->guard(), 'setEvents')) {
            $guard->setEvents();
        }
    }
}
"#,
        expect![[r#"
            19:27-19:33 warning [unused-variable] $other is assigned and never read
            20:21-20:30 error [undefined-method] Undefined method Guard::setEvents()
        "#]],
    );
}

#[test]
fn a_checked_property_is_not_reported_either() {
    check(
        r#"
//- /stubs/builtins.php  stub
<?php
function method_exists(object|string $object_or_class, string $method): bool {}
function property_exists(object|string $object_or_class, string $property): bool {}

//- /app.php
<?php
class Reader
{
    public function run(Row $row): mixed
    {
        if (property_exists($row, 'extra')) {
            return $row->extra;
        }
        return NULL;
    }
}

class Row {}
"#,
        expect![""],
    );
}

/// `isset()` asks the same of a property, and the read in the branch it guards is what it asked
/// for. Asked of another value it vouches for nothing, as `property_exists` does not.
#[test]
fn a_property_read_under_its_own_isset_is_not_reported() {
    check(
        r#"
//- /app.php
<?php
class Reader
{
    public function run(Row $row, Row $other): mixed
    {
        $extra = isset($row->extra) ? $row->extra : NULL;
        return isset($other->extra) ? $row->extra : $extra;
    }
}

class Row {}
"#,
        expect![[r#"
            7:45-7:50 error [undefined-property] Undefined property Row::$extra
        "#]],
    );
}

/// The boundary that matters most: a check about another value says nothing about this one.
#[test]
fn a_check_on_a_different_value_silences_nothing() {
    check(
        r#"
//- /stubs/builtins.php  stub
<?php
function method_exists(object|string $object_or_class, string $method): bool {}
function property_exists(object|string $object_or_class, string $property): bool {}

//- /app.php
<?php
class Offer
{
    public function run(Other $other): mixed
    {
        if (method_exists($other, 'getWarranty')) {
            return $this->getWarranty();
        }
        return NULL;
    }
}

class Other {}
"#,
        expect![[r#"
            7:27-7:38 error [undefined-method] Undefined method Offer::getWarranty()
        "#]],
    );
}

/// And a check for another name says nothing about this one.
#[test]
fn a_check_for_a_different_name_silences_nothing() {
    check(
        r#"
//- /stubs/builtins.php  stub
<?php
function method_exists(object|string $object_or_class, string $method): bool {}
function property_exists(object|string $object_or_class, string $property): bool {}

//- /app.php
<?php
class Offer
{
    public function run(): mixed
    {
        if (method_exists($this, 'getAdvance')) {
            return $this->getWarranty();
        }
        return NULL;
    }
}
"#,
        expect![[r#"
            7:27-7:38 error [undefined-method] Undefined method Offer::getWarranty()
        "#]],
    );
}

/// An `||` operand does not hold where the other one does, so a check written there vouches for
/// nothing.
#[test]
fn a_check_in_an_or_silences_nothing() {
    check(
        r#"
//- /stubs/builtins.php  stub
<?php
function method_exists(object|string $object_or_class, string $method): bool {}
function property_exists(object|string $object_or_class, string $property): bool {}

//- /app.php
<?php
class Offer
{
    public function run(bool $flag): mixed
    {
        if (method_exists($this, 'getWarranty') || $flag) {
            return $this->getWarranty();
        }
        return NULL;
    }
}
"#,
        expect![[r#"
            7:27-7:38 error [undefined-method] Undefined method Offer::getWarranty()
        "#]],
    );
}

/// An `||` whose every operand vouches does: the class an `instanceof` names, or anything the check
/// finds the method on. Symfony's translator decorators write exactly this.
#[test]
fn a_check_beside_an_instanceof_in_an_or_counts() {
    check(
        r#"
//- /stubs/builtins.php  stub
<?php
function method_exists(object|string $object_or_class, string $method): bool {}
function property_exists(object|string $object_or_class, string $property): bool {}

//- /app.php
<?php
interface Translates
{
}

interface Locales
{
}

class Translator implements Translates, Locales
{
    public function fallbacks(): array { return []; }
}

class Logging
{
    public function run(Translates&Locales $translator): array
    {
        if ($translator instanceof Translator || method_exists($translator, 'fallbacks')) {
            return $translator->fallbacks();
        }
        return [];
    }
}
"#,
        expect![[r#""#]],
    );
}

/// The guard does not reach past its branch.
#[test]
fn a_call_after_the_branch_is_still_reported() {
    check(
        r#"
//- /stubs/builtins.php  stub
<?php
function method_exists(object|string $object_or_class, string $method): bool {}
function property_exists(object|string $object_or_class, string $property): bool {}

//- /app.php
<?php
class Offer
{
    public function run(): mixed
    {
        if (method_exists($this, 'getWarranty')) {
            return $this->getWarranty();
        }
        return $this->getWarranty();
    }
}
"#,
        expect![[r#"
            9:23-9:34 error [undefined-method] Undefined method Offer::getWarranty()
        "#]],
    );
}
