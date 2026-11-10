//! `$ap|` and `Counter::$co|`: a name whose `$` the client counts as part of the word. One file
//! each, because each is an unfinished statement and a parse recovers only the first of two.
//!
//! Zed's PHP language lists `$` among its word characters, so what it filters a list by at `$ap|`
//! is `$ap` -- and an item handed over with `filterText: "app"`, the label past a `$` taken for the
//! head of what was typed, holds no `$` to match it with. The client dropped `$app` from a list the
//! server had answered correctly, and every variable stopped completing in an editor. The rest of
//! a label is handed over as `filterText` only past a character no client's word holds.

use expect_test::expect;
use serde_json::json;

use crate::tests::Fixture;

#[tokio::test]
async fn a_variable_and_a_static_property_are_filtered_by_their_labels() {
    let mut server = Fixture::started(
        r#"
//- /composer.json
{}

//- /src/Counter.php
<?php
class Counter { public static int $count = 0; }

//- /static.php
<?php
Counter::$co@property@

//- /variable.php
<?php
$app = new Counter();
$ap@variable@
"#,
        "variable.php",
    )
    .await;
    server.open("static.php").await;
    server.settle().await;

    let mut rendered = Vec::new();
    for (file, marker) in [("variable.php", "variable"), ("static.php", "property")] {
        let position = server.position(file, marker);
        let response = server
            .request(
                "textDocument/completion",
                json!({
                    "textDocument": { "uri": server.uri(file) },
                    "position": position,
                }),
            )
            .await;
        let result = serde_json::to_value(response.result().expect("completion answered"))
            .expect("serialisable");
        rendered.extend(
            result["items"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|item| {
                    format!(
                        "{marker}: {} filter={}",
                        item["label"].as_str().unwrap_or("?"),
                        item["filterText"].as_str().unwrap_or("(label)"),
                    )
                }),
        );
    }

    expect![[r#"
        [
            "variable: $app filter=(label)",
            "property: $count filter=(label)",
        ]
    "#]]
    .assert_debug_eq(&rendered);
}
