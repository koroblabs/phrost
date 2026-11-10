//! `new \App\Coun|`: a name whose edit replaces more than a client's word.
//!
//! The range this server sends covers the whole written path, so that accepting does not leave the
//! namespace behind twice. A client that filters by the *word* at the cursor reads `Coun` and keeps
//! `Counter`. One that filters by the text the edit replaces reads `\App\Coun`, finds no `\` in the
//! label, and drops an item the server offered -- so the item is handed the text it writes, which
//! holds both. A bare name, whose label already holds everything typed, is left to its label. One
//! file each: an unfinished statement, and a parse recovers only the first of two.

use expect_test::expect;
use serde_json::json;

use crate::tests::Fixture;

#[tokio::test]
async fn a_rooted_name_is_filtered_by_what_it_writes_and_a_bare_one_by_its_label() {
    let mut server = Fixture::started(
        r#"
//- /composer.json
{}

//- /src/Counter.php
<?php
namespace App;

class Counter {}

//- /bare.php
<?php
namespace App;

new Coun@bare@

//- /rooted.php
<?php
new \App\Coun@rooted@
"#,
        "rooted.php",
    )
    .await;
    server.open("bare.php").await;
    server.settle().await;

    let mut rendered = Vec::new();
    for (file, marker) in [("rooted.php", "rooted"), ("bare.php", "bare")] {
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
                        "{marker}: {} writes {} filter={}",
                        item["label"].as_str().unwrap_or("?"),
                        item["textEdit"]["newText"].as_str().unwrap_or("?"),
                        item["filterText"].as_str().unwrap_or("(label)"),
                    )
                }),
        );
    }

    expect![[r#"
        [
            "rooted: Counter writes \\App\\Counter filter=\\App\\Counter",
            "bare: Counter writes Counter filter=(label)",
        ]
    "#]]
    .assert_debug_eq(&rendered);
}
