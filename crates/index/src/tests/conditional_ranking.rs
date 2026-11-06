//! Where a declaration that says it is a fallback comes in the order.
//!
//! The catalog section of the dump lists every declarer of a name in ranked order, best first, so
//! this is the whole answer rather than a probe of one lookup. What it has to show is that a
//! polyfill loses to the stub it polyfills and still wins when it is the only declaration there is
//! -- the two halves of `if (!function_exists(..))`, which no other test can state together.

use expect_test::expect;

use super::{Step, check_incremental, check_index};

#[test]
fn a_guarded_polyfill_loses_to_the_stub_it_polyfills() {
    // The measured failure this whole mechanism exists to prevent. Ranked by origin alone a vendor
    // declarer beats a stub, so `str_starts_with` would answer `?string` from the polyfill and every
    // `possibly-null-argument` against it would quietly close -- the `?T where T expected`
    // suppression the project forbids, arriving through the index rather than through a filter.
    check_index(
        r#"
//- /stubs/core.php  stub
<?php
function str_starts_with(string $haystack, string $needle): bool {}

//- /vendor/symfony/polyfill-php80/bootstrap.php  vendor
<?php
if (!function_exists('str_starts_with')) {
    function str_starts_with(?string $haystack, ?string $needle): bool {}
}
"#,
        expect![[r#"
            state ready
            file stubs/core.php [Stub]
              function str_starts_with @ 2:10-2:25
              function str_starts_with(string $haystack, string $needle): bool
            file vendor/symfony/polyfill-php80/bootstrap.php [Vendor]
              function str_starts_with [conditional] @ 3:14-3:29
              function str_starts_with(?string $haystack, ?string $needle): bool
            catalog
              [function] str_starts_with -> stubs/core.php (str_starts_with) | vendor/symfony/polyfill-php80/bootstrap.php (str_starts_with)
            namespaces
            declared keys
            dispatched keys
        "#]],
    );
}

#[test]
fn a_polyfill_nothing_else_declares_is_still_the_answer() {
    // Ranked last and not skipped. `dump()` exists in exactly one place in a real project and it is
    // inside the guard, so dropping a conditional declaration would trade one wrong report for
    // another.
    check_index(
        r#"
//- /vendor/symfony/var-dumper/Resources/functions/dump.php  vendor
<?php
if (!function_exists('dump')) {
    function dump(mixed ...$vars): mixed {}
}
"#,
        expect![[r#"
            state ready
            file vendor/symfony/var-dumper/Resources/functions/dump.php [Vendor]
              function dump [conditional] @ 3:14-3:18
              function dump(mixed ...$vars): mixed
            catalog
              [function] dump -> vendor/symfony/var-dumper/Resources/functions/dump.php (dump)
            namespaces
            declared keys
            dispatched keys
        "#]],
    );
}

#[test]
fn a_guarded_declaration_loses_to_user_code_and_to_plain_vendor_code() {
    // Below all four tiers, not only below the stubs: a guarded declaration states its own
    // precedence, and it is the same statement whoever wrote the file.
    check_index(
        r#"
//- /src/App.php  user
<?php
if (!class_exists('Legacy')) {
    class Legacy {}
}

//- /stubs/core.php  stub
<?php
class Legacy {}

//- /vendor/acme/lib/Legacy.php  vendor
<?php
class Legacy {}
"#,
        expect![[r#"
            state ready
            file src/App.php [User]
              class Legacy [conditional] @ 3:11-3:17
              Legacy is class
            file stubs/core.php [Stub]
              class Legacy @ 2:7-2:13
              Legacy is class
            file vendor/acme/lib/Legacy.php [Vendor]
              class Legacy @ 2:7-2:13
              Legacy is class
            catalog
              [class] legacy -> vendor/acme/lib/Legacy.php (Legacy) | stubs/core.php (Legacy) | src/App.php (Legacy)
            namespaces
            declared keys
            dispatched keys
        "#]],
    );
}

#[test]
fn removing_the_guard_restores_the_declaration_to_its_own_tier() {
    // The `incremental == from-scratch` dimension, over the one bit this change adds. A flag carried
    // on a symbol is exactly the kind of thing a re-ingest can leave stale: nothing else in the
    // rendering would move, and the only symptom would be a signature answering from the wrong file.
    check_incremental(
        r#"
//- /stubs/core.php  stub
<?php
function str_contains(string $haystack, string $needle): bool {}

//- /vendor/acme/polyfill/bootstrap.php  vendor
<?php
if (!function_exists('str_contains')) {
    function str_contains(?string $haystack, ?string $needle): bool {}
}
"#,
        &[Step::write(
            "vendor/acme/polyfill/bootstrap.php",
            r#"<?php
function str_contains(?string $haystack, ?string $needle): bool {}
"#,
        )],
        expect![[r#"
            state ready
            file stubs/core.php [Stub]
              function str_contains @ 2:10-2:22
              function str_contains(string $haystack, string $needle): bool
            file vendor/acme/polyfill/bootstrap.php [Vendor]
              function str_contains @ 2:10-2:22
              function str_contains(?string $haystack, ?string $needle): bool
            catalog
              [function] str_contains -> vendor/acme/polyfill/bootstrap.php (str_contains) | stubs/core.php (str_contains)
            namespaces
            declared keys
            dispatched keys
        "#]],
    );
}
