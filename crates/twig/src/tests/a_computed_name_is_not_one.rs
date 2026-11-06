//! A literal that is part of a computed name is not a name, and neither is one in markup.

use expect_test::expect;

use super::render_names;

#[test]
fn a_literal_inside_a_computation_names_nothing() {
    let text = r##"{% include 'cards/' ~ kind ~ '.html.twig' %}
{% include section %}
{% include "#{theme}/page.html.twig" %}
{% extends active_theme_path() ~ '/templates/page.html.twig' %}
{{ source(componentMetadata.path ~ '/icon.svg') }}
{% include ('a' ~ 'b') %}
{% include x ? 'a' ~ y : 'b.html.twig' %}
{% import _self as forms %}
"##;
    expect![[r#"
        Include "b.html.twig" at 274..285
    "#]]
    .assert_eq(&render_names(text));
}

#[test]
fn what_is_not_a_tag_names_nothing() {
    let text = r#"<p>{% include 'x' %} is how you include</p>
{# {% include 'commented.html.twig' %} #}
{% verbatim %}{% include 'shown.html.twig' %}{% endverbatim %}
{{ loader.source('not-the-function.twig') }}
{{ 'include'|upper }}
{% set include = 'not.html.twig' %}
"#;
    expect![[r#"
        Include "x" at 15..16
    "#]]
    .assert_eq(&render_names(text));
}
