//! Reaching a member through a value that can be null.
//!
//! Mostly about the silence. The check is only liveable because every way PHP has of ruling null out
//! is understood one layer down, and a fixture per way is what says so -- a regression in any of them
//! turns this check into the thing people switch off.

use expect_test::expect;

use super::diagnostics::{check, check_contracted};

/// The declaration says the value may be null and nothing rules it out.
#[test]
fn a_nullable_receiver_is_reported() {
    check(
        r#"
//- /app.php
<?php
class Repo
{
    public int $size = 0;
    public function find(): void {}
}

class Service
{
    public function run(?Repo $repo): void
    {
        $repo->find();
        echo $repo->size;
    }
}
"#,
        expect![[r#"
            12:9-12:14 warning [possibly-null-receiver] Cannot call method find() on ?Repo
            13:14-13:19 warning [possibly-null-receiver] Cannot access property $size on ?Repo
        "#]],
    );
}

/// `Repo|null` written the other way round, which is how a docblock spells it.
#[test]
fn a_union_with_null_is_the_same_claim() {
    check(
        r#"
//- /app.php
<?php
class Repo
{
    public function find(): void {}
}

class Service
{
    /**
     * @param Repo|null $repo
     */
    public function run($repo): void
    {
        $repo->find();
    }
}
"#,
        expect![[r#"
            14:9-14:14 warning [possibly-null-receiver] Cannot call method find() on ?Repo
        "#]],
    );
}

/// Each of the five ways a guard rules the null out, and the nullsafe arrow, which handles it.
#[test]
fn every_guard_silences_it() {
    check(
        r#"
//- /stubs/builtins.php  stub
<?php
function is_null(mixed $value): bool {}

//- /app.php
<?php
class Repo
{
    public function find(): void {}
}

class Service
{
    public function truthiness(?Repo $repo): void
    {
        if ($repo) {
            $repo->find();
        }
    }

    public function identity(?Repo $repo): void
    {
        if ($repo === null) {
            return;
        }
        $repo->find();
    }

    public function negated_identity(?Repo $repo): void
    {
        if ($repo !== null) {
            $repo->find();
        }
    }

    public function present(?Repo $repo): void
    {
        if (isset($repo)) {
            $repo->find();
        }
    }

    public function is_null_call(?Repo $repo): void
    {
        if (is_null($repo)) {
            return;
        }
        $repo->find();
    }

    public function nullsafe(?Repo $repo): void
    {
        $repo?->find();
    }

    public function short_circuit(?Repo $repo): void
    {
        echo $repo && $repo->find();
    }
}
"#,
        expect![""],
    );
}

/// A guard on a property, which is the shape a form class is built out of -- and the assignment
/// after it, which makes the guard stale rather than the property null.
#[test]
fn a_guard_on_a_property_is_a_guard() {
    check(
        r#"
//- /app.php
<?php
class Repo
{
    public function find(): void {}
}

class Manager
{
    public function load(): ?Repo
    {
        return null;
    }
}

class Service
{
    private ?Repo $repo = null;

    public function __construct(private Manager $manager) {}

    public function guarded(): void
    {
        if ($this->repo === null) {
            return;
        }
        $this->repo->find();
    }

    public function assigned_after_the_guard(): void
    {
        if (isset($this->repo)) {
            return;
        }
        $this->repo = $this->manager->load();
        $this->repo->find();
    }
}
"#,
        expect![[r#"
            35:9-35:20 warning [possibly-null-receiver] Cannot call method find() on ?Repo
        "#]],
    );
}

/// PHP reads a property through a null without complaining inside an isset context, so an author who
/// wrote one handled the null rather than missing it. A *call* there is not covered, and PHP agrees.
#[test]
fn an_isset_context_reads_through_the_null() {
    check(
        r#"
//- /app.php
<?php
class Repo
{
    public int $size = 0;
    public function find(): int
    {
        return 0;
    }
}

class Service
{
    public function run(?Repo $repo): void
    {
        echo $repo->size ?? 0;
        echo isset($repo->size);
        echo empty($repo->size);
        echo $repo->find() ?? 0;
    }
}
"#,
        expect![[r#"
            18:14-18:19 warning [possibly-null-receiver] Cannot call method find() on ?Repo
        "#]],
    );
}

/// `@phpstan-assert !null $param` on the callee, which is how PHPUnit's `assertNotNull` is written
/// and so how every test proves a nullable is there.
#[test]
fn a_call_that_checked_the_argument() {
    check(
        r#"
//- /app.php
<?php
class Repo
{
    public function find(): void {}
}

class Assert
{
    /**
     * @phpstan-assert !null $actual
     */
    public function assertNotNull(mixed $actual): void {}

    public function unchecked(mixed $actual): void {}
}

class Service extends Assert
{
    public function checked(?Repo $repo): void
    {
        $this->assertNotNull($repo);
        $repo->find();
    }

    public function not_checked(?Repo $repo): void
    {
        $this->unchecked($repo);
        $repo->find();
    }
}
"#,
        expect![[r#"
            28:9-28:14 warning [possibly-null-receiver] Cannot call method find() on ?Repo
        "#]],
    );
}

/// A value that is null on every run, where the member is beside the point.
#[test]
fn a_receiver_that_is_only_null() {
    check(
        r#"
//- /app.php
<?php
class Service
{
    public function run(): void
    {
        $repo = null;
        $repo->find();
    }
}
"#,
        expect![[r#"
            7:9-7:14 error [null-receiver] Cannot call method find() on null
        "#]],
    );
}

/// A nullable produced by a ternary, used once unguarded and once behind a guard, both inside a
/// `for` loop. Two things that were invisible before: a ternary was not a value at all, and a `for`
/// loop kept no statements, so a guard written in one was not a guard.
#[test]
fn a_ternary_inside_a_for_loop() {
    check(
        r#"
//- /app.php
<?php
class Repo
{
    public function find(): Repo
    {
        return $this;
    }
    public function save(): void {}
}

class Service
{
    public function run(int $count, bool $ok): void
    {
        for ($i = 0; $i < $count; $i++) {
            $repo = $ok ? $this->find() : null;
            $repo->save();
            if ($repo) {
                $repo->save();
            }
        }
    }

    private function find(): Repo
    {
        return new Repo();
    }
}
"#,
        expect![[r#"
            17:13-17:18 warning [possibly-null-receiver] Cannot call method save() on ?Repo
        "#]],
    );
}

/// A framework says its own `?T` never arrives as null, and the check believes it.
///
/// The seam's end: `phrost-framework` states the contract, the server installs it, and this is what
/// it buys. On one real Drupal project it is the difference between 379 findings and 261 -- every
/// `->query()` and `->execute()` result in the project, none of them a bug.
#[test]
fn a_contracted_return_is_not_reported() {
    let fixture = r#"
//- /stubs/builtins.php  stub
<?php
function print_r(mixed $value, bool $return = false): string|bool {}

//- /app.php
<?php
namespace Db;

interface Statement
{
    public function fetchAll(): array;
}

class Connection
{
    public function query(string $sql): ?Statement
    {
        return NULL;
    }
}

class Service
{
    public function run(Connection $db): void
    {
        print_r($db->query('SELECT 1')->fetchAll());
    }
}
"#;

    check(
        fixture,
        expect![[r#"
            21:17-21:39 warning [possibly-null-receiver] Cannot call method fetchAll() on ?Db\Statement
        "#]],
    );
    check_contracted(fixture, &[("Db\\Connection", "query")], expect![""]);
}

/// The contract follows the hierarchy, because a framework declares it once and every driver
/// overrides the signature.
#[test]
fn a_contract_on_a_base_class_covers_an_override() {
    check_contracted(
        r#"
//- /stubs/builtins.php  stub
<?php
function print_r(mixed $value, bool $return = false): string|bool {}

//- /app.php
<?php
namespace Db;

interface Statement
{
    public function fetchAll(): array;
}

abstract class Connection
{
    abstract public function query(string $sql): ?Statement;
}

class MysqlConnection extends Connection
{
    public function query(string $sql): ?Statement
    {
        return NULL;
    }
}

class Service
{
    public function run(MysqlConnection $db): void
    {
        print_r($db->query('SELECT 1')->fetchAll());
    }
}
"#,
        &[("Db\\Connection", "query")],
        expect![""],
    );
}

/// A contract about one method says nothing about the class's other nullable returns.
#[test]
fn a_contract_is_about_the_one_method_it_names() {
    check_contracted(
        r#"
//- /stubs/builtins.php  stub
<?php
function print_r(mixed $value, bool $return = false): string|bool {}

//- /app.php
<?php
namespace Db;

interface Statement
{
    public function fetchAll(): array;
}

class Connection
{
    public function query(string $sql): ?Statement
    {
        return NULL;
    }

    public function prepare(string $sql): ?Statement
    {
        return NULL;
    }
}

class Service
{
    public function run(Connection $db): void
    {
        print_r($db->prepare('SELECT 1')->fetchAll());
    }
}
"#,
        &[("Db\\Connection", "query")],
        expect![[r#"
            26:17-26:41 warning [possibly-null-receiver] Cannot call method fetchAll() on ?Db\Statement
        "#]],
    );
}

/// The report this exists for: a property filled on one line and passed on the next, reported as
/// possibly null because the declaration says `?T` and nothing read the assignment.
///
/// Both halves in one fixture, because the silence is only worth anything if the check is still
/// live: `fill` writes the property on the way and says nothing, `maybe` writes it in a branch that
/// may not have run and is still reported.
#[test]
fn a_property_the_body_filled_is_not_reported_as_null() {
    check(
        r#"
//- /app.php
<?php
class DomainException {}

class Request {}

class Engine
{
    public function run(Request $request): void {}
}

class Runner
{
    private ?Request $request = NULL;

    public function fill(): void
    {
        try {
            $this->request = $this->build();
            $this->engine()->run($this->request);
        }
        catch (\DomainException $exception) {
        }
    }

    public function maybe(bool $when): void
    {
        if ($when) {
            $this->request = $this->build();
        }
        $this->engine()->run($this->request);
    }

    private function build(): Request
    {
    }

    private function engine(): Engine
    {
    }
}
"#,
        expect![[r#"
            30:30-30:44 warning [possibly-null-argument] $request takes \Request, and this may be null
        "#]],
    );
}

/// `?->` yields null the moment its receiver is, so a non-null result proves the receiver was
/// there -- and the branch is written with a plain arrow, because by then the author knows.
///
/// Both halves in one fixture: the guarded read says nothing, and the same read outside the branch
/// is still reported.
#[test]
fn a_nullsafe_chain_tested_against_null_proves_its_receiver() {
    check(
        r#"
//- /app.php
<?php
class Request
{
    public ?string $date = NULL;

    public function label(): ?string
    {
    }
}

class Runner
{
    public function go(?Request $request): void
    {
        if ($request?->date !== NULL) {
            echo $request->date;
        }
        if ($request?->label()) {
            echo $request->date;
        }
        echo $request->date;
    }
}
"#,
        expect![[r#"
            21:14-21:22 warning [possibly-null-receiver] Cannot access property $date on ?Request
        "#]],
    );
}

/// A loop's condition is a guard over its own body, and the commonest one written in PHP: `while
/// ($row = fgetcsv($handle))` is how a file is read.
///
/// The `do ... while` beside it is the reason the two are told apart: the same words there prove
/// the row is present only from the second turn onwards, so believing them on the first would be a
/// claim about code that has not run.
#[test]
fn a_while_condition_narrows_inside_its_own_body() {
    check(
        r#"
//- /app.php
<?php
class Row {}

class Runner
{
    public function next(): ?Row
    {
    }

    public function takes(Row $row): void
    {
    }

    public function go(): void
    {
        while ($row = $this->next()) {
            $this->takes($row);
        }
        do {
            $this->takes($row);
        } while ($row = $this->next());
    }
}
"#,
        expect![[r#"
            20:26-20:30 warning [possibly-null-argument] $row takes \Row, and this may be null
        "#]],
    );
}

/// A branch entered because the value was of some *kind* leaves the other alternatives behind it,
/// exactly as one entered because the value was null does.
///
/// Without that, `if (is_int($doc)) { $doc = $this->load($doc); }` answers `Doc|Doc|int` below --
/// the write joined with a value that still lists the int the write exists to replace, and the
/// duplicate is what the join could not reduce because of it.
#[test]
fn a_branch_entered_on_a_kind_leaves_that_kind_behind_it() {
    check(
        r#"
//- /stubs/builtins.php  stub
<?php
function is_int(mixed $value): bool {}

//- /app.php
<?php
class Doc {}

class Runner
{
    public function load(int $id): Doc
    {
    }

    public function takes(Doc $doc): void
    {
    }

    public function go(Doc|int $doc, string|int $other): void
    {
        if (is_int($doc)) {
            $doc = $this->load($doc);
        }
        $this->takes($doc);

        // The control: nothing replaces the int, so it is still one of the alternatives.
        if (is_int($other)) {
        }
        $this->takes($other);
    }
}
"#,
        expect![[r#"
            24:22-24:28 error [incompatible-argument] $doc takes \Doc, and this is string|int
        "#]],
    );
}
