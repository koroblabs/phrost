//! Where a declaration its author marked `@ignore` comes in the order.
//!
//! The sibling of `conditional_ranking`, and it exists because of a shape no other project had:
//! WordPress declares nineteen of its commonest functions twice. `wp-admin/includes/noop.php` has
//! `function __() {}` for one minimal load path and `wp-includes/l10n.php` has the real
//! `__( $text, $domain = 'default' )`. Both are unconditional user code in one checkout, the two
//! files are never loaded together, and nothing in the source says so except the tag.
//!
//! Believing the noop was **18 906 of 23 092 findings** on a clean checkout -- 82 % of everything,
//! from nineteen declarations, every one of them `__() takes at most 0 arguments, 1 given`.

use expect_test::expect;

use super::check_index;

#[test]
fn an_ignored_declaration_loses_to_the_real_one() {
    check_index(
        r#"
//- /wp-admin/includes/noop.php
<?php
/**
 * @ignore
 */
function __() {}

//- /wp-includes/l10n.php
<?php
function __( $text, $domain = 'default' ) {
    return $text;
}
"#,
        expect![[r#"
            state ready
            file wp-admin/includes/noop.php [User]
              function __ [ignored] @ 5:10-5:12
              function __()
            file wp-includes/l10n.php [User]
              function __ @ 2:10-2:12
              function __($text, $domain = 'default')
            catalog
              [function] __ -> wp-includes/l10n.php (__) | wp-admin/includes/noop.php (__)
            namespaces
            declared keys
            dispatched keys
        "#]],
    );
}

/// The other half, and the reason the tier is last rather than skipped: with one declarer the tag
/// changes nothing, so a project that marks a real function this way keeps it.
#[test]
fn an_ignored_declaration_is_still_the_answer_when_it_is_the_only_one() {
    check_index(
        r#"
//- /wp-admin/includes/noop.php
<?php
/**
 * @ignore
 */
function wp_guess_url() {}
"#,
        expect![[r#"
            state ready
            file wp-admin/includes/noop.php [User]
              function wp_guess_url [ignored] @ 5:10-5:22
              function wp_guess_url()
            catalog
              [function] wp_guess_url -> wp-admin/includes/noop.php (wp_guess_url)
            namespaces
            declared keys
            dispatched keys
        "#]],
    );
}

/// The tag inside an `@code` example does not mark the declaration.
///
/// §8 records a text scan reading Drupal's own documentation of a syntax as an instance of it; mago
/// folds an `@code` block into an element of its own, so parsing rather than scanning is what makes
/// this free instead of careful.
#[test]
fn a_documented_ignore_is_not_an_ignored_declaration() {
    check_index(
        r#"
//- /wp-includes/docs.php
<?php
/**
 * How to hide a function from the documentation.
 *
 * @code
 * @ignore
 * @endcode
 */
function explain_ignore() {}
"#,
        expect![[r#"
            state ready
            file wp-includes/docs.php [User]
              function explain_ignore @ 9:10-9:24
              function explain_ignore()
            catalog
              [function] explain_ignore -> wp-includes/docs.php (explain_ignore)
            namespaces
            declared keys
            dispatched keys
        "#]],
    );
}
