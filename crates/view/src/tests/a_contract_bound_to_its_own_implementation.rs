//! A framework's contract the container always binds to one class that implements it.
//!
//! `Cache::store()` declares `Contracts\Cache\Repository`, and what it hands back is the concrete
//! repository, which declares more and forwards the rest through `__call`. The implied mixin names
//! the implementation -- which implements the contract, so the two point at each other, and every
//! walk over a class's supertypes has to be one that stops.

use expect_test::expect;

use super::{Query, check_view_with};

#[test]
fn a_contract_reaches_what_its_implementation_declares() {
    check_view_with(
        r#"
//- /src/Cache.php
<?php
namespace Orm;

interface Contract
{
    public function get(string $key): mixed;
}

class Lock {}

interface Store
{
    public function lock(string $name): Lock;
}

/** @mixin Store */
class Concrete implements Contract
{
    public function get(string $key): mixed {}
    public function tags(array $names): static {}
    public function __call(string $method, array $arguments): mixed {}
}

function run(Contract $cache): void
{
    $t@tags@ags = $cache->tags([]);
    $g@get@et = $cache->get('x');
    $l@lock@ock = $cache->lock('x');
}
"#,
        &[
            Query::variable_type("declared by the implementation", "tags"),
            Query::variable_type("declared by the contract", "get"),
            Query::variable_type("forwarded by the implementation's own mixin", "lock"),
        ],
        |world| world.add_implied_mixin("Orm\\Contract", "Orm\\Concrete", false),
        expect![[r#"
            declared by the implementation
            - Concrete
            declared by the contract
            - mixed
            forwarded by the implementation's own mixin
            - Lock
        "#]],
    );
}
