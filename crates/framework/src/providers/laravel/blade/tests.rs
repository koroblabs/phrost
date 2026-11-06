//! Each construct as the PHP it becomes, a blank shown as `·`.
//!
//! Every case also holds the two properties the whole design rests on: the PHP is exactly as long
//! as the template, and every newline is where it was -- so an offset and a line mean the same
//! thing in both.

use expect_test::{Expect, expect};

use super::to_php;

fn check(template: &str, expect: Expect) {
    let (php, failures) = to_php(template);
    assert_eq!(
        php.len(),
        template.len(),
        "the PHP is as long as the template"
    );
    let newlines =
        |text: &str| -> Vec<usize> { text.match_indices('\n').map(|(at, _)| at).collect() };
    assert_eq!(
        newlines(&php),
        newlines(template),
        "every newline stays where it was"
    );
    let mut shown = php.replace(' ', "·");
    if failures > 0 {
        shown.push_str(&format!("\n[{failures} not written in place]"));
    }
    expect.assert_eq(&shown);
}

#[test]
fn an_echo_is_its_expression() {
    check(
        "<p>{{ $user->name }}</p>\n<p>{!! $html !!}</p>",
        expect![[r#"
            <?php·$user->name·;·····
            ·······$html·;······"#]],
    );
}

#[test]
fn a_condition_keeps_its_expression_and_opens_a_block() {
    check(
        "<div>\n@if ($user)\n  {{ $user->name }}\n@elseif ($guest)\n  x\n@else\n  y\n@endif\n</div>",
        expect![[r#"
            <?···
            ··if($user)
            {····$user->name·;·
            ·}elseif($guest)
            {··
            }else
            {··
            ·····}
            ······"#]],
    );
}

#[test]
fn a_loop_binds_its_names() {
    check(
        "<ul>\n@foreach ($posts as $post)\n<li>{{ $post->title }}</li>\n@endforeach\n</ul>",
        expect![[r#"
            <?··
            ··foreach($posts·as·$post)
            {······$post->title·;······
            ··········}
            ·····"#]],
    );
}

#[test]
fn forelse_has_a_block_for_an_empty_list() {
    check(
        "<ul>\n@forelse ($posts as $post)\n<li>{{ $post }}</li>\n@empty\n<li>none</li>\n@endforelse\n</ul>",
        expect![[r#"
            <?··
            ··foreach($posts·as·$post)
            {······$post·;······
            ····}{
            ·············
            ··········}
            ·····"#]],
    );
}

#[test]
fn a_directive_with_arguments_is_a_list_of_them() {
    check(
        "<html>\n@extends('layout', ['post' => $post])\n@csrf\n</html>",
        expect![[r#"
            <?php·
            ········['layout',·['post'·=>·$post]]
            ;····
            ·······"#]],
    );
}

#[test]
fn a_block_whose_arguments_do_not_narrow() {
    check(
        "<div>\n@isset($user)\n{{ $user }}\n@endisset\n@auth\nin\n@endauth\n</div>",
        expect![[r#"
            <?···
            ···if([$user]
            ){·$user·;·
            ········}
            if(1)
            {·
            ·······}
            ······"#]],
    );
}

#[test]
fn raw_php_is_kept_and_its_end_ends_a_statement() {
    check(
        "<div>\n@php\n    $total = count($items)\n@endphp\n{{ $total }}\n</div>",
        expect![[r#"
            <?···
            ····
            ····$total·=·count($items)
            ;······
            ···$total·;·
            ······"#]],
    );
}

#[test]
fn a_blade_comment_is_a_docblock_and_can_carry_the_opening_tag() {
    check(
        "{{-- @var \\App\\User $user --}}\n<p>{{ $user->name }}</p>",
        expect![[r#"
            <?/**·@var·\App\User·$user·*/·
            ······$user->name·;·····"#]],
    );
}

#[test]
fn markup_outside_ascii_is_kept_in_a_comment_so_its_columns_stay() {
    check(
        "<html>\n<p>Привіт, {{ $name }}</p>\n</html>",
        expect![[r#"
            <?php·
            ·/*Привіт*/···$name·;·····
            ·······"#]],
    );
}

#[test]
fn what_blade_does_not_read_stays_markup() {
    check(
        "<style>@media (max-width: 600px) {}</style>\n<a href=\"mailto:a@b.com\" @click=\"go\">@@if @{{ raw }}</a>\n{{ $x }}",
        expect![[r#"
            <?php······································
            ························································
            ···$x·;·"#]],
    );
}

#[test]
fn a_switch_is_a_switch() {
    check(
        "<div>\n@switch($kind)\n@case(1)\none\n@break\n@default\nother\n@endswitch\n</div>",
        expect![[r#"
            <?···
            ·switch($kind)
            {case(1)
            :··
            break;
            default:
            ·····
            ·········}
            ······"#]],
    );
}

#[test]
fn a_condition_at_the_first_byte_keeps_its_expression_without_a_block() {
    // Nowhere for both the opening tag and `if`: the condition is kept as an expression -- hover and
    // goto still answer in it -- and its end is a blank rather than a brace nothing opened. 26 of
    // BookStack's 325 templates begin this way, and every one used to read as markup.
    check(
        "@if(count($items) > 0)\n<ul>{{ $items[0] }}</ul>\n@endif\n",
        expect![[r#"
            <?·[count($items)·>·0]
            ;······$items[0]·;······
            ······
        "#]],
    );
}

#[test]
fn a_comment_over_several_lines_at_the_first_byte_carries_a_plain_opening_tag() {
    check(
        "{{--\n$name - string\n--}}\n@if($errors->has($name))\nx\n@endif",
        expect![[r#"
            <?/*
            $name·-·string
            ·*/·
            ·if($errors->has($name))
            {
            ·····}"#]],
    );
}

#[test]
fn an_include_is_the_call_it_is() {
    // So the index records it where it records a controller's, and the template it names reads
    // its variables from this one -- every one of them for an include, and only the array's for a
    // component, which is why the two are different calls.
    check(
        "<div>\n@include('posts.row', ['post' => $post])\n@component('alert', ['type' => $kind])\n</div>",
        expect![[r#"
            <?···
            ····_inc('posts.row',·['post'·=>·$post])
            ;·····view('alert',·['type'·=>·$kind])
            ;·····"#]],
    );
}

#[test]
fn a_components_bound_attribute_is_an_expression_and_alpines_is_not() {
    check(
        "<div :class=\"open\">\n<x-alert :post=\"$post\" ::class=\"x\" class=\"a {{ $c }}\" :items=\"[$a, $b]\" />\n</div>",
        expect![[r#"
            <?php··············
            x(\alert,post:··$post································,items:··[$a,·$b]··);
            ······"#]],
    );
}

/// Every prefix of a template -- which is what a buffer being typed is -- masks without panicking,
/// to the same length with every newline in place.
#[test]
fn every_prefix_of_a_template_keeps_its_shape() {
    let template = "{{-- @var \\App\\User $user --}}\n<p>Привіт, {{ $user->name }}</p>\n\
        @if ($user->isAdmin())\n  <x-badge :user=\"$user\" class=\"{{ $c }}\" />\n@elseif ($guest)\n\
        @foreach ($posts as $post)\n    @include('row', ['post' => $post])\n  @empty\n@endforeach\n\
        @php $x = ['a' => '}}']; @endphp\n{!! $html !!} @{{ raw }} @@if a@b.c\n@endif\n<?php echo 1 ?>\n\
        @switch($k) @case(1) one @break @default x @endswitch {{-- multi\nline --}}";
    for end in 0..=template.len() {
        if !template.is_char_boundary(end) {
            continue;
        }
        let prefix = &template[..end];
        let (php, _) = to_php(prefix);
        assert_eq!(php.len(), prefix.len(), "length kept at {end}");
        assert_eq!(
            php.match_indices('\n')
                .map(|(at, _)| at)
                .collect::<Vec<_>>(),
            prefix
                .match_indices('\n')
                .map(|(at, _)| at)
                .collect::<Vec<_>>(),
            "newlines kept at {end}"
        );
    }
}

#[test]
fn a_component_is_the_call_it_is() {
    // Every attribute a named argument can be: the camel-cased name a component reads, a plain
    // string, the one echo a value is. A tag with children owes its `;` to the next blank.
    check(
        "<html>\n<x-buttons.primary type=\"submit\" :show-refresh=\"$on\" class=\"{{ $c }}\" disabled wire:click=\"go\">\n  {{ $label }}\n</x-buttons.primary>\n<x-slot:title>t</x-slot:title>\n</html>",
        expect![[r#"
            <?php·
            x(\buttons\primary,type:"submit",showRefresh:···$on·,class:····$c·····························)
            ;····$label·;·
            ····················
            ······························
            ·······"#]],
    );
}

#[test]
fn a_livewire_tag_is_the_call_it_is() {
    // The same call as a component's under another name, and a `wire:` attribute left blank.
    check(
        "<div>\n<livewire:posts.likes :subject=\"$post\" type=\"post\" wire:key=\"k\" />\n</div>",
        expect![[r#"
            <?···
            livewire(\posts\likes,subject:··$post·,type:"post"··············);
            ······"#]],
    );
}

#[test]
fn a_component_right_after_a_statement_carries_its_semicolon() {
    check(
        "@props(['name'])\n\n<x-forms.input :name=\"$name\" />",
        expect![[r#"
            <?php·[['name']]

            ;x(forms\input,name:··$name··);"#]],
    );
}

/// What does not pair, by what each half compiles to: a closer of another structure ends what was
/// opened inside the block it does end, a block reaching the end of the file is never closed, and
/// `@auth ... @endif` is no problem at all, since both halves are an `if`.
#[test]
fn a_block_that_does_not_pair_is_named_where_it_opens() {
    let template = "@auth\n  ok\n@endif\n@foreach ($a as $b)\n  @if ($b)\n    x\n@endforeach\n@error('body')\n@else\n@forelse ($a as $b)\n@endforelse\n@endwhile\n@php\n";
    let found = super::problems(template)
        .into_iter()
        .map(|(at, problem)| format!("{}: {problem}", &template[at]))
        .collect::<Vec<_>>();
    expect![[r#"
        [
            "@if: `@if` is never closed",
            "@error: `@error` is never closed",
            "@forelse: `@forelse` ends without an `@empty`",
            "@endwhile: `@endwhile` ends no block that is open",
            "@php: `@php` is never closed",
        ]
    "#]]
    .assert_debug_eq(&found);
}

#[test]
fn an_import_is_the_use_statement_it_compiles_to() {
    check(
        "@use('App\\Models\\Post')\n@use('App\\Models\\User', 'Person')\n<p>{{ Post::count() }}</p>",
        expect![[r#"
            <?use·App\Models\Post;·
            ··use·App\Models\User·as·Person;·
            ······Post::count()·;·····"#]],
    );
}
