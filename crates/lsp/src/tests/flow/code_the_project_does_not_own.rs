//! Diagnostics for a file somebody opened to read.
//!
//! `SourceOrigin` is the package manager's directory and nothing more elaborate, which its own
//! doc-comment insists on and is right to. What it cannot see is a framework that keeps its code
//! somewhere else: Drupal registers `Drupal\Core\` at `<docroot>/core`, which composer installs,
//! gitignores, and does not put under `vendor/` at all.
//!
//! So core counted as the project's own -- **not swept**, because discovery honours the gitignore
//! that hides it, and **fully checked the moment a file was opened**. Which is the worse half of
//! the two: eleven thousand files of somebody else's code, each one lighting up in the editor of
//! the person who opened it to read one method, with nothing they can do about any of it.
//!
//! Only this layer can say so. `dependency_paths` is a framework provider's answer and a provider
//! is installed here, so no view unit test has one to read.

use expect_test::expect;

use serde_json::json;

use crate::{
    tests::{Fixture, error_message},
    workspace::Workspace,
};

/// What was reported, as a set -- publishing is four worker threads' order, and a project swept
/// twice publishes twice. See `unused_declarations::reported`, which this is a copy of on purpose:
/// the two tests are about different rules and neither should be able to move the other's
/// snapshot.
fn reported(fixture: &Fixture) -> Vec<String> {
    let mut lines = fixture.diagnostics_published();
    lines.sort();
    lines.dedup();
    lines
}

/// A Drupal project, core beside it, and the same mistake in each.
const A_PROJECT_AND_THE_FRAMEWORK_IT_READS: &str = r#"
//- /composer.json
{"type": "project", "autoload": {"psr-4": {"App\\": "src/"}}}
//- /phrost.toml
framework = ["drupal"]
//- /docroot/core/lib/Drupal.php
<?php
class Drupal {}
//- /docroot/core/lib/Drupal/Core/Borrowed.php
<?php
namespace Drupal\Core;

class Borrowed
{
    public function run(): void
    {
        $this->nope();
    }
}
//- /docroot/modules/contrib/vendored/src/Vendored.php
<?php
namespace Drupal\vendored;

class Vendored
{
    public function run(): void
    {
        $this->nope();
    }
}
//- /docroot/modules/custom/ours/src/Ours.php
<?php
namespace Drupal\ours;

class Ours
{
    public function run(): void
    {
        $this->nope();
    }
}
"#;

/// Core and contrib are read and not checked; the module beside them is the project's own.
///
/// Nothing about the *index* changes -- the classes in there stay resolvable, which is the whole
/// reason they are indexed: goto and hover into a framework's source is most of what a person does
/// with it all day. What changes is who the findings are addressed to.
#[tokio::test]
async fn the_framework_a_project_reads_is_indexed_and_not_checked() {
    let mut fixture = Fixture::new(A_PROJECT_AND_THE_FRAMEWORK_IT_READS);
    fixture.initialize(false, false).await;
    fixture.initialized().await;

    fixture
        .open("docroot/core/lib/Drupal/Core/Borrowed.php")
        .await;
    fixture
        .open("docroot/modules/contrib/vendored/src/Vendored.php")
        .await;
    fixture
        .open("docroot/modules/custom/ours/src/Ours.php")
        .await;

    expect![[r#"
        [
            "Borrowed.php: none",
            "Ours.php:8: undefined-method Undefined method Drupal\\ours\\Ours::nope()",
            "Vendored.php: none",
        ]
    "#]]
    .assert_debug_eq(&reported(&fixture));
}

/// The other side of the same rule, and the reason it is worth stating twice.
///
/// A rename sweeps the project's files, so renaming a core method would rewrite every call site it
/// can see and leave core declaring the old name -- until the next `composer update` reverted the
/// declaration too. `vendor/` was already refused for exactly this; a framework outside `vendor/`
/// was not, and is now.
#[tokio::test]
async fn a_declaration_in_the_framework_cannot_be_renamed() {
    let mut fixture = Fixture::new(A_PROJECT_AND_THE_FRAMEWORK_IT_READS);
    fixture.initialize(false, false).await;
    fixture.initialized().await;
    fixture
        .open("docroot/core/lib/Drupal/Core/Borrowed.php")
        .await;

    let refused = fixture
        .request(
            "textDocument/prepareRename",
            json!({
                "textDocument": {
                    "uri": fixture.uri("docroot/core/lib/Drupal/Core/Borrowed.php"),
                },
                "position": { "line": 3, "character": 7 },
            }),
        )
        .await;

    expect!["this declaration is not in this project's own code"]
        .assert_eq(&error_message(&refused).unwrap_or_default());
}

/// The standard library, opened by somebody who followed a goto into it.
///
/// The origin discovery gave a file was recomputed every time a buffer for it was opened, from the
/// one rule that cannot see a stub at all: `source_origin` reads the `vendor/` prefix, and the stub
/// set is extracted to a cache directory outside the project entirely. So a stub came back as the
/// project's own code -- body-lowered against the memory rule, and checked. Cmd-clicking `DateTime`
/// on the reference project published **222** `doc-missing-tags` about a file nobody can edit.
///
/// The real set, into a temporary cache: a made-up stub would be a file this test called a stub,
/// and what failed was precisely the step that decides which files those are.
///
/// Its own fixture, with the check that fired turned on. Sharing the one above would have made this
/// an `expect![]` that is empty because nothing asked -- the §8.4 trap, and the exact check whose
/// 222 findings are what was reported.
const A_PROJECT_THAT_ASKS_FOR_DOC_TAGS: &str = r#"
//- /composer.json
{"type": "project", "autoload": {"psr-4": {"App\": "src/"}}}
//- /phrost.toml
[lint]
on = ["doc-missing-tags"]
//- /src/Ours.php
<?php
namespace App;

class Ours
{
    /**
     * Hands back what it was given, which is the shape every stub in the set has: a summary in
     * prose, and not one tag.
     */
    public function go(int $count): int
    {
        return $count;
    }
}
"#;

#[tokio::test]
async fn a_stub_opened_by_following_a_goto_is_read_and_not_checked() {
    let cache = tempfile::tempdir().expect("a temporary directory");
    let stubs = phrost_stubs::StubSet::install_in(cache.path()).expect("the embedded stubs");
    let one = stubs
        .files()
        .iter()
        .find(|path| path.ends_with("date.php"))
        .expect("the date stubs")
        .clone();

    let mut fixture = Fixture::over(A_PROJECT_THAT_ASKS_FOR_DOC_TAGS, Workspace::new(stubs));
    fixture.initialize(false, false).await;
    fixture.initialized().await;
    // The project's own file first, so the snapshot shows the check is live and the
    // stub's silence is the rule and not the configuration.
    fixture.open("src/Ours.php").await;
    fixture.open_absolute(&one).await;

    expect![[r#"
        [
            "Ours.php:10: doc-missing-tags returns a value and has no @return\nOurs.php:10: doc-missing-tags $count has no @param",
            "date.php: none",
        ]
    "#]]
    .assert_debug_eq(&reported(&fixture));
}
