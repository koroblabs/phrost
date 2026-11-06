//! A `composer update`, as an editor reports it.
//!
//! Composer writes `composer.lock` *before* it touches a package, then replaces the packages, then
//! records the install in `vendor/composer/installed.json`. Measured on composer 2.10 upgrading one
//! package: the lock at 8.0 s, the package's files at 10.6 s, the record 11 ms after them -- and the
//! autoload map not at all, because composer only rewrites it when its content changes, which a
//! version bump does not. An editor that does not walk an ignored `vendor/` (Zed keeps no entries
//! for one) reports the lock and nothing after it. So a server that reindexed on the lock read a
//! vendor tree half replaced, and was never told to look again: every class of a package that was
//! mid-replacement read as undefined until a restart.

use std::time::{Duration, SystemTime};

use expect_test::expect;

use super::{Fixture, definition_target};

/// A project with one dependency, installed and recorded.
const ONE_DEPENDENCY: &str = r#"
//- /composer.json
{"require": {"acme/lib": "^1.0"}}

//- /composer.lock
{"packages": [{"name": "acme/lib", "version": "1.0.0"}]}

//- /vendor/composer/installed.json
{"packages": [{"name": "acme/lib", "version": "1.0.0"}]}

//- /vendor/acme/lib/src/Base.php
<?php
namespace Lib;

class Base {}

//- /src/Repo.php
<?php
namespace App;

class Repo extends \Lib\Ba@parent@se {}
"#;

/// Sets a file's modification time to `seconds` past a fixed moment, so which of two files is the
/// newer is a fact of the fixture rather than of how fast the test ran.
fn stamp(server: &Fixture, relative: &str, seconds: u64) {
    let moment = SystemTime::UNIX_EPOCH + Duration::from_secs(1_800_000_000 + seconds);
    std::fs::File::options()
        .write(true)
        .open(server.project.path(relative))
        .and_then(|file| file.set_modified(moment))
        .expect("the fixture file can be stamped");
}

async fn parent_of_repo(server: &mut Fixture) -> Option<String> {
    let answer = server.definition("src/Repo.php", "parent").await;
    definition_target(&answer, server.project.root())
}

/// The run as composer does it, against an editor that reports only the lock.
///
/// The old file goes before the lock is reported rather than after it, which is the order a server
/// sees whenever its pass is a moment behind the notification -- and the order that makes a
/// reindex on the lock observable rather than a race the test might win.
#[tokio::test]
async fn a_lock_is_not_read_until_the_install_it_announces_is_recorded() {
    let mut server = Fixture::started(ONE_DEPENDENCY, "src/Repo.php").await;
    stamp(&server, "composer.lock", 0);
    stamp(&server, "vendor/composer/installed.json", 10);
    assert_eq!(
        parent_of_repo(&mut server).await.as_deref(),
        Some("vendor/acme/lib/src/Base.php"),
        "the installed package has to resolve first, or this proves nothing"
    );

    let passes = server.watch_passes();
    let said = server.logged_at(3).len();
    server.project.remove("vendor/acme/lib/src/Base.php");
    server.project.write_more(
        r#"
//- /composer.lock
{"packages": [{"name": "acme/lib", "version": "2.0.0"}]}
"#,
    );
    stamp(&server, "composer.lock", 20);
    server.notify_changed("composer.lock").await;

    // Mid-install: the index that was right before the run is a better answer than a tree half
    // replaced, so nothing is read. Asked of what the server said rather than of an answer, because
    // the old file is gone and no answer can point into it -- and rather than of whether it is
    // indexing, because a pass over a fixture this size is over before anything can look.
    server.settle().await;
    expect![[r#"
        [
            "composer is installing; the project is read again once it has finished",
        ]
    "#]]
    .assert_debug_eq(&&server.logged_at(3)[said..]);

    // The new version, in a layout of its own, and the record -- neither of them reported.
    server.project.write_more(
        r#"
//- /vendor/acme/lib/lib/Base.php
<?php
namespace Lib;

class Base {}

//- /vendor/composer/installed.json
{"packages": [{"name": "acme/lib", "version": "2.0.0"}]}
"#,
    );
    stamp(&server, "vendor/composer/installed.json", 30);
    server.pass_ends(passes).await;

    assert_eq!(
        parent_of_repo(&mut server).await.as_deref(),
        Some("vendor/acme/lib/lib/Base.php"),
    );
}

/// `composer install` after a pull that brought a new lock: the lock is not written, so the record
/// is the only file that says anything changed.
#[tokio::test]
async fn an_install_record_the_editor_reports_reads_the_project_again() {
    let mut server = Fixture::started(ONE_DEPENDENCY, "src/Repo.php").await;
    stamp(&server, "composer.lock", 0);
    stamp(&server, "vendor/composer/installed.json", 10);

    let passes = server.watch_passes();
    server.project.remove("vendor/acme/lib/src/Base.php");
    server
        .write_and_notify(
            r#"
//- /vendor/acme/lib/lib/Base.php
<?php
namespace Lib;

class Base {}
"#,
        )
        .await;
    server.project.write_more(
        r#"
//- /vendor/composer/installed.json
{"packages": [{"name": "acme/lib", "version": "2.0.0"}]}
"#,
    );
    stamp(&server, "vendor/composer/installed.json", 20);
    server
        .notify_changed("vendor/composer/installed.json")
        .await;
    server.pass_ends(passes).await;

    assert_eq!(
        parent_of_repo(&mut server).await.as_deref(),
        Some("vendor/acme/lib/lib/Base.php"),
    );
}

/// A checkout opened before `composer install` ever ran: no lock changes, and the directory the
/// install record would be written in does not exist yet, so nothing the client was asked to watch
/// under it can be reported. What an editor does report is the dependency directory appearing --
/// it is an entry of the root, which every client keeps -- and that is a composer run like any
/// other. The case of a person joining a project, and of `rm -rf vendor && composer install`.
#[tokio::test]
async fn a_dependency_directory_that_appears_is_a_composer_run() {
    let mut server = Fixture::started(
        r#"
//- /composer.json
{"require": {"acme/lib": "^1.0"}}

//- /composer.lock
{"packages": [{"name": "acme/lib", "version": "1.0.0"}]}

//- /src/Repo.php
<?php
namespace App;

class Repo extends \Lib\Ba@parent@se {}
"#,
        "src/Repo.php",
    )
    .await;
    stamp(&server, "composer.lock", 0);
    assert_eq!(
        parent_of_repo(&mut server).await,
        None,
        "nothing installed yet"
    );

    let passes = server.watch_passes();
    server.project.write_more(
        r#"
//- /vendor/acme/lib/src/Base.php
<?php
namespace Lib;

class Base {}

//- /vendor/composer/installed.json
{"packages": [{"name": "acme/lib", "version": "1.0.0"}]}
"#,
    );
    stamp(&server, "vendor/composer/installed.json", 10);
    server.notify_created("vendor").await;
    server.pass_ends(passes).await;

    assert_eq!(
        parent_of_repo(&mut server).await.as_deref(),
        Some("vendor/acme/lib/src/Base.php"),
    );
}

/// The directory going away is the start of a run as surely as a lock is: the packages are gone
/// and the install that brings them back has not happened. So it is waited out, and the install
/// record appearing ends the wait whether or not anything reports it.
#[tokio::test]
async fn a_dependency_directory_removed_and_reinstalled_is_read_once_the_install_is_recorded() {
    let mut server = Fixture::started(ONE_DEPENDENCY, "src/Repo.php").await;
    stamp(&server, "composer.lock", 0);
    stamp(&server, "vendor/composer/installed.json", 10);

    let passes = server.watch_passes();
    std::fs::remove_dir_all(server.project.path("vendor")).expect("the fixture's vendor goes");
    server.notify_deleted("vendor").await;

    server.project.write_more(
        r#"
//- /vendor/acme/lib/lib/Base.php
<?php
namespace Lib;

class Base {}

//- /vendor/composer/installed.json
{"packages": [{"name": "acme/lib", "version": "2.0.0"}]}
"#,
    );
    stamp(&server, "vendor/composer/installed.json", 20);
    server.pass_ends(passes).await;

    assert_eq!(
        parent_of_repo(&mut server).await.as_deref(),
        Some("vendor/acme/lib/lib/Base.php"),
    );
}
