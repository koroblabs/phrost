//! What a symbol search puts on the wire for a member.
//!
//! The protocol half of the feature, which the view tests cannot see: a member's row carries the
//! bare name and names its class in `containerName`, because that is the field a client draws
//! beside the name -- and forty rows called `find` with nothing beside them is a list nobody can
//! choose from. The kind numbers are checked too: an editor picks the icon from those, and a
//! property arriving as a class is a bug with no other symptom.

use expect_test::expect;
use lspkit_runtime::protocol::jsonrpc::Response;
use serde_json::json;

use crate::tests::Fixture;

#[tokio::test]
async fn a_member_search_names_its_class_on_the_wire() {
    let mut server = Fixture::new(
        r#"
//- /composer.json
{"autoload": {"psr-4": {"App\\": "src"}}}

//- /src/ItemStorage.php
<?php
namespace App;

class ItemStorage
{
    public const LOAD_LIMIT = 50;

    public string $loadedAt = '';

    public function loadByItemNumber(string $number): void {}
}

//- /src/Loader.php
<?php
namespace App;

class Loader {}
"#,
    );
    server.initialize(false, false).await;
    server.initialized().await;

    let found = server
        .request("workspace/symbol", json!({ "query": "load" }))
        .await;

    expect![[r#"
        [
            "5 App\\Loader",
            "7 loadedAt in App\\ItemStorage",
            "14 LOAD_LIMIT in App\\ItemStorage",
            "6 loadByItemNumber in App\\ItemStorage",
        ]
    "#]]
    .assert_debug_eq(&render(&found));
}

/// Each hit as `kind name in container`, the three fields a picker draws.
fn render(response: &Response) -> Vec<String> {
    let value = serde_json::to_value(response.result().expect("a result")).expect("json");
    value
        .as_array()
        .into_iter()
        .flatten()
        .map(|hit| {
            let name = hit["name"].as_str().unwrap_or("?");
            let kind = hit["kind"].as_i64().unwrap_or(0);
            match hit["containerName"].as_str() {
                Some(container) => format!("{kind} {name} in {container}"),
                None => format!("{kind} {name}"),
            }
        })
        .collect()
}
