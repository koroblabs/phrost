//! What a client needs in order to draw a declaration struck through.
//!
//! The protocol carries it three ways, one per surface: `CompletionItemTag::DEPRECATED` on a row,
//! and `SymbolTag::DEPRECATED` on an outline entry and on a workspace-symbol hit. phrost set none
//! of them, so a deprecated class read exactly like a current one everywhere a person picks a name.
//!
//! On the *list* and not on the card `completionItem/resolve` fetches: a warning that arrives when
//! somebody highlights a row has arrived after they chose it.

use expect_test::expect;
use serde_json::{Value, json};

use crate::tests::Fixture;

const FIXTURE: &str = r#"
//- /composer.json
{"autoload": {"psr-4": {"App\\": "src/"}}}
//- /src/Old.php
<?php
namespace App;

/**
 * @deprecated 2.0 use Fresh instead
 */
class Old
{
    #[\Deprecated(message: 'use open()', since: '2.0')]
    public function lock(): bool
    {
        return true;
    }

    public function fine(): void {}
}
//- /src/Fresh.php
<?php
namespace App;

class Fresh {}
//- /src/Use.php
<?php
namespace App;

class Caller
{
    public function run(): void
    {
        $x = Ol
    }
}
"#;

/// `name [tags]` per row, so the test is about the tag rather than about what else is offered.
fn tagged(value: &Value, name: &str, tags: &str) -> Vec<String> {
    let rows = match value {
        Value::Array(rows) => rows.clone(),
        Value::Object(map) => match map.get("items") {
            Some(Value::Array(rows)) => rows.clone(),
            _ => vec![value.clone()],
        },
        _ => return vec![format!("not a list: {value}")],
    };
    rows.iter()
        .filter_map(|row| {
            let label = row.get(name)?.as_str()?;
            let marked = match row.get(tags) {
                Some(Value::Array(found)) if !found.is_empty() => "  deprecated",
                _ => "",
            };
            Some(format!("{label}{marked}"))
        })
        .collect()
}

async fn opened() -> Fixture {
    Fixture::started(FIXTURE, "src/Use.php").await
}

#[tokio::test]
async fn a_completion_row_carries_the_tag() {
    let mut server = opened().await;
    let answer = server
        .request(
            "textDocument/completion",
            json!({
                "textDocument": { "uri": server.uri("src/Use.php") },
                "position": { "line": 7, "character": 16 },
            }),
        )
        .await;
    let value = serde_json::to_value(answer.result().expect("a result")).expect("json");

    expect![[r#"
        Old  deprecated"#]]
    .assert_eq(&tagged(&value, "label", "tags").join("\n"));
}

#[tokio::test]
async fn an_outline_entry_carries_the_tag() {
    let mut server = opened().await;
    server.open("src/Old.php").await;
    let answer = server
        .request(
            "textDocument/documentSymbol",
            json!({ "textDocument": { "uri": server.uri("src/Old.php") } }),
        )
        .await;
    let value = serde_json::to_value(answer.result().expect("a result")).expect("json");
    let children = value
        .get(0)
        .and_then(|first| first.get("children"))
        .cloned()
        .unwrap_or(Value::Null);

    expect![[r#"
        Old  deprecated
        --
        lock  deprecated
        fine"#]]
    .assert_eq(&format!(
        "{}\n--\n{}",
        tagged(&value, "name", "tags").join("\n"),
        tagged(&children, "name", "tags").join("\n")
    ));
}

#[tokio::test]
async fn a_workspace_symbol_hit_carries_the_tag() {
    let mut server = opened().await;
    let mut found = Vec::new();
    // Both arms, because a tag set on every row reads the same as a tag set on the right one.
    for query in ["Old", "Fresh"] {
        let answer = server
            .request("workspace/symbol", json!({ "query": query }))
            .await;
        let value = serde_json::to_value(answer.result().expect("a result")).expect("json");
        found.extend(tagged(&value, "name", "tags"));
    }

    expect![[r#"
        App\Old  deprecated
        App\Fresh"#]]
    .assert_eq(&found.join("\n"));
}
