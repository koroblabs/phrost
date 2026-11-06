//! A class that does not declare what its supertypes require.
//!
//! A fatal error at class *load*: PHP refuses the declaration before a line of it runs. So, like the
//! override rules, it can sit in a codebase until something first loads the class.
//!
//! Most of these tests are about the silence. A method can legitimately arrive from a trait, from a
//! parent five levels up, from a trait a parent uses, or from a `@method` tag -- and a check that
//! misses one of those reports a working class as broken, which is the one failure that makes a
//! person switch the checker off.

use expect_test::expect;

use super::diagnostics::check;

#[test]
fn an_interface_method_nothing_declares() {
    check(
        r#"
//- /app.php
<?php
interface Runner
{
    public function run(): void;
}

class Job implements Runner
{
}
"#,
        expect![[r#"
            7:7-7:10 error [unimplemented-method] Job does not implement Runner::run()
        "#]],
    );
}

#[test]
fn every_missing_method_is_named_in_one_finding() {
    check(
        r#"
//- /app.php
<?php
interface Runner
{
    public function run(): void;

    public function stop(): void;
}

class Job implements Runner
{
    public function stop(): void {}
}

class Idle implements Runner
{
}
"#,
        expect![[r#"
            9:7-9:10 error [unimplemented-method] Job does not implement Runner::run()
            14:7-14:11 error [unimplemented-method] Idle does not implement Runner::run(), Runner::stop()
        "#]],
    );
}

/// An abstract parent's abstract method is the other half of the same fatal, and PHP reports it the
/// same way.
#[test]
fn an_abstract_parents_method_nothing_declares() {
    check(
        r#"
//- /app.php
<?php
abstract class Task
{
    abstract public function run(): void;

    public function name(): string
    {
        return 'task';
    }
}

class Job extends Task
{
}
"#,
        expect![[r#"
            12:7-12:10 error [unimplemented-method] Job does not implement Task::run()
        "#]],
    );
}

/// Everything that legally provides the method. All of it silent, and all in one file because the
/// failure this guards against is one report against any of them.
#[test]
fn a_method_provided_any_legal_way_says_nothing() {
    check(
        r#"
//- /app.php
<?php
interface Runner
{
    public function run(): void;
}

trait Runs
{
    public function run(): void {}
}

class ItsOwn implements Runner
{
    public function run(): void {}
}

class FromATrait implements Runner
{
    use Runs;
}

abstract class Middle implements Runner
{
    public function run(): void {}
}

class FromAGrandparent extends Middle
{
}

class FromAParentsTrait extends FromATrait
{
}
"#,
        expect![""],
    );
}

/// A class-load error is about a class that can be loaded. Nothing that cannot be instantiated has
/// to answer for a requirement -- it exists so that something below it does.
#[test]
fn nothing_that_may_leave_a_requirement_is_reported() {
    check(
        r#"
//- /app.php
<?php
interface Runner
{
    public function run(): void;
}

abstract class Task implements Runner
{
}

interface Wider extends Runner
{
    public function stop(): void;
}

trait Partial
{
    abstract public function run(): void;
}
"#,
        expect![""],
    );
}

/// The interface is a name this index does not hold, so what it requires is unknown. Reporting here
/// would be reporting on a gap in the index, and a vendor package phrost never walked is the
/// ordinary case rather than the corner one.
#[test]
fn an_unknown_supertype_is_silent() {
    check(
        r#"
//- /app.php
<?php
class Job implements \Vendor\Runner
{
}
"#,
        expect![[r#"
            2:22-2:36 error [undefined-type] Undefined type Vendor\Runner
        "#]],
    );
}

/// PHP checks declarations at class load, and a magic method is not one. A class relying on `__call`
/// to answer an interface is a class that never loads, so the report stands.
#[test]
fn a_magic_call_does_not_stand_in_for_a_declaration() {
    check(
        r#"
//- /app.php
<?php
interface Runner
{
    public function run(): void;
}

class Job implements Runner
{
    public function __call(string $name, array $arguments): mixed
    {
        return null;
    }
}
"#,
        expect![[r#"
            7:7-7:10 error [unimplemented-method] Job does not implement Runner::run()
        "#]],
    );
}

/// An `@method` tag is the only thing in the source saying the name answers, and a class carrying one
/// has an author who has thought about it. Silence is not a claim that PHP agrees.
#[test]
fn a_documented_method_is_left_alone() {
    check(
        r#"
//- /app.php
<?php
interface Runner
{
    public function run(): void;
}

/**
 * @method void run()
 */
class Job implements Runner
{
}
"#,
        expect![""],
    );
}

/// Enums implement interfaces like anything else, and must declare what they promise.
#[test]
fn an_enum_answers_for_its_interfaces() {
    check(
        r#"
//- /stubs/enums.php  stub
<?php
interface UnitEnum
{
    public static function cases(): array;
}

interface BackedEnum extends UnitEnum
{
    public static function from(int|string $value): static;

    public static function tryFrom(int|string $value): ?static;
}

//- /app.php
<?php
interface Labelled
{
    public function label(): string;
}

enum Stage: string implements Labelled
{
    case Open = 'open';
}

enum Ready: string implements Labelled
{
    case Open = 'open';

    public function label(): string
    {
        return 'open';
    }
}
"#,
        expect![[r#"
            7:6-7:11 error [unimplemented-method] Stage does not implement Labelled::label()
        "#]],
    );
}

/// `cases()`, `from()` and `tryFrom()` are PHP's, not the enum's, and they reach the member table
/// through `UnitEnum` exactly as a requirement would. Nothing wrote that interface, so nothing
/// requires it -- and without that rule every enum in a project is reported.
#[test]
fn an_enums_implicit_interfaces_require_nothing() {
    check(
        r#"
//- /stubs/enums.php  stub
<?php
interface UnitEnum
{
    public static function cases(): array;
}

interface BackedEnum extends UnitEnum
{
    public static function from(int|string $value): static;

    public static function tryFrom(int|string $value): ?static;
}

//- /app.php
<?php
enum Stage: string
{
    case Open = 'open';
}

enum Pure
{
    case Open;
}
"#,
        expect![""],
    );
}

/// A `@mixin` says a member answers at runtime; it says nothing about what the class has to declare.
/// It reaches the flattened table with the provenance of an inherited member, which is why the
/// requirement has to be traced back to a written `extends` or `implements`.
///
/// `Job` implements something as well, so the finding is decided by that trace rather than by the
/// cheaper "nothing is written above it" gate in front of it.
#[test]
fn a_mixin_requires_nothing() {
    check(
        r#"
//- /app.php
<?php
interface Runner
{
    public function run(): void;
}

abstract class Helper
{
    abstract public function help(): void;
}

/**
 * @mixin Helper
 */
class Job implements Runner
{
    public function run(): void {}
}
"#,
        expect![""],
    );
}

/// A trait's abstract method is satisfiable by a declaration the class inherits, and the merge order
/// puts the trait's row over the parent's -- so the table cannot tell "unmet" from "met above".
/// Silence is the safe direction, and `Job::go()` here is the deliberate miss it costs: PHP would
/// refuse this class.
///
/// Written with an interface as well, so the silence comes from the trait rule and not from the
/// "nothing is written above it" gate.
#[test]
fn a_traits_abstract_method_is_not_reported() {
    check(
        r#"
//- /app.php
<?php
interface Runner
{
    public function run(): void;
}

trait Needs
{
    abstract public function go(): void;
}

class Job implements Runner
{
    use Needs;

    public function run(): void {}
}
"#,
        expect![""],
    );
}
