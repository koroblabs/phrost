//! What a call naming the class a callee's closures run as records: the classes along its chain, and
//! the directories an `in()` at its end applies it to -- `__DIR__` being the file's own. A call that
//! names no class records nothing.

use expect_test::expect;

use super::check_index;

#[test]
fn a_configuring_call_records_its_classes_and_directories() {
    check_index(
        r#"
//- /tests/Setup.php
<?php
use App\Suite;
use App\Helpers;

configure(Suite::class)->with(Helpers::class)->in('Feature', __DIR__);
configure(Helpers::class);
configure()->in('Unit');
"#,
        expect![[r#"
            state ready
            file tests/Setup.php [User]
              configures $this as App\Suite & App\Helpers in Feature, .
              configures $this as App\Helpers
              use App\Suite as Suite  [class]
              use App\Helpers as Helpers  [class]
            catalog
            namespaces
            declared keys
            dispatched keys
        "#]],
    );
}
