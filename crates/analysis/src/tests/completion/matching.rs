//! What a typed prefix matches, and why a member is matched differently from a name.
//!
//! Reported from an editor: typing `ship` on a class with `isShipped`, `getShipDate` and
//! `setShipDate` offered nothing. PHP member names carry their subject in the middle, so a prefix
//! test is the wrong test -- and a client cannot recover from it, because it can only narrow what it
//! was sent.

use expect_test::expect;

use crate::tests::{Query, check};

const CLASS: &str = r#"
//- /src/App.php
<?php
namespace App;

class Application
{
    public function isShipped(): bool {}

    public function getShipDate(): string {}

    public function setShipDate(int $at): void {}

    public function account(): string {}
}

class Caller
{
    public function run(Application $app): void
    {
        $app->ship@here@
    }
}
"#;

/// The reported case. All three carry `ship` and none of them starts with it.
#[test]
fn a_member_matches_what_is_typed_anywhere_in_its_name() {
    check(
        CLASS,
        &[Query::completion("ship", "here")],
        expect![[r#"
            ship
            - getShipDate [method]  : string  writes `();`
            - isShipped [method]  : bool  writes `();`
            - setShipDate [method]  : void  writes `(|);`
            - ... and the client must ask again
            - accepting the first leaves `$app->getShipDate`
        "#]],
    );
}

/// And it is a subsequence and not a substring test, so the letters have to be in order.
#[test]
fn the_letters_have_to_be_in_order() {
    check(
        r#"
//- /src/App.php
<?php
namespace App;

class Application
{
    public function isShipped(): bool {}
}

class Caller
{
    public function run(Application $app): void
    {
        $app->pihs@here@
    }
}
"#,
        &[Query::completion("backwards", "here")],
        expect![[r#"
            backwards
            - (nothing offered)
        "#]],
    );
}

/// The same class, with something else already typed after the arrow.
fn typing(typed: &str) -> String {
    CLASS.replace("ship@here@", &format!("{typed}@here@"))
}

/// One letter is not a subsequence worth having. Every name containing it satisfies one, so the
/// answer is most of the class in an order that has nothing to do with what was typed --
/// `getShipDate` and `setShipDate` for an `a`. The constraint between letters is the only thing
/// subsequence matching has going for it, and one letter has none.
#[test]
fn the_first_letter_typed_has_to_start_the_name() {
    check(
        &typing("a"),
        &[Query::completion("one letter", "here")],
        expect![[r#"
            one letter
            - account [method]  : string  writes `();`
            - ... and the client must ask again
            - accepting the first leaves `$app->account`
        "#]],
    );
}

/// The second letter brings it back, because two letters in order say something a prefix test
/// cannot: none of these three starts with `sd`.
#[test]
fn a_second_letter_restores_the_subsequence_test() {
    check(
        &typing("sd"),
        &[Query::completion("two letters", "here")],
        expect![[r#"
            two letters
            - getShipDate [method]  : string  writes `();`
            - isShipped [method]  : bool  writes `();`
            - setShipDate [method]  : void  writes `(|);`
            - ... and the client must ask again
            - accepting the first leaves `$app->getShipDate`
        "#]],
    );
}

/// A bare name is reachable from the middle too, and what orders the list is how well it matched.
///
/// This test used to pin the opposite, on a reason that was half right: "there are thousands of
/// them, a one-letter subsequence would match most of a project, and a name is written from its
/// start". The first half survives as [`LOOSE_FLOOR`](phrost_view) -- three characters before a
/// loose match is tried, because `re` is a subsequence of a third of everything. The second half was
/// never true: `OrderManager` is what a person means by `OrdMan`, and every other PHP server
/// finds it.
///
/// What makes it liveable is the *order*. A prefix hit comes first and the rest are ranked by
/// `phrost_model::match_score`, which pays for a character that opens a word and for one that runs
/// on from the last -- so a name whose words the letters open beats one they merely appear in.
#[test]
fn a_bare_name_is_reachable_from_the_middle_and_ranked_by_how_well_it_fits() {
    check(
        r#"
//- /src/App.php
<?php
namespace App;

class Customer {}

class CustomerLimit {}

class Runner
{
    public function run(): void
    {
        new stom@middle@
    }
}
"#,
        &[Query::completion("from the middle", "middle")],
        expect![[r#"
            from the middle
            - Customer [class]  App\Customer
            - CustomerLimit [class]  App\CustomerLimit
            - ... and the client must ask again
            - accepting the first leaves `new Customer`
        "#]],
    );
}

/// And two characters are still read as the start of a name.
///
/// The floor, and it is a fact about the population rather than about the matcher: `om` is a
/// subsequence of a great many names, and a list of them ordered by a two-letter score is a list
/// nobody reads. A class's *members* are thirty and use two -- same test, different crowd.
#[test]
fn two_characters_are_too_few_to_mean_a_subsequence() {
    check(
        r#"
//- /src/App.php
<?php
namespace App;

class Customer {}

class Runner
{
    public function run(): void
    {
        new om@short@
    }
}
"#,
        &[Query::completion("two characters", "short")],
        expect![[r#"
            two characters
            - (nothing offered)
        "#]],
    );
}

/// What begins with the prefix outranks what merely holds it, whoever declared either -- and the
/// name exactly as typed leads.
///
/// Reported as a missing function: typing `time` on a real project offered two hundred names and
/// not PHP's `time()`, because the project's own loose matches (`ShipmentLineSegment` holds `t`, `i`,
/// `m`, `e` in order) shared a front buffer with its prefix matches and filled the page before a
/// dependency's prefix match was reached.
#[test]
fn a_prefix_match_from_anywhere_outranks_a_loose_match_from_the_project() {
    check(
        r#"
//- /vendor/php/time.php
<?php
function time(): int {}
function timezone_open(string $timezone) {}

//- /src/Trade.php
<?php
namespace App;

class ShipmentLineSegment {}
class TimeTrait {}

function run(): void
{
    $now = time@here@
}
"#,
        &[Query::completion("typed in full", "here")],
        expect![[r#"
            typed in full
            - time [function]  time
            - TimeTrait [class]  App\TimeTrait
            - timezone_open [function]  timezone_open
            - ShipmentLineSegment [class]  App\ShipmentLineSegment
            - ... and the client must ask again
            - accepting the first leaves `$now = time`
        "#]],
    );
}
