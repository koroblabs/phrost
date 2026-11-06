//! Editing the file a framework reads is a reason to read the project again.
//!
//! What a provider contributes -- a service id, a route, a module's namespace, a field's kind -- is
//! read once, by `set_root`, out of the project's own `.yml`. So adding a service to a
//! `services.yml` changed nothing until the server was restarted, and a person who had just written
//! the id read that as the server not knowing about their own code.
//!
//! Two halves, and the first is the one that was missing entirely: the client is never told about a
//! file it was not asked to watch, so the globs are as much the fix as the rule is.

use crate::tests::Fixture;

/// A Drupal docroot, which is what makes a provider say it reads configuration at all.
///
/// The marker file is the whole of the probe -- see the provider's `layout` module -- so a class
/// with the right path is enough, and nothing here depends on what is in it.
const A_DRUPAL_PROJECT: &str = r#"
//- /composer.json
{}

//- /docroot/core/lib/Drupal.php
<?php
class Drupal {}

//- /docroot/modules/custom/mine/mine.info.yml
name: Mine
type: module

//- /docroot/modules/custom/mine/mine.services.yml
services:
  mine.thing:
    class: Drupal\mine\Thing

//- /docroot/modules/custom/mine/src/Thing.php
<?php
namespace Drupal\mine;

class Thing {}
"#;

/// The same without the marker, so nothing detects a framework.
const A_PLAIN_PROJECT: &str = r#"
//- /composer.json
{}

//- /src/Repo.php
<?php
namespace App;

class Repo {}
"#;

#[tokio::test]
async fn a_project_whose_framework_reads_yaml_asks_the_client_to_watch_it() {
    let mut server = Fixture::new(A_DRUPAL_PROJECT);
    server.initialize(false, true).await;
    server.initialized().await;

    let globs = server.watcher_globs();
    assert!(
        globs.contains(&"**/*.yml".to_owned()),
        "a framework that reads its answers out of YAML has to hear about a YAML change: {globs:?}"
    );
}

#[tokio::test]
async fn a_project_with_no_framework_watches_no_yaml() {
    // The gate, and the reason it is the framework's own answer rather than a constant: a project
    // that declares nothing in YAML must not have its client report every YAML it happens to
    // contain -- a CI workflow, a docker-compose -- each of which would cost it a re-read.
    let mut server = Fixture::new(A_PLAIN_PROJECT);
    server.initialize(false, true).await;
    server.initialized().await;

    let globs = server.watcher_globs();
    assert!(
        !globs.iter().any(|glob| glob.ends_with(".yml")),
        "nothing said it reads YAML: {globs:?}"
    );
}

#[tokio::test]
async fn changing_one_reindexes_the_project() {
    let mut server = Fixture::new(A_DRUPAL_PROJECT);
    server.initialize(false, true).await;
    server.initialized().await;

    // An indexing pass and not merely a file's re-ingest: what the providers said is derived from
    // every one of these files at once, so there is no such thing as re-reading one of them.
    let passes = server.watch_passes();
    server
        .notify_changed("docroot/modules/custom/mine/mine.services.yml")
        .await;
    server.pass_ends(passes).await;
}
