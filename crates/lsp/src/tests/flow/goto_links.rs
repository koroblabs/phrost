//! What a goto answer says about the click itself, not only about where it lands.
//!
//! The wire half of `phrost-analysis`'s `the_span_a_jump_is_asked_from`. The span is worked out
//! there; what this layer owns is that it arrives converted against the *asking* file, which is a
//! different file from the one every other range in the answer belongs to -- and a byte offset
//! converted against the wrong text is a line and column that exist, in the wrong place. Two files
//! of different lengths are what make that visible, so a driven server is the instrument.

use expect_test::expect;
use lspkit_runtime::protocol::jsonrpc::Response;
use serde_json::Value;

use crate::tests::{Fixture, flow::render_span};

/// A short file that declares, and a longer one that points at it from further down.
const TWO_FILES: &str = r#"
//- /src/Nested/Thing.php
<?php
namespace App\Nested;

class Thing {}

//- /src/Repo.php
<?php
namespace App;

/**
 * Holds things.
 */
class R@declaration@epo
{
    public function run(\App\Nes@qualified@ted\Thing $given): void {}
}
"#;

#[tokio::test]
async fn the_answer_names_the_whole_written_name_as_the_span_that_was_clicked() {
    // A client told nothing works the span out of the buffer by its own idea of where a word ends,
    // and a backslash is a word character to some editors and a separator to others -- so the link
    // it draws over `\App\Nested\Thing` is either the whole name or the segment under the mouse,
    // depending on which editor is asking. Saying it removes the guess.
    let mut server = Fixture::started(TWO_FILES, "src/Repo.php").await;

    let response = server.definition("src/Repo.php", "qualified").await;
    expect!["clicked 8:24-8:41 -> Thing.php 3:6-3:11 (full 3:0-3:14)"]
        .assert_eq(&render(&response));
}

#[tokio::test]
async fn a_click_on_a_declarations_own_name_is_answered_with_that_declaration() {
    // Deliberate, and worth pinning because of what a client then does with it: the answer is the
    // position the reader is already on, so nothing moves. Editors differ on whether that means
    // "no navigation" -- some then offer the references instead -- and the server's part is to say
    // plainly where the click was and where it goes, rather than to withhold an answer it has.
    let mut server = Fixture::started(TWO_FILES, "src/Repo.php").await;

    let response = server.definition("src/Repo.php", "declaration").await;
    expect!["clicked 6:6-6:10 -> Repo.php 6:6-6:10 (full 6:0-9:1)"].assert_eq(&render(&response));
}

/// Every link, as the span it was asked from and the one it leads to.
fn render(response: &Response) -> String {
    let value = serde_json::to_value(response.result().expect("a result")).expect("json");
    let Some(links) = value.as_array() else {
        return "(nothing to go to)".to_owned();
    };

    links
        .iter()
        .map(|link| {
            let file = link["targetUri"]
                .as_str()
                .unwrap_or_default()
                .rsplit('/')
                .next()
                .unwrap_or_default()
                .to_owned();
            let clicked = match &link["originSelectionRange"] {
                Value::Null => "(not said)".to_owned(),
                range => render_span(range),
            };
            format!(
                "clicked {clicked} -> {file} {} (full {})",
                render_span(&link["targetSelectionRange"]),
                render_span(&link["targetRange"]),
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// A Drupal module whose names are written outside PHP code: a class and an optional service id in
/// its YAML, and a route name in a PHP string.
const OUTSIDE_PHP: &str = r#"
//- /composer.json
{}
//- /web/core/lib/Drupal.php
<?php class Drupal {}
//- /web/core/lib/Drupal/Core/Url.php
<?php
namespace Drupal\Core;

class Url
{
    public static function fromRoute(string $route_name): static { return new static(); }
}
//- /web/modules/mine/mine.info.yml
name: Mine
//- /web/modules/mine/mine.routing.yml
mine.page:
  path: '/page'
//- /web/modules/mine/mine.services.yml
services:
  mine.repo:
    class: Drupal\mine\Re@class@po
  mine.other:
    class: Drupal\mine\Repo
    arguments: ['@?mine.re@optional@po']
//- /web/modules/mine/src/Repo.php
<?php
namespace Drupal\mine;

use Drupal\Core\Url;

class Repo
{
    public function page(): Url { return Url::fromRoute('mine.pa@route@ge'); }
}
"#;

const SERVICES: &str = "web/modules/mine/mine.services.yml";

#[tokio::test]
async fn a_class_named_in_yaml_is_clicked_as_the_whole_name() {
    // The reason the PHP answer says its span holds here twice over: a YAML client's idea of a word
    // stops at the backslashes of a class and the dots of a service id, so left to itself it
    // underlines `mine` in `mine.repo` and `Repo` in `Drupal\mine\Repo`.
    let mut server = Fixture::started(OUTSIDE_PHP, "web/modules/mine/src/Repo.php").await;
    server.open(SERVICES).await;

    let response = server.definition(SERVICES, "class").await;
    expect!["clicked 2:11-2:27 -> Repo.php 5:6-5:10 (full 5:6-5:10)"].assert_eq(&render(&response));
}

#[tokio::test]
async fn a_service_id_in_yaml_is_clicked_as_the_id_without_its_marks() {
    let mut server = Fixture::started(OUTSIDE_PHP, "web/modules/mine/src/Repo.php").await;
    server.open(SERVICES).await;

    let response = server.definition(SERVICES, "optional").await;
    expect!["clicked 5:19-5:28 -> Repo.php 5:6-5:10 (full 5:6-5:10)"].assert_eq(&render(&response));
}

#[tokio::test]
async fn a_key_in_a_php_string_is_clicked_as_the_whole_key() {
    // A route name is answered by a different path from a class -- it names a place in a file the
    // index holds no rows for -- and that path is the one that said nothing about the click.
    let mut server = Fixture::started(OUTSIDE_PHP, "web/modules/mine/src/Repo.php").await;

    let response = server
        .definition("web/modules/mine/src/Repo.php", "route")
        .await;
    expect!["clicked 7:57-7:66 -> mine.routing.yml 0:0-0:9 (full 0:0-0:9)"]
        .assert_eq(&render(&response));
}
