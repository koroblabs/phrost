//! A value whose class was decided by an argument to the call before it.
//!
//! The shape a same-call binding cannot express. `getStorage('node')` hands back a class that serves
//! dozens of entity types at once, so neither the `load()` call nor its receiver names the one being
//! asked for -- the id one link back is the only thing that does.
//!
//! Owned at this layer because this is where a value's class is decided. The features above ask for
//! it and would each need their own version of this test otherwise.

use expect_test::expect;
use phrost_index::Forwarded;

use super::{Query, check_view_with};

/// Two interfaces and two entities, so a wrong answer is a *different* class rather than nothing.
const FIXTURE: &str = r#"
//- /src/Storage.php
<?php
namespace Dep;

interface EntityInterface {}

interface StorageInterface
{
    /** @return \Dep\EntityInterface|null */
    public function load($id);

    /** @return \Dep\EntityInterface */
    public function create(array $values = []);

    /** @return \Dep\EntityInterface[] */
    public function loadMultiple(array $ids = []);
}

/// One class for every entity type there is, which is the whole difficulty.
class SqlStorage implements StorageInterface
{
    public function load($id) {}
    public function create(array $values = []) {}
    public function loadMultiple(array $ids = []) {}
}

interface ManagerInterface
{
    /** @return \Dep\StorageInterface */
    public function getStorage($id);
}

//- /src/Entities.php
<?php
namespace App;

use Dep\EntityInterface;

class Node implements EntityInterface
{
    public function title(): string
    {
        return '';
    }
}

class Comment implements EntityInterface {}
"#;

fn with_entities(world: &mut phrost_harness::World) {
    world.contracts_mut().forward_argument(
        "Dep\\StorageInterface",
        "load",
        "getStorage",
        "entity",
        Forwarded::Instance,
    );
    world.contracts_mut().forward_argument(
        "Dep\\StorageInterface",
        "create",
        "getStorage",
        "entity",
        Forwarded::Instance,
    );
    // The array-returning shape, so the substitution into an element is covered here too.
    world.contracts_mut().forward_argument(
        "Dep\\StorageInterface",
        "loadMultiple",
        "getStorage",
        "entity",
        Forwarded::ManyOf,
    );
    world
        .contracts_mut()
        .catalog_entry("entity", "node", Some("App\\Node"));
    world
        .contracts_mut()
        .catalog_entry("entity", "comment", Some("App\\Comment"));
}

#[test]
fn the_key_from_the_previous_call_names_the_class() {
    check_view_with(
        &format!(
            r#"{FIXTURE}
//- /src/Caller.php
<?php
namespace App;

use Dep\ManagerInterface;

class Caller
{{
    public function run(ManagerInterface $manager): void
    {{
        $storage = $manager->getStorage('node');
        $node = $storage->load(1);
        $no@one@de;
        $fresh = $storage->create();
        $fre@made@sh;
    }}
}}
"#
        ),
        &[
            Query::variable_class("loaded", "one", "node"),
            Query::variable_class("created", "made", "fresh"),
        ],
        with_entities,
        expect![[r#"
            loaded
            - App\Node
            created
            - App\Node
        "#]],
    );
}

#[test]
fn the_chain_needs_no_variable_in_between() {
    check_view_with(
        &format!(
            r#"{FIXTURE}
//- /src/Caller.php
<?php
namespace App;

use Dep\ManagerInterface;

class Caller
{{
    public function run(ManagerInterface $manager): void
    {{
        $node = $manager->getStorage('comment')->load(1);
        $no@one@de;
    }}
}}
"#
        ),
        &[Query::variable_class("straight through", "one", "node")],
        with_entities,
        expect![[r#"
            straight through
            - App\Comment
        "#]],
    );
}

#[test]
fn a_key_the_catalog_does_not_hold_leaves_the_declaration_alone() {
    // The half that keeps this from being a guess. An id nothing registers is not an occasion to
    // pick some other entity's class -- the declared interface is the honest answer, and it is a
    // useful one.
    check_view_with(
        &format!(
            r#"{FIXTURE}
//- /src/Caller.php
<?php
namespace App;

use Dep\ManagerInterface;

class Caller
{{
    public function run(ManagerInterface $manager): void
    {{
        $storage = $manager->getStorage('nothing_registers_this');
        $node = $storage->load(1);
        $no@one@de;
    }}
}}
"#
        ),
        &[Query::variable_class("unregistered", "one", "node")],
        with_entities,
        expect![[r#"
            unregistered
            - Dep\EntityInterface
        "#]],
    );
}

#[test]
fn a_different_call_in_between_ends_the_forwarding() {
    // The contract is about the call that produced the receiver, not about anything earlier in the
    // chain. A step past `getStorage` and the id no longer describes what is in hand.
    check_view_with(
        &format!(
            r#"{FIXTURE}
//- /src/Caller.php
<?php
namespace App;

use Dep\ManagerInterface;

class Caller
{{
    public function run(ManagerInterface $manager): void
    {{
        $node = $manager->getStorage('node')->create()->load(1);
        $no@one@de;
    }}
}}
"#
        ),
        &[Query::variable_class("one step too far", "one", "node")],
        with_entities,
        expect![[r#"
            one step too far
            - (type not known here)
        "#]],
    );
}

#[test]
fn a_method_nothing_contracted_is_untouched() {
    check_view_with(
        &format!(
            r#"{FIXTURE}
//- /src/Caller.php
<?php
namespace App;

use Dep\ManagerInterface;

class Caller
{{
    public function run(ManagerInterface $manager): void
    {{
        $storage = $manager->getStorage('node');
        $sto@here@rage;
    }}
}}
"#
        ),
        &[Query::variable_class(
            "the storage itself",
            "here",
            "storage",
        )],
        with_entities,
        expect![[r#"
            the storage itself
            - Dep\StorageInterface
        "#]],
    );
}

#[test]
fn a_guard_written_between_the_two_calls_does_not_lose_the_key() {
    // How the idiom is actually written, because `load()` returns null for an id that matches no
    // row. The guard puts a whole node in the way of the lookback, and the id is still the only
    // thing in the file that names the entity.
    check_view_with(
        &format!(
            r#"{FIXTURE}
//- /src/Caller.php
<?php
namespace App;

use Dep\ManagerInterface;

class Caller
{{
    public function run(ManagerInterface $manager): void
    {{
        $storage = $manager->getStorage('node');
        if ($storage) {{
            $node = $storage->load(1);
            $no@one@de;
        }}
    }}
}}
"#
        ),
        &[Query::variable_class("through a guard", "one", "node")],
        with_entities,
        expect![[r#"
            through a guard
            - App\Node
        "#]],
    );
}

/// The array-returning shape: the key decides what the array *holds*, not what it is.
///
/// The half left out when this was built, on the grounds that substituting into an element is a
/// different substitution from replacing a return. It is -- and the declaration cannot decide which,
/// because `EntityInterface` and `EntityInterface[]` are equally silent about *which* entity. So the
/// rule carries the shape, and both ways of getting one out of the array are pinned here: reading it
/// by index, which is what `reset()` amounts to, and iterating it.
#[test]
fn a_method_returning_many_gives_an_array_of_the_class() {
    check_view_with(
        &format!(
            r#"{FIXTURE}
//- /src/Caller.php
<?php
namespace App;

use Dep\ManagerInterface;

class Caller
{{
    public function byIndex(ManagerInterface $manager): void
    {{
        $nodes = $manager->getStorage('node')->loadMultiple([1, 2]);
        $first = $nodes[0];
        $fi@indexed@rst;
    }}

    public function each(ManagerInterface $manager): void
    {{
        foreach ($manager->getStorage('node')->loadMultiple([1, 2]) as $node) {{
            $no@iterated@de;
        }}
    }}
}}
"#
        ),
        &[
            Query::variable_class("read by index", "indexed", "first"),
            Query::variable_class("iterated", "iterated", "node"),
        ],
        with_entities,
        expect![[r#"
            read by index
            - App\Node
            iterated
            - App\Node
        "#]],
    );
}
