//! What is offered outside PHP's tags, which is a template and not code.
//!
//! Measured before this existed: `<?php` half typed at the top of a new file offered **200**
//! items, every one of them a `PHP_*` constant matched against the word `php`. A person opening a
//! file and pressing return got a constant.

use expect_test::expect;

use crate::tests::{Query, check};

/// The tag being typed offers the tag, and replaces what has been typed of it.
///
/// The replacement is the half worth pinning: the range begins at the `<`, so accepting leaves
/// `<?php` rather than `<?<?php`.
#[test]
fn the_opening_tag_is_what_is_offered_while_it_is_being_typed() {
    check(
        r#"
//- /src/Page.php
<?p@here@
"#,
        &[Query::completion("the tag", "here")],
        expect![[r#"
            the tag
            - <?php [keyword]
            - accepting the first leaves `<?php`
        "#]],
    );
}

/// A template's prose is prose. The client's own word completion is a better answer there than
/// anything this crate knows, and a list of class names over a paragraph hides it.
#[test]
fn nothing_is_offered_in_a_templates_text() {
    check(
        r#"
//- /src/Page.php
<html><body>Wel@here@come</body></html>
"#,
        &[Query::completion("in the prose", "here")],
        expect![[r#"
            in the prose
            - (nothing offered)
        "#]],
    );
}

/// After the tag, PHP is what is written, so the names come back.
///
/// The other side of the gate: a rule that refused everything outside a *complete* file would
/// take completion away from every `.module` in a Drupal project.
#[test]
fn names_come_back_after_the_tag_closes_and_opens_again() {
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repo {}

//- /src/Page.php
<?php $x = 1; ?>
<p>text</p>
<?php
use App\Repo;
$repo = new Rep@here@
"#,
        &[Query::completion("after the second tag", "here")],
        expect![[r#"
            after the second tag
            - Repo [class]  App\Repo
            - ... and the client must ask again
            - accepting the first leaves `$repo = new Repo`
        "#]],
    );
}
