//! Where a project's settings come from, driven through a real server.
//!
//! `phrost-project` owns *which* source wins; this owns the wiring -- that `initializationOptions`
//! reaches the one place that resolves settings at all, that it gets there before the root is fixed
//! and the file is read, and that a payload the server cannot understand is said out loud instead of
//! quietly switching a project's policy off.

use expect_test::expect;
use serde_json::json;

use crate::tests::Fixture;

/// One method whose docblock documents one of its two parameters.
///
/// `doc-missing-tags` is off by default and is the index's own check rather than a syntax rule, so a
/// finding here proves both that the setting arrived and that it reached the engine a person naming a
/// rule does not think about.
const UNDOCUMENTED: &str = r#"
//- /composer.json
{"autoload": {"psr-4": {"App\\": "src"}}}

//- /src/Repo.php
<?php
namespace App;

class Repo
{
    /**
     * Finds a row.
     *
     * @param int $id
     *   The identifier.
     */
    public function find(int $id, bool $strict): void {}
}
"#;

/// The same project, with the rule switched on by a committed file instead.
const UNDOCUMENTED_WITH_FILE: &str = r#"
//- /composer.json
{"autoload": {"psr-4": {"App\\": "src"}}}

//- /phrost.toml
[lint]
on = ["doc-missing-tags"]

//- /src/Repo.php
<?php
namespace App;

class Repo
{
    /**
     * Finds a row.
     *
     * @param int $id
     *   The identifier.
     */
    public function find(int $id, bool $strict): void {}
}
"#;

#[tokio::test]
async fn a_rule_the_editor_names_is_reported_with_no_file_at_all() {
    // The case the feature exists for: nothing committed, the settings live in `.zed/settings.json`
    // and reach the server as `initializationOptions`.
    let mut server = Fixture::started_with_settings(
        UNDOCUMENTED,
        json!({ "lint": { "on": ["doc-missing-tags"] } }),
    )
    .await;
    server.open("src/Repo.php").await;

    expect![[r#"
        [
            "Repo.php:12: doc-missing-tags $strict has no @param",
            "Repo.php:12: doc-missing-tags $strict has no @param",
        ]
    "#]]
    .assert_debug_eq(&server.diagnostics_published());
}

#[tokio::test]
async fn the_same_rule_named_only_by_the_file_is_reported_too() {
    // The committed half has to keep working unchanged, or `phrost check` and every measurement
    // built on it stop meaning anything.
    let mut server = Fixture::started_with_settings(UNDOCUMENTED_WITH_FILE, json!(null)).await;
    server.open("src/Repo.php").await;

    expect![[r#"
        [
            "Repo.php:12: doc-missing-tags $strict has no @param",
            "Repo.php:12: doc-missing-tags $strict has no @param",
        ]
    "#]]
    .assert_debug_eq(&server.diagnostics_published());
}

#[tokio::test]
async fn the_editor_can_take_back_a_rule_the_file_switched_on() {
    // The direction that only works because the editor *replaces* the list rather than extending it:
    // a person who disagrees with the committed policy has to be able to contradict it, not only add
    // to it. Nothing is reported, which is the whole assertion.
    let mut server =
        Fixture::started_with_settings(UNDOCUMENTED_WITH_FILE, json!({ "lint": { "on": [] } }))
            .await;
    server.open("src/Repo.php").await;

    expect![[r#"
        [
            "Repo.php: none",
        ]
    "#]]
    .assert_debug_eq(&server.diagnostics_published());
}

#[tokio::test]
async fn the_editors_settings_survive_a_composer_change() {
    // Composer configuration changing throws the index away and reads the root again, which reads
    // `phrost.toml` again. The editor sends `initializationOptions` exactly once, at startup, so if
    // that re-read did not still have them a person's settings would vanish partway through a
    // session -- the worst shape this failure can take, because it works until something unrelated
    // happens.
    let mut server = Fixture::started_with_settings(
        UNDOCUMENTED,
        json!({ "lint": { "on": ["doc-missing-tags"] } }),
    )
    .await;
    // Waited out, because the reindex publishes for the closed file again once its own project
    // check runs -- and reading the list before that is a race the workers' timing decides.
    let passes = server.watch_passes();
    server.notify_changed("composer.json").await;
    server.pass_ends(passes).await;
    server.open("src/Repo.php").await;

    // The first project check, the one after the reindex, and the buffer on open.
    expect![[r#"
        [
            "Repo.php:12: doc-missing-tags $strict has no @param",
            "Repo.php:12: doc-missing-tags $strict has no @param",
            "Repo.php:12: doc-missing-tags $strict has no @param",
        ]
    "#]]
    .assert_debug_eq(&server.diagnostics_published());
}

#[tokio::test]
async fn a_misspelled_editor_setting_is_logged_and_the_file_still_applies() {
    // A typo in a settings file that silently disabled every rule would look exactly like the server
    // being broken. So it is reported, and what was already in force stays in force.
    let mut server =
        Fixture::started_with_settings(UNDOCUMENTED_WITH_FILE, json!({ "lint": { "of": [] } }))
            .await;
    server.open("src/Repo.php").await;

    // Every warning, not the one this test is about: a snapshot of the whole log is what would show
    // a second, contradictory complaint about the same settings.
    expect![[r#"
        [
            "this client cannot watch files, so changes made outside the editor (a branch switch, a composer install) will not be picked up",
            "initializationOptions: unknown field `of`, expected one of `categories`, `off`, `on` -- it was ignored, so its settings are not in force",
        ]
    "#]]
    .assert_debug_eq(&server.logged_warnings());
    expect![[r#"
        [
            "Repo.php:12: doc-missing-tags $strict has no @param",
            "Repo.php:12: doc-missing-tags $strict has no @param",
        ]
    "#]]
    .assert_debug_eq(&server.diagnostics_published());
}

#[tokio::test]
async fn a_rule_the_file_switches_on_while_a_buffer_is_open_reaches_that_buffer() {
    // The order a person actually works in, which is the whole point: the file is already open when
    // the policy changes. Reading the configuration again is not enough -- the buffer on screen has
    // to be told, or the setting looks like it did nothing until the file is touched.
    let mut server = Fixture::started_with_settings(UNDOCUMENTED, json!(null)).await;
    server.open("src/Repo.php").await;
    expect![[r#"
        [
            "Repo.php: none",
        ]
    "#]]
    .assert_debug_eq(&server.diagnostics_published());

    let passes = server.watch_passes();
    server
        .write_and_notify(
            r#"
//- /phrost.toml
[lint]
on = ["doc-missing-tags"]
"#,
        )
        .await;
    server.pass_ends(passes).await;

    expect![[r#"
        [
            "Repo.php: none",
            "Repo.php:12: doc-missing-tags $strict has no @param",
        ]
    "#]]
    .assert_debug_eq(&server.diagnostics_published());
}

/// A framework the project switches on itself, because detection did not find it.
///
/// Detection probes the filesystem, so it answers no for a layout nobody anticipated -- a docroot
/// under an unusual name, an application assembled by hand, a package holding a framework's classes
/// without being an application. The symptom is every service id, hook and generated route
/// resolving to nothing, which reads as the server being broken rather than as one probe answering
/// no. Read off the startup log, because that is the only place a person can see which it was.
const NO_MARKERS: &str = r#"
//- /composer.json
{"autoload": {"psr-4": {"App\\": "src"}}}

//- /src/Repo.php
<?php
namespace App;

class Repo {}
"#;

#[tokio::test]
async fn a_framework_the_project_names_is_switched_on_and_said_so() {
    let server =
        Fixture::started_with_settings(NO_MARKERS, json!({ "framework": ["laravel"] })).await;

    expect![
        "phrost indexed 1 of 1 discovered files (0 reused from cache, 0 of them without reading), framework laravel (configured)"
    ]
    .assert_eq(server.logged_indexing().as_deref().unwrap_or_default());
}

/// The same project with nothing named, so the pair says which half the marker is testing.
#[tokio::test]
async fn a_project_that_names_none_reports_none() {
    let server = Fixture::started_with_settings(NO_MARKERS, json!({})).await;

    expect![
        "phrost indexed 1 of 1 discovered files (0 reused from cache, 0 of them without reading), framework none detected"
    ]
    .assert_eq(server.logged_indexing().as_deref().unwrap_or_default());
}

/// A misspelling is said out loud rather than quietly doing nothing.
///
/// The whole failure this guards against: `larvel` in a settings file turns a framework's entire
/// knowledge off and looks exactly like the feature not working.
#[tokio::test]
async fn a_framework_that_names_nothing_is_reported() {
    let server =
        Fixture::started_with_settings(NO_MARKERS, json!({ "framework": ["larvel"] })).await;

    let said = server.logged_warnings().join("\n");
    assert!(
        said.contains("unknown framework larvel"),
        "the misspelling has to be visible; log was:\n{said}"
    );
    assert!(
        said.contains("drupal"),
        "and the known ids beside it; log was:\n{said}"
    );
}
