//! A catalog key nothing writes down, and what a reader gets when they hover or misspell it.
//!
//! Drupal generates one route per entry of an entity type's `links` -- `entity.node.canonical` from
//! `'canonical' => '/node/{node}'` -- and none of those names is in any `*.routing.yml`. On the
//! reference project they are 298 route names against the 1 052 the YAML declares, and they are the
//! ones its own code writes most: `entity.node.canonical` alone is 42 call sites.
//!
//! Every one of them used to be a key the catalog did not hold, and that showed up twice. Hovering
//! one said nothing. And the near-miss report is deliberately quiet about a key that is merely
//! absent -- the whole gate on it is that a typo nearly hits a *registered* key -- so a typo of one
//! of these did not report either, while a typo of a YAML route did. That asymmetry read from
//! outside as a defect in the report, and was a gap in the catalog.
//!
//! What a generated key cannot do is send a reader anywhere: the class carrying the link template is
//! not the route, so there is a description and no place.
//!
//! Neutral names in the fixtures, as everywhere below the framework seam.

use expect_test::expect;

use super::{Query, check_with, diagnostics::check_framework};

const FIXTURE: &str = r#"
//- /src/Url.php
<?php
namespace Dep;

class Url
{
    public static function fromRoute(string $name): mixed
    {
        return NULL;
    }
}

//- /src/Report.php
<?php
namespace App;

#[Plugin(id: 'report', links: ['canonical' => '/report/{report}', 'edit-form' => '/report/{report}/edit'])]
class Report {}

//- /src/Caller.php
<?php
namespace App;

use Dep\Url;

class Caller
{
    public function run(): void
    {
        Url::fromRoute('page.report.@here@canonical');
    }

    public function slip(): void
    {
        Url::fromRoute('page.report.canonicl');
    }
}
"#;

/// The rule that generates the keys, plus the call that reads one.
fn registered(world: &mut phrost_harness::World) {
    world.contracts_mut().derive_keys_with(
        "Plugin",
        "links",
        "id",
        "page.{name}.{key}",
        &[("-", "_")],
        "page",
    );
    world
        .contracts_mut()
        .bind_argument("Dep\\Url", "fromRoute", 0, "", "page");
    world.set_keyed_attributes(&["Plugin"], &["links"]);
}

#[test]
fn hovering_a_generated_key_says_what_its_own_entry_wrote_and_offers_nowhere_to_go() {
    check_with(
        FIXTURE,
        &[
            Query::hover("a key nothing declares", "here"),
            Query::goto("and nowhere to send a reader", "here"),
        ],
        registered,
        expect![[r#"
            a key nothing declares
            - code: 'page.report.canonical'
            - note: /report/{report}
            and nowhere to send a reader
            - (nothing to go to)
        "#]],
    );
}

#[test]
fn a_typo_of_a_generated_key_reports_like_a_typo_of_a_declared_one() {
    check_framework(
        FIXTURE,
        registered,
        expect![[r#"
        15:24-15:46 warning [unknown-catalog-key] No page is registered as `page.report.canonicl`. Did you mean `page.report.canonical`?
    "#]],
    );
}
