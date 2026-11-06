//! The count above a declaration, and the two requests it takes.
//!
//! The listing must not sweep -- a file declares tens of things and a sweep is milliseconds each
//! -- so `textDocument/codeLens` answers with unresolved places and `codeLens/resolve` puts the
//! count on the one the client is about to paint. Both halves are tested because a listing that
//! silently resolved would be correct and unusable, and a resolve that answered nothing would be
//! invisible: an unresolved lens simply does not appear.

use expect_test::expect;
use serde_json::{Value, json};

use crate::tests::Fixture;
use crate::tests::flow::render_span;

const COUNTED: &str = r#"
//- /composer.json
{"autoload": {"psr-4": {"App\\": "src/"}}}
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    public function used(): int { return 1; }

    public function unused(): int { return 2; }
}
//- /src/Service.php
<?php
namespace App;

use App\Repo;

class Service
{
    public function run(Repo $repo): int
    {
        return $repo->used() + $repo->used();
    }
}
//- /src/Main.php
<?php
namespace App;

class Main
{
    public function go(Service $service): void {}
}
"#;

/// Every lens, as a place and whatever title it carries.
fn render(lenses: &Value) -> Vec<String> {
    lenses
        .as_array()
        .into_iter()
        .flatten()
        .map(|lens| {
            format!(
                "{} {}",
                render_span(&lens["range"]),
                lens["command"]["title"].as_str().unwrap_or("<unresolved>")
            )
        })
        .collect()
}

#[tokio::test]
async fn the_listing_names_the_declarations_and_counts_none_of_them() {
    let mut server = Fixture::new(COUNTED);
    server.initialize(false, false).await;
    server.initialized().await;
    server.open("src/Repo.php").await;

    let lenses = server
        .request(
            "textDocument/codeLens",
            json!({ "textDocument": { "uri": server.uri("src/Repo.php") } }),
        )
        .await;
    let value = serde_json::to_value(lenses.result().expect("a result")).expect("json");

    // The class and its two methods, and no title on any of them: the count is the other request.
    expect![[r#"
        [
            "3:6-3:10 <unresolved>",
            "5:20-5:24 <unresolved>",
            "7:20-7:26 <unresolved>",
        ]
    "#]]
    .assert_debug_eq(&render(&value));
}

#[tokio::test]
async fn resolving_counts_the_uses_and_not_the_declaration() {
    let mut server = Fixture::new(COUNTED);
    server.initialize(false, false).await;
    server.initialized().await;
    server.open("src/Repo.php").await;

    let lenses = server
        .request(
            "textDocument/codeLens",
            json!({ "textDocument": { "uri": server.uri("src/Repo.php") } }),
        )
        .await;
    let value = serde_json::to_value(lenses.result().expect("a result")).expect("json");

    let mut resolved = Vec::new();
    for lens in value.as_array().into_iter().flatten() {
        let one = server.request("codeLens/resolve", lens.clone()).await;
        let one = serde_json::to_value(one.result().expect("a result")).expect("json");
        resolved.push(render(&json!([one])).remove(0));
    }

    // `Repo` is written twice in the other file -- the import and the type hint -- and its
    // declaration is not a use of itself. `used()` is called twice on one line. `unused()` is the
    // answer somebody opens a file to get, and it has to be a lens rather than an absence.
    expect![[r#"
        [
            "3:6-3:10 2 references",
            "5:20-5:24 2 references",
            "7:20-7:26 0 references",
        ]
    "#]]
    .assert_debug_eq(&resolved);
}

#[tokio::test]
async fn one_use_is_written_in_the_singular() {
    let mut server = Fixture::new(COUNTED);
    server.initialize(false, false).await;
    server.initialized().await;
    server.open("src/Service.php").await;

    let lenses = server
        .request(
            "textDocument/codeLens",
            json!({ "textDocument": { "uri": server.uri("src/Service.php") } }),
        )
        .await;
    let value = serde_json::to_value(lenses.result().expect("a result")).expect("json");
    let first = value
        .as_array()
        .and_then(|all| all.first())
        .expect("a lens");
    let one = server.request("codeLens/resolve", first.clone()).await;
    let one = serde_json::to_value(one.result().expect("a result")).expect("json");

    // `Service` is written once, in `Main`'s parameter -- so this is the case the plural would
    // have been wrong for, and the test is named after it.
    expect![[r#"
        [
            "5:6-5:13 1 reference",
        ]
    "#]]
    .assert_debug_eq(&render(&json!([one])));
}

/// A service class and a controller action, each named once in YAML and nowhere else.
const NAMED_IN_YAML: &str = r#"
//- /composer.json
{"autoload": {"psr-4": {"App\\": "src/"}}}
//- /phrost.toml
framework = ["drupal"]
//- /app.services.yml
services:
  app.provider:
    class: App\Provider
//- /app.routing.yml
app.run:
  path: '/run'
  defaults:
    _controller: '\App\Provider::run'
//- /src/Provider.php
<?php
namespace App;

class Provider
{
    public function run(): int { return 1; }
}
"#;

#[tokio::test]
async fn the_count_includes_the_places_outside_php_that_find_references_lists() {
    // A service's `class:` and a route's `_controller` are the only things that name either of
    // these, and find-references has listed both since configuration became part of the
    // reference space. The lens had grown its own idea of what a reference is and counted the
    // PHP alone -- so over a class wired up in `services.yml` it said nought while the list it
    // sits above opened with one, which is the way to make a reader stop believing a number.
    let mut server = Fixture::new(NAMED_IN_YAML);
    server.initialize(false, false).await;
    server.initialized().await;
    server.open("src/Provider.php").await;

    let lenses = server
        .request(
            "textDocument/codeLens",
            json!({ "textDocument": { "uri": server.uri("src/Provider.php") } }),
        )
        .await;
    let value = serde_json::to_value(lenses.result().expect("a result")).expect("json");

    let mut resolved = Vec::new();
    for lens in value.as_array().into_iter().flatten() {
        let one = server.request("codeLens/resolve", lens.clone()).await;
        resolved.push(serde_json::to_value(one.result().expect("a result")).expect("json"));
    }

    expect![[r#"
        [
            "3:6-3:14 2 references",
            "5:20-5:23 1 reference",
        ]
    "#]]
    .assert_debug_eq(&render(&Value::Array(resolved)));
}

#[tokio::test]
async fn the_client_is_told_to_ask_again_once_there_is_something_to_count() {
    // A lens is a count of references and there is nothing honest to count before the index is
    // ready, so `textDocument/codeLens` answers with silence until it is. A client asks for a
    // file's lenses when it opens the file and then not again -- and an editor restores its tabs
    // the instant `initialized` goes out, so on every real start that ask is the early one.
    //
    // Without this nothing tells it to ask again: the lenses of every file open at start-up are
    // simply absent, and typing a character in one is what summons them. Which reads exactly like
    // a cache that will not invalidate, and is a race.
    let mut server = Fixture::new(COUNTED);
    server.initialize(false, false).await;
    server.initialized().await;

    assert!(
        server
            .methods_sent()
            .iter()
            .any(|method| method == "workspace/codeLens/refresh"),
        "the client was never told to re-ask: {:?}",
        server.methods_sent()
    );
}
