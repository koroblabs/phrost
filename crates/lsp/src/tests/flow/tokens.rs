//! Semantic tokens, decoded back out of the wire format.
//!
//! What each name *is* belongs to `phrost-syntax-features` and is settled there. What belongs here
//! is the encoding, and it is the part most able to be silently wrong: the client receives five
//! numbers per token, each relative to the token before it, so one bad delta moves every token after
//! it and the file is coloured plausibly and incorrectly. So these tests decode the stream back to
//! absolute positions and read them.

use expect_test::expect;
use serde_json::Value;

use crate::tests::Fixture;

const NAMED: &str = r#"
//- /src/Repo.php
<?php
namespace App;

use App\Nested\Reads;

class Repo implements Reads
{
    public int $count = 0;

    public function find(int $id): void
    {
        $this->count = $id;
    }
}
"#;

#[tokio::test]
async fn the_stream_decodes_back_to_the_names_it_was_made_from() {
    let mut server = Fixture::started(NAMED, "src/Repo.php").await;

    let response = server.semantic_tokens("src/Repo.php").await;
    expect![[r#"
        0:0    5   keyword    
        1:0    9   keyword    
        1:10   3   namespace  
        3:0    3   keyword    
        3:4    11  namespace  
        3:15   5   class      
        5:0    5   keyword    
        5:6    4   class      declaration
        5:11   10  keyword    
        5:22   5   interface  
        7:4    6   keyword    
        7:11   3   type       
        7:15   6   property   declaration
        7:24   1   number     
        9:4    6   keyword    
        9:11   8   keyword    
        9:20   4   method     declaration
        9:25   3   type       
        9:29   3   parameter  declaration
        9:35   4   type       
        11:8   5   keyword    
        11:15  5   property   
        11:23  3   parameter
    "#]]
    .assert_eq(&decode(&response));
}

#[tokio::test]
async fn a_range_request_answers_about_that_range_only() {
    // A client asks per viewport on a large file. The first token of the answer still has to be
    // encoded against the start of the document, not against the start of the range.
    let mut server = Fixture::started(NAMED, "src/Repo.php").await;

    let response = server.semantic_tokens_in("src/Repo.php", (9, 12)).await;
    expect![[r#"
        9:4    6   keyword    
        9:11   8   keyword    
        9:20   4   method     declaration
        9:25   3   type       
        9:29   3   parameter  declaration
        9:35   4   type       
        11:8   5   keyword    
        11:15  5   property   
        11:23  3   parameter
    "#]]
    .assert_eq(&decode(&response));
}

#[tokio::test]
async fn colouring_is_answered_while_the_project_is_still_being_indexed() {
    // The same argument as folding: it reads the buffer, so refusing would be a lie about what the
    // server can do -- and an editor asks for colours the instant a file opens.
    let mut server = Fixture::new(NAMED);
    server.initialize(false, true).await;
    server.open("src/Repo.php").await;

    let during = decode(&server.semantic_tokens("src/Repo.php").await);

    // Against the answer once the project is ready rather than against a count, which is the
    // invariant this test is actually about -- and a number here goes stale every time the token set
    // grows, which is how it came to assert ten of them long after there were more.
    server.initialized().await;
    let after = decode(&server.semantic_tokens("src/Repo.php").await);

    assert_eq!(
        during, after,
        "colouring must not depend on whether the project has been indexed"
    );
    assert!(during.lines().count() > 1, "and must not be empty");
}

/// The wire format read back as absolute positions.
///
/// Deliberately a decoder rather than a dump of the numbers: the numbers are unreadable, and a test
/// nobody can read is a test that gets accepted wrong.
fn decode(response: &crate::tests::Response) -> String {
    let Some(result) = response.result() else {
        return "no result".to_owned();
    };
    let Ok(result) = serde_json::to_value(result) else {
        return "unreadable".to_owned();
    };
    let Some(data) = result["data"].as_array() else {
        return "no answer".to_owned();
    };

    let (mut line, mut character) = (0_i64, 0_i64);
    let mut rendered = String::new();
    for token in data.chunks(5) {
        let [delta_line, delta_start, length, kind, modifiers] = token else {
            return "truncated stream".to_owned();
        };
        let delta_line = number(delta_line);
        line += delta_line;
        character = if delta_line == 0 {
            character + number(delta_start)
        } else {
            number(delta_start)
        };

        let at = format!("{line}:{character}");
        rendered.push_str(&format!(
            "{at:<7}{:<4}{:<11}{}\n",
            number(length),
            TYPES[number(kind) as usize],
            names(number(modifiers)),
        ));
    }
    rendered.trim_end().to_owned() + "\n"
}

/// The legend the server declares, repeated here on purpose: a test that asks the server what its
/// own indices mean cannot catch the server renumbering them.
const TYPES: &[&str] = &[
    "namespace",
    "class",
    "interface",
    "enum",
    "enumMember",
    "property",
    "method",
    "function",
    "parameter",
    "variable",
    "decorator",
    "type",
    "keyword",
    "comment",
    "string",
    "number",
];

const MODIFIERS: &[&str] = &["declaration", "static", "readonly", "abstract"];

fn names(bits: i64) -> String {
    MODIFIERS
        .iter()
        .enumerate()
        .filter(|(bit, _)| bits & (1 << bit) != 0)
        .map(|(_, name)| *name)
        .collect::<Vec<_>>()
        .join(" ")
}

fn number(value: &Value) -> i64 {
    value.as_i64().unwrap_or(-1)
}
