//! A Twig template's variables, driven through a real server: what the render array handing them
//! over says they are, and what an attribute read off one is.
//!
//! The wiring this owns: a template buffer held as text has the file rendering it lowered beside it
//! once it opens, so the value in that file's body can be read -- and a request on a variable or an
//! attribute reaches the typer where it is on no template name.

use expect_test::expect;
use serde_json::json;

use crate::tests::{Fixture, definition_target};

const PROJECT: &str = r#"
//- /composer.json
{}
//- /web/core/lib/Drupal.php
<?php class Drupal {}
//- /web/modules/mine/mine.info.yml
name: Mine
//- /web/modules/mine/src/Node.php
<?php
namespace Drupal\mine;

class Node
{
    public function getLabel(): string { return ''; }
}
//- /web/modules/mine/src/Certificate.php
<?php
namespace Drupal\mine;

class Certificate
{
    public function build(Node $node): array
    {
        return [
            '#theme' => 'mine_certificate',
            '#node' => $node,
            '#data' => ['supplier' => 'x'],
        ];
    }
}
//- /web/modules/mine/templates/mine-certificate.html.twig
<h1>{{ node.lab@label@el }}</h1>
<p>{{ da@data@ta.supplier }}</p>
<p>{{ node.@typed@ }}</p>
{% include '@mine/parts/badge.html.twig' with {'subject': node} %}
//- /web/modules/mine/templates/parts/badge.html.twig
<span>{{ subj@subject@ect.label }}</span>
"#;

const PART: &str = "web/modules/mine/templates/parts/badge.html.twig";

const TEMPLATE: &str = "web/modules/mine/templates/mine-certificate.html.twig";

#[tokio::test]
async fn an_attribute_goes_to_the_getter_twig_reads() {
    let mut server = Fixture::started(PROJECT, "web/modules/mine/src/Node.php").await;
    server.open(TEMPLATE).await;
    server.settle().await;

    let response = server.definition(TEMPLATE, "label").await;
    expect![[r#"
        Some(
            "web/modules/mine/src/Node.php",
        )
    "#]]
    .assert_debug_eq(&definition_target(&response, server.project.root()));
}

#[tokio::test]
async fn a_variable_says_what_the_render_array_handed_over() {
    let mut server = Fixture::started(PROJECT, "web/modules/mine/src/Node.php").await;
    server.open(TEMPLATE).await;
    server.settle().await;

    let response = server.hover(TEMPLATE, "data").await;
    let result = serde_json::to_value(response.result()).expect("serialisable");
    expect![[r#"
        ```php
        array{supplier: non-falsy-string} $data
        ```

        Handed over in `Certificate.php:11`."#]]
    .assert_eq(result["contents"]["value"].as_str().unwrap_or("(nothing)"));
}

#[tokio::test]
async fn an_attribute_being_written_is_offered_the_getters() {
    let mut server = Fixture::started(PROJECT, "web/modules/mine/src/Node.php").await;
    server.open(TEMPLATE).await;
    server.settle().await;

    let position = server.position(TEMPLATE, "typed");
    let response = server
        .request(
            "textDocument/completion",
            json!({
                "textDocument": { "uri": server.uri(TEMPLATE) },
                "position": position,
            }),
        )
        .await;
    let result = serde_json::to_value(response.result().expect("completion answered"))
        .expect("serialisable");
    let labels = result["items"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|item| {
            format!(
                "{} {}",
                item["label"].as_str().unwrap_or("?"),
                item["detail"].as_str().unwrap_or("")
            )
        })
        .collect::<Vec<_>>();
    expect![[r#"
        [
            "label string",
        ]
    "#]]
    .assert_debug_eq(&labels);
}

/// A part is handed its variables by the template including it, which is read from disk beside it
/// with what renders that template in turn.
#[tokio::test]
async fn a_part_says_which_template_handed_its_variable_over() {
    let mut server = Fixture::started(PROJECT, "web/modules/mine/src/Node.php").await;
    server.open(PART).await;
    server.settle().await;

    let response = server.hover(PART, "subject").await;
    let result = serde_json::to_value(response.result()).expect("serialisable");
    expect![[r#"
        ```php
        Node $subject
        ```

        Handed over by `mine-certificate.html.twig:4`."#]]
    .assert_eq(result["contents"]["value"].as_str().unwrap_or("(nothing)"));

    let response = server.definition(PART, "subject").await;
    expect![[r#"
        Some(
            "web/modules/mine/templates/mine-certificate.html.twig",
        )
    "#]]
    .assert_debug_eq(&definition_target(&response, server.project.root()));
}
