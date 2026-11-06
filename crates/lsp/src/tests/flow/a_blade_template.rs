//! A Blade template, read as the PHP its expressions are.
//!
//! The Laravel provider says a `*.blade.php` file is a template and how to read it: every
//! expression keeps its bytes, and everything around it is blanked -- so a position in the buffer is
//! the same position in what is analysed, and goto, hover and the checks answer inside `{{ }}` and a
//! directive's condition as they would in a class. What is *not* answered is anything that would
//! rewrite or paint the buffer, which holds markup, and any finding about what the reader wrote
//! rather than the person: the calls a tag and an include become, and the PHP a construct could not
//! be written as in place.

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

//- /app/Models/Post.php
<?php
namespace App\Models;

class Post
{
    public string $title = '';

    public function author(): User {}
}

//- /app/Models/User.php
<?php
namespace App\Models;

class User
{
    public string $name = '';
}

//- /resources/views/posts/show.blade.php
{{-- @var \App\Models\Post $post --}}
<h1>{{ $post->ti@title@tle }}</h1>
@if ($po@cond@st)
    <p>Привіт, {{ $post->author()->na@name@me }}</p>
@endif
{{ $post->subtitle }}
<x-alert type="info" />
@error('title')
@include('posts.row')"#;

const TEMPLATE: &str = "resources/views/posts/show.blade.php";

#[tokio::test]
async fn goto_and_hover_answer_inside_a_template() {
    let mut server = Fixture::started(PROJECT, TEMPLATE).await;
    server.settle().await;

    let title = server.definition(TEMPLATE, "title").await;
    let name = server.definition(TEMPLATE, "name").await;
    let hover = server.hover(TEMPLATE, "cond").await;
    let root = server.project.root().to_path_buf();

    expect![[r#"
        (
            Some(
                "app/Models/Post.php",
            ),
            Some(
                "app/Models/User.php",
            ),
            true,
        )
    "#]]
    .assert_debug_eq(&(
        definition_target(&title, &root),
        definition_target(&name, &root),
        serde_json::to_string(&hover.result())
            .unwrap_or_default()
            .contains("Post"),
    ));
}

#[tokio::test]
async fn a_template_is_checked_and_not_formatted_or_painted() {
    let mut server = Fixture::started(PROJECT, TEMPLATE).await;
    server.settle().await;

    let formatted = server
        .request(
            "textDocument/formatting",
            json!({
                "textDocument": { "uri": server.uri(TEMPLATE) },
                "options": { "tabSize": 4, "insertSpaces": true },
            }),
        )
        .await;
    let tokens = server.semantic_tokens(TEMPLATE).await;
    let checked = server
        .diagnostics_published()
        .into_iter()
        .filter(|line| line.starts_with("show.blade.php") && !line.ends_with(": none"))
        .collect::<Vec<_>>();

    expect![[r#"
        (
            "null",
            "null",
            [
                "show.blade.php:6: undefined-property Undefined property App\\Models\\Post::$subtitle\nshow.blade.php:8: unbalanced-directive `@error` is never closed",
                "show.blade.php:6: undefined-property Undefined property App\\Models\\Post::$subtitle\nshow.blade.php:8: unbalanced-directive `@error` is never closed",
            ],
        )
    "#]]
    .assert_debug_eq(&(
        serde_json::to_string(&formatted.result()).unwrap_or_default(),
        serde_json::to_string(&tokens.result()).unwrap_or_default(),
        checked,
    ));
}

/// A template's variables are what the calls rendering it hand it, read where they are written.
///
/// The controller renders `posts.index` with its `Post $post`; that template includes
/// `posts.row`, handing the same post on -- so a variable two files away from any declaration is a
/// `Post`, because a partial's variables are whatever the template including it was handed.
const RENDERED: &str = r#"
//- /composer.json
{"type": "project", "autoload": {"psr-4": {"App\\": "app/", "Illuminate\\": "vendor/illuminate/"}}}

//- /vendor/illuminate/Support/ViewErrorBag.php
<?php
namespace Illuminate\Support;

class ViewErrorBag
{
    public function any(): bool {}
}

//- /artisan
#!/usr/bin/env php

//- /bootstrap/app.php
<?php

//- /app/Models/Post.php
<?php
namespace App\Models;

class Post
{
    public string $title = '';
}

//- /app/Http/PostController.php
<?php
namespace App\Http;

use App\Models\Post;

class PostController
{
    public function index(Post $post)
    {
        return view('posts.in@named@dex', ['post' => $post]);
    }

    public function show(Post $post)
    {
        return view('posts.show', compact('post'));
    }
}

//- /resources/views/posts/index.blade.php
<div>
    <h1>{{ $po@page@st->title }}</h1>
    @include('posts.r@included@ow', ['row' => $post])
    <x-post-card :post="$post" />
    <p>{{ $po@after@st->title }}</p>
</div>

//- /resources/views/components/post-card.blade.php
@props(['post'])
<article {{ $attri@attrs@butes }}>{{ $po@carded@st->title }}</article>

//- /vendor/illuminate/View/ComponentAttributeBag.php
<?php
namespace Illuminate\View;

class ComponentAttributeBag {}

//- /resources/views/posts/show.blade.php
<h1>{{ $po@shown@st->title }}</h1>
<footer>{{ $si@site@te->name }}</footer>

//- /app/Providers/ViewServiceProvider.php
<?php
namespace App\Providers;

use App\Models\Site;
use Illuminate\Support\Facades\View;

class ViewServiceProvider
{
    public function boot(): void
    {
        View::share('site', new Site());
    }
}

//- /app/Models/Site.php
<?php
namespace App\Models;

class Site
{
    public string $name = '';
}

//- /resources/views/posts/empty.blade.php
<p>{{ $@typing@ }}</p>

//- /resources/views/posts/row.blade.php
<div>
    <p>{{ $r@row@ow->ti@title@tle }}</p>
    @if ($err@errors@ors->any())
        x
    @endif
</div>
"#;

#[tokio::test]
async fn a_variable_is_what_the_call_rendering_the_template_hands_it() {
    let mut server = Fixture::started(RENDERED, "resources/views/posts/index.blade.php").await;
    server.open("resources/views/posts/row.blade.php").await;
    server.settle().await;

    let page = server
        .hover("resources/views/posts/index.blade.php", "page")
        .await;
    let row = server
        .hover("resources/views/posts/row.blade.php", "row")
        .await;
    let title = server
        .definition("resources/views/posts/row.blade.php", "title")
        .await;
    let errors = server
        .hover("resources/views/posts/row.blade.php", "errors")
        .await;
    let page_declared = server
        .definition("resources/views/posts/index.blade.php", "page")
        .await;
    let row_declared = server
        .definition("resources/views/posts/row.blade.php", "row")
        .await;
    let root = server.project.root().to_path_buf();
    let says = |response: &lspkit_runtime::protocol::jsonrpc::Response, word: &str| {
        serde_json::to_string(&response.result())
            .unwrap_or_default()
            .contains(word)
    };

    expect![[r#"
        (
            true,
            true,
            Some(
                "app/Models/Post.php",
            ),
            true,
            Some(
                "app/Http/PostController.php",
            ),
            Some(
                "resources/views/posts/index.blade.php",
            ),
        )
    "#]]
    .assert_debug_eq(&(
        says(&page, "Post"),
        says(&row, "Post"),
        definition_target(&title, &root),
        says(&errors, "ViewErrorBag"),
        definition_target(&page_declared, &root),
        definition_target(&row_declared, &root),
    ));
}

/// A template's name goes to its file, from a controller's `view()` and a template's `@include`
/// alike, and the names that exist are offered while one is typed.
#[tokio::test]
async fn a_template_name_goes_to_its_file_and_is_offered() {
    let mut server = Fixture::started(RENDERED, "app/Http/PostController.php").await;
    server.open("resources/views/posts/index.blade.php").await;
    server.settle().await;

    let named = server
        .definition("app/Http/PostController.php", "named")
        .await;
    let included = server
        .definition("resources/views/posts/index.blade.php", "included")
        .await;
    let params = json!({
        "textDocument": { "uri": server.uri("app/Http/PostController.php") },
        "position": server.position("app/Http/PostController.php", "named"),
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
                    .collect::<Vec<_>>(),
            )
        })
        .unwrap_or_default();
    let root = server.project.root().to_path_buf();

    expect![[r#"
        (
            Some(
                "resources/views/posts/index.blade.php",
            ),
            Some(
                "resources/views/posts/row.blade.php",
            ),
            [
                "posts.index",
            ],
        )
    "#]]
    .assert_debug_eq(&(
        definition_target(&named, &root),
        definition_target(&included, &root),
        labels,
    ));
}

/// `compact('post')` hands the template the local of that name where the call is.
#[tokio::test]
async fn compact_hands_a_template_the_locals_it_names() {
    let mut server = Fixture::started(RENDERED, "resources/views/posts/show.blade.php").await;
    server.settle().await;

    let hover = server
        .hover("resources/views/posts/show.blade.php", "shown")
        .await;
    let declared = server
        .definition("resources/views/posts/show.blade.php", "shown")
        .await;
    let shared = server
        .hover("resources/views/posts/show.blade.php", "site")
        .await;
    let root = server.project.root().to_path_buf();

    expect![[r#"
        (
            true,
            Some(
                "app/Http/PostController.php",
            ),
            true,
        )
    "#]]
    .assert_debug_eq(&(
        serde_json::to_string(&hover.result())
            .unwrap_or_default()
            .contains("Post"),
        definition_target(&declared, &root),
        // And one every template has, which a service provider shared.
        serde_json::to_string(&shared.result())
            .unwrap_or_default()
            .contains("Site"),
    ));
}

/// A template's variables are offered as its locals are: the ones every template has here, since
/// nothing renders this one.
#[tokio::test]
async fn a_templates_variables_are_offered() {
    let mut server = Fixture::started(RENDERED, "resources/views/posts/empty.blade.php").await;
    server.settle().await;

    let params = json!({
        "textDocument": { "uri": server.uri("resources/views/posts/empty.blade.php") },
        "position": server.position("resources/views/posts/empty.blade.php", "typing"),
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
            "$site",
            "$errors",
        ]
    "#]]
    .assert_debug_eq(&labels);
}

/// A component's variables are the attributes the tag that uses it binds: `<x-post-card
/// :post="$post" />` hands `components/post-card.blade.php` its `$post` -- and passing `$post` to the
/// tag writes nothing back, so the variable after the tag is still the controller's.
#[tokio::test]
async fn a_component_reads_the_attributes_its_tag_binds() {
    let mut server =
        Fixture::started(RENDERED, "resources/views/components/post-card.blade.php").await;
    server.open("resources/views/posts/index.blade.php").await;
    server.settle().await;

    let carded = server
        .hover("resources/views/components/post-card.blade.php", "carded")
        .await;
    let declared = server
        .definition("resources/views/components/post-card.blade.php", "carded")
        .await;
    let after = server
        .hover("resources/views/posts/index.blade.php", "after")
        .await;
    let attributes = server
        .hover("resources/views/components/post-card.blade.php", "attrs")
        .await;
    let root = server.project.root().to_path_buf();
    let says = |response: &lspkit_runtime::protocol::jsonrpc::Response| {
        serde_json::to_string(&response.result())
            .unwrap_or_default()
            .contains("Post")
    };

    expect![[r#"
        (
            true,
            Some(
                "resources/views/posts/index.blade.php",
            ),
            true,
            true,
        )
    "#]]
    .assert_debug_eq(&(
        says(&carded),
        definition_target(&declared, &root),
        says(&after),
        // And what every component's template has: its attributes.
        serde_json::to_string(&attributes.result())
            .unwrap_or_default()
            .contains("ComponentAttributeBag"),
    ));
}

/// `view()->share('locale', ...)` in a middleware is the facade's `View::share` spelled through the
/// helper, and hands every template the variable the same way.
#[tokio::test]
async fn a_variable_shared_through_the_view_helper_is_every_templates() {
    let mut server = Fixture::started(
        r#"
//- /composer.json
{"type": "project", "autoload": {"psr-4": {"App\\": "app/"}}}

//- /artisan
#!/usr/bin/env php

//- /bootstrap/app.php
<?php

//- /app/Http/Middleware/Localization.php
<?php
namespace App\Http\Middleware;

use App\Locale;

class Localization
{
    public function handle(): void
    {
        view()->share('locale', new Locale());
    }
}

//- /app/Locale.php
<?php
namespace App;

class Locale
{
    public string $code = '';
}

//- /resources/views/home.blade.php
<html lang="{{ $lo@locale@cale->code }}"></html>
"#,
        "resources/views/home.blade.php",
    )
    .await;
    server.settle().await;

    let hover = server
        .hover("resources/views/home.blade.php", "locale")
        .await;
    expect![[r#"
        true
    "#]]
    .assert_debug_eq(
        &serde_json::to_string(&hover.result())
            .unwrap_or_default()
            .contains("Locale"),
    );
}
