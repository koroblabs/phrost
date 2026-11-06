//! Finding a declaration by name across the whole project.
//!
//! The layer that owns ranking, so the snapshots here are of the **whole** result and in order. An
//! assertion that a name appears somewhere in the list would pass for a search that buried the
//! project's own method under two hundred framework hits, which is the failure this feature is most
//! likely to have.
//!
//! Run through the open-versus-closed dimension with no cursor file: a project-wide search reads the
//! skeleton, so which buffers happen to be open must not reach the answer at all.

use expect_test::{Expect, expect};
use phrost_harness::{World, with_open_scopes};

use crate::{IndexView, SymbolLocation};

/// How many hits the tests ask for. Small enough that a fixture can reach it deliberately.
const LIMIT: usize = 8;

/// Every hit for `query`, in rank order, as `kind name [container] path`.
fn check(fixture: &str, query: &str, expect: Expect) {
    check_capped(fixture, query, LIMIT, expect);
}

fn check_capped(fixture: &str, query: &str, limit: usize, expect: Expect) {
    let world = World::parse(fixture);

    let rendered = with_open_scopes(&world, None, |open| {
        let view = IndexView::new(world.skeleton(), open);
        let found = view
            .search_symbols(query, limit)
            .iter()
            .map(|hit| render(&world, hit))
            .collect::<String>();

        if found.is_empty() {
            "<nothing found>\n".to_owned()
        } else {
            found
        }
    });

    expect.assert_eq(&rendered);
}

fn render(world: &World, hit: &SymbolLocation) -> String {
    let container = hit
        .container
        .as_ref()
        .map(|owner| format!(" [{}]", owner.as_str()))
        .unwrap_or_default();
    format!(
        "{} {}{} {} {}\n",
        hit.kind.as_str(),
        hit.fqn.as_str(),
        container,
        world.path_of(hit.file),
        world.render_range(hit.file, hit.name_range)
    )
}

/// A project that declares the same verb as a class, a function, and four kinds of member -- and a
/// dependency that declares the lot again, so ranking has something to get wrong.
const ITEMS: &str = r#"
//- /src/ItemStorage.php
<?php
namespace App;

class ItemStorage
{
    public const LOAD_LIMIT = 50;

    public string $loadedAt = '';

    public function load(int $id): void {}

    public function loadByItemNumber(string $number): void {}

    private function unrelated(): void {}
}

//- /src/Loader.php
<?php
namespace App;

class Loader {}

function loadAll(): void {}

//- /src/Stage.php
<?php
namespace App;

enum Stage: string
{
    case Loaded = 'loaded';
    case Draft = 'draft';
}

//- /vendor/pkg/Loader.php
<?php
namespace Pkg;

class Loader
{
    public function load(): void {}

    public function loadByItemNumber(string $number): void {}
}
"#;

#[test]
fn a_member_is_found_by_its_own_name() {
    // The whole point of the feature: a method nothing but its name is known about. Before this,
    // the catalog held top-level names only and the answer was empty.
    check(
        ITEMS,
        "loadByItemNumber",
        expect![[r#"
            method loadByItemNumber [App\ItemStorage] src/ItemStorage.php 12:21-12:37
        "#]],
    );
}

#[test]
fn methods_properties_constants_and_cases_all_answer() {
    // Four member kinds and two top-level ones in one ranked list. Exact matches first because the
    // key measures the name the query was typed against, then longer names, and the project's own
    // code before the dependency's.
    check(
        ITEMS,
        "load",
        expect![[r#"
            method load [App\ItemStorage] src/ItemStorage.php 10:21-10:25
            case Loaded [App\Stage] src/Stage.php 6:10-6:16
            class App\Loader src/Loader.php 4:7-4:13
            function App\loadAll src/Loader.php 6:10-6:17
            property loadedAt [App\ItemStorage] src/ItemStorage.php 8:19-8:28
            const LOAD_LIMIT [App\ItemStorage] src/ItemStorage.php 6:18-6:28
            method loadByItemNumber [App\ItemStorage] src/ItemStorage.php 12:21-12:37
            class Pkg\Loader vendor/pkg/Loader.php 4:7-4:13
        "#]],
    );
}

#[test]
fn a_dependencys_members_are_not_offered() {
    // `Pkg\Loader::load` and `Pkg\Loader::loadByItemNumber` exist and are deliberately absent: a
    // member of a dependency is arrived at by navigating from a call, and scanning every member row
    // in `vendor/` per keystroke is the cost this index exists not to pay. The dependency's *class*
    // is still offered, because the catalog holds it for nothing.
    check(
        ITEMS,
        "loadbyitemnumber",
        expect![[r#"
            method loadByItemNumber [App\ItemStorage] src/ItemStorage.php 12:21-12:37
        "#]],
    );
}

#[test]
fn the_page_is_the_best_hits_and_not_the_first_ones_found() {
    // The cap decides what a person sees, and the protocol has no way to say a search was cut
    // short -- so what survives it has to be chosen by rank rather than by what the walk reached
    // first. The member half of the walk runs after the catalog half, so a cap applied during
    // collection would have returned three classes and no method at all.
    check_capped(
        ITEMS,
        "load",
        3,
        expect![[r#"
            method load [App\ItemStorage] src/ItemStorage.php 10:21-10:25
            case Loaded [App\Stage] src/Stage.php 6:10-6:16
            class App\Loader src/Loader.php 4:7-4:13
        "#]],
    );
}

#[test]
fn a_query_matching_nothing_answers_nothing() {
    check(ITEMS, "unrelatedentirely", expect!["<nothing found>\n"]);
}

#[test]
fn a_private_member_is_still_findable() {
    // Visibility governs what may be *reached* from a place in the code. A symbol search has no
    // such place -- it is a person looking for something they wrote -- so filtering by it would
    // hide exactly the helper somebody is trying to find their way back to.
    check(
        ITEMS,
        "unrelated",
        expect![[r#"
            method unrelated [App\ItemStorage] src/ItemStorage.php 14:22-14:31
        "#]],
    );
}

#[test]
fn a_member_of_an_anonymous_class_is_not_offered() {
    // Its declaring class has a synthesised name nobody can write, so the container beside the row
    // would name something a reader cannot go and look at. The named class in the same file still
    // answers, so this is not the walk failing to reach the file.
    check(
        r#"
//- /src/Factory.php
<?php
namespace App;

class Factory
{
    public function tagged(): object
    {
        return new class {
            public function taggedInner(): void {}
        };
    }
}
"#,
        "tagged",
        expect![[r#"
            method tagged [App\Factory] src/Factory.php 6:21-6:27
        "#]],
    );
}

#[test]
fn a_member_a_docblock_declares_is_offered_like_any_other() {
    // `@method` and `@property` are how a framework states the API of code it generates, so leaving
    // them out would make a class's whole documented surface unfindable. Goto lands in the comment,
    // which is what the declaration is -- the location says so, and hover explains it.
    check(
        r#"
//- /src/Magic.php
<?php
namespace App;

/**
 * @method void loadTagged(int $id)
 * @property string $loadedTag
 */
class Magic {}
"#,
        "load",
        expect![[r#"
            property loadedTag [App\Magic] src/Magic.php 6:21-6:31
            method loadTagged [App\Magic] src/Magic.php 5:17-5:27
        "#]],
    );
}

/// From three characters on, a name is also found the way completion finds one: by subsequence,
/// ranked by how well the letters fit -- after every prefix hit, which is what a person usually
/// means. `lbi` is `loadByItemNumber` by its humps, and the dependency's copy of the method stays
/// out for the reason a dependency's members always do.
#[test]
fn a_member_is_found_by_its_humps() {
    check(
        ITEMS,
        "lbi",
        expect![[r#"
            method loadByItemNumber [App\ItemStorage] src/ItemStorage.php 12:21-12:37
        "#]],
    );
}

#[test]
fn a_class_is_found_by_its_humps() {
    check(
        ITEMS,
        "ItSt",
        expect![[r#"
            class App\ItemStorage src/ItemStorage.php 4:7-4:18
        "#]],
    );
}

#[test]
fn prefix_hits_come_before_loose_ones() {
    // `loade` begins `Loaded`, `loadedAt` and both `Loader`s, and is scattered through
    // `loadByItemNumber` -- which comes last however well it scores, because it does not begin
    // with what was typed.
    check(
        ITEMS,
        "loade",
        expect![[r#"
            case Loaded [App\Stage] src/Stage.php 6:10-6:16
            class App\Loader src/Loader.php 4:7-4:13
            property loadedAt [App\ItemStorage] src/ItemStorage.php 8:19-8:28
            method loadByItemNumber [App\ItemStorage] src/ItemStorage.php 12:21-12:37
            class Pkg\Loader vendor/pkg/Loader.php 4:7-4:13
        "#]],
    );
}

#[test]
fn two_letters_are_matched_by_prefix_only() {
    // Below the floor the ordering constraint says nothing -- `lb` is a subsequence of a great many
    // names -- so nothing loose is offered and a query that begins nothing finds nothing.
    check(
        ITEMS,
        "lb",
        expect![[r#"
        <nothing found>
    "#]],
    );
}
