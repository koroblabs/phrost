//! A slot's value is read where it was written, not where the slot is read.
//!
//! The memoising shape every Drupal service has somewhere:
//!
//! ```php
//! static $apps = [];
//! if (!isset($apps[$n])) {
//!     $application = $this->manager->load($n);   // ?Application
//!     if (!$application) { throw new NotFound(); }
//!     $apps[$n] = $application;                    // an Application went in
//! }
//! return $apps[$n];                                // and came back as ?Application
//! ```
//!
//! `element_written` found the write and handed back `$application`, and the reader resolved that
//! name at the *return*, where the `if` had closed and its throwing guard no longer held -- so the
//! null the guard had taken away came back. The write carries its position now, and the value is
//! resolved there, which is the rule every joined value already follows.

use expect_test::expect;

use super::diagnostics::check;

#[test]
fn a_value_guarded_where_it_was_written_stays_guarded_in_the_slot() {
    check(
        r#"
//- /app.php
<?php

class Exception {}

class NotFound extends \Exception {}

class Application
{
    public function isPending(): bool { return TRUE; }
}

class Manager
{
    public function load(int $n): ?Application { return NULL; }

    public function isEditAllowed(Application $application): bool { return TRUE; }
}

class Handler
{
    public function __construct(private Manager $manager) {}

    public function guarded(int $n): Application
    {
        static $apps = [];
        if (!isset($apps[$n])) {
            $application = $this->manager->load($n);
            if (!$application) {
                throw new NotFound();
            }
            $apps[$n] = $application;
        }
        return $apps[$n];
    }

    public function unguarded(int $n): Application
    {
        static $apps = [];
        if (!isset($apps[$n])) {
            $apps[$n] = $this->manager->load($n);
        }
        return $apps[$n];
    }

    /// Three operands, the third a call taking the value as an argument: the first two had
    /// composed into a contract whose receiver carried the not-null, and composing the third
    /// dropped that receiver.
    public function threeOperandGuard(int $n, bool $reload = FALSE): Application
    {
        static $apps = [];
        if (!isset($apps[$n]) || $reload) {
            $application = $this->manager->load($n);
            if (!$application || !$application->isPending() || !$this->manager->isEditAllowed($application)) {
                unset($apps[$n]);
                throw new NotFound();
            }
            $apps[$n] = $application;
        }
        return $apps[$n];
    }
}
"#,
        expect![[r#"
            42:16-42:25 warning [possibly-null-return] ?Application returned where \Application is declared
        "#]],
    );
}
