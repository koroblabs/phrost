//! A Twig template, driven through a real server: the names it loads other templates by.
//!
//! A `.html.twig` buffer is held as text the way a configuration buffer is, and answered from the
//! catalogs Drupal fills -- so these are the wiring: that the buffer is held at all once a framework
//! says it renders Twig, that a position in it reaches the template catalog, and that a misspelt
//! name is published on the schedule a PHP buffer's findings are.

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
//- /web/modules/mine/src/Repo.php
<?php
namespace Drupal\mine;

class Repo {}
//- /web/modules/mine/templates/parts/thumb.html.twig
<img src="{{ src }}">
//- /web/modules/mine/templates/parts/badge.html.twig
<span>{{ label }}</span>
//- /web/core/modules/system/system.info.yml
name: System
//- /web/core/modules/system/templates/broken.html.twig
{% include '@system/gone.html.twig' %}
//- /web/modules/mine/templates/page.html.twig
<div>
  {% include '@mine/parts/th@here@umb.html.twig' with {src: file} %}
  {{ include('@mine/parts/b@typed@') }}
</div>
"#;

const PAGE: &str = "web/modules/mine/templates/page.html.twig";

#[tokio::test]
async fn a_name_goes_to_the_template_it_names() {
    let mut server = Fixture::started(PROJECT, "web/modules/mine/src/Repo.php").await;
    server.open(PAGE).await;
    server.settle().await;

    let response = server.definition(PAGE, "here").await;
    expect![[r#"
        Some(
            "web/modules/mine/templates/parts/thumb.html.twig",
        )
    "#]]
    .assert_debug_eq(&definition_target(&response, server.project.root()));
}

#[tokio::test]
async fn a_name_being_written_is_offered_the_templates_there_are() {
    let mut server = Fixture::started(PROJECT, "web/modules/mine/src/Repo.php").await;
    server.open(PAGE).await;
    server.settle().await;

    let position = server.position(PAGE, "typed");
    let response = server
        .request(
            "textDocument/completion",
            json!({
                "textDocument": { "uri": server.uri(PAGE) },
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
        .map(|item| item["label"].as_str().unwrap_or("?").to_owned())
        .collect::<Vec<_>>();
    expect![[r#"
        [
            "@mine/parts/badge.html.twig",
            "@mine/parts/thumb.html.twig",
        ]
    "#]]
    .assert_debug_eq(&labels);
}

#[tokio::test(start_paused = true)]
async fn a_misspelt_name_is_published_as_the_buffer_settles() {
    let mut server = Fixture::started(PROJECT, "web/modules/mine/src/Repo.php").await;
    server.open(PAGE).await;
    server.settle().await;

    server
        .change(
            PAGE,
            2,
            "<div>\n  {% include '@mine/parts/thum.html.twig' %}\n  {% include '@mine/parts/gone.html.twig' %}\n</div>\n",
        )
        .await;
    server.wait_out_the_quiet().await;

    let published = server.diagnostics_published();
    expect![[r#"
        [
            "page.html.twig: none",
            "page.html.twig:2: unknown-catalog-key Nothing registers the template `@mine/parts/thum.html.twig`. Did you mean `@mine/parts/thumb.html.twig`?\npage.html.twig:3: unknown-catalog-key Nothing registers the template `@mine/parts/gone.html.twig`",
        ]
    "#]]
    .assert_debug_eq(
        &published
            .iter()
            .filter(|line| line.starts_with("page.html.twig"))
            .collect::<Vec<_>>(),
    );
}

/// Core's templates are read, not checked: a name missing there is not the reader's to fix.
#[tokio::test(start_paused = true)]
async fn a_dependency_template_is_not_checked() {
    let mut server = Fixture::started(PROJECT, "web/modules/mine/src/Repo.php").await;
    server
        .open("web/core/modules/system/templates/broken.html.twig")
        .await;
    server.settle().await;

    let published = server.diagnostics_published();
    expect![[r#"
        [
            "broken.html.twig: none",
        ]
    "#]]
    .assert_debug_eq(
        &published
            .iter()
            .filter(|line| line.starts_with("broken.html.twig"))
            .collect::<Vec<_>>(),
    );
}
