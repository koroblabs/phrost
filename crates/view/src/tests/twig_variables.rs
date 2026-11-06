//! What a Twig template's variables hold, read where the array rendering the template hands them
//! over, and what every attribute read off them is.
//!
//! Neutral, as below the framework seam everywhere: the test states the rule a provider would --
//! which key names the template, which prefix hands a variable over -- and nothing here knows it is
//! Drupal's.

use expect_test::{Expect, expect};
use phrost_harness::{OpenScope, World};
use phrost_index::OpenSet;
use phrost_index::Place;
use phrost_testkit::FixtureSpec;

use crate::{
    IndexView,
    twig::{TwigOrigin, TwigSymbolKind},
};

const PHP: &str = r#"
//- /src/Node.php
<?php
namespace App;

class Node
{
    public string $title = '';
    private string $secret = '';
    public function getLabel(): string { return ''; }
    public function isPublished(): bool { return true; }
    public function owner(): User { return new User(); }
    public static function create(): self { return new self(); }
}

//- /src/User.php
<?php
namespace App;

class User
{
    public function getName(): string { return ''; }
}

//- /src/Certificate.php
<?php
namespace App;

class Certificate
{
    /** @param list<Node> $nodes */
    public function build(Node $node, array $nodes): array
    {
        return [
            '#theme' => 'pdf__cert',
            '#node' => $node,
            '#nodes' => $nodes,
            '#data' => ['name' => 'x', 'count' => 3],
            '#cache' => [],
        ];
    }
}
//- /src/Notice.php
<?php
namespace App;

class Notice
{
    public function build(Node $node, User $owner): array
    {
        $build['#theme'] = 'pdf__cert';
        $build['#signer'] = $node;
        $build['#signer'] = $owner;
        return $build;
    }
}

//- /src/Letter.php
<?php
namespace App;

class Letter
{
    public function build(User $owner): array
    {
        $build['#theme'] = 'generated_letter__spanish';
        $build['#owner'] = $owner;
        return $build;
    }
}
"#;

const TEMPLATE: &str = "templates/pdf--cert.html.twig";

/// A template the page includes by a computed name.
const SECTION: &str = "templates/sections/info.html.twig";

/// A part the page includes by name, handing it a variable.
const PART: &str = "templates/parts/row.html.twig";

/// The templates' text, for the ones held beside the template asked about.
const TEXTS: &[(&str, &str)] = &[(
    TEMPLATE,
    "{% include '@app/parts/row.html.twig' with {'row': node} %}\n{% include '@app/parts/row.html.twig' with {'row': data} only %}",
)];

fn world() -> World {
    World::build_with(&FixtureSpec::parse(PHP), |world| {
        let template = world.root().join(TEMPLATE);
        let section = world.root().join(SECTION);
        let part = world.root().join(PART);
        world.set_twig_includes(&template, &["@app/parts/row.html.twig"]);
        let contracts = world.contracts_mut();
        contracts.bind_render_array(phrost_index::RenderArrayRule {
            key: "#theme".into(),
            catalog: "theme".into(),
            prefix: "#".into(),
            own: ["#theme".into(), "#cache".into()].into(),
            file_name: Some((".html.twig".into(), '-', '_')),
        });
        contracts.add_twig_loader(phrost_index::TwigLoader {
            templates: "template".into(),
            aliases: "template path".into(),
            namespaces: "template namespace".into(),
            unwalked: Box::default(),
            globals: [("attributes".into(), "App\\User".into())].into(),
        });
        // The page's name and a section's, as a loader would file them, and the statement that the
        // section sees the page's variables -- the project's, since nothing in either file says so.
        for (key, path) in [
            ("@app/pdf--cert.html.twig", template.clone()),
            ("@app/sections/info.html.twig", section),
            ("@app/parts/row.html.twig", part),
        ] {
            contracts.catalog_place(
                "template",
                key,
                Place {
                    path: path.as_path().into(),
                    line: 0,
                    character: 0,
                    detail: "".into(),
                },
            );
        }
        contracts.add_twig_context("@app/sections/**", "@app/pdf--cert.html.twig");
        contracts.catalog_place(
            "theme",
            "pdf__cert",
            Place {
                path: template.as_path().into(),
                line: 0,
                character: 0,
                detail: "".into(),
            },
        );
    })
}

/// `answer` with the template open as the server holds it -- nothing open but the files rendering it,
/// lowered beside it -- and with every file open, which must agree.
///
/// The harness's own pair of scopes opens the cursor's file or every file, and a template held as
/// text has no file to be the cursor: what the server opens for it is its renderers, which is what
/// `render_arrays_of` says and what is opened here.
fn with_template_open(
    world: &World,
    template: &std::path::Path,
    answer: impl Fn(&OpenSet) -> String,
) -> String {
    // The templates including it, held as text, and what renders each -- as the workspace settles
    // them beside an open template.
    let mut including = vec![template.to_path_buf()];
    let mut at = 0;
    while at < including.len() {
        for found in world.skeleton().twig_includers_of(&including[at]) {
            if !including.iter().any(|known| known == found) {
                including.push(found.to_path_buf());
            }
        }
        at += 1;
    }
    let held = |open: &mut OpenSet| {
        for path in &including[1..] {
            let text = TEXTS
                .iter()
                .find(|(written, _)| world.root().join(written) == *path)
                .map(|(_, text)| *text)
                .expect("a template the test wrote");
            open.hold_template(path, text.into());
        }
    };
    let mut beside = world.open(OpenScope::CursorFileOnly, None);
    held(&mut beside);
    for path in &including {
        for (file, ..) in world.skeleton().render_arrays_of(path) {
            beside.open(file, world.text_of(file).to_owned());
        }
    }
    let alone = answer(&beside);
    let mut every = world.open(OpenScope::EveryFile, None);
    held(&mut every);
    let everything = answer(&every);
    assert_eq!(
        alone, everything,
        "the answer changed when other files were opened; a template's variables must come from \
         its renderers and nothing else"
    );
    alone
}

/// Renders what the name at every `¦` in `twig` is -- not `|`, which is a filter's.
fn check(twig: &str, expect: Expect) {
    let world = world();
    let mut text = String::with_capacity(twig.len());
    let mut offsets = Vec::new();
    for ch in twig.chars() {
        match ch {
            '¦' => offsets.push(text.len()),
            _ => text.push(ch),
        }
    }
    let template = world.root().join(TEMPLATE);
    let rendered = with_template_open(&world, &template, |open| {
        let view = IndexView::new(world.skeleton(), open);
        let mut out = String::new();
        for offset in &offsets {
            out.push_str(&format!("{offset} "));
            let Some(found) = view.twig_symbol_at(&template, &text, *offset) else {
                out.push_str("(nothing)\n");
                continue;
            };
            let ty = found.ty.as_deref().unwrap_or("?");
            let what = match &found.kind {
                TwigSymbolKind::Variable { name, origin } => {
                    let origin = match origin {
                        TwigOrigin::Bound(range) => format!("bound at {range:?}"),
                        TwigOrigin::Rendered { file, range } => {
                            format!("handed over at {}", world.render_range(*file, *range))
                        }
                        TwigOrigin::Global => "global".to_owned(),
                        TwigOrigin::Unbound => "unbound".to_owned(),
                        TwigOrigin::Included { range, .. } => format!("included at {range:?}"),
                    };
                    format!("variable {name}: {ty} ({origin})")
                }
                TwigSymbolKind::Member { location, kind } => format!(
                    "{kind:?} {}::{}: {ty}",
                    location
                        .container
                        .as_ref()
                        .map_or("", |class| class.as_str()),
                    location.fqn
                ),
                TwigSymbolKind::Key { name } => format!("key {name}: {ty}"),
                TwigSymbolKind::Called(called) => format!("called {}: {ty}", called.name),
            };
            out.push_str(&what);
            out.push('\n');
        }
        out
    });
    expect.assert_eq(&rendered);
}

/// Renders what is offered at the one `¦` in `twig`.
fn complete(twig: &str, expect: Expect) {
    let world = world();
    let offset = twig.find('¦').expect("a cursor");
    let text = twig.replacen('¦', "", 1);
    let template = world.root().join(TEMPLATE);
    let rendered = with_template_open(&world, &template, |open| {
        let view = IndexView::new(world.skeleton(), open);
        let Some((replaced, items)) = view.twig_completions(&template, &text, offset, 50) else {
            return "(nothing)\n".to_owned();
        };
        let mut out = format!("replaces {replaced:?}\n");
        for item in items {
            out.push_str(&format!(
                "{:?} {} {}\n",
                item.kind,
                item.name,
                item.detail.as_deref().unwrap_or("?")
            ));
        }
        out
    });
    expect.assert_eq(&rendered);
}

#[test]
fn a_variable_holds_what_the_array_rendering_the_template_hands_over() {
    check(
        "{{ no¦de.title }}{{ da¦ta.count }}{{ attri¦butes }}{{ miss¦ing }}",
        expect![[r#"
            5 variable node: Node (handed over at 11:13-11:20)
            21 variable data: array{name: non-falsy-string, count: int} (handed over at 13:13-13:20)
            40 variable attributes: User (global)
            55 variable missing: ? (unbound)
        "#]],
    );
}

/// Twig's attribute rule: a property, then the method of that name, then its getters -- and a
/// member PHP would refuse outside the class is not one.
#[test]
fn an_attribute_is_the_member_twig_reads() {
    check(
        "{{ node.tit¦le }}{{ node.lab¦el }}{{ node.publi¦shed }}{{ node.owner.na¦me }}{{ node.sec¦ret }}{{ data.na¦me }}",
        expect![[r#"
            11 Property App\Node::title: string
            27 Method App\Node::getLabel: string
            45 Method App\Node::isPublished: bool
            68 Method App\User::getName: string
            84 (nothing)
            100 key name: non-falsy-string
        "#]],
    );
}

#[test]
fn a_loop_binds_an_element_and_loop() {
    check(
        "{% for item in nodes %}{{ it¦em.ti¦tle }}{{ loop.ind¦ex }}{% endfor %}{% set first = nodes|first %}{{ fir¦st.label }}",
        expect![[r#"
            28 variable item: Node (bound at 7..11)
            33 Property App\Node::title: string
            50 key index: int
            102 variable first: Node (bound at 74..79)
        "#]],
    );
}

#[test]
fn an_attribute_being_written_is_offered_what_twig_can_read() {
    complete(
        "{{ node.¦ }}",
        expect![[r#"
        replaces 8..8
        Property label string
        Property published bool
        Method owner User
        Property title string
    "#]],
    );
    complete(
        "{{ data.c¦ }}",
        expect![[r#"
        replaces 8..9
        Key name non-falsy-string
        Key count int
    "#]],
    );
}

#[test]
fn a_variable_being_written_is_offered_every_one_in_reach() {
    complete(
        "{% for item in nodes %}{{ i¦ }}{% endfor %}",
        expect![[r#"
            replaces 26..27
            Variable item Node
            Variable loop ?
            Variable node Node
            Variable nodes array<int, Node>
            Variable data array{name: non-falsy-string, count: int}
            Variable signer User
            Variable attributes User
        "#]],
    );
}

/// An array built one key at a time hands over what its last whole write of each key holds, from a
/// file of its own beside the literal's.
#[test]
fn a_variable_written_into_a_render_array_is_its_last_write() {
    let world = world();
    let template = world.root().join(TEMPLATE);
    let text = "{{ signer.name }}";
    let rendered = with_template_open(&world, &template, |open| {
        let view = IndexView::new(world.skeleton(), open);
        let Some(found) = view.twig_symbol_at(&template, text, 4) else {
            return "(nothing)".to_owned();
        };
        let TwigSymbolKind::Variable { origin, .. } = &found.kind else {
            return "(not a variable)".to_owned();
        };
        let origin = match origin {
            TwigOrigin::Rendered { file, range } => world.render_range(*file, *range),
            other => format!("{other:?}"),
        };
        format!("{} from {origin}", found.ty.as_deref().unwrap_or("?"))
    });
    expect!["User from 10:16-10:25"].assert_eq(&rendered);
}

/// A template no catalog places -- a hook's `path` put it outside every `templates/` -- is still the
/// one its file name says it is.
#[test]
fn a_template_outside_every_catalog_is_named_by_its_file() {
    let world = world();
    let template = world
        .root()
        .join("src/Generated/twig/generated-letter--spanish.html.twig");
    let text = "{{ owner.name }}";
    let rendered = with_template_open(&world, &template, |open| {
        let view = IndexView::new(world.skeleton(), open);
        view.twig_symbol_at(&template, text, 11)
            .and_then(|found| found.ty)
            .map_or_else(|| "(nothing)".to_owned(), String::from)
    });
    expect!["string"].assert_eq(&rendered);
}

/// A section a page includes by a computed name sees the page's variables, because the project said
/// which page -- and nothing else does, so a section nothing is said about sees none.
#[test]
fn a_section_sees_the_variables_of_the_page_the_project_names() {
    let world = world();
    let section = world.root().join(SECTION);
    let text = "{{ node.title }}";
    let rendered = with_template_open(&world, &section, |open| {
        let view = IndexView::new(world.skeleton(), open);
        let Some(found) = view.twig_symbol_at(&section, text, 4) else {
            return "(nothing)".to_owned();
        };
        let TwigSymbolKind::Variable { origin, .. } = &found.kind else {
            return "(not a variable)".to_owned();
        };
        let origin = match origin {
            TwigOrigin::Rendered { file, range } => format!(
                "{} {}",
                world.path_of(*file),
                world.render_range(*file, *range)
            ),
            other => format!("{other:?}"),
        };
        format!("{} from {origin}", found.ty.as_deref().unwrap_or("?"))
    });
    expect!["Node from src/Certificate.php 11:13-11:20"].assert_eq(&rendered);

    let elsewhere = world.root().join("templates/other/info.html.twig");
    let alone = with_template_open(&world, &elsewhere, |open| {
        let view = IndexView::new(world.skeleton(), open);
        format!(
            "{:?}",
            view.twig_symbol_at(&elsewhere, text, 4)
                .and_then(|found| found.ty)
        )
    });
    assert_eq!(alone, "None");
}

/// A part sees what the template including it hands over -- the key of its `with`, from every
/// include of it -- and, where an include does not say `only`, that template's own variables.
#[test]
fn a_part_sees_what_the_template_including_it_hands_over() {
    let world = world();
    let part = world.root().join(PART);
    let text = "{{ row.title }}{{ data.name }}";
    let rendered = with_template_open(&world, &part, |open| {
        let view = IndexView::new(world.skeleton(), open);
        let mut out = String::new();
        for offset in [4, 19] {
            let Some(found) = view.twig_symbol_at(&part, text, offset) else {
                out.push_str("(nothing)\n");
                continue;
            };
            let TwigSymbolKind::Variable { name, origin } = &found.kind else {
                continue;
            };
            let origin = match origin {
                TwigOrigin::Included { template, range } => format!(
                    "handed over by {} at {range:?}",
                    template
                        .strip_prefix(world.root())
                        .unwrap_or(template)
                        .display()
                ),
                TwigOrigin::Rendered { file, .. } => {
                    format!("rendered in {}", world.path_of(*file))
                }
                other => format!("{other:?}"),
            };
            out.push_str(&format!(
                "{name}: {} ({origin})\n",
                found.ty.as_deref().unwrap_or("?")
            ));
        }
        out
    });
    expect![[r#"
        row: Node|array{name: non-falsy-string, count: int} (handed over by templates/pdf--cert.html.twig at 45..48)
        data: array{name: non-falsy-string, count: int} (rendered in src/Certificate.php)
    "#]].assert_eq(&rendered);
}
