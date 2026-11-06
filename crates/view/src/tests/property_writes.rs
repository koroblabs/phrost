//! What an assignment to a property leaves the property holding.
//!
//! The half of `slot_value` that did not exist: a guard about `$this->x` was already read, and an
//! assignment to it was not, so a property filled on one line and read on the next still answered
//! the `?T` its declaration says. What the rule gives up is calls -- see the module's own note --
//! and what it refuses is anything this body wrote again.

use expect_test::expect;

use super::{Query, check_view};

/// The shape reported from the reference project, down to the `try` around it.
#[test]
fn a_property_holds_what_the_line_above_wrote_into_it() {
    check_view(
        r#"
//- /src/Request.php
<?php
namespace App;

class Request {}

//- /src/Runner.php
<?php
namespace App;

class Runner
{
    private ?Request $request = NULL;

    public function go(): void
    {
        try {
            $this->request = $this->build();
            $c@after@urrent = $this->request;
        }
        catch (\DomainException $exception) {
        }
    }

    private function build(): Request
    {
    }
}
"#,
        &[Query::variable_type("after the write", "after")],
        expect![[r#"
            after the write
            - Request
        "#]],
    );
}

/// A write control did not have to go through is a second answer, and nobody can say which ran.
#[test]
fn a_write_that_may_have_run_since_withdraws_the_claim() {
    check_view(
        r#"
//- /src/Request.php
<?php
namespace App;

class Request {}

//- /src/Runner.php
<?php
namespace App;

class Runner
{
    private ?Request $request = NULL;

    public function go(bool $reset): void
    {
        $this->request = $this->build();
        if ($reset) {
            $this->request = NULL;
        }
        $c@after@urrent = $this->request;
    }

    private function build(): Request
    {
    }
}
"#,
        &[Query::variable_type("after the branch", "after")],
        expect![[r#"
            after the branch
            - ?Request
        "#]],
    );
}

/// The mirror of it: the branch is behind the certain write, so the write is what happened last.
#[test]
fn a_write_before_the_certain_one_is_replaced_by_it() {
    check_view(
        r#"
//- /src/Request.php
<?php
namespace App;

class Request {}

//- /src/Runner.php
<?php
namespace App;

class Runner
{
    private ?Request $request = NULL;

    public function go(bool $reset): void
    {
        if ($reset) {
            $this->request = NULL;
        }
        $this->request = $this->build();
        $c@after@urrent = $this->request;
    }

    private function build(): Request
    {
    }
}
"#,
        &[Query::variable_type("after the write", "after")],
        expect![[r#"
            after the write
            - Request
        "#]],
    );
}

/// A write in a branch the reader is not in never ran on the way here.
#[test]
fn a_write_in_another_branch_says_nothing() {
    check_view(
        r#"
//- /src/Request.php
<?php
namespace App;

class Request {}

//- /src/Runner.php
<?php
namespace App;

class Runner
{
    private ?Request $request = NULL;

    public function go(bool $fill): void
    {
        if ($fill) {
            $this->request = $this->build();
        }
        $c@after@urrent = $this->request;
    }

    private function build(): Request
    {
    }
}
"#,
        &[Query::variable_type("after the branch", "after")],
        expect![[r#"
            after the branch
            - ?Request
        "#]],
    );
}

/// Rebinding the value the slot is reached *through* replaces the slot too.
#[test]
fn a_write_to_the_receiver_withdraws_what_was_written_through_it() {
    check_view(
        r#"
//- /src/Request.php
<?php
namespace App;

class Request {}

//- /src/Order.php
<?php
namespace App;

class Order
{
    public ?Request $request = NULL;
}

//- /src/Runner.php
<?php
namespace App;

class Runner
{
    public function go(Order $order): void
    {
        $order->request = $this->build();
        $order = $this->other();
        $c@after@urrent = $order->request;
    }

    private function build(): Request
    {
    }

    private function other(): Order
    {
    }
}
"#,
        &[Query::variable_type("after the rebinding", "after")],
        expect![[r#"
            after the rebinding
            - ?Request
        "#]],
    );
}

/// A guard written after the assignment is the later claim, and the more specific one.
#[test]
fn a_guard_after_the_write_is_what_holds() {
    check_view(
        r#"
//- /src/Request.php
<?php
namespace App;

class Request {}

class Urgent extends Request {}

//- /src/Runner.php
<?php
namespace App;

class Runner
{
    private ?Request $request = NULL;

    public function go(): void
    {
        $this->request = $this->build();
        if ($this->request instanceof Urgent) {
            $c@inside@urrent = $this->request;
        }
    }

    private function build(): Request
    {
    }
}
"#,
        &[Query::variable_type("inside the guard", "inside")],
        expect![[r#"
            inside the guard
            - Urgent
        "#]],
    );
}
