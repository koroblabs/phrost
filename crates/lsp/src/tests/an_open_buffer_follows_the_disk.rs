//! A change made outside the editor reaches the file the person is looking at.
//!
//! The project check that a watched change starts reads `own_closed_files`, deliberately: it is the
//! pass for the files nobody has open. So for a while the one file on screen was the only file in
//! the project still showing what was wrong before the change -- a method a colleague's commit
//! added, or a `composer update` added, stayed underlined there and nowhere else, and nothing but a
//! restart or a keystroke in that buffer corrected it.
//!
//! The mirror of `flow::project_diagnostics::a_change_on_disk_checks_the_project_again_and_clears_
//! what_was_fixed`, which makes the same claim about a file that is *closed*.

use expect_test::expect;

use crate::tests::Fixture;

const CALLING_A_METHOD_THAT_IS_NOT_THERE_YET: &str = r#"
//- /composer.json
{}

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
        $repo->store(1);
    }
}
"#;

#[tokio::test]
async fn an_open_buffer_is_checked_again_when_the_disk_changes_under_it() {
    let mut server = Fixture::new(CALLING_A_METHOD_THAT_IS_NOT_THERE_YET);
    server.initialize(false, true).await;
    server.open("src/Service.php").await;
    server.initialized().await;

    let project = server.watch_project_passes();
    // The declaration gains the method, on disk, in a file the editor does not have open.
    server
        .write_and_notify(
            r#"
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    public function find(int $id): void {}
    public function store(int $id): void {}
}
"#,
        )
        .await;
    server.project_pass_ends(project).await;

    // Twice: the problem when the buffer was opened, and nothing once the disk answered it. Without
    // the second the editor keeps the first forever.
    expect![[r#"
        [
            "Service.php:8: undefined-method Undefined method App\\Repo::store()",
            "Service.php: none",
        ]
    "#]]
    .assert_debug_eq(&server.diagnostics_published());
}
