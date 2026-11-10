//! `use App\Http\Resp|`: an item that goes on from what was typed replaces only the last word.
//!
//! Clients filter a list two ways and the server cannot ask which it is talking to. One reads the
//! *word* at the cursor -- `Resp` -- and one reads the text the item's edit *replaces*. While the
//! edit covered the whole written path those were two different strings, and no `filterText` could
//! answer both: `Response` suited the first and hid the item from the second, which looked for
//! `App\Http\Resp` in it.
//!
//! An item that begins with everything typed before the word would only write it back, so its edit
//! leaves that standing. What is replaced is then the word, for both. The exception is an item
//! that does not go on from what was typed -- a path spelled in another case -- whose edit still
//! covers all of it, because that is the one thing about it that has to change.
//!
//! One file each: an unfinished statement, and a parse recovers only the first of two.

use expect_test::expect;
use serde_json::json;

use crate::tests::Fixture;

#[tokio::test]
async fn what_is_replaced_is_the_word_wherever_the_item_goes_on_from_the_rest() {
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

//- /import.php
<?php
use App\Coun@import@

//- /other_case.php
<?php
use app\Coun@other_case@
"#,
        "rooted.php",
    )
    .await;
    for file in ["bare.php", "import.php", "other_case.php"] {
        server.open(file).await;
    }
    server.settle().await;

    let mut rendered = Vec::new();
    for marker in ["bare", "rooted", "import", "other_case"] {
        let file = format!("{marker}.php");
        let position = server.position(&file, marker);
        let response = server
            .request(
                "textDocument/completion",
                json!({
                    "textDocument": { "uri": server.uri(&file) },
                    "position": position,
                }),
            )
            .await;
        let result = serde_json::to_value(response.result().expect("completion answered"))
            .expect("serialisable");
        let text = server.text(&file);
        rendered.extend(
            result["items"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|item| format!("{marker}: {}", offered(&text, item))),
        );
    }

    expect![[r#"
        [
            "bare: Counter replaces `Coun` with `Counter` filter=(label)",
            "rooted: Counter replaces `Coun` with `Counter` filter=(label)",
            "import: App\\Counter replaces `Coun` with `Counter` filter=Counter",
            "other_case: App\\Counter replaces `app\\Coun` with `App\\Counter` filter=Counter",
        ]
    "#]]
    .assert_debug_eq(&rendered);
}

/// One item as a client meets it: the text its edit replaces, what it writes there, and what the
/// list is filtered by.
pub(super) fn offered(text: &str, item: &serde_json::Value) -> String {
    let range = &item["textEdit"]["range"];
    let column = |end: &str| {
        usize::try_from(range[end]["character"].as_u64().unwrap_or_default()).unwrap_or_default()
    };
    let line = usize::try_from(range["start"]["line"].as_u64().unwrap_or_default())
        .ok()
        .and_then(|line| text.lines().nth(line))
        .unwrap_or_default();
    format!(
        "{} replaces `{}` with `{}` filter={}",
        item["label"].as_str().unwrap_or("?"),
        line.get(column("start")..column("end")).unwrap_or("?"),
        item["textEdit"]["newText"].as_str().unwrap_or("?"),
        item["filterText"].as_str().unwrap_or("(label)"),
    )
}
