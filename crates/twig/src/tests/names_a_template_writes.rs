//! Every construct that hands the loader a template's name.

use expect_test::expect;

use super::render_names;

#[test]
fn each_tag_that_loads_a_template() {
    let text = r#"{% extends '@classy/layout/page.html.twig' %}
{% include "block.html.twig" %}
{%- embed '@olivero/card.html.twig' with {title: 'x'} only -%}{% endembed %}
{% import 'macros.twig' as forms %}
{% from 'macros.twig' import input as field %}
{% use 'blocks.html.twig' with sidebar as base_sidebar %}
"#;
    expect![[r#"
        Extends "@classy/layout/page.html.twig" at 12..41
        Include "block.html.twig" at 58..73
        Embed "@olivero/card.html.twig" at 89..112
        Import "macros.twig" at 166..177
        From "macros.twig" at 200..211
        Use "blocks.html.twig" at 246..262
    "#]]
    .assert_eq(&render_names(text));
}

#[test]
fn each_function_that_takes_a_template() {
    let text = r#"{{ include('@mine/parts/thumb.html.twig', {src: file}) }}
{% set svg = source('icons/logo.svg') %}
{{ block('title', 'common.html.twig') }}
{{ block('title') }}
"#;
    expect![[r#"
        IncludeFunction "@mine/parts/thumb.html.twig" at 12..39
        SourceFunction "icons/logo.svg" at 79..93
        BlockFunction "common.html.twig" at 118..134
    "#]]
    .assert_eq(&render_names(text));
}

#[test]
fn a_template_allowed_to_be_missing_says_so() {
    let text = r#"{% include 'sidebar.html.twig' ignore missing with {a: 1} %}
{{ include('maybe.html.twig', ignore_missing = true) }}
{{ include('also.html.twig', {}, true, true) }}
{{ source('raw.txt', true) }}
{{ include('required.html.twig', ignore_missing = false) }}
"#;
    expect![[r#"
        Include "sidebar.html.twig" at 12..29 (may be missing)
        IncludeFunction "maybe.html.twig" at 73..88 (may be missing)
        IncludeFunction "also.html.twig" at 129..143 (may be missing)
        SourceFunction "raw.txt" at 176..183 (may be missing)
        IncludeFunction "required.html.twig" at 207..225
    "#]]
    .assert_eq(&render_names(text));
}

/// A list is the loader trying each in turn, so any one of them missing is allowed; a condition
/// renders one arm or the other, and each must exist.
#[test]
fn candidates_and_conditions_name_each_template() {
    let text = r#"{% include ['special.html.twig', 'default.html.twig'] %}
{% extends request.ajax ? 'ajax.html.twig' : 'page.html.twig' %}
{% include (['only.html.twig']) %}
"#;
    expect![[r#"
        Include "special.html.twig" at 13..30 (may be missing)
        Include "default.html.twig" at 34..51 (may be missing)
        Extends "ajax.html.twig" at 84..98
        Extends "page.html.twig" at 103..117
        Include "only.html.twig" at 136..150
    "#]]
    .assert_eq(&render_names(text));
}

/// The name being typed has no closing quote yet and no closer after it, and it is the one a
/// completion is asked about.
#[test]
fn a_name_still_being_typed_runs_to_the_end_of_the_text() {
    let text = "<div>\n{% include '@mine/par";
    expect![[r#"
        Include "@mine/par" at 18..27
    "#]]
    .assert_eq(&render_names(text));
}
