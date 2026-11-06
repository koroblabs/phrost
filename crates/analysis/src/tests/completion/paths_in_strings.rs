//! A class or function written as its path inside a string.
//!
//! `$factory->createNamed('name', 'Symfony\Component\Form\Extension\Core\Type\TextType')` -- how
//! Symfony's form tests name every type -- and `class_exists('App\Repo')`. Goto followed the path
//! and completion offered nothing, since a string offers only catalog and documented keys. A path
//! from the opening quote with a separator past its first segment is offered what a `use` line
//! would be: a string's name is read from the root, whatever the file imports.

use expect_test::expect;

use crate::tests::{Query, check};

const DECLARED: &str = r#"
//- /src/Form/TextType.php
<?php
namespace App\Form;

class TextType {}

//- /src/Form/Extension/Core.php
<?php
namespace App\Form\Extension;

class Core {}
"#;

fn check_declared(client: &str, queries: &[Query], expect: expect_test::Expect) {
    check(&format!("{DECLARED}\n{client}"), queries, expect);
}

#[test]
fn a_path_in_a_string_goes_on_into_its_namespace() {
    check_declared(
        r#"
//- /src/Test.php
<?php
namespace Tests;

function build(object $factory): void
{
    $factory->createNamed('name', 'App\Form\Te@here@');
}
"#,
        &[Query::completion("a path", "here")],
        expect![[r#"
            a path
            - App\Form\TextType [class]
            - ... and the client must ask again
            - accepting the first leaves `$factory->createNamed('name', 'App\Form\TextType');`
        "#]],
    );
}

#[test]
fn a_leading_separator_stays() {
    check_declared(
        r#"
//- /src/Test.php
<?php
namespace Tests;

function exists(): bool
{
    return class_exists('\App\Form\Te@here@');
}
"#,
        &[Query::completion("anchored", "here")],
        expect![[r#"
            anchored
            - App\Form\TextType [class]  insert `\App\Form\TextType`
            - ... and the client must ask again
            - accepting the first leaves `return class_exists('\App\Form\TextType');`
        "#]],
    );
}

#[test]
fn a_sentence_or_a_bare_word_is_not_a_path() {
    check_declared(
        r#"
//- /src/Test.php
<?php
namespace Tests;

function words(): array
{
    return ['see App\Form\Te@sentence@', 'Tex@bare@'];
}
"#,
        &[
            Query::completion("a path inside a sentence", "sentence"),
            Query::completion("a word with no separator", "bare"),
        ],
        expect![[r#"
            a path inside a sentence
            - (nothing offered)
            a word with no separator
            - (nothing offered)
        "#]],
    );
}
