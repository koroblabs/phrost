//! The chains a template reads and the names it binds, rendered whole.

use expect_test::expect;

use super::render_reading;

#[test]
fn a_chain_is_a_variable_and_what_is_read_off_it() {
    expect![[r#"
        binding loop Loop visible 61..103
        binding panel Element(Chain customer_panels |filter |sort) visible 61..103
        chain customer_panels
        binding p Parameter visible 44..52
        chain p .active
        chain panel .customer .node
        chain panel .label
        chain order .edit_buttons
        chain row ['value'] [0] .format() [..]
        chain index
    "#]]
    .assert_eq(&render_reading(
        "{% for panel in customer_panels|filter(p => p.active)|sort %}{{ panel.customer.node }}{{ panel.label }}{% endfor %}{% if order.edit_buttons %}{{ row['value'][0].format('Y')[index] }}{% endif %}",
    ));
}

/// What is not a variable: a function, a filter, a test, a hash key, a named argument, a literal,
/// an operator spelled as a word, and a tag's own words.
#[test]
fn only_a_variable_starts_a_chain() {
    expect![[r#"
        chain node .id
        chain a
        chain b
        binding loop Loop visible 222..222
        binding x Element(Chain items) visible 222..222
        chain items
        chain x
    "#]]
    .assert_eq(&render_reading(
        "{{ url('route', {'node': node.id ?? 0}) }}{{ 'Hello'|t }}{{ path('x', {a: 1}) }}\n{{ date('now')|date(format = 'Y') }}{% if a is not defined and b is same as(true) %}{% endif %}{{ true or null or none }}{% for x in items %}{% else %}{% endfor %}{% block title %}{% endblock %}{{ x }}",
    ));
}

#[test]
fn each_tag_binds_what_it_names() {
    expect![[r#"
        binding count Assigned(Number) visible 23..260
        binding title Assigned(Chain node .title |label) visible 58..260
        chain node .title
        binding body Captured visible 93..260
        binding forms Macros visible 128..260
        binding field Macro visible 176..260
        binding input Macro visible 176..260
        binding customer Assigned(Chain panel .customer) visible 214..233
        chain panel .customer
        chain customer .name
        chain customer
    "#]]
    .assert_eq(&render_reading(
        "{% set count = 1 + 2 %}\n{% set title = node.title|label %}\n{% set body %}<p>x</p>{% endset %}\n{% import 'forms.twig' as forms %}\n{% from 'm.twig' import field, text as input %}\n{% with {customer: panel.customer} %}{{ customer.name }}{% endwith %}{{ customer }}",
    ));
}

/// A macro sees its own arguments and nothing of the template around it.
#[test]
fn a_macro_binds_its_arguments_inside_itself_only() {
    expect![[r#"
        binding varargs Parameter visible 37..59
        binding label Parameter visible 37..59
        binding value Parameter visible 37..59
        chain value
        chain label
        isolated 37..59
    "#]]
    .assert_eq(&render_reading(
        "{% macro field(label, value = 'x') %}{{ value }}{{ label }}{% endmacro %}",
    ));
}

/// A type written for a variable, either way the template can write one.
#[test]
fn a_declared_type_is_kept_as_written() {
    expect![[r#"
        binding post Declared("\\App\\Entity\\Post") visible 32..120
        binding count Declared("int") visible 103..120
        binding user Declared("\\App\\User") visible 103..120
        binding tags Declared("list<string>") visible 103..120
        chain post .title
    "#]]
    .assert_eq(&render_reading(
        "{# @var post \\App\\Entity\\Post #}\n{% types {count?: 'int', user: '\\\\App\\\\User', tags: 'list<string>'} %}\n{{ post.title }}",
    ));
}

/// The attribute being typed is a step with no name yet, where the dot ends.
#[test]
fn a_dot_with_nothing_after_it_is_the_step_being_written() {
    expect![[r#"
        chain panel .customer .
    "#]]
    .assert_eq(&render_reading("{{ panel.customer. }}"));
}

/// A cursor on a chain inside a computed key is on that chain, and not on the step around it.
#[test]
fn the_narrowest_chain_holds_a_cursor() {
    let text = "{{ rows[index].name }}";
    let reading = crate::read(text);
    let at = |offset: usize| {
        let (chain, step) = reading.chain_at(offset).expect("a chain");
        let part = match step {
            None => chain.root.clone(),
            Some(step) => chain.steps[step].name.clone(),
        };
        text[part].to_owned()
    };
    assert_eq!(
        [at(4), at(10), at(17)],
        ["rows".to_owned(), "index".to_owned(), "name".to_owned()]
    );
}

/// Every place a template hands its variables on: what each hands over besides, and whether that
/// is all it hands over.
#[test]
fn a_template_handing_its_variables_on_is_recorded() {
    let text = "{% extends 'base.html.twig' %}{% include 'a.twig' with {'item': row.item} only %}{{ include('b.twig', {n: 1}) }}{% embed 'c.twig' %}{% endembed %}{% use 'blocks.twig' %}";
    let reading = crate::read(text);
    let rendered = reading
        .includes
        .iter()
        .map(|include| {
            format!(
                "{} with {:?} only {}\n",
                &text[include.range.clone()],
                include.with.as_ref().map(|with| match with {
                    crate::Value::Hash(entries) => entries
                        .iter()
                        .map(|(key, _)| key.as_str())
                        .collect::<Vec<_>>()
                        .join(","),
                    other => format!("{other:?}"),
                }),
                include.only
            )
        })
        .collect::<String>();
    expect![[r#"
        {% extends 'base.html.twig' %} with None only false
        {% include 'a.twig' with {'item': row.item} only %} with Some("item") only true
        include('b.twig', {n: 1}) with Some("n") only false
        {% embed 'c.twig' %} with None only false
    "#]]
    .assert_eq(&rendered);
}

/// Every filter, function and test called by name, including a filter still to be typed.
#[test]
fn a_filter_a_function_and_a_test_are_called_by_name() {
    let text = "{{ path('x')|t|number_format(2) }}{% if a is same as(b) and c is defined %}{% endif %}{{ d| }}";
    let reading = crate::read(text);
    let rendered = reading
        .called
        .iter()
        .map(|called| format!("{:?} {:?}\n", called.kind, &text[called.name.clone()]))
        .collect::<String>();
    expect![[r#"
        Function "path"
        Filter "t"
        Filter "number_format"
        Test "same as"
        Test "defined"
        Filter ""
    "#]]
    .assert_eq(&rendered);
}
