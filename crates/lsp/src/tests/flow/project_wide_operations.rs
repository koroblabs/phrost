//! Operations that see the whole project rather than one open document: symbols, references, rename.
//!
//! Each of these has to work from the index alone -- `references` visits three files while only one
//! is open, and a sweep like that is only affordable because each candidate is analysed into a
//! scratch set the sweep drops, so nothing it touched outlives the request. Rename additionally has
//! to refuse a declaration outside the project's own code, and edit only the last segment of a
//! qualified path.

use expect_test::expect;
use lspkit_runtime::protocol::jsonrpc::Response;
use serde_json::{Value, json};

use crate::tests::flow::{render_edits, render_span};
use crate::tests::{Fixture, error_message};

#[tokio::test]
async fn symbols_are_listed_for_a_file_and_searched_across_the_project() {
    let mut server = Fixture::new(
        r#"
//- /composer.json
{"autoload": {"psr-4": {"App\\": "src"}}}

//- /src/Repo.php
<?php
namespace App;

class Repository
{
    public function find(int $id): void {}
}

//- /src/Service.php
<?php
namespace App;

class Service
{
    public function run(): void {}
}
"#,
    );
    server.initialize(false, false).await;
    server.initialized().await;

    let outline = server
        .request(
            "textDocument/documentSymbol",
            json!({ "textDocument": { "uri": server.uri("src/Repo.php") } }),
        )
        .await;
    let found = server
        .request("workspace/symbol", json!({ "query": "rep" }))
        .await;

    expect![[r#"
        [
            "class Repository 3:0-6:1",
            "  method find 5:4-5:42",
        ]
    "#]]
    .assert_debug_eq(&render_outline(&outline));
    expect![[r#"
        [
            "App\\Repository src/Repo.php 3:6-3:16",
        ]
    "#]]
    .assert_debug_eq(&render_hits(&found));
}

/// Renders a `documentSymbol` response as one line per entry, children indented.
fn render_outline(response: &Response) -> Vec<String> {
    let value = serde_json::to_value(response.result().expect("a result")).expect("json");
    let mut out = Vec::new();
    for entry in value.as_array().into_iter().flatten() {
        out.push(render_symbol(entry, 0));
        for child in entry["children"].as_array().into_iter().flatten() {
            out.push(render_symbol(child, 1));
        }
    }
    out
}

fn render_symbol(entry: &Value, depth: usize) -> String {
    format!(
        "{}{} {} {}",
        "  ".repeat(depth),
        symbol_kind_name(entry["kind"].as_i64().unwrap_or(0)),
        entry["name"].as_str().unwrap_or("?"),
        render_span(&entry["range"])
    )
}

/// Renders a `workspace/symbol` response, path-relative so it does not depend on the temp dir.
fn render_hits(response: &Response) -> Vec<String> {
    let value = serde_json::to_value(response.result().expect("a result")).expect("json");
    value
        .as_array()
        .into_iter()
        .flatten()
        .map(|hit| {
            let uri = hit["location"]["uri"].as_str().unwrap_or("");
            let path = uri.rsplit("/src/").next().unwrap_or(uri);
            format!(
                "{} src/{path} {}",
                hit["name"].as_str().unwrap_or("?"),
                render_span(&hit["location"]["range"])
            )
        })
        .collect()
}

/// The two `SymbolKind` values this test can see, by their protocol numbers.
fn symbol_kind_name(kind: i64) -> &'static str {
    match kind {
        5 => "class",
        6 => "method",
        _ => "other",
    }
}

#[tokio::test]
async fn references_are_found_across_files_without_opening_them() {
    // Only one file is open, and the answer covers three. What makes that affordable is that each
    // candidate is analysed into a scratch set the sweep drops, so nothing it touched outlives the
    // request.
    let mut server = Fixture::new(
        r#"
//- /composer.json
{"autoload": {"psr-4": {"App\\": "src"}}}

//- /src/Repo.php
<?php
namespace App;

class Repo {}

//- /src/One.php
<?php
namespace App;

class One
{
    public function run(Repo $repo): void {}
}

//- /src/Two.php
<?php
namespace App;

use App\Repo;

class Two
{
    public function run(): Repo
    {
        return new Repo();
    }
}

//- /src/Unrelated.php
<?php
namespace App;

class Unrelated
{
    public function run(): void {}
}
"#,
    );
    server.initialize(false, false).await;
    server.initialized().await;
    server.open("src/Repo.php").await;

    let found = server
        .request(
            "textDocument/references",
            json!({
                "textDocument": { "uri": server.uri("src/Repo.php") },
                "position": { "line": 3, "character": 7 },
                "context": { "includeDeclaration": true },
            }),
        )
        .await;

    expect![[r#"
        [
            "One.php 5:24-5:28",
            "Repo.php 3:6-3:10",
            "Two.php 3:4-3:12",
            "Two.php 7:27-7:31",
            "Two.php 9:19-9:23",
        ]
    "#]]
    .assert_debug_eq(&render_places(&found));
}

/// Renders a `references` response, filename-relative so it does not depend on the temp directory.
fn render_places(response: &Response) -> Vec<String> {
    let value = serde_json::to_value(response.result().expect("a result")).expect("json");
    value
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
        .collect()
}

/// A project where one class is used from two other files, and a dependency declares one too.
const RENAMEABLE: &str = r#"
//- /composer.json
{"autoload": {"psr-4": {"App\\": "src"}}}

//- /src/Repo.php
<?php
namespace App;

class Repo {}

//- /src/One.php
<?php
namespace App;

use App\Repo;

class One
{
    public function run(): Repo
    {
        return new Repo();
    }
}

//- /vendor/pkg/Vendored.php
<?php
namespace Pkg;

class Vendored {}
"#;

#[tokio::test]
async fn a_rename_edits_every_use_and_only_the_last_segment_of_a_path() {
    let mut server = Fixture::new(RENAMEABLE);
    server.initialize(false, false).await;
    server.initialized().await;
    server.open("src/Repo.php").await;

    let prepared = server
        .request(
            "textDocument/prepareRename",
            json!({
                "textDocument": { "uri": server.uri("src/Repo.php") },
                "position": { "line": 3, "character": 7 },
            }),
        )
        .await;
    let edit = server
        .request(
            "textDocument/rename",
            json!({
                "textDocument": { "uri": server.uri("src/Repo.php") },
                "position": { "line": 3, "character": 7 },
                "newName": "Ledger",
            }),
        )
        .await;

    expect![[r#""placeholder": "Repo" at 3:6-3:10"#]].assert_eq(&render_prepared(&prepared));
    // `use App\Repo` is edited at the last segment only: replacing the whole path would import a
    // class in the global namespace.
    expect![[r#"
        [
            "One.php 3:8-3:12 -> Ledger",
            "One.php 7:27-7:31 -> Ledger",
            "One.php 9:19-9:23 -> Ledger",
            "Repo.php 3:6-3:10 -> Ledger",
        ]
    "#]]
    .assert_debug_eq(&render_edits(&edit));
}

#[tokio::test]
async fn a_declaration_outside_the_project_is_refused() {
    // A sweep reads the project's files, so renaming into a dependency would rewrite the uses it can
    // see and leave the dependency's own naming something that no longer exists.
    let mut server = Fixture::new(RENAMEABLE);
    server.initialize(false, false).await;
    server.initialized().await;
    server.open("vendor/pkg/Vendored.php").await;

    let refused = server
        .request(
            "textDocument/prepareRename",
            json!({
                "textDocument": { "uri": server.uri("vendor/pkg/Vendored.php") },
                "position": { "line": 3, "character": 7 },
            }),
        )
        .await;
    let bad_name = server
        .request(
            "textDocument/rename",
            json!({
                "textDocument": { "uri": server.uri("src/Repo.php") },
                "position": { "line": 3, "character": 7 },
                "newName": "2Ledger",
            }),
        )
        .await;

    expect!["this declaration is not in this project's own code"]
        .assert_eq(&error_message(&refused).unwrap_or_default());
    expect!["that is not a name PHP would accept"]
        .assert_eq(&error_message(&bad_name).unwrap_or_default());
}

/// Renders a `prepareRename` response as its placeholder and range.
fn render_prepared(response: &Response) -> String {
    let value = serde_json::to_value(response.result().expect("a result")).expect("json");
    format!(
        r#""placeholder": {} at {}"#,
        value["placeholder"],
        render_span(&value["range"])
    )
}

/// A local named the same thing in two files, so a sweep would visibly overreach.
const LOCAL_RENAME: &str = r#"
//- /composer.json
{"autoload": {"psr-4": {"App\\": "src/"}}}
//- /src/One.php
<?php
namespace App;

class One
{
    public function go(): int
    {
        $item = 1;
        $later = fn () => $item;
        return $item + $later();
    }
}
//- /src/Two.php
<?php
namespace App;

class Two
{
    public function go(): int
    {
        $item = 2;
        return $item;
    }
}
"#;

#[tokio::test]
async fn renaming_a_local_stays_inside_its_own_function() {
    // The one operation where the sweep must not run. It matches a *name*, and `$item` in `Two::go`
    // is a different variable -- so a rename that swept would rewrite it, and the edit would be
    // applied to a file the person never looked at.
    let mut server = Fixture::new(LOCAL_RENAME);
    server.initialize(false, false).await;
    server.initialized().await;
    server.open("src/One.php").await;

    let prepared = server
        .request(
            "textDocument/prepareRename",
            json!({
                "textDocument": { "uri": server.uri("src/One.php") },
                "position": { "line": 7, "character": 10 },
            }),
        )
        .await;
    let edit = server
        .request(
            "textDocument/rename",
            json!({
                "textDocument": { "uri": server.uri("src/One.php") },
                "position": { "line": 7, "character": 10 },
                "newName": "row",
            }),
        )
        .await;

    // The `$` is not part of the name: a person handed `$item` types `$row`, and the file would
    // then say `$$row`.
    expect![[r#""placeholder": "item" at 7:9-7:13"#]].assert_eq(&render_prepared(&prepared));
    expect![[r#"
        [
            "One.php 7:9-7:13 -> row",
            "One.php 8:27-8:31 -> row",
            "One.php 9:16-9:20 -> row",
        ]
    "#]]
    .assert_debug_eq(&render_edits(&edit));
}

#[tokio::test]
async fn highlighting_a_local_marks_every_use_in_the_buffer() {
    let mut server = Fixture::new(LOCAL_RENAME);
    server.initialize(false, false).await;
    server.initialized().await;
    server.open("src/One.php").await;

    let response = server
        .request(
            "textDocument/documentHighlight",
            json!({
                "textDocument": { "uri": server.uri("src/One.php") },
                "position": { "line": 7, "character": 10 },
            }),
        )
        .await;

    let value = serde_json::to_value(response.result().expect("a result")).expect("json");
    let spans = value
        .as_array()
        .expect("a list")
        .iter()
        .map(|entry| render_span(&entry["range"]))
        .collect::<Vec<_>>();
    expect![[r#"
        [
            "7:8-7:13",
            "8:26-8:31",
            "9:15-9:20",
        ]
    "#]]
    .assert_debug_eq(&spans);
}

#[tokio::test]
async fn a_rename_answers_from_the_buffer_and_not_from_the_disk() {
    // The failure this guards is the one that reads as "it works sometimes": the sweep opens each
    // candidate file, and for a file the editor already has open it has to read *that* text. A
    // reference deleted in an unsaved buffer is not a reference, and an edit produced for it lands
    // at a range the file no longer has -- corrupting a file the person was not even renaming in.
    let mut server = Fixture::new(RENAMEABLE);
    server.initialize(false, false).await;
    server.initialized().await;
    server.open("src/Repo.php").await;
    server.open("src/One.php").await;

    // The use of `Repo` is gone from the buffer, and one line shorter, so a stale answer would be
    // visible as a range rather than merely as a count.
    server
        .change(
            "src/One.php",
            2,
            "<?php\nnamespace App;\n\nclass One\n{\n    public function run(): void\n    {\n    }\n}\n",
        )
        .await;

    let edit = server
        .request(
            "textDocument/rename",
            json!({
                "textDocument": { "uri": server.uri("src/Repo.php") },
                "position": { "line": 3, "character": 7 },
                "newName": "Ledger",
            }),
        )
        .await;

    expect![[r#"
        [
            "Repo.php 3:6-3:10 -> Ledger",
        ]
    "#]]
    .assert_debug_eq(&render_edits(&edit));
}
