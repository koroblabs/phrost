//! Which declaration a call is written inside, and which declaration it reaches.
//!
//! Both halves in one snapshot, because a call graph is only useful when the two agree: a caller
//! attributed to the wrong declaration and a callee resolved to the wrong one produce the same
//! symptom -- a tree that looks right and jumps wrong -- and separate snapshots would let either
//! break while the other read as fine.
//!
//! Every fixture runs through the open-versus-closed dimension. That is the load-bearing one here:
//! a callee is nearly always declared in another file, and resolving it must not depend on whether
//! that file happens to be open.

use std::fmt::Write as _;

use expect_test::{Expect, expect};
use phrost_harness::{World, with_open_scopes};
use phrost_model::TextRange;

use crate::{IndexView, SymbolLocation};

/// Renders every call the marked file writes: where it sits, and where it lands.
fn check(fixture: &str, expect: Expect) {
    let world = World::parse(fixture);
    let file = world.file_id("app.php");

    let rendered = with_open_scopes(&world, Some(file), |open| {
        let view = IndexView::in_file(world.skeleton(), open, file);
        let whole = TextRange::new(
            0.into(),
            u32::try_from(world.text_of(file).len()).unwrap().into(),
        );

        let mut out = String::new();
        for call in view.calls_within(whole) {
            let caller = view
                .enclosing_declaration(call.at.start())
                .map_or_else(|| "<top level>".to_owned(), |found| declaration(&found));
            let _ = writeln!(
                out,
                "{} {caller} -> {}",
                world.render_range(file, call.at),
                declaration(&call.callee)
            );
        }
        out
    });

    expect.assert_eq(&rendered);
}

/// Renders every call in the marked file that reaches one function or one class's constructor.
///
/// The incoming side's two projections, which nothing above can substitute for: a free function is
/// in PHP's other symbol table, and `new Repo()` never writes the name of what it calls.
fn check_reaching(fixture: &str, function: &str, class: &str, expect: Expect) {
    let world = World::parse(fixture);
    let file = world.file_id("app.php");

    let rendered = with_open_scopes(&world, Some(file), |open| {
        let view = IndexView::in_file(world.skeleton(), open, file);

        let mut out = String::new();
        let _ = writeln!(out, "calls to {function}()");
        for at in view.function_calls_to(function) {
            let _ = writeln!(out, "  {}", world.render_range(file, at));
        }
        let _ = writeln!(out, "new {class}");
        for at in view.constructions_of(class) {
            let _ = writeln!(out, "  {}", world.render_range(file, at));
        }
        out
    });

    expect.assert_eq(&rendered);
}

/// Renders every `new` in the marked file that runs each named class's constructor.
///
/// Several classes in one snapshot, because the answer is a partition: a `new` belongs to exactly
/// one of them, and a rule that leaked a site into two lists -- or into none -- reads as a correct
/// list for whichever class is looked at alone.
fn check_constructions(fixture: &str, classes: &[&str], expect: Expect) {
    let world = World::parse(fixture);
    let file = world.file_id("app.php");

    let rendered = with_open_scopes(&world, Some(file), |open| {
        let view = IndexView::in_file(world.skeleton(), open, file);

        let mut out = String::new();
        for class in classes {
            let _ = writeln!(out, "runs {class}::__construct");
            for at in view.constructions_of(class) {
                let _ = writeln!(out, "  {}", world.render_range(file, at));
            }
        }
        out
    });

    expect.assert_eq(&rendered);
}

/// A declaration as `Class::member`, or as its own name when it has no container.
fn declaration(location: &SymbolLocation) -> String {
    match &location.container {
        Some(class) => format!("{class}::{}", location.fqn),
        None => location.fqn.to_string(),
    }
}

#[test]
fn a_call_is_attributed_to_the_method_it_is_written_in_and_to_what_it_reaches() {
    check(
        r#"
//- /repo.php
<?php
namespace App;

class Repo
{
    public function find(int $id): void {}
    public static function create(): self {}
}

//- /app.php
<?php
namespace App;

class Service
{
    private Repo $repo;

    public function run(): void
    {
        $this->repo->find(1);
        Repo::create();
    }

    public function again(): void
    {
        $this->run();
    }
}
"#,
        expect![[r#"
            10:22-10:26 App\Service::run -> App\Repo::find
            11:15-11:21 App\Service::run -> App\Repo::create
            16:16-16:19 App\Service::again -> App\Service::run
        "#]],
    );
}

#[test]
fn a_call_inside_a_closure_belongs_to_the_declaration_the_closure_is_written_in() {
    // The narrowest *declaration*, not the narrowest scope. A closure is not something a call
    // hierarchy can name, and attributing its calls to the class instead would put them a level too
    // high.
    check(
        r#"
//- /repo.php
<?php
namespace App;

class Repo
{
    public function find(int $id): void {}
}

//- /app.php
<?php
namespace App;

class Service
{
    public function run(Repo $repo, array $ids): void
    {
        array_map(fn (int $id) => $repo->find($id), $ids);
    }
}
"#,
        expect![[r#"
            8:42-8:46 App\Service::run -> App\Repo::find
        "#]],
    );
}

#[test]
fn a_call_written_at_a_files_top_level_has_no_enclosing_declaration() {
    check(
        r#"
//- /repo.php
<?php
namespace App;

class Repo
{
    public static function create(): self {}
}

//- /app.php
<?php
namespace App;

Repo::create();
"#,
        expect![[r#"
            4:7-4:13 <top level> -> App\Repo::create
        "#]],
    );
}

#[test]
fn a_new_reaches_the_constructor_and_a_class_without_one_reaches_the_class() {
    check(
        r#"
//- /repo.php
<?php
namespace App;

class Repo
{
    public function __construct(private int $id) {}
}

class Plain {}

//- /app.php
<?php
namespace App;

class Service
{
    public function run(): void
    {
        new Repo(1);
        new Plain();
    }
}
"#,
        expect![[r#"
            8:13-8:17 App\Service::run -> App\Repo::__construct
            9:13-9:18 App\Service::run -> App\Plain
        "#]],
    );
}

#[test]
fn a_call_through_a_receiver_of_unknown_type_reaches_nothing() {
    // Silence rather than a guess. A row that pointed at the wrong `find` would be offered by the
    // tree and jumped to, which is worse than an absent row.
    check(
        r#"
//- /repo.php
<?php
namespace App;

class Repo
{
    public function find(int $id): void {}
}

//- /app.php
<?php
namespace App;

class Service
{
    public function run($anything): void
    {
        $anything->find(1);
    }
}
"#,
        expect![""],
    );
}

#[test]
fn a_function_call_and_a_new_are_matched_by_what_they_resolve_to() {
    // Not by the text. Two functions and two classes share a short name here, and a search that
    // answered for both would be worse than useless for deciding whether a change is safe.
    check_reaching(
        r#"
//- /lib.php
<?php
namespace App;

function format(string $text): string {}

class Repo {}

//- /other.php
<?php
namespace Other;

function format(string $text): string {}

class Repo {}

//- /app.php
<?php
namespace App;

use Other\Repo as OtherRepo;

class Service
{
    public function run(): void
    {
        format('a');
        \Other\format('b');
        new Repo();
        new OtherRepo();
    }
}
"#,
        "App\\format",
        "App\\Repo",
        expect![[r#"
            calls to App\format()
              10:9-10:15
            new App\Repo
              12:13-12:17
        "#]],
    );
}

#[test]
fn a_new_of_a_subclass_runs_the_constructor_it_inherits() {
    // The one spelling of a call that names neither the member nor the class declaring it. Matched
    // by what the `new` resolves to, which is the rule `member_uses` applies to every other call --
    // written against the instantiated name instead, a constructor's caller list holds only the
    // sites that happen to name the base class, and on real code that is a small minority of them.
    check_constructions(
        r#"
//- /grades.php
<?php
namespace App;

class BaseTier
{
    public function __construct(private int $score) {}
}

class AutoTier extends BaseTier {}

class DeepTier extends AutoTier {}

class OwnTier extends BaseTier
{
    public function __construct()
    {
        parent::__construct(1);
    }
}

class UnderOwn extends OwnTier {}

//- /app.php
<?php
namespace App;

class Service
{
    public function run(): void
    {
        new BaseTier(1);
        new AutoTier(2);
        new DeepTier(3);
        new OwnTier();
        new UnderOwn();
    }
}
"#,
        &["App\\BaseTier", "App\\OwnTier"],
        expect![[r#"
            runs App\BaseTier::__construct
              8:13-8:21
              9:13-9:21
              10:13-10:21
            runs App\OwnTier::__construct
              11:13-11:20
              12:13-12:21
        "#]],
    );
}

#[test]
fn a_new_of_a_class_that_declares_no_constructor_anywhere_stands_for_the_class() {
    // The other half of the same rule, and the reason it cannot be written as "the declaring
    // class": nothing declares one, so what `new` reaches is the class itself -- which is also what
    // the outgoing half of the call graph resolves such a `new` to.
    check_constructions(
        r#"
//- /plain.php
<?php
namespace App;

class Plain {}

class AlsoPlain extends Plain {}

//- /app.php
<?php
namespace App;

class Service
{
    public function run(): void
    {
        new Plain();
        new AlsoPlain();
    }
}
"#,
        &["App\\Plain", "App\\AlsoPlain"],
        expect![[r#"
            runs App\Plain::__construct
              8:13-8:18
            runs App\AlsoPlain::__construct
              9:13-9:22
        "#]],
    );
}
