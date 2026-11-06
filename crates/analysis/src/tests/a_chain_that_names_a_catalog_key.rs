//! A stored chain whose class is in a string, not in the chain.
//!
//! `SqlBase::select()` is the shape `ReturnedRow` was built for: a body that names no type and
//! describes one exactly, stored as a root and a list of member names. `return
//! \Drupal::service('utilities.manager');` is the same idea with the class in none of those places
//! and all of it in the argument -- the chain describes `Drupal::service()`, which describes every
//! service there is.
//!
//! So the row carries the first step's first literal, which is what a catalog contract means by
//! "the argument", and the walk asks the catalog before it asks the member. Before the member,
//! because `service()` is declared `object` and the contract exists to improve on exactly that.
//!
//! One `Option<Name>` on a sparse table: measured on the reference project, the whole index went
//! from 104.0 MB to 104.1 MB.
//!
//! What makes it reachable at all is the other half, which is about `object`: a declaration saying
//! only that its value is an object says nothing a class would not say better, so lowering reads
//! the body there and the view prefers what the body describes. `refined` has ranked a *docblock*
//! above an `object` hint for the same reason all along, and the body was the third source with no
//! such rule.

use expect_test::expect;

use super::diagnostics::check_framework;

/// A container whose `get` is keyed by a service id, and an accessor written the way Drupal writes
/// one: `object` in the hint, `object` in the docblock, and the answer in the body.
const FIXTURE: &str = r#"
//- /src/Dep.php
<?php
namespace Dep;

class Container
{
    public static function service(string $id): object
    {
        return new \stdClass();
    }
}

//- /src/Manager.php
<?php
namespace App;

class Manager
{
    public function run(string $what): string
    {
        return $what;
    }
}

//- /src/app.php
<?php
namespace App;

use Dep\Container;

class Template
{
    /**
     * Returns the manager.
     *
     * @return object
     *   The object.
     */
    protected function manager(): object
    {
        return Container::service('app.manager');
    }

    public function build(): string
    {
        return $this->manager()->run('x');
    }
}
"#;

fn registered(world: &mut phrost_harness::World) {
    world
        .contracts_mut()
        .bind_argument("Dep\\Container", "service", 0, "", "service");
    world
        .contracts_mut()
        .catalog_entry("service", "app.manager", Some("App\\Manager"));
    world.contracts_mut().close_catalog("service");
}

#[test]
fn a_catalog_key_in_a_stored_chain_names_the_class_the_body_hands_back() {
    // Nothing is reported, and that is the claim: `run` resolves on `Manager`, so the call is
    // ordinary. The test below is what proves the silence is the chain working rather than the
    // fixture being inert.
    check_framework(FIXTURE, registered, expect![""]);
}

#[test]
fn what_the_chain_resolved_to_is_then_judged_like_anything_else() {
    // The same accessor with an argument the resolved class refuses -- a `null`, because weak mode
    // converts an int to a string and the point is a value PHP itself will not take. Before the
    // chain carried its key, `$this->manager()` was an `object` and this line was judged by nothing
    // at all -- which
    // is how `$this->utilitiesManager()->fillPdfAsync(file_get_contents($path), $fields)` on the
    // reference project passed `file_get_contents`'s `false` into a `string` unremarked.
    check_framework(
        &FIXTURE.replace(
            "return $this->manager()->run('x');",
            "return $this->manager()->run(NULL);",
        ),
        registered,
        expect![[r#"
            21:38-21:42 error [null-argument] Cannot pass null as $what, which takes string
        "#]],
    );
}
