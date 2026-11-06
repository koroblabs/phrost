//! An `if` whose arm comes back out, and what it did before coming out.
//!
//! ```php
//! if (!($application = $this->manager->loadActive($user->id()))) {
//!     if (!($application = $this->manager->loadByUserId($user->id()))) {
//!         return $this->logErrorResponse([...]);
//!     }
//! }
//! if (!$application->isBulkOrder()) {
//! ```
//!
//! Both ways out of the outer `if` leave `$application` non-null: not taking it means the first
//! load answered, and coming back out of it means the second did. Read as two unrelated guards the
//! last line is a `possibly-null-receiver` -- the inner guard is invisible from below, because
//! `visit_preceding_statements` does not descend into a *completed* branch, which is right in
//! general and wrong for exactly this shape.
//!
//! The two controls beside it are the whole of why the rule is not simply "descend": an arm that
//! assigns without checking, and an arm that checks and then nulls the value anyway, both still
//! report. And the value reads as `App` rather than as nothing, which is the difference between a
//! narrowing and a silence.

use expect_test::expect;

use super::Query;
use super::diagnostics::check;

#[test]
fn a_fallback_inside_a_guards_arm_is_not_a_null() {
    check(
        r#"
//- /app.php
<?php

class App
{
    public function isBulkOrder(): bool { return TRUE; }
}

class Manager
{
    public function loadActive(int $id): ?App { return NULL; }

    public function loadByUserId(int $id): ?App { return NULL; }
}

class Repo
{
    public function one(Manager $m, int $id): bool
    {
        if (!($app = $m->loadActive($id))) {
            return FALSE;
        }
        return $app->isBulkOrder();
    }

    public function two(Manager $m, int $id): bool
    {
        if (!($app = $m->loadActive($id))) {
            if (!($app = $m->loadByUserId($id))) {
                return FALSE;
            }
        }
        return $app->isBulkOrder();
    }

    public function three(Manager $m, int $id): bool
    {
        $app = $m->loadActive($id);
        if ($app === NULL) {
            $app = $m->loadByUserId($id);
            if ($app === NULL) {
                return FALSE;
            }
        }
        return $app->isBulkOrder();
    }

    public function unsound(Manager $m, int $id): bool
    {
        if (!($app = $m->loadActive($id))) {
            $app = $m->loadByUserId($id);
        }
        return $app->isBulkOrder();
    }

    public function alsoUnsound(Manager $m, int $id): bool
    {
        if (!($app = $m->loadActive($id))) {
            if (!($app = $m->loadByUserId($id))) {
                $app = NULL;
            }
        }
        return $app->isBulkOrder();
    }
}
"#,
        expect![[r#"
            52:16-52:20 warning [possibly-null-receiver] Cannot call method isBulkOrder() on ?App
            62:16-62:20 warning [possibly-null-receiver] Cannot call method isBulkOrder() on ?App
        "#]],
    );
}

#[test]
fn the_value_reads_as_the_class_rather_than_as_nothing() {
    super::check(
        r#"
//- /app.php
<?php

class App
{
    public function isBulkOrder(): bool { return TRUE; }
}

class Manager
{
    public function loadActive(int $id): ?App { return NULL; }

    public function loadByUserId(int $id): ?App { return NULL; }
}

class Repo
{
    public function two(Manager $m, int $id): bool
    {
        if (!($app = $m->loadActive($id))) {
            if (!($app = $m->loadByUserId($id))) {
                return FALSE;
            }
        }
        return $ap@here@p->isBulkOrder();
    }
}
"#,
        &[Query::hover("the value after both guards", "here")],
        expect![[r#"
            the value after both guards
            - code: App $app
            - note: narrowed by a guard
        "#]],
    );
}
