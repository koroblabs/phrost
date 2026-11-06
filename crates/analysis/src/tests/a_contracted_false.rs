//! A declared `false` a contract says no run produces.
//!
//! The sibling of the non-null contract, and it exists because the standard library reports failure
//! two ways and a *runtime setting* decides which. `PDO::prepare()` is declared `PDOStatement|false`
//! and returns `false` only under `ERRMODE_SILENT`; `ERRMODE_EXCEPTION` throws and has been the
//! default since PHP 8.0. Believing the signature reported seventeen of nineteen findings on the
//! first non-Drupal project this was measured against, all of them noise -- the same shape, and the
//! same count, as the 118 the non-null contract was written for.
//!
//! The contract applies to the value a call produces and not to the declaration, which keeps saying
//! what it says: hover still shows the `|false`.
//!
//! What made this dead before it was written is pinned by the second test. A union return was
//! claimed by the *template* substitution, which rebuilt it operand by operand into the same thing
//! and returned before the contract was consulted -- so a contract about a union could not have
//! worked, and nothing showed it while every contract written was about a `?T`.

use expect_test::expect;
use phrost_model::BuiltinTy;

use super::diagnostics::check_framework;

const FIXTURE: &str = r#"
//- /app.php
<?php
class Stmt
{
    public function execute(): bool
    {
        return TRUE;
    }
}

class Driver
{
    public function prepare(string $sql): Stmt|false {}

    public function uncontracted(string $sql): Stmt|false {}
}
"#;

/// The contracted method's `false` is taken off the value the call produces.
#[test]
fn a_contracted_false_is_not_reported() {
    check_framework(
        &format!(
            r#"{FIXTURE}
//- /use.php
<?php
class Runner
{{
    public function run(Driver $driver): void
    {{
        $driver->prepare('x')->execute();
    }}
}}
"#
        ),
        |world| {
            world
                .contracts_mut()
                .add_unproduced("Driver", "prepare", BuiltinTy::False, None, None);
        },
        expect![""],
    );
}

/// The negative control, and the one that would have caught the dead path: the same shape with no
/// contract still reports, so the silence above is the contract and not the union going unread.
#[test]
fn an_uncontracted_false_is_still_reported() {
    check_framework(
        &format!(
            r#"{FIXTURE}
//- /use.php
<?php
class Runner
{{
    public function run(Driver $driver): void
    {{
        $driver->uncontracted('x')->execute();
    }}
}}
"#
        ),
        |world| {
            world
                .contracts_mut()
                .add_unproduced("Driver", "prepare", BuiltinTy::False, None, None);
        },
        expect![[r#"
            6:9-6:35 warning [possibly-false-receiver] Cannot call method execute() on Stmt|false
        "#]],
    );
}
