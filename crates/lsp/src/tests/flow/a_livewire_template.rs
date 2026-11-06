//! A Livewire component's template, whose `$this` is the component.
//!
//! Livewire renders a component's template inside a closure bound to the component, so `$this` in
//! it is the instance and every public property the component declares is a variable. The class is
//! named by the `view()` its `render()` returns, or -- with no `render()` -- by the template's own
//! name under `App\Livewire`. And a `<livewire:...>` tag hands the template what it binds, as a
//! component tag does.

use expect_test::expect;
use serde_json::json;

use crate::tests::{Fixture, definition_target};

const PROJECT: &str = r#"
//- /composer.json
{"type": "project", "autoload": {"psr-4": {"App\\": "app/", "Livewire\\": "lib/livewire/"}}}

//- /artisan
#!/usr/bin/env php

//- /bootstrap/app.php
<?php

//- /lib/livewire/Component.php
<?php
namespace Livewire;

abstract class Component
{
    public $id;
}

//- /app/Models/Post.php
<?php
namespace App\Models;

class Post
{
    public string $title = '';
}

//- /app/Livewire/ShowPost.php
<?php
namespace App\Livewire;

use App\Models\Post;
use Livewire\Component;

class ShowPost extends Component
{
    public Post $post;

    public $likes;

    public function render()
    {
        return view('livewire.single-post');
    }
}

//- /app/Livewire/Counter.php
<?php
namespace App\Livewire;

use Livewire\Component;

class Counter extends Component
{
    public int $count = 0;

    protected int $step = 1;

    public function increment(): void {}
}

//- /resources/views/livewire/single-post.blade.php
<h1>{{ $this->post->ti@title@tle }}</h1>
<p>{{ $po@post@st->title }}</p>
<p>{{ $lik@likes@es->title }}</p>

//- /resources/views/livewire/counter.blade.php
<button>{{ $this->incr@increment@ement() }} {{ $this->st@step@ep }}</button>
<p>{{ $@typing@ }}</p>

//- /resources/views/posts/show.blade.php
{{-- @var \App\Models\Post $post --}}
<livewire:single-post :post="$post" :likes="$post" />
"#;

#[tokio::test]
async fn a_components_template_reads_the_component() {
    let mut server =
        Fixture::started(PROJECT, "resources/views/livewire/single-post.blade.php").await;
    server
        .open("resources/views/livewire/counter.blade.php")
        .await;
    server.settle().await;

    let single = "resources/views/livewire/single-post.blade.php";
    let counter = "resources/views/livewire/counter.blade.php";
    let title = server.definition(single, "title").await;
    let post = server.hover(single, "post").await;
    let post_declared = server.definition(single, "post").await;
    // An untyped property is what the tag rendering the component hands it.
    let likes = server.hover(single, "likes").await;
    // The class with no `render()` renders the template its name spells, and the template is in its
    // scope: a protected member is as reachable as in a method.
    let increment = server.definition(counter, "increment").await;
    let step = server.definition(counter, "step").await;
    let root = server.project.root().to_path_buf();
    let says = |response: &lspkit_runtime::protocol::jsonrpc::Response, word: &str| {
        serde_json::to_string(&response.result())
            .unwrap_or_default()
            .contains(word)
    };

    expect![[r#"
        (
            Some(
                "app/Models/Post.php",
            ),
            true,
            Some(
                "app/Livewire/ShowPost.php",
            ),
            true,
            Some(
                "app/Livewire/Counter.php",
            ),
            Some(
                "app/Livewire/Counter.php",
            ),
        )
    "#]]
    .assert_debug_eq(&(
        definition_target(&title, &root),
        says(&post, "Post"),
        definition_target(&post_declared, &root),
        says(&likes, "Post"),
        definition_target(&increment, &root),
        definition_target(&step, &root),
    ));
}

/// What a component's template offers: its public properties and `$this` -- not what the framework's
/// own class declares, and not a protected property, which Livewire does not hand over.
#[tokio::test]
async fn a_components_template_offers_its_properties() {
    let mut server = Fixture::started(PROJECT, "resources/views/livewire/counter.blade.php").await;
    server.settle().await;

    let params = json!({
        "textDocument": { "uri": server.uri("resources/views/livewire/counter.blade.php") },
        "position": server.position("resources/views/livewire/counter.blade.php", "typing"),
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
            "$this",
            "$errors",
            "$count",
        ]
    "#]]
    .assert_debug_eq(&labels);
}
