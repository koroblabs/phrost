//! The content hover and completion hand back: markdown, kinds, details, and edit ranges.
//!
//! Hover has to read a docblock from a file that may not be open, using the path the index stored
//! rather than one relative to wherever the test runs from. Completion has to tell the client the
//! exact range to replace -- a qualified name already typed is not a word by the client's own
//! reckoning, so leaving it to guess duplicates the prefix.

use expect_test::expect;
use lspkit_runtime::protocol::jsonrpc::Response;
use serde_json::json;

use crate::tests::Fixture;
use crate::tests::flow::{TWO_FILES, render_span};

#[tokio::test]
async fn hover_comes_back_as_markdown() {
    let mut server = Fixture::started(TWO_FILES, "src/Repo.php").await;
    let response = server.hover("src/Repo.php", "parent").await;

    let contents = serde_json::to_value(response.result().expect("hover answered"))
        .expect("hover result is serialisable");
    expect!["markdown"].assert_eq(
        contents["contents"]["kind"]
            .as_str()
            .expect("a markup kind"),
    );
    expect![[r#"
        ```php
        <?php
        namespace App;

        class Base
        ```"#]]
    .assert_eq(
        contents["contents"]["value"]
            .as_str()
            .expect("markup value"),
    );
}

/// Two files where the documentation lives in the one that stays closed.
const DOCUMENTED_PARENT: &str = r#"
//- /src/Base.php
<?php
namespace App;

class Base
{
    /**
     * Shared behaviour.
     */
    public function shared(): void {}
}

//- /src/Repo.php
<?php
namespace App;

class Repo extends Base
{
    public function run(): void
    {
        $this->sh@call@ared();
    }
}
"#;

#[tokio::test]
async fn documentation_from_a_file_that_is_not_open_reaches_the_client() {
    // The one thing only a real server exercises: a docblock is read on demand from a range in a
    // file, and for a closed file that means reading it from disk with the path the index stored. A
    // relative path there works everywhere a test runs from the project root and fails in a server,
    // which is launched from wherever the editor happens to be -- silently, for every closed file.
    let mut server = Fixture::started(DOCUMENTED_PARENT, "src/Repo.php").await;
    let response = server.hover("src/Repo.php", "call").await;

    let contents = serde_json::to_value(response.result().expect("hover answered"))
        .expect("hover result is serialisable");
    expect![[r#"
        ```php
        <?php
        namespace App;

        class Base
        {
            public function shared(): void
        }
        ```

        inherited App\\Base

        ---

        Shared behaviour."#]]
    .assert_eq(
        contents["contents"]["value"]
            .as_str()
            .expect("markup value"),
    );
}

#[tokio::test]
async fn completion_comes_back_with_kinds_and_details() {
    let mut server = Fixture::started(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    public function find(int $id): void {}

    public function run(): void
    {
        $this->@here@
    }
}
"#,
        "src/Repo.php",
    )
    .await;

    let position = server.position("src/Repo.php", "here");
    let params = serde_json::json!({
        "textDocument": { "uri": server.uri("src/Repo.php") },
        "position": position,
    });
    let response = server.request("textDocument/completion", params).await;

    let result = serde_json::to_value(response.result().expect("completion answered"))
        .expect("serialisable");
    let labels = result["items"]
        .as_array()
        .expect("a list of items")
        .iter()
        .map(|item| {
            format!(
                "{} kind={} detail={}",
                item["label"].as_str().unwrap_or("?"),
                item["kind"],
                item["detail"].as_str().unwrap_or("")
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    // Kind 2 is METHOD, which is the icon a client draws.
    expect![[r#"
        find kind=2 detail=: void
        run kind=2 detail=: void"#]]
    .assert_eq(&labels);
    assert_eq!(result["isIncomplete"], false);
}

#[tokio::test]
async fn a_completion_finishes_the_path_already_typed() {
    // What the client is told to replace, not what it would guess -- and the two agree. Its idea
    // of a word stops at the separator; the edit replaces that word with the rest of the name and
    // leaves the path before it standing, so no client ends up with the prefix twice over.
    //
    // What comes of it is the path as written, finished: a person writing a path from the root is
    // choosing that spelling, and a short name with an import is the answer for a *bare* word --
    // see `a_bare_name_comes_with_its_import` below for that half over the protocol.
    let mut server = Fixture::new(
        r#"
//- /composer.json
{"autoload": {"psr-4": {"App\\": "src"}}}

//- /src/Repo.php
<?php
namespace App\Nested;

class Repository {}

//- /src/app.php
<?php
namespace Other;

\App\Nested\Rep;
"#,
    );
    server.initialize(false, false).await;
    server.initialized().await;
    server.open("src/app.php").await;

    let offered = server
        .request(
            "textDocument/completion",
            json!({
                "textDocument": { "uri": server.uri("src/app.php") },
                // Immediately after `Rep`, at the end of the qualified path.
                "position": { "line": 3, "character": 15 },
            }),
        )
        .await;

    expect![[r#"
        [
            "Repository 3:12-3:15 -> Repository",
        ]
    "#]]
    .assert_debug_eq(&render_offered(&offered));
}

#[tokio::test]
async fn a_bare_name_comes_with_its_import() {
    // The other half: a bare word inserts the short name, which only means the right class because
    // the `use` line arrives in the same edit -- a client applies all of an item's edits or none.
    let mut server = Fixture::new(
        r#"
//- /composer.json
{"autoload": {"psr-4": {"App\\": "src"}}}

//- /src/Repo.php
<?php
namespace App\Nested;

class Repository {}

//- /src/app.php
<?php
namespace Other;

new Rep;
"#,
    );
    server.initialize(false, false).await;
    server.initialized().await;
    server.open("src/app.php").await;

    let offered = server
        .request(
            "textDocument/completion",
            json!({
                "textDocument": { "uri": server.uri("src/app.php") },
                // Immediately after `Rep`.
                "position": { "line": 3, "character": 7 },
            }),
        )
        .await;

    expect![[r#"
        [
            "Repository 3:4-3:7 -> Repository + use App\\Nested\\Repository; at 1:16-1:16",
        ]
    "#]]
    .assert_debug_eq(&render_offered(&offered));
}

#[tokio::test]
async fn a_docblock_gets_the_qualified_name_and_no_import() {
    // The same completion one line up writes a short name and a `use`. In a comment it must not: an
    // import is a change to the code, and a type written in a comment is not a reason to make one.
    let mut server = Fixture::new(
        r#"
//- /composer.json
{"autoload": {"psr-4": {"App\\": "src"}}}

//- /src/Repo.php
<?php
namespace App\Nested;

class Repository {}

//- /src/app.php
<?php
namespace Other;

/** @var Rep */
$x = 1;
"#,
    );
    server.initialize(false, false).await;
    server.initialized().await;
    server.open("src/app.php").await;

    let offered = server
        .request(
            "textDocument/completion",
            json!({
                "textDocument": { "uri": server.uri("src/app.php") },
                // Immediately after `Rep`, inside the docblock.
                "position": { "line": 3, "character": 12 },
            }),
        )
        .await;

    expect![[r#"
        [
            "Repository 3:9-3:12 -> \\App\\Nested\\Repository",
        ]
    "#]]
    .assert_debug_eq(&render_offered(&offered));
}

/// Renders a completion response as label, the range each item replaces, what it writes, and any
/// `use` line it writes alongside — which a client applies with the name or not at all.
fn render_offered(response: &Response) -> Vec<String> {
    let value = serde_json::to_value(response.result().expect("a result")).expect("json");
    value["items"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|item| {
            let edit = &item["textEdit"];
            let also = item["additionalTextEdits"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|extra| {
                    format!(
                        " + {} at {}",
                        extra["newText"].as_str().unwrap_or("?").trim(),
                        render_span(&extra["range"])
                    )
                })
                .collect::<String>();
            format!(
                "{} {} -> {}{also}",
                item["label"].as_str().unwrap_or("?"),
                render_span(&edit["range"]),
                edit["newText"].as_str().unwrap_or("?")
            )
        })
        .collect()
}

#[tokio::test]
async fn a_completion_item_is_described_when_the_client_asks() {
    // Two requests, which is the point of the pair: the list carries the type and the address of the
    // member, and the declaration is built once for the row a person highlighted rather than two
    // hundred times for rows nobody looks at.
    let mut server = Fixture::started(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    public function find(int $id, ?string $name = null): ?Repo {}

    public function run(): void
    {
        $this->fi@here@
    }
}
"#,
        "src/Repo.php",
    )
    .await;

    let position = server.position("src/Repo.php", "here");
    let response = server
        .request(
            "textDocument/completion",
            serde_json::json!({
                "textDocument": { "uri": server.uri("src/Repo.php") },
                "position": position,
            }),
        )
        .await;
    let list = serde_json::to_value(response.result().expect("completion answered"))
        .expect("serialisable");
    let item = list["items"]
        .as_array()
        .expect("a list of items")
        .first()
        .expect("one item")
        .clone();

    // The list itself says nothing about the declaration.
    assert_eq!(item["documentation"], serde_json::Value::Null);

    let resolved = server.request("completionItem/resolve", item).await;
    let resolved =
        serde_json::to_value(resolved.result().expect("resolve answered")).expect("serialisable");

    expect![[r#"
        find
        ```php
        <?php
        namespace App;

        class Repo
        {
            public function find(int $id, ?string $name = null): ?Repo
        }
        ```"#]]
    .assert_eq(&format!(
        "{}\n{}",
        resolved["label"].as_str().unwrap_or("?"),
        resolved["documentation"]["value"].as_str().unwrap_or("?")
    ));
}

/// A class-like row resolves to its header, the way a member row resolves to its signature.
///
/// This answered nothing at all, on the stated grounds that there was nothing to say about a
/// class-like name until its docblock could be read without a disk read. True of the docblock, and
/// not of the declaration -- `final class Repo extends Base implements Contract` comes out of the
/// skeleton, and it is what a person is choosing between when two names look alike.
#[tokio::test]
async fn a_class_row_resolves_to_its_declaration() {
    let mut server = Fixture::started(
        r#"
//- /src/Base.php
<?php
namespace App;

interface Contract {}

abstract class Base {}

//- /src/Caller.php
<?php
namespace App;

final class Repository extends Base implements Contract {}

class Caller
{
    public function run(): void
    {
        new Reposi@here@
    }
}
"#,
        "src/Caller.php",
    )
    .await;

    let position = server.position("src/Caller.php", "here");
    let response = server
        .request(
            "textDocument/completion",
            serde_json::json!({
                "textDocument": { "uri": server.uri("src/Caller.php") },
                "position": position,
            }),
        )
        .await;
    let list = serde_json::to_value(response.result().expect("completion answered"))
        .expect("serialisable");
    let item = list["items"]
        .as_array()
        .expect("a list of items")
        .first()
        .expect("one item")
        .clone();

    assert_eq!(item["documentation"], serde_json::Value::Null);

    let resolved = server.request("completionItem/resolve", item).await;
    let resolved =
        serde_json::to_value(resolved.result().expect("resolve answered")).expect("serialisable");

    expect![[r#"
        Repository
        ```php
        <?php
        namespace App;

        final class Repository extends Base implements Contract
        ```"#]]
    .assert_eq(&format!(
        "{}\n{}",
        resolved["label"].as_str().unwrap_or("?"),
        resolved["documentation"]["value"].as_str().unwrap_or("?")
    ));
}

/// A resolve is answered for a buffer the server no longer holds, and carries no file on the wire.
///
/// Both halves of one change. The item's `data` used to name the file the list came from, which is
/// 200 copies of an absolute path per keystroke -- 25 KB of a 135 KB response -- and it decided
/// nothing: every lookup behind a description is project-wide. Sent from a closed buffer, and with
/// nothing in the item that could name one, the answer is the same as it ever was.
#[tokio::test]
async fn a_row_resolves_with_no_file_named_and_no_buffer_open() {
    let mut server = Fixture::started(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    /** Finds one by its id. */
    public function find(int $id): ?Repo {}
}
"#,
        "src/Repo.php",
    )
    .await;
    server.close("src/Repo.php").await;

    let resolved = server
        .request(
            "completionItem/resolve",
            json!({
                "label": "find",
                "data": {
                    "target": "member",
                    "class": "App\\Repo",
                    "kind": "method",
                    "name": "find",
                },
            }),
        )
        .await;
    let resolved =
        serde_json::to_value(resolved.result().expect("resolve answered")).expect("serialisable");

    expect![[r#"
        ```php
        <?php
        namespace App;

        class Repo
        {
            public function find(int $id): ?Repo
        }
        ```

        ---

        Finds one by its id."#]]
    .assert_eq(resolved["documentation"]["value"].as_str().unwrap_or("?"));
}
#[tokio::test]
async fn a_slot_narrowed_only_by_a_contract_offers_its_members() {
    let mut server = Fixture::started(
        r#"
//- /composer.json
{"autoload": {"psr-4": {"App\\": "src"}}}

//- /src/Case.php
<?php
namespace App;

class Node
{
    public function image(): string {}
}

class Assert
{
    /**
     * @template ExpectedType of object
     * @param class-string<ExpectedType> $expected
     * @phpstan-assert =ExpectedType $actual
     */
    public static function isA(string $expected, object $actual): void {}
}

class Case_ extends Assert
{
    /**
     * @param array<string, mixed> $constants
     */
    public function run(array $constants): void
    {
        static::isA(Node::class, $constants['image']);
        $constants['image']->@slot@
    }
}
"#,
        "src/Case.php",
    )
    .await;

    let position = server.position("src/Case.php", "slot");
    let params = serde_json::json!({
        "textDocument": { "uri": server.uri("src/Case.php") },
        "position": position,
    });
    let response = server.request("textDocument/completion", params).await;
    let result = serde_json::to_value(response.result().expect("completion answered"))
        .expect("serialisable");
    let labels = result["items"]
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item["label"].as_str())
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default();
    expect!["image"].assert_eq(&labels);
}
