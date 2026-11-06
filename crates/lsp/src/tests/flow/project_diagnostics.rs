//! The whole project is checked in the background, and the problems panel shows all of it.
//!
//! Once the index is ready, every file the project owns that nobody has open is checked and
//! published; a client keeps only what it was sent, so a file with nothing wrong is not published at
//! all unless it was reported before. Open buffers are the editor's and are left alone. A change on
//! disk checks the project again, and closing a tab keeps what the disk says rather than clearing it.

use expect_test::expect;
use serde_json::json;

use crate::tests::Fixture;

/// A project with one clean file, one with a problem, and a dependency with a problem of its own.
const PROJECT: &str = r#"
//- /composer.json
{"autoload": {"psr-4": {"App\\": "src"}}, "require": {"acme/lib": "*"}}

//- /vendor/composer/autoload_psr4.php
<?php
$vendorDir = dirname(__DIR__);
$baseDir = dirname($vendorDir);
return array(
    'App\\' => array($baseDir . '/src'),
    'Acme\\' => array($vendorDir . '/acme/lib/src'),
);

//- /src/Repo.php
<?php
namespace App;

class Repo
{
    public function find(int $id): void {}
}

//- /src/Service.php
<?php
namespace App;

class Service
{
    public function run(Repo $repo): void
    {
        $repo->find(1);
        $repo->fnid(1);
    }
}

//- /vendor/acme/lib/src/Thing.php
<?php
namespace Acme;

class Thing
{
    public function go(Thing $other): void
    {
        $other->nope();
    }
}
"#;

#[tokio::test]
async fn every_file_the_project_owns_is_checked_once_the_index_is_ready() {
    let mut server = Fixture::new(PROJECT);
    server.initialize(true, false).await;
    server.initialized().await;

    // The project's own file with a problem, published without anybody opening it; the clean one
    // and the dependency's not published at all.
    expect![[r#"
        [
            "Service.php:9: undefined-method Undefined method App\\Repo::fnid()",
        ]
    "#]]
    .assert_debug_eq(&server.diagnostics_published());

    // Its own bar, after the indexing one: two tokens created, two begin/end pairs.
    let created = server
        .methods_sent()
        .iter()
        .filter(|method| *method == "window/workDoneProgress/create")
        .count();
    assert_eq!(created, 2, "{:?}", server.methods_sent());
    let kinds = server.progress_kinds();
    assert_eq!(
        kinds.iter().filter(|kind| *kind == "begin").count(),
        2,
        "{kinds:?}"
    );
    assert_eq!(kinds.last().map(String::as_str), Some("end"), "{kinds:?}");
}

#[tokio::test]
async fn an_open_buffer_is_left_to_the_editor() {
    // Opened before the index is ready, so the pass finds it open: what is published for it comes
    // from the buffer, once, when bootstrap checks the open documents -- not a second time from
    // disk. The other file is published by the pass, as before.
    let mut server = Fixture::new(PROJECT);
    server.initialize(false, false).await;
    server.open("src/Service.php").await;
    server.initialized().await;

    expect![[r#"
        [
            "Service.php:9: undefined-method Undefined method App\\Repo::fnid()",
        ]
    "#]]
    .assert_debug_eq(&server.diagnostics_published());
}

#[tokio::test]
async fn closing_a_tab_keeps_what_the_disk_says() {
    let mut server = Fixture::new(PROJECT);
    server.initialize(false, false).await;
    server.initialized().await;

    server.open("src/Service.php").await;
    server.close("src/Service.php").await;

    // The pass, the buffer, and the disk again on close: three times the same problem, never
    // "none", because the file on disk still has it.
    expect![[r#"
        [
            "Service.php:9: undefined-method Undefined method App\\Repo::fnid()",
            "Service.php:9: undefined-method Undefined method App\\Repo::fnid()",
            "Service.php:9: undefined-method Undefined method App\\Repo::fnid()",
        ]
    "#]]
    .assert_debug_eq(&server.diagnostics_published());
}

#[tokio::test]
async fn a_change_on_disk_checks_the_project_again_and_clears_what_was_fixed() {
    let mut server = Fixture::new(PROJECT);
    server.initialize(false, true).await;
    server.initialized().await;

    // The method gains the name the caller uses, on disk, as a save or a branch switch would do it.
    let project = server.watch_project_passes();
    server
        .write_and_notify(
            r#"
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    public function find(int $id): void {}
    public function fnid(int $id): void {}
}
"#,
        )
        .await;
    server.project_pass_ends(project).await;

    // Reported once with the problem, and once more with none when the problem went -- that file
    // and no other, because it is the one the client had been told about.
    expect![[r#"
        [
            "Service.php:9: undefined-method Undefined method App\\Repo::fnid()",
            "Service.php: none",
        ]
    "#]]
    .assert_debug_eq(&server.diagnostics_published());
}

#[tokio::test]
async fn the_project_check_can_be_switched_off() {
    let server =
        Fixture::started_with_settings(PROJECT, json!({"diagnostics": {"project": false}})).await;

    assert!(
        server.diagnostics_published().is_empty(),
        "nothing is open, so nothing should have been published: {:?}",
        server.diagnostics_published()
    );
}
