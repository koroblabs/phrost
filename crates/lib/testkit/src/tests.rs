//! Tests for what phrost adds to the fixture format: what its flags mean, the relative paths every
//! layer above names files by, materialising a fixture on disk, and the snapshot shape. The format
//! itself -- markers, escapes, indentation, the asked-about check -- is pinned in `lspkit-fixture`.

use expect_test::expect;
use phrost_model::{LineIndex, PhpVersion, SourceOrigin, TextRange, TextSize};

use crate::{FixtureSpec, Snapshot, render_range};

const REPO_FIXTURE: &str = r#"
//- /composer.json
{"autoload": {"psr-4": {"App\\": "src/"}}}

//- /src/Repo.php
<?php
namespace App;
class Repo {
    public function find(int $id): ?User {}
}

//- /src/Use.php  open  php:8.5
<?php
namespace App;
function go(Repo $r) {
    $u = $r->fi@member@nd(1);
}

//- /vendor/psr/log/LoggerInterface.php  vendor
<?php
namespace Psr\Log;
interface LoggerInterface {}
"#;

#[test]
fn reads_paths_flags_and_text() {
    let spec = FixtureSpec::parse(REPO_FIXTURE);

    let mut snapshot = Snapshot::new();
    snapshot.section("files");
    for file in spec.files() {
        snapshot.row(format!(
            "{} origin={:?} open={} php={}",
            file.path(),
            file.origin(),
            file.is_open(),
            file.php_version()
                .map_or_else(|| "-".to_owned(), |version| version.to_string()),
        ));
    }

    expect![[r#"
        files
        - composer.json origin=User open=false php=-
        - src/Repo.php origin=User open=false php=-
        - src/Use.php origin=User open=true php=8.5
        - vendor/psr/log/LoggerInterface.php origin=Vendor open=false php=-
    "#]]
    .assert_eq(&snapshot.finish());
}

#[test]
fn strips_the_marker_and_points_at_the_right_offset() {
    let spec = FixtureSpec::parse(REPO_FIXTURE);
    let file = spec.file("src/Use.php");

    // The marker is gone and the method name is whole again.
    assert!(file.text().contains("$r->find(1);"));

    let marker = spec.markers().position("member");
    assert_eq!(marker.path, "src/Use.php");

    // It sits between `fi` and `nd`, which is what makes it a useful member-resolution cursor.
    let expected = TextSize::from(
        u32::try_from(
            file.text()
                .find("nd(1)")
                .expect("method call survives stripping"),
        )
        .expect("offset fits"),
    );
    assert_eq!(marker.offset, expected);

    let index = LineIndex::new(file.text());
    assert_eq!(
        render_range(&index, TextRange::empty(marker.offset)),
        "4:16-4:16"
    );

    spec.markers().assert_all_used();
}

#[test]
#[should_panic(expected = "unknown fixture header flag `vendr`")]
fn a_misspelled_flag_is_rejected_rather_than_ignored() {
    FixtureSpec::parse(
        r#"
//- /a.php  vendr
<?php
"#,
    );
}

#[test]
fn per_file_php_version_falls_back_to_the_project_default() {
    let spec = FixtureSpec::parse(
        r#"
//- /old.php  php:8.1
<?php
//- /new.php
<?php
"#,
    );

    assert_eq!(
        spec.file("old.php").php_version(),
        Some(PhpVersion::PHP_8_1)
    );
    assert_eq!(spec.file("new.php").php_version(), None);
}

#[test]
fn stub_origin_is_distinct_from_vendor() {
    let spec = FixtureSpec::parse(
        r#"
//- /stubs/Core.php  stub
<?php
function strlen(string $s): int {}
"#,
    );

    let file = spec.file("stubs/Core.php");
    assert_eq!(file.origin(), SourceOrigin::Stub);
    assert!(!file.origin().wants_full_analysis());
}

#[test]
fn materialized_fixture_round_trips_through_the_filesystem() {
    let fixture = crate::MaterializedFixture::parse(REPO_FIXTURE);

    assert_eq!(
        std::fs::read_to_string(fixture.path("src/Repo.php")).expect("file written"),
        fixture.spec().file("src/Repo.php").text()
    );

    fixture.write_more(
        r#"
//- /src/User.php
<?php
namespace App;
class User {}
"#,
    );
    assert!(fixture.path("src/User.php").exists());

    fixture.remove("src/User.php");
    assert!(!fixture.path("src/User.php").exists());
}

#[test]
fn snapshot_shape_is_stable() {
    let mut snapshot = Snapshot::new();
    snapshot
        .section("member goto")
        .row("method App\\Repo::find @ src/Repo.php 4:21-4:25")
        .section("prop completion")
        .rows(["name : string", "getName() : string"])
        .truncated(12)
        .section("receiver type")
        .none();

    expect![[r#"
        member goto
        - method App\Repo::find @ src/Repo.php 4:21-4:25

        prop completion
        - name : string
        - getName() : string
        - ... +12 more

        receiver type
        - (none)
    "#]]
    .assert_eq(&snapshot.finish());
}
