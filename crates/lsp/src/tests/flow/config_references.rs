//! A class named in configuration is a reference to it.
//!
//! PSR-4 is not the only thing outside PHP that names PHP. A Drupal service definition writes its
//! class in `*.services.yml`, a route writes its controller in `*.routing.yml`, and until these
//! were read at indexing time none of it was reachable: `references` said a service class had one
//! use -- its own declaration -- and a rename rewrote the PHP and left the YAML pointing at a
//! class that no longer existed. Nothing reported that, because the file is not PHP and nothing
//! was checking it.
//!
//! Nothing new is contributed by the framework here. The rule saying `class:` in a `*.services.yml`
//! names a class has been there as long as goto *from* a YAML buffer has; what was missing is that
//! it was only ever applied to a file somebody had open.

use expect_test::expect;
use serde_json::json;

use crate::tests::Fixture;
use crate::tests::flow::{render_edits, render_span};

/// A service whose class is named in YAML and nowhere else.
const WIRED_IN_YAML: &str = r#"
//- /composer.json
{"autoload": {"psr-4": {"App\\": "src/"}}}
//- /phrost.toml
framework = ["drupal"]
//- /app.services.yml
services:
  app.provider:
    class: App\Provider
    arguments: ['@logger']
  App\Wired:
    arguments: ['@logger']
//- /app.routing.yml
app.run:
  path: '/run'
  defaults:
    _controller: '\App\Provider::run'
//- /src/Wired.php
<?php
namespace App;

class Wired {}
//- /src/Provider.php
<?php
namespace App;

class Provider
{
    public function run(): int { return 1; }
}
"#;

#[tokio::test]
async fn a_class_named_in_yaml_is_listed_among_its_references() {
    let mut server = Fixture::new(WIRED_IN_YAML);
    server.initialize(false, false).await;
    server.initialized().await;
    server.open("src/Provider.php").await;

    let found = server
        .request(
            "textDocument/references",
            json!({
                "textDocument": { "uri": server.uri("src/Provider.php") },
                "position": { "line": 3, "character": 7 },
                "context": { "includeDeclaration": true },
            }),
        )
        .await;
    let value = serde_json::to_value(found.result().expect("a result")).expect("json");
    let mut out: Vec<String> = value
        .as_array()
        .into_iter()
        .flatten()
        .map(|at| {
            let uri = at["uri"].as_str().unwrap_or("?");
            format!(
                "{} {}",
                uri.rsplit('/').next().unwrap_or(uri),
                render_span(&at["range"])
            )
        })
        .collect();
    out.sort();

    expect![[r#"
        [
            "Provider.php 3:6-3:14",
            "app.routing.yml 3:18-3:31",
            "app.services.yml 2:11-2:23",
        ]
    "#]]
    .assert_debug_eq(&out);
}

#[tokio::test]
async fn renaming_the_class_rewrites_the_yaml_that_wires_it() {
    let mut server = Fixture::new(WIRED_IN_YAML);
    server.initialize(false, false).await;
    server.initialized().await;
    server.open("src/Provider.php").await;

    let edit = server
        .request(
            "textDocument/rename",
            json!({
                "textDocument": { "uri": server.uri("src/Provider.php") },
                "position": { "line": 3, "character": 7 },
                "newName": "Client",
            }),
        )
        .await;

    // The YAML writes the class fully qualified, so what changes there is the last segment --
    // `App\Provider` becomes `App\Client` and the namespace in front of it is left alone.
    expect![[r#"
        [
            "Provider.php 3:6-3:14 -> Client",
            "app.routing.yml 3:23-3:31 -> Client",
            "app.services.yml 2:15-2:23 -> Client",
        ]
    "#]]
    .assert_debug_eq(&render_edits(&edit));
}

#[tokio::test]
async fn a_method_a_route_names_is_a_reference_to_it_and_renames_with_it() {
    // A route writes its controller as `Class::action`, so the action is named outside PHP as
    // surely as the class is -- and renaming it without rewriting the route leaves the route
    // pointing at a method that is gone.
    let mut server = Fixture::new(WIRED_IN_YAML);
    server.initialize(false, false).await;
    server.initialized().await;
    server.open("src/Provider.php").await;

    let edit = server
        .request(
            "textDocument/rename",
            json!({
                "textDocument": { "uri": server.uri("src/Provider.php") },
                "position": { "line": 5, "character": 21 },
                "newName": "handle",
            }),
        )
        .await;

    expect![[r#"
        [
            "Provider.php 5:20-5:23 -> handle",
            "app.routing.yml 3:33-3:36 -> handle",
        ]
    "#]]
    .assert_debug_eq(&render_edits(&edit));
}

#[tokio::test]
async fn a_service_keyed_by_its_own_class_names_it_as_surely_as_a_class_key_does() {
    // How a container written for autowiring names a service: the key *is* the class, and there
    // is no value for a rule about values to match. The separator is the whole test --
    // `app.provider` beside it is an id somebody chose, and both are in this one file.
    let mut server = Fixture::new(WIRED_IN_YAML);
    server.initialize(false, false).await;
    server.initialized().await;
    server.open("src/Wired.php").await;

    let edit = server
        .request(
            "textDocument/rename",
            json!({
                "textDocument": { "uri": server.uri("src/Wired.php") },
                "position": { "line": 3, "character": 7 },
                "newName": "Bound",
            }),
        )
        .await;

    expect![[r#"
        [
            "Wired.php 3:6-3:11 -> Bound",
            "app.services.yml 4:6-4:11 -> Bound",
        ]
    "#]]
    .assert_debug_eq(&render_edits(&edit));
}
