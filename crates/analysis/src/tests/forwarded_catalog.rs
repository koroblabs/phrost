//! What a reader gets from a value the previous call's argument named.
//!
//! The view decides the class; these are the two things that decision has to survive to be worth
//! anything -- the members are offered, and the null the declaration promised is still there.

use expect_test::expect;

use super::{Query, check_with};

/// A storage handler that serves every entity type, which is the reason none of them can be named
/// by a return type.
const FIXTURE: &str = r#"
//- /src/Storage.php
<?php
namespace Dep;

interface EntityInterface {}

interface StorageInterface
{
    /** @return \Dep\EntityInterface|null */
    public function load($id);
}

interface ManagerInterface
{
    /** @return \Dep\StorageInterface */
    public function getStorage($id);
}

//- /src/Node.php
<?php
namespace App;

use Dep\EntityInterface;

class Node implements EntityInterface
{
    public function title(): string
    {
        return '';
    }

    public function published(): bool
    {
        return TRUE;
    }
}
"#;

fn with_entities(world: &mut phrost_harness::World) {
    world.contracts_mut().forward_argument(
        "Dep\\StorageInterface",
        "load",
        "getStorage",
        "entity",
        phrost_harness::Forwarded::Instance,
    );
    world
        .contracts_mut()
        .catalog_entry("entity", "node", Some("App\\Node"));
}

#[test]
fn the_members_of_the_named_class_are_offered() {
    check_with(
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
        $node = $manager->getStorage('node')->load(1);
        if ($node !== NULL) {{
            $node->@offer@
        }}
    }}
}}
"#
        ),
        &[Query::completion("members of the entity", "offer")],
        with_entities,
        expect![[r#"
            members of the entity
            - published [method]  : bool  writes `();`
            - title [method]  : string  writes `();`
            - accepting the first leaves `$node->published`
        "#]],
    );
}

#[test]
fn the_null_the_declaration_promised_is_still_there() {
    // The difference from a service id, where the declaration said `object` and had nothing worth
    // keeping. An entity id that matches no row really does load nothing, so replacing the class
    // must not quietly replace the `?` with it -- that would turn the one check a person has to
    // write into a check nothing asks for.
    check_with(
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
        $node = $manager->getStorage('node')->load(1);
        $no@hover@de;
    }}
}}
"#
        ),
        &[Query::hover("nullable, and the class is known", "hover")],
        with_entities,
        expect![[r#"
            nullable, and the class is known
            - code: ?Node $node
            - note: assigned
        "#]],
    );
}
