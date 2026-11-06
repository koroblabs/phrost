//! Every place the current file names a member, and what resolving it reached.

use expect_test::{Expect, expect};
use phrost_harness::{OpenScope, World};

use crate::{IndexView, MemberAccessOutcome, MemberKind};

/// Renders each access with the outcome, which is what a checker and a coverage sweep both read.
fn check(fixture: &str, expect: Expect) {
    let world = World::parse(fixture);
    let file = world.any_file().expect("the fixture declares a file");
    let open = world.open(OpenScope::CursorFileOnly, Some(file));
    let view = IndexView::in_file(world.skeleton(), &open, file);

    let rendered = view
        .member_accesses()
        .iter()
        .map(|access| {
            let outcome = match &access.outcome {
                MemberAccessOutcome::Resolved { class } => format!("-> {class}"),
                MemberAccessOutcome::UnknownReceiver => "unknown receiver".to_owned(),
                MemberAccessOutcome::UnknownClass { class } => format!("unknown class {class}"),
                MemberAccessOutcome::IncompleteHierarchy { class } => {
                    format!("incomplete {class}")
                }
                MemberAccessOutcome::Dynamic { class } => format!("dynamic {class}"),
                MemberAccessOutcome::MissingMember { class } => format!("missing on {class}"),
                MemberAccessOutcome::NotAnObject { kind } => {
                    format!("not an object: {}", kind.as_str())
                }
                MemberAccessOutcome::AnyObject => "any object".to_owned(),
                MemberAccessOutcome::ShapeProperty => "a property its shape names".to_owned(),
            };
            format!(
                "{} {} {} {outcome}\n",
                world.render_range(file, access.range),
                access.kind.as_str(),
                access.name
            )
        })
        .collect::<String>();

    expect.assert_eq(&rendered);
}

#[test]
fn every_property_and_method_access_in_source_order() {
    check(
        r#"
//- /app.php
<?php
class Repo {
    public function find(int $id): void {}
}
class Service {
    private Repo $repo;
    public function run(): void {
        $this->repo->find(1);
        self::$shared->reset();
        Repo::create();
    }
}
"#,
        expect![[r#"
            8:16-8:20 property repo -> Service
            8:22-8:26 method find -> Repo
            9:15-9:22 property shared missing on Service
            9:24-9:29 method reset unknown receiver
            10:15-10:21 method create missing on Repo
        "#]],
    );
}

#[test]
fn a_closure_body_is_not_skipped() {
    check(
        r#"
//- /app.php
<?php
class Service {
    public function run(): void {
        $callback = function () {
            $this->helper->go();
        };
        $this->direct();
    }
}
"#,
        expect![[r#"
            5:20-5:26 property helper missing on Service
            5:28-5:30 method go unknown receiver
            7:16-7:22 method direct missing on Service
        "#]],
    );
}

#[test]
fn a_declaration_is_not_a_reference() {
    check(
        r#"
//- /app.php
<?php
class Service {
    public int $count = 0;
    public function run(): void {}
}
"#,
        expect![""],
    );
}

#[test]
fn an_access_that_does_not_resolve_says_which_step_failed() {
    check(
        r#"
//- /app.php
<?php
class Repo {
    public function find(int $id): void {}
}
class Service {
    private Repo $repo;
    private $untyped;
    private Missing $absent;
    public function run(): void {
        $this->repo->find(1);
        $this->repo->nope();
        $this->untyped->anything();
        $this->absent->whatever();
    }
}
"#,
        expect![[r#"
            10:16-10:20 property repo -> Service
            10:22-10:26 method find -> Repo
            11:16-11:20 property repo -> Service
            11:22-11:26 method nope missing on Repo
            12:16-12:23 property untyped -> Service
            12:25-12:33 method anything unknown receiver
            13:16-13:22 property absent -> Service
            13:24-13:32 method whatever unknown class Missing
        "#]],
    );
}

/// `object` is an answer, and a `string` is a different answer, and neither is the absence of one.
///
/// The three sit next to each other here because a coverage sweep ranks the next piece of work by
/// telling them apart: an untyped receiver is a gap inference could close, an `object` is the author
/// saying every class at once, and a `string` is proof the line cannot run. Collapsing the middle
/// one into the first put 232 unfixable sites at the top of a list of things to go and fix.
///
/// `mixed` stays with the untyped, and that is the load-bearing half: it reads the same from outside
/// and is as often a docblock that could have said more.
#[test]
fn a_receiver_declared_object_is_apart_from_one_with_no_type_at_all() {
    check(
        r#"
//- /app.php
<?php
class Service {
    public function run(object $any, mixed $whatever, string $name, $nothing): void {
        $any->anything();
        $whatever->anything();
        $name->anything();
        $nothing->anything();
    }
}
"#,
        expect![[r#"
            4:15-4:23 method anything any object
            5:20-5:28 method anything unknown receiver
            6:16-6:24 method anything not an object: string
            7:19-7:27 method anything unknown receiver
        "#]],
    );
}

/// An object shape names its properties, so reaching one is as settled as reaching a declared
/// member -- and anything else on it is the bare `object` it otherwise is.
#[test]
fn a_property_an_object_shape_names_is_reached() {
    check(
        r#"
//- /app.php
<?php
class Service {
    /** @param object{name: string} $row */
    public function run(object $row): void {
        $row->name;
        $row->other;
        $row->name();
    }
}
"#,
        expect![[r#"
            5:15-5:19 property name a property its shape names
            6:15-6:20 property other any object
            7:15-7:19 method name any object
        "#]],
    );
}

#[test]
fn a_broken_hierarchy_is_not_a_missing_member() {
    // The distinction a checker lives or dies by: one unindexed base class does not turn every
    // inherited call into a defect.
    check(
        r#"
//- /app.php
<?php
class Complete {
    public function known(): void {}
}
class Derived extends Complete {
    public function run(): void {
        $this->known();
        $this->typo();
    }
}
class Broken extends NotIndexed {
    public function run(): void {
        $this->inherited();
    }
}
"#,
        expect![[r#"
            7:16-7:21 method known -> Derived
            8:16-8:20 method typo missing on Derived
            13:16-13:25 method inherited incomplete Broken
        "#]],
    );
}

/// Whether asking for one name gives the same answer as filtering the whole file's walk.
///
/// The invariant behind the narrowed projection: a project-wide search asks every candidate file
/// for one member, and resolving all two hundred of that file's accesses to find it is the single
/// most expensive thing a request can do. The saving is only sound while the two agree, and they
/// are two separate walks -- so this compares the whole result rather than a count.
fn check_named(fixture: &str, name: &str, kind: MemberKind, expect: Expect) {
    let world = World::parse(fixture);
    let file = world.any_file().expect("the fixture declares a file");
    let open = world.open(OpenScope::CursorFileOnly, Some(file));
    let view = IndexView::in_file(world.skeleton(), &open, file);

    let narrowed = view.member_accesses_named(name, kind);
    let filtered = view
        .member_accesses()
        .iter()
        .filter(|access| match kind == MemberKind::Method {
            true => access.name.as_str().eq_ignore_ascii_case(name),
            false => access.name.as_str() == name,
        })
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(
        narrowed, filtered,
        "asking for one name disagreed with filtering the whole file"
    );

    let rendered = narrowed
        .iter()
        .map(|access| {
            format!(
                "{} {} {}\n",
                world.render_range(file, access.range),
                access.kind.as_str(),
                access.name
            )
        })
        .collect::<String>();
    expect.assert_eq(&rendered);
}

#[test]
fn asking_for_one_member_name_finds_every_spelling_of_reaching_it_and_nothing_else() {
    check_named(
        r#"
//- /app.php
<?php
class Repo {
    public int $find = 1;
    public function find(int $id): void {}
    public static function findAll(): void {}
}
class Service {
    public function run(Repo $repo): void {
        $repo->find(1);
        Repo::findAll();
        $fn = $repo->find(...);
        $property = $repo->find;
        $repo->other();
    }
}
"#,
        "find",
        MemberKind::Method,
        expect![[r#"
            9:16-9:20 method find
            11:22-11:26 method find
            12:28-12:32 property find
        "#]],
    );
}

/// PHP's method names are case-insensitive, so a call written in another case reaches the same
/// declaration -- and a search that missed it left a rename rewriting some of a method's call sites
/// and not the rest. A property is not: `$r->Find` and `$r->find` are two properties.
#[test]
fn a_method_is_found_in_any_case_and_a_property_only_in_its_own() {
    check_named(
        r#"
//- /app.php
<?php
class Repo {
    public int $find = 1;
    public function find(int $id): void {}
}
class Service {
    public function run(Repo $repo): void {
        $repo->FIND(1);
        $repo->Find(2);
        $property = $repo->Find;
    }
}
"#,
        "find",
        MemberKind::Method,
        expect![[r#"
            8:16-8:20 method FIND
            9:16-9:20 method Find
            10:28-10:32 property Find
        "#]],
    );
    check_named(
        r#"
//- /app.php
<?php
class Repo {
    public int $find = 1;
}
class Service {
    public function run(Repo $repo): void {
        $a = $repo->find;
        $b = $repo->Find;
    }
}
"#,
        "find",
        MemberKind::Property,
        expect![[r#"
            7:21-7:25 property find
        "#]],
    );
}

#[test]
fn the_class_constant_names_the_class_itself() {
    check(
        r#"
//- /app.php
<?php
class Repo {}
class Service {
    public function run(): void {
        $name = Repo::class;
    }
}
"#,
        expect![[r#"
            5:23-5:28 const class -> Repo
        "#]],
    );
}

#[test]
fn this_in_a_trait_is_also_what_the_trait_requires() {
    // The trait's own member is the trait's, the interface's member is the interface's, and a
    // member neither declares is one the composing class may -- which is what the trait alone has
    // always answered. An intersection nothing in which declares the member is missing on it.
    check(
        r#"
//- /app.php
<?php
interface Clock {
    public function month(): int;
}
/** @require-implements Clock */
trait Months {
    public function next(): int {
        $this->month();
        $this->own();
        $this->elsewhere();
        return 1;
    }
    public function own(): void {}
}
interface Sized {
    public function size(): int;
}
class Repo {
    public function run(Clock&Sized $both): void {
        $both->size();
        $both->nope();
    }
}
"#,
        expect![[r#"
            8:16-8:21 method month -> Clock
            9:16-9:19 method own -> Months
            10:16-10:25 method elsewhere dynamic Months
            20:16-20:20 method size -> Sized
            21:16-21:20 method nope missing on Clock
        "#]],
    );
}

/// The same beside a `false`, which is how a cache read is declared: `: FALSE|object` with the
/// item's shape in a `@phpstan-type` on the trait that reads it. The parameter is the written half
/// of the same rule.
#[test]
fn a_property_of_a_shape_beside_false_is_reached() {
    check(
        r#"
//- /app.php
<?php
/**
 * @phpstan-type CachedItem object{data: string, valid: bool}
 */
trait Cache {
    /** @return FALSE|CachedItem */
    public function cacheGet(): FALSE|object {}

    /** @param false|object{data: string} $given */
    public function run(false|object $given): void {
        $item = $this->cacheGet();
        if ($item) {
            $item->valid;
            $item->other;
        }
        if ($given) {
            $given->data;
        }
    }
}
"#,
        expect![[r#"
            11:24-11:32 method cacheGet -> Cache
            13:20-13:25 property valid a property its shape names
            14:20-14:25 property other any object
            17:21-17:25 property data a property its shape names
        "#]],
    );
}
