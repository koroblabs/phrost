//! What lands in the buffer when a completion is accepted.
//!
//! The analysis layer decides the *facts* -- this is a call, it takes arguments, its statement has
//! no `;` -- and the protocol layer turns them into one string. Two clients read that string
//! differently, so both are driven here: one that understands `$0` as a cursor and one that would
//! put the two characters in the file.

use expect_test::expect;
use serde_json::{Value, json};

use crate::tests::Fixture;

/// A call that is the whole of its statement, and one that is the right-hand side of an assignment.
///
/// Two files rather than two markers in one, because a completion marker leaves a dangling `->`
/// and only the first survives recovery.
const TWO_POSITIONS: &str = r#"
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    public function run(): void {}

    public function find(int $id): void {}

    public function send(string $body, bool $urgent): void {}

    public function slice(array $rows, int $from = 0, bool $keys = false): void {}

    public function page(int $size = 20): void {}
}

//- /src/Alone.php
<?php
namespace App;

class Alone
{
    public function go(Repo $repo): void
    {
        $repo->@alone@
    }
}

//- /src/Assigned.php
<?php
namespace App;

class Assigned
{
    public function go(Repo $repo): void
    {
        $held = $repo->@assigned@
    }
}
"#;

#[tokio::test]
async fn a_snippet_client_is_sent_the_brackets_the_cursor_and_the_semicolon() {
    let mut server = started(TWO_POSITIONS, "src/Alone.php", true).await;
    let offered = offered_at(&mut server, "src/Alone.php", "alone").await;

    // `find` takes an argument, so the cursor goes between the brackets and the `;` after them;
    // `run` takes none, so the whole call is written and the cursor lands past the `;`.
    //
    // And a parameter is written out as a stop of its own, which is the difference between a
    // completion that finishes the *name* and one that finishes the *call*. Only the required ones:
    // `slice` stops at `$rows` because `$from` and `$keys` have defaults, and `page` -- whose only
    // parameter has one -- gets brackets and a caret between them, exactly as before.
    //
    // The `$` of each name is escaped. Unescaped, a client reads the placeholder's own text as a
    // second variable reference and inserts nothing where the name should be.
    expect![[r#"
        [
            "find -> find(${1:\\$id});$0",
            "page -> page($0);",
            "run -> run();$0",
            "send -> send(${1:\\$body}, ${2:\\$urgent});$0",
            "slice -> slice(${1:\\$rows});$0",
        ]
    "#]]
    .assert_debug_eq(&offered);
}

#[tokio::test]
async fn a_client_without_snippets_still_gets_the_semicolon() {
    // Only the cursor placement is lost. A literal `$0` in the buffer would be worse than pressing
    // End, and a missing `;` is the thing this feature exists to stop.
    let mut server = started(TWO_POSITIONS, "src/Alone.php", false).await;
    let offered = offered_at(&mut server, "src/Alone.php", "alone").await;

    expect![[r#"
        [
            "find -> find();",
            "page -> page();",
            "run -> run();",
            "send -> send();",
            "slice -> slice();",
        ]
    "#]]
    .assert_debug_eq(&offered);
}

#[tokio::test]
async fn an_assignment_is_sent_no_semicolon() {
    let mut server = started(TWO_POSITIONS, "src/Assigned.php", true).await;
    let offered = offered_at(&mut server, "src/Assigned.php", "assigned").await;

    expect![[r#"
        [
            "find -> find(${1:\\$id})$0",
            "page -> page($0)",
            "run -> run()$0",
            "send -> send(${1:\\$body}, ${2:\\$urgent})$0",
            "slice -> slice(${1:\\$rows})$0",
        ]
    "#]]
    .assert_debug_eq(&offered);
}

/// A class body over a parent whose method names a parameter and a class.
///
/// Both are snippet syntax if nothing escapes them: `$depth` would be read as a variable the client
/// substitutes, and the backslashes of `\App\Buffer` as escapes of the letters after them.
const A_STUB_TO_WRITE: &str = r#"
//- /composer.json
{"autoload": {"psr-4": {"App\\": "src"}}}

//- /src/Buffer.php
<?php
namespace App;

class Buffer {}

//- /src/Task.php
<?php
namespace App;

abstract class Task
{
    abstract public function render(Buffer $into, int $depth): int;
}

//- /src/Job.php
<?php
namespace App;

class Job extends Task
{
    ren@stub@
}
"#;

#[tokio::test]
async fn a_generated_declaration_is_escaped_around_its_cursor() {
    let mut server = started(A_STUB_TO_WRITE, "src/Job.php", true).await;
    let offered = offered_at(&mut server, "src/Job.php", "stub").await;

    expect![[r#"
        [
            "render -> public function render(\\\\App\\\\Buffer \\$into, int \\$depth): int\n    {\n        $0\n    \\}",
        ]
    "#]]
    .assert_debug_eq(&offered);
}

#[tokio::test]
async fn a_client_without_snippets_gets_the_declaration_and_no_escapes() {
    // Nothing to escape, because nothing will be read as syntax. The cursor is what is lost, and a
    // literal `\$depth` in the buffer would be worse than losing it.
    let mut server = started(A_STUB_TO_WRITE, "src/Job.php", false).await;
    let offered = offered_at(&mut server, "src/Job.php", "stub").await;

    expect![[r#"
        [
            "render -> public function render(\\App\\Buffer $into, int $depth): int\n    {\n        \n    }",
        ]
    "#]]
    .assert_debug_eq(&offered);
}

/// A server whose client either does or does not read `$0` as a cursor.
///
/// Spelled out here rather than through the shared lifecycle helper, because the capability is the
/// variable this file is about and nothing else needs to state it.
async fn started(fixture: &str, open: &str, snippets: bool) -> Fixture {
    let mut server = Fixture::new(fixture);
    let root = server.project.root().to_path_buf();
    server
        .request(
            "initialize",
            json!({
                "capabilities": {
                    "textDocument": {
                        "completion": { "completionItem": { "snippetSupport": snippets } },
                    },
                },
                "workspaceFolders": [{
                    "uri": format!("file://{}", root.display()),
                    "name": "fixture",
                }],
            }),
        )
        .await;
    server.initialized().await;
    server.open(open).await;
    server
}

/// Each offered item as its label and the text accepting it writes.
async fn offered_at(server: &mut Fixture, relative: &str, marker: &str) -> Vec<String> {
    let position = server.position(relative, marker);
    let response = server
        .request(
            "textDocument/completion",
            json!({
                "textDocument": { "uri": server.uri(relative) },
                "position": position,
            }),
        )
        .await;

    let value: Value =
        serde_json::to_value(response.result().expect("completion answered")).expect("json");
    value["items"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|item| {
            format!(
                "{} -> {}",
                item["label"].as_str().unwrap_or("?"),
                item["textEdit"]["newText"].as_str().unwrap_or("?")
            )
        })
        .collect()
}
