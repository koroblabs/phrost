//! Who calls this, and what does this call.
//!
//! One fixture, three sections, because the three requests are one feature and the way they go
//! wrong is by disagreeing: an item prepared as one declaration and resolved back as another
//! produces a tree that expands to nothing, and separate snapshots would let that pass.
//!
//! The item is deliberately round-tripped through its key before either follow-up is asked, since
//! that is all a client hands back -- a follow-up that quietly used the prepared value would test a
//! path the protocol never takes.
//!
//! # Which parts carry the open-versus-closed dimension
//!
//! The head and the outgoing calls do, through the harness: both are about the file under the
//! cursor, and both resolve into other files, so a callee that only resolved for an open file would
//! fail here. Incoming calls cannot -- a project-wide search is defined as reading files nobody has
//! open, and the workspace's sweep opens each candidate itself. So that section runs with the whole
//! fixture open, which is what the sweep amounts to, exactly as the find-references test does.

use std::fmt::Write as _;

use expect_test::{Expect, expect};
use phrost_harness::{OpenScope, World, with_open_scopes};
use phrost_model::FileId;
use phrost_testkit::FixtureSpec;
use phrost_view::IndexView;

use crate::{
    CallSubject, call_hierarchy_head, callable_declaration, incoming_calls, outgoing_calls,
    subject_of,
};

fn check(fixture: &str, marker: &str, expect: Expect) {
    let spec = FixtureSpec::parse(fixture);
    let world = World::build(&spec);
    let position = spec.markers().position(marker);
    let cursor = world.file_id(&position.path);

    // The head and what it calls, under both open scopes: a callee is declared elsewhere, so this
    // is where an answer that got quietly better for open files would show up.
    let mut rendered = with_open_scopes(&world, Some(cursor), |open| {
        let view = IndexView::in_file(world.skeleton(), open, cursor);
        let Some(head) = call_hierarchy_head(&view, position.offset) else {
            return "head: nothing\n".to_owned();
        };
        // What the client gets, and what it hands back. Nothing below may use `head` directly.
        let subject = CallSubject::of_key(&subject_of(&head).key());

        let mut out = String::new();
        let _ = writeln!(
            out,
            "head: {} [{:?}] {}",
            subject.key(),
            head.kind,
            place(&world, head.file, head.selection_range)
        );
        let _ = writeln!(
            out,
            "sweeps for: {}",
            subject.sweep_query(&view).every_word().join(", ")
        );

        let _ = writeln!(out, "outgoing:");
        let Some(declared) = callable_declaration(&view, &subject) else {
            return out;
        };
        // The view has to be over the file the declaration is in, which is what the workspace
        // arranges for a closed one.
        let body = IndexView::in_file(world.skeleton(), open, declared.file);
        for call in outgoing_calls(&body, &subject) {
            let _ = writeln!(
                out,
                "  {} at {}",
                name_of(&call.callee),
                places(&world, declared.file, &call.at)
            );
        }
        out
    });

    // The sweep, as the workspace runs it: every file the project owns, one at a time.
    let open = world.open(OpenScope::EveryFile, Some(cursor));
    let view = IndexView::in_file(world.skeleton(), &open, cursor);
    let subject = match call_hierarchy_head(&view, position.offset) {
        Some(head) => CallSubject::of_key(&subject_of(&head).key()),
        None => {
            spec.markers().assert_all_used();
            expect.assert_eq(&rendered);
            return;
        }
    };

    let _ = writeln!(rendered, "incoming:");
    for file in spec
        .files()
        .iter()
        .filter(|file| file.path().ends_with(".php"))
    {
        let id = world.file_id(file.path());
        let view = IndexView::in_file(world.skeleton(), &open, id);
        for call in incoming_calls(&view, &subject) {
            let _ = writeln!(
                rendered,
                "  {} at {}",
                name_of(&call.caller),
                places(&world, id, &call.at)
            );
        }
    }

    spec.markers().assert_all_used();
    expect.assert_eq(&rendered);
}

/// A target as `Class::member`, or as its own name when it has no container.
fn name_of(target: &crate::NavigationTarget) -> String {
    match &target.container {
        Some(class) => format!("{class}::{}", target.name),
        None => target.name.to_string(),
    }
}

fn place(world: &World, file: FileId, range: phrost_model::TextRange) -> String {
    format!(
        "{} {}",
        world.path_of(file),
        world.render_range(file, range)
    )
}

fn places(world: &World, file: FileId, at: &[phrost_model::TextRange]) -> String {
    let spans = at
        .iter()
        .map(|range| world.render_range(file, *range))
        .collect::<Vec<_>>();
    format!("{} {}", world.path_of(file), spans.join(" "))
}

/// A repository with one method called from two other files, twice from one of them.
const CALLED_FROM_TWO_FILES: &str = r#"
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    public function fi@cursor@nd(int $id): void {}

    public function findAll(array $ids): void
    {
        foreach ($ids as $id) {
            $this->find($id);
        }
    }
}

//- /src/One.php
<?php
namespace App;

class One
{
    public function run(Repo $repo): void
    {
        $repo->find(1);
        $repo->find(2);
    }
}

//- /src/Two.php
<?php
namespace App;

class Two
{
    public function run(Repo $repo): void
    {
        $repo->find(3);
    }
}

//- /src/Unrelated.php
<?php
namespace App;

class Unrelated
{
    public function find(int $id): void {}

    public function run(): void
    {
        $this->find(4);
    }
}
"#;

#[test]
fn every_caller_is_listed_once_with_each_of_its_call_sites() {
    // Three callers and four sites. `One::run` appears once carrying two ranges, because that is
    // what the protocol asks for: the tree shows one row per caller and jumps to each site in turn.
    //
    // `Unrelated::run` calls a `find` of its own and must not appear. The search is for a
    // declaration, not for a name.
    check(
        CALLED_FROM_TWO_FILES,
        "cursor",
        expect![[r#"
            head: App\Repo::find [Method] src/Repo.php 6:21-6:25
            sweeps for: find
            outgoing:
            incoming:
              App\Repo::findAll at src/Repo.php 11:20-11:24
              App\One::run at src/One.php 8:16-8:20 9:16-9:20
              App\Two::run at src/Two.php 8:16-8:20
        "#]],
    );
}

#[test]
fn a_cursor_resting_in_a_body_prepares_the_declaration_it_is_written_in() {
    // How the request usually arrives: an editor's shortcut sends wherever the caret is, and a
    // refusal there reads as a broken feature. A cursor *on* a name still wins -- see the test
    // above, whose marker is on the declaration's own name.
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    public function find(int $id): void {}
}

//- /src/One.php
<?php
namespace App;

class One
{
    public function run(Repo $repo): void
    {
        $count @cursor@= 1;
        $repo->find($count);
    }
}
"#,
        "cursor",
        expect![[r#"
            head: App\One::run [Method] src/One.php 6:21-6:24
            sweeps for: run
            outgoing:
              App\Repo::find at src/One.php 9:16-9:20
            incoming:
        "#]],
    );
}

#[test]
fn outgoing_calls_group_by_callee_and_point_at_the_calls_own_names() {
    // The ranges belong to the *caller's* file and to the callee's name as written there. Reporting
    // the whole call expression would select its arguments too, and reporting the callee's own
    // declaration range would send the editor to the wrong file entirely.
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    public function find(int $id): void {}
    public function save(int $id): void {}
}

//- /src/Helpers.php
<?php
namespace App;

function normalise(int $id): int {}

//- /src/One.php
<?php
namespace App;

class One
{
    public function ru@cursor@n(Repo $repo): void
    {
        $repo->find(normalise(1));
        $repo->find(2);
        $repo->save(3);
        new Repo();
    }
}
"#,
        "cursor",
        expect![[r#"
            head: App\One::run [Method] src/One.php 6:21-6:24
            sweeps for: run
            outgoing:
              App\Repo::find at src/One.php 8:16-8:20 9:16-9:20
              App\normalise at src/One.php 8:21-8:30
              App\Repo::save at src/One.php 10:16-10:20
              App\Repo at src/One.php 11:13-11:17
            incoming:
        "#]],
    );
}

#[test]
fn a_free_function_is_found_by_what_the_written_name_resolves_to() {
    // PHP's other symbol table, which nothing else in this crate searches: a class-name sweep never
    // sees a function call, so "who calls this hook" needs its own projection. Two functions share
    // the short name here and only one of them is the answer.
    check(
        r#"
//- /src/Helpers.php
<?php
namespace App;

function norma@cursor@lise(int $id): int {}

//- /src/Other.php
<?php
namespace Other;

function normalise(int $id): int {}

//- /src/One.php
<?php
namespace App;

class One
{
    public function run(): void
    {
        normalise(1);
        \Other\normalise(2);
    }
}
"#,
        "cursor",
        expect![[r#"
            head: App\normalise [Function] src/Helpers.php 4:10-4:19
            sweeps for: normalise
            outgoing:
            incoming:
              App\One::run at src/One.php 8:9-8:18
        "#]],
    );
}

#[test]
fn a_constructor_is_reached_by_new_as_well_as_by_naming_it() {
    // The one member PHP calls without ever writing its name. A list built from the written spelling
    // alone reads as a constructor nothing uses, and the sweep has to grep for the class's short
    // name as well or every file that only says `new Repo()` is never even a candidate.
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    public function __const@cursor@ruct(int $id) {}
}

//- /src/Child.php
<?php
namespace App;

class Child extends Repo
{
    public function __construct(int $id)
    {
        parent::__construct($id);
    }
}

//- /src/One.php
<?php
namespace App;

class One
{
    public function run(): void
    {
        new Repo(1);
    }
}
"#,
        "cursor",
        expect![[r#"
            head: App\Repo::__construct [Method] src/Repo.php 6:21-6:32
            sweeps for: Repo, __construct
            outgoing:
            incoming:
              App\Child::__construct at src/Child.php 8:17-8:28
              App\One::run at src/One.php 8:13-8:17
        "#]],
    );
}

#[test]
fn an_inherited_method_called_through_a_subclass_belongs_to_the_parent_that_declares_it() {
    // The receiver decides which declaration a call reaches, and it is not always the class the
    // receiver is typed as. A hierarchy keyed on the written receiver would show two separate
    // items for one method and split its callers between them.
    check(
        r#"
//- /src/Base.php
<?php
namespace App;

class Base
{
    public function fi@cursor@nd(int $id): void {}
}

//- /src/Child.php
<?php
namespace App;

class Child extends Base {}

//- /src/One.php
<?php
namespace App;

class One
{
    public function run(Child $child, Base $base): void
    {
        $child->find(1);
        $base->find(2);
    }
}
"#,
        "cursor",
        expect![[r#"
            head: App\Base::find [Method] src/Base.php 6:21-6:25
            sweeps for: find
            outgoing:
            incoming:
              App\One::run at src/One.php 8:17-8:21 9:16-9:20
        "#]],
    );
}

#[test]
fn a_constructor_a_subclass_inherits_is_reached_by_new_on_the_subclass() {
    // `new Child()` runs `Base::__construct` and writes neither name, so the file it is in is not a
    // candidate for a sweep that greps only the two written spellings of the call -- and the caller
    // list comes back holding whichever sites happen to name the base class, which on real code is
    // a small minority. The sweep list is in the snapshot because it is where being wrong is
    // silent: a file that is never read cannot report anything.
    check(
        r#"
//- /src/Base.php
<?php
namespace App;

class Base
{
    public function __const@cursor@ruct(int $id) {}
}

//- /src/Child.php
<?php
namespace App;

class Child extends Base {}

//- /src/Own.php
<?php
namespace App;

class Own extends Base
{
    public function __construct()
    {
        parent::__construct(1);
    }
}

//- /src/One.php
<?php
namespace App;

class One
{
    public function run(): void
    {
        new Child(1);
        new Own();
    }
}
"#,
        "cursor",
        expect![[r#"
            head: App\Base::__construct [Method] src/Base.php 6:21-6:32
            sweeps for: Base, Child, __construct
            outgoing:
            incoming:
              App\Own::__construct at src/Own.php 8:17-8:28
              App\One::run at src/One.php 8:13-8:18
        "#]],
    );
}

/// A call no declaration encloses, whose caller is the file.
///
/// PHP lets one be written at a file's top level, and inside a closure the file returns -- a script,
/// a bootstrap, Symfony's PHP-DSL configuration, a WordPress plugin's body. The caller list used to
/// drop both silently, so it read as complete while it was not: 12 of 250 sampled call sites on one
/// real project and 30 of 240 on Symfony. Found by asking whether every call site goto resolves is
/// also an incoming call of what it resolves to.
///
/// Nothing calls a file, so the row is a leaf -- which is the honest shape rather than a limitation.
#[test]
fn a_call_no_declaration_encloses_is_attributed_to_the_file() {
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    public static function fi@here@nd(int $id): void {}
}

//- /src/script.php
<?php
namespace App;

Repo::find(1);

$run = static function (): void {
    Repo::find(2);
};

//- /src/Caller.php
<?php
namespace App;

class Caller
{
    public function run(): void
    {
        Repo::find(3);
    }
}
"#,
        "here",
        expect![[r#"
            head: App\Repo::find [Method] src/Repo.php 6:28-6:32
            sweeps for: find
            outgoing:
            incoming:
              src/script.php at src/script.php 4:7-4:11 7:11-7:15
              App\Caller::run at src/Caller.php 8:15-8:19
        "#]],
    );
}
