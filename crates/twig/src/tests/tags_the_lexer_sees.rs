//! What the lexer makes of a tag: Twig's own rules for where a tag ends and a string closes.

use expect_test::expect;

use super::render_tokens;

/// A tag closes at its closer only once every bracket inside it has closed.
#[test]
fn a_hash_ending_where_the_tag_does_is_still_a_hash() {
    expect![[r#"
        PrintOpen "{{"
        Punctuation "{"
        Name "a"
        Punctuation ":"
        Number "1"
        Punctuation "}"
        Close "}}"
    "#]]
    .assert_eq(&render_tokens("{{ {a: 1}}}"));
}

#[test]
fn whitespace_control_belongs_to_the_delimiters() {
    expect![[r#"
        StatementOpen "{%-"
        Name "if"
        Name "x"
        Close "~%}"
        PrintOpen "{{~"
        Name "y"
        Operator "-"
        Number "1"
        Close "-}}"
    "#]]
    .assert_eq(&render_tokens("{%- if x ~%}{{~ y - 1 -}}"));
}

/// The braces of an interpolation are its own, and so are the quotes inside it.
#[test]
fn an_interpolation_holds_an_expression_of_its_own() {
    expect![[r#"
        PrintOpen "{{"
        String interpolated "\"a #{ {b: \"}\"}.b } c\""
        Punctuation "|"
        Name "upper"
        Close "}}"
    "#]]
    .assert_eq(&render_tokens(r#"{{ "a #{ {b: "}"}.b } c"|upper }}"#));
}

#[test]
fn operators_take_the_longest_spelling() {
    expect![[r#"
        PrintOpen "{{"
        Number "1"
        Operator ".."
        Number "5"
        Punctuation ","
        Name "a"
        Operator "??"
        Name "b"
        Punctuation ","
        Name "c"
        Operator "?."
        Name "d"
        Punctuation ","
        Number "3.14"
        Punctuation ","
        Number "1_000"
        Close "}}"
    "#]]
    .assert_eq(&render_tokens("{{ 1..5, a ?? b, c?.d, 3.14, 1_000 }}"));
}

#[test]
fn a_comment_is_one_token_and_a_verbatim_block_is_markup() {
    expect![[r#"
        Comment "{# {{ no }} #}"
        PrintOpen "{{"
        Name "after"
        Close "}}"
    "#]]
    .assert_eq(&render_tokens(
        "{# {{ no }} #}{% verbatim %}{{ nor }}{%- endverbatim -%}{% raw %}{{ this }}{% endraw %}{{ after }}",
    ));
}

#[test]
fn a_multibyte_name_is_one_token() {
    expect![[r#"
        PrintOpen "{{"
        Name "café"
        Punctuation "."
        Name "naïve"
        Close "}}"
    "#]]
    .assert_eq(&render_tokens("{{ café.naïve }}"));
}
