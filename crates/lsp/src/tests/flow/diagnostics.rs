//! Diagnostics: published on open, republished from disk on close, and never from a half-built
//! index.
//!
//! A client keeps showing the last set it was sent, so closing a buffer has to say something out
//! loud -- what the file on disk holds now, since the whole project is reported -- and opening one
//! while bootstrap is still running has to say nothing at all until the catalog it would be checked
//! against is complete.

use expect_test::expect;

use crate::tests::Fixture;

#[tokio::test]
async fn diagnostics_are_published_on_open_and_from_disk_on_close() {
    // A client keeps showing the last set it was sent, so closing a buffer has to say something out
    // loud. With the whole project reported that is what the disk holds: the problem is still in
    // the file, so it stays in the panel after the tab closes, as it would had the tab never opened.
    let mut fixture = Fixture::new(
        r#"
//- /composer.json
{"autoload": {"psr-4": {"App\\": "src"}}}

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
"#,
    );
    fixture.initialize(false, false).await;
    fixture.initialized().await;

    fixture.open("src/Service.php").await;
    fixture.close("src/Service.php").await;

    expect![[r#"
        [
            "Service.php:9: undefined-method Undefined method App\\Repo::fnid()",
            "Service.php:9: undefined-method Undefined method App\\Repo::fnid()",
            "Service.php:9: undefined-method Undefined method App\\Repo::fnid()",
        ]
    "#]]
    .assert_debug_eq(&fixture.diagnostics_published());
}

#[tokio::test]
async fn a_file_opened_while_indexing_is_checked_once_the_project_is_ready() {
    // The failure this prevents was visible: a file opened during bootstrap reported `\Drupal` as an
    // undefined type, because half an index had not reached it yet -- and nothing ever went back.
    let mut server = Fixture::new(
        r#"
//- /composer.json
{"autoload": {"psr-4": {"App\\": "src"}}}

//- /src/Repo.php
<?php
namespace App;

class Repo {}

//- /src/Service.php
<?php
namespace App;

class Service
{
    public function run(): void
    {
        new Repo();
    }
}
"#,
    );
    server.initialize(false, false).await;

    // Before `initialized`, so bootstrap has not run and the catalog is empty.
    server.open("src/Service.php").await;
    let while_indexing = server.diagnostics_published();
    server.initialized().await;

    assert!(
        while_indexing.is_empty(),
        "nothing may be published from a half-filled index, got {while_indexing:?}"
    );
    expect![[r#"
        [
            "Service.php: none",
        ]
    "#]]
    .assert_debug_eq(&server.diagnostics_published());
}

/// One file with a mistake in it, plus the class the mistake is about.
const BROKEN: &str = r#"
//- /composer.json
{"autoload": {"psr-4": {"App\\": "src"}}}

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
        $repo->fnid(1);
    }
}
"#;

/// The buffer as it is being typed: `fni` on the way to `find`.
const MID_EDIT: &str = r#"<?php
namespace App;

class Service
{
    public function run(Repo $repo): void
    {
        $repo->fni(1);
    }
}
"#;

/// The same buffer a moment later, correct.
const FINISHED: &str = r#"<?php
namespace App;

class Service
{
    public function run(Repo $repo): void
    {
        $repo->find(1);
    }
}
"#;

#[tokio::test(start_paused = true)]
async fn a_change_publishes_nothing_until_the_buffer_has_been_still() {
    // Publishing on the keystroke means an error appears and vanishes while a name is half-typed,
    // and a squiggle that flickers is worse than one that arrives a moment late.
    let mut fixture = Fixture::started(BROKEN, "src/Service.php").await;

    fixture.change("src/Service.php", 2, MID_EDIT).await;
    expect![[r#"
        [
            "Service.php:8: undefined-method Undefined method App\\Repo::fnid()",
            "Service.php:8: undefined-method Undefined method App\\Repo::fnid()",
        ]
    "#]]
    .assert_debug_eq(&fixture.diagnostics_published());

    fixture.wait_out_the_quiet().await;
    expect![[r#"
        [
            "Service.php:8: undefined-method Undefined method App\\Repo::fnid()",
            "Service.php:8: undefined-method Undefined method App\\Repo::fnid()",
            "Service.php:8: undefined-method Undefined method App\\Repo::fni()",
        ]
    "#]]
    .assert_debug_eq(&fixture.diagnostics_published());
}

#[tokio::test(start_paused = true)]
async fn a_second_change_inside_the_quiet_period_replaces_the_first() {
    // The half-typed name is never published. Nothing is cancelled to arrange that: the waiting task
    // wakes up, sees a version that moved, and goes away.
    let mut fixture = Fixture::started(BROKEN, "src/Service.php").await;

    fixture.change("src/Service.php", 2, MID_EDIT).await;
    fixture.change("src/Service.php", 3, FINISHED).await;
    fixture.wait_out_the_quiet().await;

    expect![[r#"
        [
            "Service.php:8: undefined-method Undefined method App\\Repo::fnid()",
            "Service.php:8: undefined-method Undefined method App\\Repo::fnid()",
            "Service.php: none",
        ]
    "#]]
    .assert_debug_eq(&fixture.diagnostics_published());
}

#[tokio::test]
async fn a_dependency_opened_to_be_read_is_not_checked() {
    // The same rule the project pass and a closed file follow. Nothing reported about a vendor
    // file's docblocks or a stub's `__get` is something the person reading it can act on, and
    // `check` never counts them -- so an open vendor buffer publishes an empty list, while the
    // project's own file beside it is checked as before.
    let mut fixture = Fixture::new(
        r#"
//- /composer.json
{"autoload": {"psr-4": {"App\\": "src"}}}
//- /vendor/composer/autoload_psr4.php
<?php
$vendorDir = dirname(__DIR__);
$baseDir = dirname($vendorDir);
return array(
    'Pkg\\' => array($vendorDir . '/pkg/src'),
    'App\\' => array($baseDir . '/src'),
);
//- /vendor/pkg/src/Thing.php
<?php
namespace Pkg;
class Thing
{
    public function run(): void
    {
        $this->missing();
    }
}
//- /src/Service.php
<?php
namespace App;
class Service
{
    public function run(\Pkg\Thing $thing): void
    {
        $thing->missing();
    }
}
"#,
    );
    fixture.initialize(false, false).await;
    fixture.initialized().await;
    fixture.open("vendor/pkg/src/Thing.php").await;
    fixture.open("src/Service.php").await;
    expect![[r#"
        [
            "Service.php:7: undefined-method Undefined method Pkg\\Thing::missing()",
            "Thing.php: none",
            "Service.php:7: undefined-method Undefined method Pkg\\Thing::missing()",
        ]
    "#]]
    .assert_debug_eq(&fixture.diagnostics_published());
}

/// Closing a buffer mid-typing does not leave the client holding what the buffer said.
///
/// The race: an edit schedules a publish for 300 ms later, and the tab closes before it fires.
/// Closing publishes what the *disk* holds -- see the test above for why it says anything at all --
/// so a debounced task landing after that would leave the client with findings describing unsaved
/// text that exists nowhere, for a file it no longer has open, until something else touched it.
///
/// Nothing cancels the waiting task, by design. What has to hold is the version it wakes up to: a
/// closed document has no version, so `held != Some(version)` and the task goes away.
///
/// **`fni()` against `fnid()` is the whole test.** The buffer's half-typed name produces a
/// different message from the one on disk, so a leaked publish is visible as a *different finding*
/// rather than as one more copy of the same. Written with matching texts this test passes while
/// proving nothing, which is what the first draft of it did.
///
/// Three publishes and none of them the buffer's: the open, the project-wide pass reporting the
/// same file from disk, and the close. All three say `fnid`, which is what the disk says.
#[tokio::test(start_paused = true)]
async fn closing_a_buffer_mid_typing_leaves_the_disk_findings_last() {
    let mut fixture = Fixture::started(BROKEN, "src/Service.php").await;

    fixture.change("src/Service.php", 2, MID_EDIT).await;
    fixture.close("src/Service.php").await;
    fixture.wait_out_the_quiet().await;

    let published = fixture.diagnostics_published();
    assert!(
        !published.iter().any(|set| set.contains("fni()")),
        "the buffer's half-typed name reached the client after the tab closed: {published:#?}"
    );
    expect![[r#"
        [
            "Service.php:8: undefined-method Undefined method App\\Repo::fnid()",
            "Service.php:8: undefined-method Undefined method App\\Repo::fnid()",
            "Service.php:8: undefined-method Undefined method App\\Repo::fnid()",
        ]
    "#]]
    .assert_debug_eq(&published);
}
