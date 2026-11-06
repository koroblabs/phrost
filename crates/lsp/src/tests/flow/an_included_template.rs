//! A template an include renders has every variable of the template including it.
//!
//! Blade compiles `@include('pages.form')` to a render handed `get_defined_vars()` beside its array,
//! so a partial reads `$page` that nothing names to it -- what the including template was handed,
//! or bound itself, where the include is. `@component` hands over only its array.

use expect_test::expect;
use serde_json::json;

use crate::tests::{Fixture, definition_target};

const PROJECT: &str = r#"
//- /composer.json
{"type": "project", "autoload": {"psr-4": {"App\\": "app/"}}}

//- /artisan
#!/usr/bin/env php

//- /bootstrap/app.php
<?php

//- /app/Models/Page.php
<?php
namespace App\Models;

class Page
{
    public string $title = '';

    /** @return list<Tag> */
    public function tags(): array {}
}

//- /app/Models/Tag.php
<?php
namespace App\Models;

class Tag
{
    public string $name = '';
}

//- /app/Http/PageController.php
<?php
namespace App\Http;

use App\Models\Page;

class PageController
{
    public function edit(Page $page)
    {
        return view('pages.edit', ['page' => $page]);
    }
}

//- /resources/views/pages/edit.blade.php
<div>
    @include('pages.form', ['model' => $page])
    @component('pages.card')
    @endcomponent
</div>

//- /resources/views/pages/form.blade.php
<form>
    @foreach ($model->tags() as $tag)
        @include('pages.tag')
    @endforeach
    @include('pages.toolbox')
</form>

//- /resources/views/pages/toolbox.blade.php
<aside>{{ $pa@page@ge->title }} {{ $mo@model@del->title }}</aside>
<p>{{ $@typing@ }}</p>

//- /resources/views/pages/tag.blade.php
<span>{{ $ta@tag@g->name }}</span>

//- /resources/views/pages/card.blade.php
<div>{{ $pa@card@ge }}</div>
"#;

#[tokio::test]
async fn an_included_template_reads_the_including_ones_variables() {
    let toolbox = "resources/views/pages/toolbox.blade.php";
    let tag = "resources/views/pages/tag.blade.php";
    let card = "resources/views/pages/card.blade.php";
    let mut server = Fixture::started(PROJECT, toolbox).await;
    server.open(tag).await;
    server.open(card).await;
    server.settle().await;

    let says = |response: &lspkit_runtime::protocol::jsonrpc::Response, word: &str| {
        serde_json::to_string(&response.result())
            .unwrap_or_default()
            .contains(word)
    };
    // Two includes up, through one that names it to nothing: the controller's.
    let page = server.hover(toolbox, "page").await;
    let page_declared = server.definition(toolbox, "page").await;
    // What the include one level up handed under another name.
    let model = server.hover(toolbox, "model").await;
    // What the including template bound where the include is.
    let tagged = server.hover(tag, "tag").await;
    let tag_declared = server.definition(tag, "tag").await;
    // A component is handed its array and nothing else.
    let carded = server.hover(card, "card").await;
    let root = server.project.root().to_path_buf();

    expect![[r#"
        (
            true,
            Some(
                "app/Http/PageController.php",
            ),
            true,
            true,
            Some(
                "resources/views/pages/form.blade.php",
            ),
            false,
        )
    "#]]
    .assert_debug_eq(&(
        says(&page, "Page"),
        definition_target(&page_declared, &root),
        says(&model, "Page"),
        says(&tagged, "Tag"),
        definition_target(&tag_declared, &root),
        says(&carded, "Page"),
    ));
}

#[tokio::test]
async fn an_included_template_offers_the_including_ones_variables() {
    let toolbox = "resources/views/pages/toolbox.blade.php";
    let mut server = Fixture::started(PROJECT, toolbox).await;
    server.settle().await;

    let params = json!({
        "textDocument": { "uri": server.uri(toolbox) },
        "position": server.position(toolbox, "typing"),
    });
    let offered = server.request("textDocument/completion", params).await;
    let labels = serde_json::to_value(offered.result())
        .ok()
        .and_then(|result| {
            Some(
                result["items"]
                    .as_array()?
                    .iter()
                    .filter_map(|item| item["label"].as_str().map(str::to_owned))
                    .filter(|label| !label.starts_with("$_") && label != "$GLOBALS")
                    .collect::<Vec<_>>(),
            )
        })
        .unwrap_or_default();

    expect![[r#"
        [
            "$tag",
            "$model",
            "$page",
            "$errors",
        ]
    "#]]
    .assert_debug_eq(&labels);
}
