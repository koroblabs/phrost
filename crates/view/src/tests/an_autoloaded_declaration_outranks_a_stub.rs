//! A class two dependencies declare is the one the autoloader would load.
//!
//! Eloquent Power Joins ships `.stubs.php`, an IDE helper declaring
//! `Illuminate\Database\Eloquent\Builder` with only its own six methods. It sorts before
//! `laravel/framework` by path, and ranked by path it shadowed every method of the real builder.
//! Composer decides which file PHP reads for a class, so within one tier the file a PSR-4 rule maps
//! the name onto comes first.

use expect_test::expect;

use super::{Query, check_view_ingested_with};

#[test]
fn the_file_the_autoloader_maps_the_name_onto_wins() {
    check_view_ingested_with(
        r#"
//- /vendor/aaa/power-joins/.stubs.php
<?php
namespace Lib {
    class Builder
    {
        public function joinRelationship(): static {}
    }
}

//- /vendor/lib/src/Builder.php
<?php
namespace Lib;

class Builder
{
    public function get(): array {}
}

//- /src/Query.php
<?php
namespace App;

function run(\Lib\Builder $builder): void
{
    $builder->g@get@et();
}
"#,
        &[Query::goto("the autoloaded declaration answers", "get")],
        |world| world.set_namespace_rules(&[("Lib\\", "vendor/lib/src")]),
        expect![[r#"
            the autoloaded declaration answers
            - method Lib\Builder::get @ vendor/lib/src/Builder.php 6:21-6:24
        "#]],
    );
}
