//! What a file is reported as having wrong with it.
//!
//! Every test here is as much about the silence as about the finding: the cases that must *not* be
//! reported are the ones that decide whether anyone leaves the checker on.

use expect_test::{Expect, expect};
use phrost_harness::{World, with_open_scopes};
use phrost_testkit::FixtureSpec;
use phrost_view::IndexView;

use crate::{AskedFor, Severity, diagnostics_asked_for};

/// The entry point under test, with the switched-on codes threaded through.
fn phrost_analysis_diagnostics(
    view: &IndexView<'_>,
    asked: &[String],
    off: &[String],
) -> Vec<crate::Diagnostic> {
    diagnostics_asked_for(view, AskedFor { codes: asked, off })
}

/// Renders every diagnostic in the fixture's last file.
pub(super) fn check(fixture: &str, expect: Expect) {
    check_contracted(fixture, &[], expect);
}

/// The same, with framework contracts stated first.
///
/// A separate entry point rather than a parameter on [`check`], so that the thirty tests about what
/// PHP itself says keep reading as tests about PHP.
pub(super) fn check_contracted(fixture: &str, non_null: &[(&str, &str)], expect: Expect) {
    check_with(
        fixture,
        |world| {
            for (class, method) in non_null {
                world.contracts_mut().add_unproduced(
                    class,
                    method,
                    phrost_model::BuiltinTy::Null,
                    None,
                    None,
                );
            }
        },
        &[],
        expect,
    );
}

/// The same, for a rule that reads whatever a framework contributed rather than one named contract.
///
/// A closure rather than a list, because what a convention states is not one shape: a docblock rule
/// arrives with the words it is written in and the catalog it is checked against, and a test that
/// could only state non-null returns would have to grow a parameter per kind.
pub(super) fn check_framework(fixture: &str, state: impl FnOnce(&mut World), expect: Expect) {
    check_with(fixture, state, &[], expect);
}

/// The same, with the optional checks a project switched on.
///
/// Named by code, exactly as a `phrost.toml` names them, so a test about a rule that is off by
/// default reads the way switching it on does.
pub(super) fn check_asked_for(fixture: &str, codes: &[&str], expect: Expect) {
    let owned = codes
        .iter()
        .map(|code| (*code).to_owned())
        .collect::<Vec<_>>();
    check_with(fixture, |_| {}, &owned, expect);
}

/// Both at once: the checks a project switched on, and what a framework contributed to them.
///
/// The pair a rule that a framework can *silence* needs -- there is no reading of an exemption that
/// does not also show the rule firing where the exemption does not reach.
pub(super) fn check_asked_for_with(
    fixture: &str,
    codes: &[&str],
    state: impl FnOnce(&mut World),
    expect: Expect,
) {
    let owned = codes
        .iter()
        .map(|code| (*code).to_owned())
        .collect::<Vec<_>>();
    check_with(fixture, state, &owned, expect);
}

fn check_with(fixture: &str, state: impl FnOnce(&mut World), asked: &[String], expect: Expect) {
    check_with_off(fixture, state, asked, &[], expect);
}

/// Every diagnostic in the fixture's last file but the codes a project switched off with
/// `lint.off`.
pub(super) fn check_switched_off(fixture: &str, off: &[&str], expect: Expect) {
    let off = off
        .iter()
        .map(|code| (*code).to_owned())
        .collect::<Vec<_>>();
    check_with_off(fixture, |_| {}, &[], &off, expect);
}

fn check_with_off(
    fixture: &str,
    state: impl FnOnce(&mut World),
    asked: &[String],
    off: &[String],
    expect: Expect,
) {
    let spec = FixtureSpec::parse(fixture);
    // Before the files, because a fact derived at ingest -- an extension point a call declares --
    // is never re-read: see `World::build_with`.
    let world = World::build_with(&spec, state);
    let path = spec
        .files()
        .last()
        .expect("the fixture declares a file")
        .path()
        .to_owned();
    let file = world.file_id(&path);

    let rendered = with_open_scopes(&world, Some(file), |open| {
        let view = IndexView::in_file(world.skeleton(), open, file);
        phrost_analysis_diagnostics(&view, asked, off)
            .into_iter()
            .map(|found| {
                format!(
                    "{} {} [{}] {}\n",
                    world.render_range(file, found.range),
                    match found.severity {
                        Severity::Error => "error",
                        Severity::Warning => "warning",
                        Severity::Note => "note",
                    },
                    found.code,
                    found.message
                )
            })
            .collect::<String>()
    });

    expect.assert_eq(&rendered);
}

#[test]
fn a_member_the_class_does_not_have() {
    check(
        r#"
//- /app.php
<?php
class Repo
{
    public int $size = 0;
    public function find(int $id): void {}
}

class Service
{
    public function run(Repo $repo): void
    {
        $repo->find(1);
        $repo->fnid(1);
        $repo->size;
        $repo->sixe;
    }
}
"#,
        expect![[r#"
            13:16-13:20 error [undefined-method] Undefined method Repo::fnid()
            15:16-15:20 error [undefined-property] Undefined property Repo::$sixe
        "#]],
    );
}

#[test]
fn nothing_is_reported_when_the_receiver_has_no_type() {
    // An untyped receiver says nothing about whether the member exists, and reporting it would
    // report most of an untyped codebase.
    check(
        r#"
//- /app.php
<?php
class Service
{
    private $untyped;

    public function run($whatever): void
    {
        $whatever->anything();
        $this->untyped->anything();
    }
}
"#,
        expect![""],
    );
}

#[test]
fn nothing_is_reported_when_a_parent_is_not_indexed() {
    // The member is very likely declared up there. One unresolved base class must not turn every
    // inherited call into a finding.
    check(
        r#"
//- /app.php
<?php
class Service extends NotIndexed
{
    public function run(): void
    {
        $this->inherited();
    }
}
"#,
        expect![[r#"
            2:23-2:33 error [undefined-type] Undefined type NotIndexed
        "#]],
    );
}

#[test]
fn nothing_is_reported_for_a_class_that_answers_unknown_names() {
    // `__call` and `__get` make any name legal, which is how much of Drupal and Laravel works.
    check(
        r#"
//- /app.php
<?php
class Magic
{
    public function __call(string $name, array $arguments) {}
    public function __get(string $name) {}
}

class Service
{
    public function run(Magic $magic): void
    {
        $magic->whatever();
        $magic->anything;
    }
}
"#,
        expect![""],
    );
}

#[test]
fn an_inherited_member_is_found_through_the_whole_hierarchy() {
    check(
        r#"
//- /app.php
<?php
trait Loggable
{
    public function log(string $message): void {}
}

class Base
{
    public function shared(): void {}
}

class Child extends Base
{
    use Loggable;
}

class Service
{
    public function run(Child $child): void
    {
        $child->shared();
        $child->log('x');
        $child->missing();
    }
}
"#,
        expect![[r#"
            23:17-23:24 error [undefined-method] Undefined method Child::missing()
        "#]],
    );
}

#[test]
fn a_name_that_names_nothing() {
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repo
{
}

//- /src/app.php
<?php
namespace App;

use App\Repo;
use App\Gone;

class Service
{
    public function run(Repo $repo, Gone $gone, Missing $missing): void
    {
        new Absent();
    }
}
"#,
        expect![[r#"
            5:5-5:13 error [undefined-type] Undefined type App\Gone
            9:37-9:41 error [undefined-type] Undefined type Gone
            9:49-9:56 error [undefined-type] Undefined type Missing
            11:13-11:19 error [undefined-type] Undefined type Absent
        "#]],
    );
}

#[test]
fn an_import_of_a_namespace_is_not_a_missing_class() {
    // `use App\Nested;` followed by `Nested\Thing` is ordinary PHP, and with attributes it is the
    // common form. Nothing declares `App\Nested` itself.
    check(
        r#"
//- /src/Thing.php
<?php
namespace App\Nested;

class Thing
{
}

//- /src/app.php
<?php
namespace App;

use App\Nested;

class Service
{
    public function run(): void
    {
        new Nested\Thing();
    }
}
"#,
        expect![[r#""#]],
    );
}

#[test]
fn a_docblock_name_is_judged_under_its_own_code_and_a_template_not_at_all() {
    // A docblock names things PHP does not -- a `@template` parameter first among them -- so an
    // unresolved name there is never `undefined-type`, which is a fatal error in code. A name that
    // could be a class and is none is a wrong annotation, and `doc-undefined-type` says so.
    check(
        r#"
//- /src/app.php
<?php
namespace App;

class Service
{
    /**
     * @template T
     *
     * @param T $value
     * @return NotAClass
     */
    public function run($value)
    {
        return $value;
    }
}
"#,
        expect![[r#"
            10:16-10:25 warning [doc-undefined-type] Undefined type NotAClass in a docblock
        "#]],
    );
}

#[test]
fn text_that_does_not_parse_is_reported() {
    check(
        r#"
//- /app.php
<?php
class Service
{
    public function run(): void
    {
        $x = 1
    }
}
"#,
        expect![[r#"
            7:5-7:6 error [syntax] Expected one of `Semicolon`, `CloseTag`, found `RightBrace`
        "#]],
    );
}

#[test]
fn a_file_that_does_not_parse_gets_no_semantic_findings() {
    // Recovery drops statements after the break, so everything a semantic check would say is about
    // a file with pieces missing. The `fnid` typo below is real and is still not reported: the
    // missing `;` above it is the line worth reading.
    check(
        r#"
//- /app.php
<?php
class Repo
{
    public function find(int $id): void {}
}

class Service
{
    public function run(Repo $repo): void
    {
        $x = 1
        $repo->fnid(2);
    }
}
"#,
        expect![[r#"
            12:9-12:14 error [syntax] Expected one of `Semicolon`, `CloseTag`, found `Variable`
        "#]],
    );
}

#[test]
fn a_method_called_on_a_string_is_a_fatal_and_not_a_silence() {
    // A method on a string is `Call to a member function title() on string` at runtime. The index
    // knew the type and said nothing, because a receiver with no *class* was read as a receiver with
    // no type -- and the two are opposites: one is an absence of knowledge and the other is proof.
    check(
        r#"
//- /app.php
<?php
class Stage
{
    public function name(): string
    {
        return 'shipped';
    }

    public function title(): string
    {
        return 'x';
    }
}

function run(Stage $stage, int $count): void
{
    $shipped = $stage->name();
    $shipped->title();
    $shipped->length;
    $count->title();
    $rows = [1, 2];
    $rows->first();
}
"#,
        expect![[r#"
            18:15-18:20 error [non-object-receiver] Cannot call method title() on string
            19:15-19:21 error [non-object-receiver] Cannot access property $length on string
            20:13-20:18 error [non-object-receiver] Cannot call method title() on int
            22:12-22:17 error [non-object-receiver] Cannot call method first() on array
        "#]],
    );
}

#[test]
fn a_type_an_object_can_inhabit_stays_silent() {
    // `object`, `mixed`, `iterable` and `callable` are all types an object is, so none of them
    // settles anything -- and a null belongs to the null check, which knows how it got there.
    check(
        r#"
//- /app.php
<?php
class Repo
{
    public function run(object $one, mixed $two, iterable $three, callable $four): void
    {
        $one->go();
        $two->go();
        $three->go();
        $four->go();
    }
}
"#,
        expect![[r#""#]],
    );
}
