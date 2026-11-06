//! A method named only inside a string, in a file whose comments hold apostrophes.
//!
//! The exact reachability record is fed by a text scan for quoted strings, and the scan had no
//! notion of a comment. `Drupal's` in a docblock read as an opening quote and swallowed everything
//! to the next apostrophe, inverting the quote state for the rest of the file -- so a handler named
//! `'onException'` in a `getSubscribedEvents()` map landed *outside* a string, was recorded as
//! reached by nothing, and a live event handler was reported as dead.
//!
//! Only the server shows it. `phrost check` answers the coarse rule -- a plain substring test,
//! immune to quoting -- and the exact record is only consulted once every file has had its say,
//! which is after a full pass. So the two instruments disagreed, and the one a person reads all day
//! was the wrong one.

use expect_test::expect;

use crate::tests::Fixture;

/// What was reported, as a set. See `unused_declarations::reported` for why.
fn reported(fixture: &Fixture) -> Vec<String> {
    let mut lines = fixture.diagnostics_published();
    lines.sort();
    lines.dedup();
    lines
}

/// A subscriber whose docblock has an apostrophe above the map that names its handlers.
const A_SUBSCRIBER_WITH_PROSE: &str = r#"
//- /composer.json
{"type": "project", "autoload": {"psr-4": {"App\\": "src/"}}}
//- /phrost.toml
[lint]
on = ["unused-declaration"]
//- /src/Kernel.php
<?php
namespace App;

// Names the class, so the member rule is what this fixture exercises rather than the class rule --
// a subject that is itself unreached pins the wrong half, which §8.4 records.
class Kernel
{
    public function boot(): Subscriber
    {
        return new Subscriber();
    }
}
//- /src/Subscriber.php
<?php
namespace App;

class Subscriber
{
    /**
     * Routes that bypass Drupal's native authentication stack.
     *
     * The apostrophe above is the whole fixture: everything after it used to read as one string.
     */
    public static function getSubscribedEvents(): array
    {
        return ['kernel.exception' => [['onException', 49]]];
    }

    public function onException(): void {}

    public function reachedByNothingAtAll(): void {}
}
"#;

/// The handler the map names is alive; the method nothing names is not.
///
/// Both in one fixture, because what has to be pinned is that the scan tells them apart -- a test
/// showing only the silent half would pass just as well with the whole check switched off.
#[tokio::test]
async fn a_handler_named_in_a_string_survives_an_apostrophe_in_the_prose_above_it() {
    let mut fixture = Fixture::new(A_SUBSCRIBER_WITH_PROSE);
    fixture.initialize(false, false).await;
    fixture.initialized().await;
    fixture.open("src/Subscriber.php").await;

    expect![[r#"
        [
            "Kernel.php:8: unused-declaration method Kernel::boot is reached from nowhere in this project",
            "Subscriber.php:11: unused-declaration method Subscriber::getSubscribedEvents is reached from nowhere in this project\nSubscriber.php:18: unused-declaration method Subscriber::reachedByNothingAtAll is reached from nowhere in this project",
        ]
    "#]]
    .assert_debug_eq(&reported(&fixture));
}
