//! Folding, which is the one answer that does not wait for the project.
//!
//! What folds is settled next door, in `phrost-syntax-features`, over a hundred lines of PHP that
//! never reach a server. What is left for this layer is the sequencing: that the answer arrives
//! before indexing has finished, that it describes the buffer rather than the file on disk, and that
//! a document nobody opened gets no answer instead of a disk read.

use expect_test::expect;
use serde_json::Value;

use crate::tests::{Fixture, flow::render_span};

/// One file with something to fold at three nesting levels, and a second that is never opened.
const FOLDABLE: &str = r#"
//- /src/Repo.php
<?php
namespace App;

use App\Nested\Reads;
use App\Nested\Writes;

class Repo
{
    public function find(int $id): void
    {
        return;
    }
}

//- /src/Other.php
<?php
namespace App;

class Other
{
}
"#;

#[tokio::test]
async fn folding_is_answered_while_the_project_is_still_being_indexed() {
    // Every other request is refused until bootstrap finishes, and rightly. This one reads the
    // buffer and nothing else, so refusing it would be a lie about what the server can do -- and
    // the first seconds after opening an editor are when a reader is most likely to ask.
    let mut server = Fixture::new(FOLDABLE);
    server.initialize(false, true).await;
    server.open("src/Repo.php").await;

    let response = server.folding("src/Repo.php").await;
    expect![[r#"
        imports 3:0-4:22
        block   7:0-12:1
        block   9:4-11:5
    "#]]
    .assert_eq(&render(&response));
}

#[tokio::test]
async fn the_answer_describes_the_buffer_and_not_the_file_on_disk() {
    // Folds are drawn at line numbers. Answering from a stale revision does not merely lose a fold,
    // it puts the surviving ones on the wrong lines.
    let mut server = Fixture::started(FOLDABLE, "src/Repo.php").await;
    server
        .change(
            "src/Repo.php",
            2,
            "<?php\n\n$build = [\n    'a' => 1,\n];\n",
        )
        .await;

    let response = server.folding("src/Repo.php").await;
    expect![[r#"
        block   2:9-4:1
    "#]]
    .assert_eq(&render(&response));
}

#[tokio::test]
async fn a_document_that_was_never_opened_is_not_read_from_disk_to_answer() {
    // An editor only asks about what it has open. Reading the file instead would be a fallback --
    // and one that answers about text the editor may already have changed.
    let mut server = Fixture::started(FOLDABLE, "src/Repo.php").await;

    let response = server.folding("src/Other.php").await;
    expect!["no answer"].assert_eq(&render(&response));
}

/// Every fold as its protocol kind and span, in the order the server sent them.
fn render(response: &crate::tests::Response) -> String {
    let Some(result) = response.result() else {
        return "no result".to_owned();
    };
    let Ok(folds) = serde_json::to_value(result) else {
        return "unreadable".to_owned();
    };
    let Some(folds) = folds.as_array() else {
        return "no answer".to_owned();
    };

    folds
        .iter()
        .map(|fold| {
            format!(
                "{:<7} {}\n",
                fold["kind"].as_str().unwrap_or("block"),
                render_span(&span_of(fold))
            )
        })
        .collect()
}

/// A folding range is four flat fields; `render_span` reads the nested shape every other range in
/// these tests has.
fn span_of(fold: &Value) -> Value {
    serde_json::json!({
        "start": { "line": fold["startLine"], "character": fold["startCharacter"] },
        "end": { "line": fold["endLine"], "character": fold["endCharacter"] },
    })
}
