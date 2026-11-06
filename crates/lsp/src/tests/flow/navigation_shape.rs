//! What shape a navigation answer comes back in.
//!
//! `LocationLink` carries the span the answer was asked *from*, which is what lets an editor
//! underline the word under the cursor -- and the protocol makes it opt-in, one bit per request
//! under `textDocument.definition.linkSupport` and its three neighbours. A client that did not ask
//! cannot deserialise one, so a server that sends it anyway reads as goto answering nothing at all.
//!
//! This one sent links unconditionally until a probe declaring `linkSupport: false` got them back
//! and had no field to read.

use expect_test::expect;
use serde_json::{Value, json};

use crate::tests::Fixture;

const ONE_CLASS: &str = r#"
//- /composer.json
{"autoload": {"psr-4": {"App\\": "src/"}}}
//- /src/Repo.php
<?php
namespace App;

class Repo {}
//- /src/Service.php
<?php
namespace App;

class Service
{
    public function run(Repo $repo): void {}
}
"#;

/// Renders whichever shape came back, so the test is about the shape rather than about the target.
fn shape(response: &lspkit_runtime::protocol::jsonrpc::Response) -> String {
    let value = serde_json::to_value(response.result().expect("a result")).expect("json");
    let Value::Array(found) = &value else {
        return format!("not an array: {value}");
    };
    found
        .iter()
        .map(|one| match one.get("targetUri") {
            Some(_) => "link".to_owned(),
            None => "location".to_owned(),
        })
        .collect::<Vec<_>>()
        .join(", ")
}

async fn goto(link_support: bool) -> String {
    let mut server = Fixture::new(ONE_CLASS);
    server
        .request(
            "initialize",
            json!({
                "capabilities": {
                    "textDocument": { "definition": { "linkSupport": link_support } },
                },
                "workspaceFolders": [{
                    "uri": format!("file://{}", server.project.root().display()),
                    "name": "fixture",
                }],
            }),
        )
        .await;
    server.initialized().await;
    server.open("src/Service.php").await;

    let answer = server
        .request(
            "textDocument/definition",
            json!({
                "textDocument": { "uri": server.uri("src/Service.php") },
                "position": { "line": 5, "character": 25 },
            }),
        )
        .await;
    shape(&answer)
}

#[tokio::test]
async fn a_client_that_asks_for_links_gets_links() {
    expect!["link"].assert_eq(&goto(true).await);
}

#[tokio::test]
async fn a_client_that_does_not_gets_plain_locations() {
    expect!["location"].assert_eq(&goto(false).await);
}
