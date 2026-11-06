//! A method a Twig template calls and no PHP file names.
//!
//! The fourth source of *reached* for the dead-code pass, and the one its own doc warned about
//! leaving out: "leaving any of them out turns this into a report nobody trusts twice." A template
//! renders `{{ trade.status.getDescription }}` and nothing in PHP writes `getDescription` anywhere,
//! so the method read as reached from nowhere.
//!
//! Twig's shorthand is the half that makes this more than a name search. `{{ node.title }}` is
//! `$node->title`, `$node->title()`, `$node->getTitle()`, `$node->isTitle()` or `$node->hasTitle()`,
//! whichever the object has -- so the name a template writes is frequently not the name the method
//! carries, and the getter spellings are recorded beside every word.
//!
//! Words rather than parsed calls, which is coarse and is the direction this has to be coarse in:
//! the set is only ever consulted to *withhold* a finding, exactly as the member half of the rule
//! already is -- "one `->run()` anywhere keeps every `run` alive". Twenty-five of the reference
//! project's 124 findings were one of these.

use expect_test::expect;

use super::diagnostics::check_asked_for_with;

#[test]
fn a_method_a_template_reaches_is_not_dead_and_one_nothing_reaches_still_is() {
    check_asked_for_with(
        r#"
//- /composer.json
{"type": "project", "autoload": {"psr-4": {"App\\": "src/"}}}
//- /src/Caller.php
<?php
namespace App;

class Caller
{
    public function go(Status $status): Status { return $status; }

    public function called(): void {}
}
//- /src/Runner.php
<?php
namespace App;

class Runner
{
    public function run(Caller $caller): void { $caller->called(); }
}
//- /src/Status.php
<?php
namespace App;

class Status
{
    public function getDescription(): string { return 'x'; }

    public function getTitle(): string { return 'y'; }

    public function getNothingRenders(): string { return 'z'; }
}
"#,
        &["unused-declaration"],
        // What the templates write: one name in full, and one through Twig's shorthand -- which is
        // the spelling a real template uses and the one a name search would miss.
        |world| world.set_template_words(&["getDescription", "title"]),
        expect![[r#"
            10:21-10:38 warning [unused-declaration] method Status::getNothingRenders is reached from nowhere in this project
        "#]],
    );
}
