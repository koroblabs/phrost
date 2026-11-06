//! An open configuration buffer is checked on the same schedule as a PHP one.
//!
//! A `services.yml` is held as text and never given a `FileId`, so everything that found an open
//! buffer through the file table found PHP and nothing else: the version a quiet edit is compared
//! against was always absent, and the sweep that checks what is open when indexing ends walked past
//! it. Driven against a real project the findings arrived only for a file opened after indexing,
//! and then never changed again however it was edited.

use expect_test::expect;
use serde_json::json;

use crate::tests::Fixture;

const WIRED: &str = r#"
//- /composer.json
{}
//- /web/core/lib/Drupal.php
<?php class Drupal {}
//- /web/modules/mine/mine.info.yml
name: Mine
//- /web/modules/mine/src/Repo.php
<?php
namespace Drupal\mine;

class Repo {}
//- /web/modules/mine/mine.services.yml
services:
  mine.repo:
    class: Drupal\mine\Repo
  mine.other:
    class: Drupal\mine\Repo
    arguments: ['@mine.repo']
"#;

const SERVICES: &str = "web/modules/mine/mine.services.yml";

/// The same file with the argument misspelt, as a person typing into it leaves it.
const MISSPELT: &str = "services:\n  mine.repo:\n    class: Drupal\\mine\\Repo\n  mine.other:\n    class: Drupal\\mine\\Repo\n    arguments: ['@mine.repp']\n";

#[tokio::test(start_paused = true)]
async fn an_edit_is_checked_once_the_buffer_has_been_still() {
    let mut server = Fixture::started(WIRED, "web/modules/mine/src/Repo.php").await;
    server.open(SERVICES).await;
    server.settle().await;

    server.change(SERVICES, 2, MISSPELT).await;
    server.wait_out_the_quiet().await;

    let published = server.diagnostics_published();
    expect![[r#"
        [
            "mine.services.yml: none",
            "mine.services.yml:6: unknown-catalog-key Nothing registers the service `mine.repp`. Did you mean `mine.repo`?",
        ]
    "#]]
    .assert_debug_eq(
        &published
            .iter()
            .filter(|line| line.starts_with("mine.services.yml"))
            .collect::<Vec<_>>(),
    );
}

#[tokio::test]
async fn a_buffer_opened_while_indexing_is_checked_when_it_ends() {
    // Opened between `initialize` and `initialized`: the rules that say which files are read are
    // installed by the first, and the catalog that judges them is filled by the second.
    let mut server = Fixture::new(WIRED);
    server.initialize(false, false).await;
    let uri = server.uri(SERVICES);
    server
        .notify(
            "textDocument/didOpen",
            json!({ "textDocument": {
                "uri": uri, "languageId": "yaml", "version": 1, "text": MISSPELT,
            }}),
        )
        .await;
    server.initialized().await;

    let published = server.diagnostics_published();
    expect![[r#"
        [
            "mine.services.yml:6: unknown-catalog-key Nothing registers the service `mine.repp`. Did you mean `mine.repo`?",
        ]
    "#]]
    .assert_debug_eq(
        &published
            .iter()
            .filter(|line| line.starts_with("mine.services.yml"))
            .collect::<Vec<_>>(),
    );
}
