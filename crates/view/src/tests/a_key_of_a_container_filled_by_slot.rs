//! A `foreach` key over a container whose type is only what was written into its slots.
//!
//! What was last written into a slot of the container is what an *element* of it holds, and the
//! reader for a slot nobody declared answered every step after the container with it -- the key of
//! the loop too. Symfony's `foreach ($config['pools'] as $name => $pool)` read `$name` as the pool
//! array that `$config['pools']['cache.app'] = [...]` had put there, and six `incompatible-argument`
//! errors on `$container->setAlias($name)` followed.

use expect_test::expect;

use super::{Query, check_view};

#[test]
fn a_key_is_not_the_value_written_into_a_slot() {
    check_view(
        r#"
//- /src/app.php
<?php
function run( array $config, array $distribution ) {
    $config['pools'] = array();
    $config['pools']['cache.app'] = $distribution;
    foreach ( $config['pools'] as $name => $pool ) {
        $@key@name;
        $@value@pool;
    }
}
"#,
        &[
            Query::variable_type("the key", "key"),
            Query::variable_type("the value, which the write does describe", "value"),
        ],
        expect![[r#"
            the key
            - (no type)
            the value, which the write does describe
            - array
        "#]],
    );
}
