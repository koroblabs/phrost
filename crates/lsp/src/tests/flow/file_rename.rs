//! Moving a file, which under PSR-4 is a rename of the class it holds.
//!
//! Two halves that look like one feature and take opposite edits. A new *basename* renames the
//! class, so every mention of it changes -- and a qualified mention changes in the same way it
//! does for an ordinary rename, at its last segment. A new *directory* changes the namespace, so
//! the file's own `namespace` line moves and every mention that names the class outright has to
//! name the new place -- while a bare mention must not, because it leans on an import that this
//! same edit is fixing.

use expect_test::expect;
use serde_json::json;

use crate::tests::Fixture;
use crate::tests::flow::render_edits;

/// One class, reached three ways: through an import, fully qualified, and bare.
const MOVEABLE: &str = r#"
//- /composer.json
{"autoload": {"psr-4": {"App\\": "src/"}}}
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    public function find(): int { return 1; }
}
//- /src/Service.php
<?php
namespace App;

use App\Repo;

class Service
{
    public function run(Repo $repo): int
    {
        return $repo->find() + \App\Repo::COUNT;
    }
}
"#;

#[tokio::test]
async fn a_new_basename_renames_the_class_everywhere_it_is_written() {
    let mut server = Fixture::new(MOVEABLE);
    server.initialize(false, false).await;
    server.initialized().await;

    let edit = server
        .request(
            "workspace/willRenameFiles",
            json!({ "files": [{
                "oldUri": server.uri("src/Repo.php"),
                "newUri": server.uri("src/Account.php"),
            }]}),
        )
        .await;

    // The declaration, the import's last segment, the type hint, and the last segment of the
    // qualified path -- the same edits `textDocument/rename` makes, from a different question.
    expect![[r#"
        [
            "Repo.php 3:6-3:10 -> Account",
            "Service.php 3:8-3:12 -> Account",
            "Service.php 7:24-7:28 -> Account",
            "Service.php 9:36-9:40 -> Account",
        ]
    "#]]
    .assert_debug_eq(&render_edits(&edit));
}

#[tokio::test]
async fn a_new_directory_moves_the_namespace_and_the_names_that_spell_it_out() {
    let mut server = Fixture::new(MOVEABLE);
    server.initialize(false, false).await;
    server.initialized().await;

    let edit = server
        .request(
            "workspace/willRenameFiles",
            json!({ "files": [{
                "oldUri": server.uri("src/Repo.php"),
                "newUri": server.uri("src/Sub/Repo.php"),
            }]}),
        )
        .await;

    // The `namespace` line, the import, and the fully-qualified use. The `Repo` in the type hint
    // is deliberately absent: it resolves through the import above it, which this edit has just
    // pointed at the new place, and writing a qualified name there would resolve relative to the
    // namespace it sits in.
    expect![[r#"
        [
            "Repo.php 1:10-1:13 -> App\\Sub",
            "Service.php 3:4-3:12 -> App\\Sub\\Repo",
            "Service.php 9:31-9:40 -> \\App\\Sub\\Repo",
        ]
    "#]]
    .assert_debug_eq(&render_edits(&edit));
}

/// Two classes in one namespace using each other with no `use` line, because they never needed
/// one -- which is what a move turns into a bug.
const SHARED_NAMESPACE: &str = r#"
//- /composer.json
{"autoload": {"psr-4": {"App\\": "src/"}}}
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    public function helper(): Helper { return new Helper(); }
}
//- /src/Helper.php
<?php
namespace App;

class Helper {}
//- /src/Service.php
<?php
namespace App;

class Service
{
    public function run(Repo $repo): int { return 1; }
}
"#;

#[tokio::test]
async fn a_file_that_used_the_class_by_sharing_its_namespace_is_given_an_import() {
    let mut server = Fixture::new(SHARED_NAMESPACE);
    server.initialize(false, false).await;
    server.initialized().await;

    let edit = server
        .request(
            "workspace/willRenameFiles",
            json!({ "files": [{
                "oldUri": server.uri("src/Repo.php"),
                "newUri": server.uri("src/Sub/Repo.php"),
            }]}),
        )
        .await;

    // `Service` wrote `Repo` bare and imported it nowhere: it resolved by sharing the namespace,
    // and the move is what ends that. Without the added line this rename introduces the bug it
    // exists to prevent -- and `Repo` itself gets one for `Helper`, which is the same break seen
    // from inside the file that is moving.
    expect![[r#"
        [
            "Repo.php 1:10-1:13 -> App\\Sub",
            "Repo.php 1:14-1:14 -> \nuse App\\Helper;",
            "Service.php 1:14-1:14 -> \nuse App\\Sub\\Repo;",
        ]
    "#]]
    .assert_debug_eq(&render_edits(&edit));
}

#[tokio::test]
async fn a_move_within_the_same_namespace_gives_nobody_an_import() {
    // Renaming without moving breaks no bare name, so the pass that writes imports must not run.
    let mut server = Fixture::new(SHARED_NAMESPACE);
    server.initialize(false, false).await;
    server.initialized().await;

    let edit = server
        .request(
            "workspace/willRenameFiles",
            json!({ "files": [{
                "oldUri": server.uri("src/Repo.php"),
                "newUri": server.uri("src/Store.php"),
            }]}),
        )
        .await;

    expect![[r#"
        [
            "Repo.php 3:6-3:10 -> Store",
            "Service.php 5:24-5:28 -> Store",
        ]
    "#]]
    .assert_debug_eq(&render_edits(&edit));
}

#[tokio::test]
async fn a_move_that_changes_neither_half_of_the_name_asks_for_nothing() {
    let mut server = Fixture::new(MOVEABLE);
    server.initialize(false, false).await;
    server.initialized().await;

    // Same directory, same basename: the client is telling us about a move that is not one.
    let edit = server
        .request(
            "workspace/willRenameFiles",
            json!({ "files": [{
                "oldUri": server.uri("src/Repo.php"),
                "newUri": server.uri("src/Repo.php"),
            }]}),
        )
        .await;

    expect![[r#"
        []
    "#]]
    .assert_debug_eq(&render_edits(&edit));
}

#[tokio::test]
async fn a_file_that_declares_no_class_of_its_own_name_is_left_alone() {
    // A procedural include, a script, a file holding a differently-named class: PSR-4 never
    // reaches any of them by name, so there is nothing a path can say about what they declare.
    let mut server = Fixture::new(
        r#"
//- /composer.json
{"autoload": {"psr-4": {"App\\": "src/"}}}
//- /src/Helpers.php
<?php
namespace App;

function helper(): int { return 1; }
"#,
    );
    server.initialize(false, false).await;
    server.initialized().await;

    let edit = server
        .request(
            "workspace/willRenameFiles",
            json!({ "files": [{
                "oldUri": server.uri("src/Helpers.php"),
                "newUri": server.uri("src/Tools.php"),
            }]}),
        )
        .await;

    expect![[r#"
        []
    "#]]
    .assert_debug_eq(&render_edits(&edit));
}

/// A file that declares several classes takes all of them with it.
///
/// A PHP file may declare more than one, and a test file with its fixtures beside it is the
/// ordinary case. Every one of them moves, so a bare name reaching a *neighbour in the same file*
/// needs no import -- and offering one is not merely redundant: PHP refuses `use
/// App\Support\Helper;` in a file that declares `Helper`, with `Cannot use ... as Helper because
/// the name is already in use`, and the file stops loading. Written against the class the file is
/// named after alone, this offered exactly that on two of Laravel's test files, and `php -l` over
/// the result is what said so.
const SEVERAL_IN_ONE_FILE: &str = r#"
//- /composer.json
{"autoload": {"psr-4": {"App\\": "src/"}}}
//- /src/Support/Cases.php
<?php
namespace App\Support;

class Cases
{
    public function make(): Helper
    {
        return new Helper(new Outside());
    }
}

class Helper
{
}
//- /src/Support/Outside.php
<?php
namespace App\Support;

class Outside
{
}
"#;

#[tokio::test]
async fn a_neighbour_declared_in_the_moved_file_needs_no_import() {
    let mut server = Fixture::new(SEVERAL_IN_ONE_FILE);
    server.initialize(false, false).await;
    server.initialized().await;

    let edit = server
        .request(
            "workspace/willRenameFiles",
            json!({ "files": [{
                "oldUri": server.uri("src/Support/Cases.php"),
                "newUri": server.uri("src/Support/Deep/Cases.php"),
            }]}),
        )
        .await;

    // `Outside` stays behind and needs the import; `Helper` travels and must not get one.
    expect![[r#"
        [
            "Cases.php 1:10-1:21 -> App\\Support\\Deep",
            "Cases.php 1:22-1:22 -> \nuse App\\Support\\Outside;",
        ]
    "#]]
    .assert_debug_eq(&render_edits(&edit));
}
