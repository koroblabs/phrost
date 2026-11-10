//! `\Drupal::service('application.mana|')`: completing an id after its dot.
//!
//! The client filters with the word it sees at the cursor -- `mana`, past the dot -- and ranks every
//! id containing that word by how well it matches, ahead of any order the server sent. So the ids
//! starting with what was typed sank among `app_documents.manager` and the rest. An id
//! that starts with everything typed before that word is handed over with its `filterText` as the
//! rest of itself, which is what the client's word is then compared against -- and every item
//! carries its place in the list as `sortText`, which is what the client falls back on where its
//! scores tie.
//!
//! Its edit replaces that word and no more, so a client that filters by the text an edit replaces
//! compares the same two strings: see `an_item_going_on_from_what_was_typed`.

use expect_test::expect;
use serde_json::json;

use crate::tests::Fixture;

#[tokio::test]
async fn an_id_starting_with_what_was_typed_is_filtered_by_its_rest() {
    let mut server = Fixture::started(
        r#"
//- /composer.json
{}

//- /web/core/lib/Drupal.php
<?php class Drupal { public static function service(string $id): object { return new \stdClass(); } }

//- /web/modules/mine/mine.info.yml
name: Mine

//- /web/modules/mine/src/Manager.php
<?php
namespace Drupal\mine;

class Manager {}

//- /web/modules/mine/mine.services.yml
services:
  application.manager:
    class: Drupal\mine\Manager
  application.memo.manager:
    class: Drupal\mine\Manager
  app_documents.manager:
    class: Drupal\mine\Manager

//- /web/modules/mine/src/Use.php
<?php
namespace Drupal\mine;

function use_it(): object
{
    return \Drupal::service('application.mana@here@');
}
"#,
        "web/modules/mine/src/Use.php",
    )
    .await;
    server.settle().await;

    let position = server.position("web/modules/mine/src/Use.php", "here");
    let response = server
        .request(
            "textDocument/completion",
            json!({
                "textDocument": { "uri": server.uri("web/modules/mine/src/Use.php") },
                "position": position,
            }),
        )
        .await;
    let result = serde_json::to_value(response.result().expect("completion answered"))
        .expect("serialisable");
    let text = server.text("web/modules/mine/src/Use.php");
    let rendered = result["items"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|item| {
            format!(
                "{} sort={}",
                super::an_item_going_on_from_what_was_typed::offered(&text, item),
                item["sortText"].as_str().unwrap_or("(none)")
            )
        })
        .collect::<Vec<_>>();

    expect![[r#"
        (
            Bool(true),
            [
                "application.manager replaces `mana` with `manager` filter=manager sort=00000",
                "application.memo.manager replaces `mana` with `memo.manager` filter=memo.manager sort=00001",
            ],
        )
    "#]]
    .assert_debug_eq(&(result["isIncomplete"].clone(), rendered));
}

/// The same in a services file, which is answered by another path and had the same blind spot.
#[tokio::test]
async fn an_id_in_a_services_file_is_filtered_by_its_rest_too() {
    let mut server = Fixture::started(
        r#"
//- /composer.json
{}

//- /web/core/lib/Drupal.php
<?php class Drupal {}

//- /web/modules/mine/mine.info.yml
name: Mine

//- /web/modules/mine/src/Manager.php
<?php
namespace Drupal\mine;

class Manager {}

//- /web/modules/mine/mine.services.yml
services:
  application.manager:
    class: Drupal\mine\Manager
  application.memo.manager:
    class: Drupal\mine\Manager
  app_documents.manager:
    class: Drupal\mine\Manager
  mine.user:
    class: Drupal\mine\Manager
    arguments: ['@application.mana@here@']

//- /web/modules/mine/src/Use.php
<?php
namespace Drupal\mine;
"#,
        "web/modules/mine/src/Use.php",
    )
    .await;
    server.open("web/modules/mine/mine.services.yml").await;
    server.settle().await;

    let position = server.position("web/modules/mine/mine.services.yml", "here");
    let response = server
        .request(
            "textDocument/completion",
            json!({
                "textDocument": { "uri": server.uri("web/modules/mine/mine.services.yml") },
                "position": position,
            }),
        )
        .await;
    let result = serde_json::to_value(response.result().expect("completion answered"))
        .expect("serialisable");
    let text = server.text("web/modules/mine/mine.services.yml");
    let rendered = result["items"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|item| {
            format!(
                "{} sort={}",
                super::an_item_going_on_from_what_was_typed::offered(&text, item),
                item["sortText"].as_str().unwrap_or("(none)")
            )
        })
        .collect::<Vec<_>>();

    expect![[r#"
        (
            Bool(true),
            [
                "application.manager replaces `mana` with `manager` filter=manager sort=00000",
                "application.memo.manager replaces `mana` with `memo.manager` filter=memo.manager sort=00001",
            ],
        )
    "#]]
    .assert_debug_eq(&(result["isIncomplete"].clone(), rendered));
}
