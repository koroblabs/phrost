//! `define('NAME', ...)`, which declares a constant no `const` statement mentions.

use expect_test::expect;

use super::check_hir;

/// The name is the string, whole, and it carries no namespace.
///
/// Asked of PHP 8.5 rather than assumed, because the rule is the opposite of the one `const`
/// follows: inside `namespace App;`, `define('MAX', 1)` declares the **global** `MAX` and
/// `defined('App\MAX')` is false, while `define('App\INSIDE', 2)` declares exactly `App\INSIDE`.
/// Prefixing the current namespace onto the first would put a name in the index that the runtime
/// does not have, and drop the one it does.
#[test]
fn a_defined_constant_takes_its_whole_name_from_the_string() {
    check_hir(
        r#"
//- /app.php
<?php

namespace App;

define('MAX', 1);
define('App\INSIDE', 2);
define('\LEADING', 3);
"#,
        expect![[r#"
            const MAX @ 5:9-5:12
            const App\INSIDE @ 6:9-6:19
            const LEADING @ 7:9-7:17
            namespace App @ 3:11-3:14
        "#]],
    );
}

/// What it refuses, which is everything it cannot know a name from.
///
/// A computed name declares something this cannot see, and inventing one puts a name in the index
/// that the code does not have. One argument does not run at all.
#[test]
fn a_define_whose_name_is_not_a_literal_declares_nothing_here() {
    check_hir(
        r#"
//- /app.php
<?php

define($name, 1);
define('ONLY_ONE');
define(...$args);
$container->define('METHOD', 1);
"#,
        expect![[r#"
            <no declarations>
        "#]],
    );
}

/// A fallback, which is how a project ships a constant it may already have.
///
/// The walk that reached the statement has already said it is conditional, and ranking reads that
/// -- so an unguarded declarer of the same name wins, exactly as it does for a guarded `function`.
#[test]
fn a_define_behind_a_guard_is_a_conditional_declaration() {
    check_hir(
        r#"
//- /app.php
<?php

if (!defined('WPINC')) {
    define('WPINC', 'wp-includes');
}
"#,
        expect![[r#"
            const WPINC [conditional] @ 4:13-4:18
        "#]],
    );
}
