//! A constructor is reached by `new Repo()` and `new Child()`, which never write `__construct`.
//!
//! The project-wide sweep greps every file for the target's name before reading any, and a
//! constructor's name is the one word its callers do not write -- so find-references on one listed
//! the declaration and nothing else, and a rename of one of its parameters left every
//! `new Repo(table: ...)` naming the old one. The analysis tests read every file of a fixture and
//! could not see it; only a server that sweeps can.

use expect_test::expect;
use serde_json::json;

use crate::tests::Fixture;
use crate::tests::flow::{render_edits, render_span};

const PROJECT: &str = r#"
//- /composer.json
{"autoload": {"psr-4": {"App\\": "src"}}}

//- /src/Repo.php
<?php
namespace App;

class Repo
{
    public function __construct(string $table = 'rows') {}
}

class Child extends Repo {}

//- /src/Client.php
<?php
namespace App;

function go(): array
{
    return [new Repo(table: 'users'), new Child(table: 'kids')];
}

//- /src/Unrelated.php
<?php
namespace App;

function nothing(): void {}
"#;

#[tokio::test]
async fn references_on_a_constructor_reach_the_files_that_only_construct() {
    let mut server = Fixture::new(PROJECT);
    server.initialize(false, false).await;
    server.initialized().await;
    server.open("src/Repo.php").await;

    let found = server
        .request(
            "textDocument/references",
            json!({
                "textDocument": { "uri": server.uri("src/Repo.php") },
                "position": { "line": 5, "character": 22 },
                "context": { "includeDeclaration": true },
            }),
        )
        .await;
    let value = serde_json::to_value(found.result().expect("a result")).expect("json");
    let rendered = value
        .as_array()
        .into_iter()
        .flatten()
        .map(|place| {
            let uri = place["uri"].as_str().unwrap_or("");
            format!(
                "{} {}",
                uri.rsplit('/').next().unwrap_or(uri),
                render_span(&place["range"])
            )
        })
        .collect::<Vec<_>>();

    expect![[r#"
        [
            "Client.php 5:16-5:20",
            "Client.php 5:42-5:47",
            "Repo.php 5:20-5:31",
        ]
    "#]]
    .assert_debug_eq(&rendered);
}

#[tokio::test]
async fn a_constructor_parameter_renames_the_labels_of_every_new() {
    let mut server = Fixture::new(PROJECT);
    server.initialize(false, false).await;
    server.initialized().await;
    server.open("src/Repo.php").await;

    let edit = server
        .request(
            "textDocument/rename",
            json!({
                "textDocument": { "uri": server.uri("src/Repo.php") },
                "position": { "line": 5, "character": 41 },
                "newName": "name",
            }),
        )
        .await;

    expect![[r#"
        [
            "Client.php 5:21-5:26 -> name",
            "Client.php 5:48-5:53 -> name",
            "Repo.php 5:40-5:45 -> name",
        ]
    "#]]
    .assert_debug_eq(&render_edits(&edit));
}

/// Every way into a constructor, one file each, and files that only name a class running it.
///
/// The sweep reads a file's text before analysing it, and for a constructor it no longer takes a
/// class's name as reason enough -- `class Child`, `extends Child` and `use App\Child;` construct
/// nothing by themselves, and on a test base class those three are every file of the suite. So
/// each spelling that *does* reach it has to survive the reading, and a file here that the reading
/// wrongly refuses is a caller missing from this list.
const EVERY_WAY_IN: &str = r#"
//- /composer.json
{"autoload": {"psr-4": {"App\\": "src", "Other\\": "other"}}}

//- /src/Repo.php
<?php
namespace App;

class Repo
{
    public function __con@ctor@struct(string $table = 'rows') {}
}

//- /src/Child.php
<?php
namespace App;

class Child extends Repo
{
    public static function make(): static { return new static(); }
    public function copy(): self { return new self(); }
}

//- /src/Leaf.php
<?php
namespace App;

class Leaf extends Child
{
    public function __construct() { parent::__construct('leaves'); }
}

//- /src/Twig.php
<?php
namespace App;

class Twig extends Child
{
    public function sprout(): Child { return new parent(); }
}

//- /src/Plain.php
<?php
namespace App;

function plain(): Repo { return new Repo(); }

//- /src/Anonymous.php
<?php
namespace App;

function anonymous(): Repo { return new class extends Repo {}; }

//- /src/Again.php
<?php
namespace App;

function again(Repo $repo): void { $repo->__construct('again'); }

//- /src/OnlyDeclared.php
<?php
namespace App;

class OnlyDeclared extends Child {}

//- /other/Qualified.php
<?php
namespace Other;

function qualified(): \App\Child { return new \App\Child(); }

//- /other/Aliased.php
<?php
namespace Other;

use App\Child as Kid;

function aliased(): Kid { return new Kid(); }

//- /other/OnlyImported.php
<?php
namespace Other;

use App\Child;

function imported(Child $child): Child { return $child; }
"#;

/// A trait's constructor, which every class taking the trait in runs -- an anonymous one included,
/// whose `use Seeds;` is the only place the file names it.
const DECLARED_BY_A_TRAIT: &str = r#"
//- /composer.json
{"autoload": {"psr-4": {"App\\": "src", "Other\\": "other"}}}

//- /src/Seeds.php
<?php
namespace App;

trait Seeds
{
    public function __con@ctor@struct(int $count = 1) {}
}

//- /src/Field.php
<?php
namespace App;

class Field
{
    use Seeds;
}

//- /src/Farm.php
<?php
namespace App;

function farm(): Field { return new Field(3); }

//- /other/Garden.php
<?php
namespace Other;

function garden(): object { return new class { use \App\Seeds; }; }
"#;

#[tokio::test]
async fn every_way_into_a_constructor_survives_the_reading_of_the_text() {
    let callers = incoming_calls_at(EVERY_WAY_IN, "src/Repo.php").await;

    expect![[r#"
        [
            "Again.php again at 3:42-3:53",
            "Aliased.php aliased at 5:37-5:40",
            "Anonymous.php anonymous at 3:40-3:45",
            "Child.php copy at 6:46-6:50",
            "Child.php make at 5:55-5:61",
            "Leaf.php __construct at 5:44-5:55",
            "Plain.php plain at 3:36-3:40",
            "Qualified.php qualified at 3:46-3:56",
            "Twig.php sprout at 5:49-5:55",
        ]
    "#]]
    .assert_debug_eq(&callers);
}

#[tokio::test]
async fn a_trait_constructor_is_reached_through_a_class_that_takes_the_trait_in() {
    let callers = incoming_calls_at(DECLARED_BY_A_TRAIT, "src/Seeds.php").await;

    expect![[r#"
        [
            "Farm.php farm at 3:36-3:41",
            "Garden.php garden at 3:39-3:44",
        ]
    "#]]
    .assert_debug_eq(&callers);
}

/// The callers of the constructor under `@ctor@` in `file`, one line each, sorted.
async fn incoming_calls_at(fixture: &str, file: &str) -> Vec<String> {
    let mut server = Fixture::new(fixture);
    server.initialize(false, false).await;
    server.initialized().await;
    server.open(file).await;

    let prepared = server
        .request(
            "textDocument/prepareCallHierarchy",
            json!({
                "textDocument": { "uri": server.uri(file) },
                "position": server.position(file, "ctor"),
            }),
        )
        .await;
    let prepared = serde_json::to_value(prepared.result().expect("a result")).expect("json");
    let head = prepared[0].clone();

    let callers = server
        .request("callHierarchy/incomingCalls", json!({ "item": head }))
        .await;
    let value = serde_json::to_value(callers.result().expect("a result")).expect("json");
    let mut rendered = value
        .as_array()
        .into_iter()
        .flatten()
        .map(|call| {
            let uri = call["from"]["uri"].as_str().unwrap_or("");
            let spans = call["fromRanges"]
                .as_array()
                .into_iter()
                .flatten()
                .map(render_span)
                .collect::<Vec<_>>();
            format!(
                "{} {} at {}",
                uri.rsplit('/').next().unwrap_or(uri),
                call["from"]["name"].as_str().unwrap_or(""),
                spans.join(", ")
            )
        })
        .collect::<Vec<_>>();
    rendered.sort();
    rendered
}
