//! Changes made outside the editor.
//!
//! The gap this closes is the one a user cannot diagnose: after a `git switch` or a
//! `composer install`, an index that never hears about it describes a checkout that no longer
//! exists, and every answer afterwards is confidently about the wrong code.

use expect_test::expect;
use serde_json::json;

use super::{Fixture, definition_target, error_message};

/// One file referencing a class nothing declares yet.
const AWAITING_A_PARENT: &str = r#"
//- /composer.json
{}

//- /src/Repo.php
<?php
namespace App;

class Repo extends Ba@parent@se {}
"#;

#[tokio::test]
async fn a_file_created_on_disk_becomes_resolvable_without_being_opened() {
    // What a `git switch` or a code generator does. Nobody opens the file, so document sync never
    // hears about it, and only the watcher can say it is there.
    let mut server = Fixture::started(AWAITING_A_PARENT, "src/Repo.php").await;

    let before = server.definition("src/Repo.php", "parent").await;
    assert!(definition_target(&before, server.project.root()).is_none());

    server
        .write_and_notify(
            r#"
//- /src/Base.php
<?php
namespace App;

class Base {}
"#,
        )
        .await;

    let after = server.definition("src/Repo.php", "parent").await;
    expect!["src/Base.php"].assert_eq(
        &definition_target(&after, server.project.root()).expect("the new file is indexed"),
    );
}

#[tokio::test]
async fn a_file_deleted_on_disk_stops_resolving() {
    let mut server = Fixture::started(
        r#"
//- /composer.json
{}

//- /src/Base.php
<?php
namespace App;

class Base {}

//- /src/Repo.php
<?php
namespace App;

class Repo extends Ba@parent@se {}
"#,
        "src/Repo.php",
    )
    .await;

    let before = server.definition("src/Repo.php", "parent").await;
    assert_eq!(
        definition_target(&before, server.project.root()).as_deref(),
        Some("src/Base.php")
    );

    server.delete_and_notify("src/Base.php").await;

    let after = server.definition("src/Repo.php", "parent").await;
    assert!(
        definition_target(&after, server.project.root()).is_none(),
        "a deleted declaration must stop resolving, not point at a file that is gone"
    );
}

#[tokio::test]
async fn a_disk_change_under_an_open_buffer_is_ignored() {
    // The editor owns an open buffer's contents. Applying a disk change to it would replace what is
    // on screen with what is on disk -- and if the editor does reload, it says so with `didChange`.
    let mut server = Fixture::started(
        r#"
//- /composer.json
{}

//- /src/Base.php
<?php
namespace App;

class Base {}

//- /src/Repo.php
<?php
namespace App;

class Repo extends Ba@parent@se {}
"#,
        "src/Repo.php",
    )
    .await;
    server.open("src/Base.php").await;

    // The buffer says `Base`; disk now says something else.
    std::fs::write(
        server.project.path("src/Base.php"),
        "<?php\nnamespace App;\n\nclass Renamed {}\n",
    )
    .expect("the fixture file is writable");
    server.notify_changed("src/Base.php").await;

    let response = server.definition("src/Repo.php", "parent").await;
    expect!["src/Base.php"].assert_eq(
        &definition_target(&response, server.project.root())
            .expect("the open buffer still declares Base"),
    );
}

#[tokio::test]
async fn a_composer_change_rediscovers_where_the_code_is() {
    // The case that is not about one file's contents: composer's configuration decides where all the
    // code is. Here a gitignored directory holds a class nothing can find -- exactly Drupal's core
    // layout in miniature -- until the generated autoload map registers it.
    let mut server = Fixture::started(
        r#"
//- /composer.json
{}

//- /.gitignore
/generated

//- /generated/Base.php
<?php
namespace App;

class Base {}

//- /src/Repo.php
<?php
namespace App;

class Repo extends Ba@parent@se {}
"#,
        "src/Repo.php",
    )
    .await;

    let before = server.definition("src/Repo.php", "parent").await;
    assert!(
        definition_target(&before, server.project.root()).is_none(),
        "a gitignored directory nothing registered is not the developer's code"
    );

    // A changed autoload map means the whole project is indexed again, and that pass runs on its own
    // task -- off the notification handler for the same reason startup is. So the answer this test
    // is about exists one pass later, not one notification later.
    let passes = server.watch_passes();
    server
        .write_and_notify(
            r#"
//- /vendor/composer/autoload_psr4.php
<?php
$vendorDir = dirname(__DIR__);
$baseDir = dirname($vendorDir);

return array(
    'App\\' => array($baseDir . '/generated'),
);
"#,
        )
        .await;
    server.pass_ends(passes).await;

    let after = server.definition("src/Repo.php", "parent").await;
    expect!["generated/Base.php"].assert_eq(
        &definition_target(&after, server.project.root())
            .expect("a directory composer registers is discovered"),
    );
}

#[tokio::test]
async fn the_client_is_asked_to_watch_what_the_project_indexes() {
    let mut server = Fixture::new(AWAITING_A_PARENT);
    server.initialize(false, true).await;
    server.initialized().await;

    let globs = server.watcher_globs();
    assert!(globs.contains(&"**/*.php".to_owned()), "{globs:?}");
    // From the directory they live in, because `vendor/` is usually gitignored and a client is
    // entitled to answer a `**` glob from the tree it keeps, which has nothing under an ignored
    // directory. Zed does exactly that.
    assert!(
        globs.contains(&"vendor/composer|autoload_*.php".to_owned()),
        "composer's generated map has to be watched: {globs:?}"
    );
    // And the directory's own entry, which is what a fresh checkout's first `composer install`
    // changes -- as a glob, because a watch rooted there would have the client load all of it.
    assert!(
        globs.contains(&"**/vendor".to_owned()),
        "the dependency directory appearing has to be heard: {globs:?}"
    );
    assert!(
        globs.contains(&"vendor/composer|installed.json".to_owned()),
        "the record of an install is the one file every run that installs anything writes: \
         {globs:?}"
    );
    // The project's own settings, without which the file is read once and never again -- and a person
    // switching a rule off sees nothing change, which is indistinguishable from the rule being
    // broken. Root-anchored, unlike composer's: a nested `composer.json` really does decide where
    // code lives, and a nested `phrost.toml` is read by nothing.
    assert!(
        globs.contains(&"phrost.toml".to_owned()),
        "the project's own settings have to be watched: {globs:?}"
    );
    assert!(
        !globs.contains(&"**/phrost.toml".to_owned()),
        "watched everywhere rather than at the root: {globs:?}"
    );
}

/// A client that cannot root a watch is sent the glob every client was sent before.
///
/// Not as good -- a client that answers it from the tree it keeps hears nothing from an ignored
/// `vendor/` -- but the specification gives the server nothing better to say to it.
#[tokio::test]
async fn a_client_that_cannot_root_a_watch_is_sent_a_glob() {
    let mut server = Fixture::new(AWAITING_A_PARENT);
    let root = server.project.root().to_path_buf();
    server
        .request(
            "initialize",
            json!({
                "capabilities": {
                    "workspace": { "didChangeWatchedFiles": { "dynamicRegistration": true } },
                },
                "workspaceFolders": [{
                    "uri": format!("file://{}", root.display()),
                    "name": "fixture",
                }],
            }),
        )
        .await;
    server.initialized().await;

    let composer = server
        .watcher_globs()
        .into_iter()
        .filter(|glob| glob.contains("composer/"))
        .collect::<Vec<_>>();
    expect![[r#"
        [
            "**/composer/installed.json",
            "**/composer/autoload_*.php",
        ]
    "#]]
    .assert_debug_eq(&composer);
}

#[tokio::test]
async fn a_client_that_cannot_watch_files_is_told_so() {
    // Not a failure, but not silent either: the index will drift after a branch switch and the user
    // deserves to know why rather than concluding the server is broken.
    let mut server = Fixture::new(AWAITING_A_PARENT);
    server.initialize(false, false).await;
    server.initialized().await;

    assert!(server.watcher_globs().is_empty());
    assert!(
        server
            .logged_warning()
            .is_some_and(|message| message.contains("cannot watch files")),
        "got {:?}",
        server.logged_warning()
    );
}

#[tokio::test]
async fn a_watched_change_to_something_that_is_not_php_is_ignored() {
    let mut server = Fixture::started(AWAITING_A_PARENT, "src/Repo.php").await;

    server
        .write_and_notify(
            r#"
//- /README.md
Not code.
"#,
        )
        .await;

    // Still nothing to go to, and no error along the way.
    let response = server.definition("src/Repo.php", "parent").await;
    assert!(error_message(&response).is_none());
    assert!(definition_target(&response, server.project.root()).is_none());
}

/// A file created while the index is still being built is not dropped by the end of the pass.
///
/// Bootstrap's tail is `forget_undiscovered(&discovered)`, and `discovered` is a snapshot taken
/// *before* the batches ran -- so a file that appears after it and is ingested by the watcher is not
/// in the list, and `retain_files` drops it. The window is the length of a bootstrap, which on a
/// real project is seconds, and what fits in it is exactly what makes files appear: a `git switch`,
/// a `composer install`, a code generator.
///
/// The symptom is the one §8 records for open buffers, arriving from the other side: a class that is
/// there on disk reads as undefined, corrected by nothing until somebody edits something. It cannot
/// be reproduced by timing a real pass, so the ordering is reproduced instead -- take the snapshot,
/// let the watcher ingest, then run the tail against the stale snapshot.
///
/// Open buffers already have this exemption, one line above the one this needed. A file the watcher
/// brought in is the same kind of fact: something the index holds that discovery's list predates.
#[tokio::test]
async fn a_file_created_during_a_pass_survives_the_end_of_it() {
    let mut server = Fixture::started(AWAITING_A_PARENT, "src/Repo.php").await;

    // The snapshot bootstrap's tail would compare against, taken before the file exists.
    let stale = server.backend().workspace.read().await.discover().php;

    server
        .write_and_notify(
            r#"
//- /src/Base.php
<?php
namespace App;

class Base {}
"#,
        )
        .await;
    let after_creation = server.definition("src/Repo.php", "parent").await;
    assert!(
        definition_target(&after_creation, server.project.root()).is_some(),
        "the watcher's own work is what this is about, so it has to have worked first"
    );

    // The tail of the pass, against the list from before.
    server
        .backend()
        .workspace_write()
        .await
        .forget_undiscovered(&stale);

    let after_the_tail = server.definition("src/Repo.php", "parent").await;
    expect!["src/Base.php"]
        .assert_eq(&definition_target(&after_the_tail, server.project.root()).unwrap_or_default());
}

/// A buffer's unsaved edits survive the reindex a `composer.json` change triggers.
///
/// The startup analogue is
/// `flow::startup::a_buffer_opened_while_indexing_survives_the_index_being_rebuilt_under_it`; this
/// is the same claim about the other pass, and the other pass is the one that runs while somebody
/// is already working. `Pass::Reconfigured` calls `reset_index`, which empties the skeleton, and
/// then a bootstrap whose batch loop re-reads every file **from disk** -- so a declaration that
/// exists only in the editor is gone twice over unless `reingest_open_documents` puts it back. §8
/// records that being wrong once already, from the startup side.
///
/// `Extra` is deliberately a class the disk does not have, so the answer separates "the buffer was
/// put back" from "the file happens to be indexed".
#[tokio::test]
async fn unsaved_edits_survive_a_reconfiguration() {
    let mut server = Fixture::started(
        r#"
//- /composer.json
{"autoload": {"psr-4": {"App\\": "src"}}}

//- /src/Repo.php
<?php
namespace App;

class Repo
{
    public function run(Ex@type@tra $extra): void {}
}
"#,
        "src/Repo.php",
    )
    .await;

    // The declaration exists only in the editor, which is the ordinary state of a file being
    // worked in.
    let unsaved = format!(
        "{}\nclass Extra {{}}\n",
        server.text("src/Repo.php").trim_end()
    );
    server.change("src/Repo.php", 2, &unsaved).await;
    let before = server.definition("src/Repo.php", "type").await;
    assert!(
        definition_target(&before, server.project.root()).is_some(),
        "the unsaved declaration has to resolve first, or this proves nothing"
    );

    // A composer change, which resets the index and rebuilds it from disk.
    let passes = server.watch_passes();
    server
        .write_and_notify(
            r#"
//- /composer.json
{"autoload": {"psr-4": {"App\\": "src", "Other\\": "other"}}}
"#,
        )
        .await;
    server.pass_ends(passes).await;

    let after = server.definition("src/Repo.php", "type").await;
    expect!["src/Repo.php"]
        .assert_eq(&definition_target(&after, server.project.root()).unwrap_or_default());
}

/// A file deleted while the index is still being built does not come back at the end of it.
///
/// The mirror of `a_file_created_during_a_pass_survives_the_end_of_it`, and it needs no exemption
/// list: the batch loop re-reads each discovered path *from disk*, so a path that is gone by the
/// time its batch runs has nothing to be read from. What is under test is that the loop does not
/// resurrect the row from the snapshot -- a `git switch` that removes a file mid-start would
/// otherwise leave its class resolving to a path that is not there, which is the exact failure
/// `forget_undiscovered` exists to prevent on the *other* side.
///
/// The ordering is reproduced rather than timed, the same way its mirror is.
///
/// What it pins precisely: the *removal* reaching the skeleton, and the batch loop not putting the
/// row back. The second half holds because the loop reads from disk rather than trusting a stored
/// row, and that is worth a guard rather than a comment -- a fingerprinted row is trusted on its
/// content hash, so a future reuse path that skipped the `stat` would resurrect a deleted file and
/// nothing else here would notice.
#[tokio::test]
async fn a_file_deleted_during_a_pass_does_not_come_back() {
    let mut server = Fixture::started(
        r#"
//- /composer.json
{"autoload": {"psr-4": {"App\\": "src"}}}

//- /src/Base.php
<?php
namespace App;

class Base {}

//- /src/Repo.php
<?php
namespace App;

class Repo extends Ba@parent@se {}
"#,
        "src/Repo.php",
    )
    .await;

    let before = server.definition("src/Repo.php", "parent").await;
    assert!(
        definition_target(&before, server.project.root()).is_some(),
        "the parent has to resolve first, or this proves nothing"
    );

    // The snapshot a pass would compare against, taken while the file still exists.
    let stale = server.backend().workspace.read().await.discover().php;

    server.delete_and_notify("src/Base.php").await;

    // The tail of the pass, against the list from before -- batch loop included.
    {
        let mut workspace = server.backend().workspace_write().await;
        workspace.ingest_batch(&stale, true);
        workspace.forget_undiscovered(&stale);
        workspace.reingest_open_documents();
    }

    let after = server.definition("src/Repo.php", "parent").await;
    expect![""].assert_eq(&definition_target(&after, server.project.root()).unwrap_or_default());
}
