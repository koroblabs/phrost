//! View tests: what a feature actually sees.
//!
//! The layer where "the index has the fact but the feature cannot reach it" would show up. Every
//! query here goes through the same surface a feature would use.
//!
//! Every fixture is also run through the **open-versus-closed** dimension: once with only the file
//! under the cursor open, and once with every file in the fixture open. The results must be
//! identical. Today that holds structurally — cross-file answers read the skeleton, which knows
//! nothing about openness — and the dimension is what will catch the first change that quietly
//! makes an answer better for open files, which is the bug class that made the previous attempt
//! feel unpredictable.

mod a_bare_generic_a_built_chain_fills;
mod a_bare_generic_a_subclass_body_fills;
mod a_bare_page_a_framework_fills;
mod a_bare_relation_a_chain_fills;
mod a_bool_a_test_takes_half_of;
mod a_branch_entered_because_it_was_not_null;
mod a_branch_of_a_template_nothing_supplied;
mod a_builder_method_a_model_tag_declares;
mod a_call_that_never_comes_back;
mod a_callable_value_is_a_closure;
mod a_callable_written_as_a_string;
mod a_callback_a_trait_template_describes;
mod a_callback_parameter_where_it_is_declared;
mod a_callback_that_is_not_a_closure;
mod a_chain_judged_by_what_its_links_left;
mod a_chain_of_string_tests;
mod a_chain_off_a_union;
mod a_class_a_callback_fills;
mod a_class_a_loader_aliases;
mod a_class_the_call_names;
mod a_class_whose_body_states_its_table;
mod a_closure_made_from_a_callable;
mod a_closure_signature;
mod a_collection_made_from_a_container;
mod a_column_a_migration_writes;
mod a_column_of_rows;
mod a_compound_assignment;
mod a_conditional_naming_its_own_template;
mod a_conditional_on_a_left_out_argument;
mod a_conditional_whose_test_is_a_union;
mod a_constant_table_read_by_key;
mod a_constructor_binding_a_template_by_position;
mod a_constructor_keyed_by_its_argument;
mod a_constructor_union_binds_a_template;
mod a_container_beside_a_sentinel;
mod a_container_keyed_by_another;
mod a_container_of_a_template_nothing_supplied;
mod a_container_of_the_value_passed;
mod a_container_written_through_a_reference;
mod a_contract_bound_to_its_own_implementation;
mod a_contract_on_a_local_reaches_a_chain_off_it;
mod a_default_settles_a_kind_test;
mod a_default_written_as_a_value_or_a_closure;
mod a_destructured_local;
mod a_destructuring_target_under_the_cursor;
mod a_docblock_beside_a_nullable_hint;
mod a_docblock_over_a_native_union;
mod a_facade_root_its_accessor_returns;
mod a_facade_tag_written_from_its_root;
mod a_factory_making_the_model_named_after_it;
mod a_factory_named_after_its_model;
mod a_factory_that_makes_one_or_many;
mod a_factory_that_returns_two_things;
mod a_failure_a_literal_rules_out;
mod a_flag_decides_what_comes_back;
mod a_generator_the_body_yields;
mod a_generic_class_with_no_constructor;
mod a_global_constant;
mod a_guard_comparing_the_class_by_name;
mod a_guard_naming_a_subtype;
mod a_join_of_one_kind_settles_a_kind_test;
mod a_key_a_catalog_does_not_list;
mod a_key_a_container_was_found_to_hold;
mod a_key_of_a_container_filled_by_slot;
mod a_keyless_array_beside_an_iterable_hint;
mod a_kind_assertion_naming_a_template;
mod a_kind_that_is_not_a_family;
mod a_list_a_call_built_then_written;
mod a_lone_argument_is_the_value;
mod a_member_of_every_class_a_table_names;
mod a_member_php_sends_to_a_magic_accessor;
mod a_membership_test_only_filters;
mod a_method_a_macro_registration_gives;
mod a_method_a_prefix_spells;
mod a_method_called_without_its_prefix;
mod a_method_its_call_reopens;
mod a_method_tag_with_a_contract;
mod a_method_that_hands_the_receiver_back;
mod a_name_a_magic_accessor_answers;
mod a_name_compact_collects;
mod a_needle_a_strict_search_found;
mod a_new_of_one_of_several_class_names;
mod a_normalised_value;
mod a_number_class_under_arithmetic;
mod a_pair_handed_to_a_callable;
mod a_pair_invoked_where_it_stands;
mod a_parameter_of_a_closure_inside_a_closure;
mod a_property_a_declared_kind_states;
mod a_property_a_model_states;
mod a_property_a_table_names;
mod a_property_an_object_shape_names;
mod a_property_read_through_its_accessor;
mod a_property_read_through_its_method;
mod a_record_not_identical_to_a_literal;
mod a_relation_keyed_by_an_enforced_column;
mod a_relational_guard;
mod a_relations_own_mixin_answers_before_its_contracts;
mod a_relative_type_rendered;
mod a_return_a_configured_key_names;
mod a_return_the_body_states;
mod a_scope_reached_through_a_relation;
mod a_self_out_replaces_the_receiver;
mod a_shape_the_body_returns;
mod a_signed_number;
mod a_slot_filled_in_a_branch;
mod a_slot_the_call_settles;
mod a_string_that_holds_something;
mod a_string_that_names_a_class;
mod a_subclass_a_guard_narrows_to_keeps_its_arguments;
mod a_subscript_an_object_answers;
mod a_table_of_class_names;
mod a_template_a_written_mixin_supplies;
mod a_template_filled_with_the_receivers_own;
mod a_template_inside_an_intersection_in_a_union;
mod a_template_inside_an_intersection_in_an_argument;
mod a_template_left_out_is_its_default;
mod a_template_nothing_supplied;
mod a_template_passed_through_an_extends;
mod a_template_return_an_override_carried;
mod a_template_return_beside_a_hint;
mod a_test_against_a_class;
mod a_throw_where_a_value_goes;
mod a_type_only_the_body_says;
mod a_type_predicate_that_settles_it;
mod a_typed_constant_its_value_refines;
mod a_union_bound_two_ways;
mod a_union_of_intersections;
mod a_union_renders_flat;
mod a_union_told_apart_by_a_property;
mod a_use_tag_above_the_trait_use;
mod a_value_identical_to_a_constant;
mod a_value_that_tested_falsy;
mod a_write_after_a_reference_call;
mod alias_binding;
mod an_accumulator_written_as_a_union;
mod an_annotation_above_its_own_assignment;
mod an_argument_two_hops_up_a_bounded_chain;
mod an_array_a_filter_dropped_nulls_from;
mod an_array_a_guard_found_empty;
mod an_array_coalesced_with_an_empty_one;
mod an_autoloaded_declaration_outranks_a_stub;
mod an_enum_constant_naming_a_case;
mod an_immediately_invoked_closure;
mod an_inherited_parameter_of_a_template;
mod an_input_a_command_declares;
mod an_object_cast;
mod an_object_storage;
mod an_override_that_declares_nothing;
mod an_unsettled_conditional_names_a_shared_operand_once;
mod an_untyped_parameter_of_an_override;
mod anonymous_class;
mod applied_generics;
mod argument_assertions;
mod argument_class_return;
mod argument_keys;
mod assert_narrows_a_stepped_receiver;
mod callable_return;
mod callbacks_in_a_chain;
mod calls;
mod candidates;
mod catalog_cursor;
mod class_string_new;
mod classes_of_one_name;
mod closure_invocation;
mod closure_this;
mod coalescing_assignment;
mod conditional_branch_element;
mod configuration;
mod configured_closure_this;
mod defaulted_return;
mod dispatched_names;
mod documentation_claims;
mod documented_declaration;
mod documented_keys;
mod element_catalog;
mod elvis_rules_out_false;
mod first_class_callable_values;
mod forwarded_catalog;
mod forwarded_catalog_uses;
mod free_function_contract;
mod function_references;
mod guarding_a_joined_value;
mod implementers;
mod implied_mixin;
mod import_alias;
mod inferred_closure_return;
mod iterated_through_a_method;
mod iterated_value;
mod iterating_a_union;
mod joined_arms_read_where_written;
mod last_write_through_a_reference;
mod late_static_binding;
mod library_catalog;
mod list_literal;
mod magic_property;
mod match_value;
mod memo;
mod navigation;
mod non_empty_settles_a_conditional;
mod one_argument_to_a_container_is_its_value;
mod property_writes;
mod record_literal;
mod recovered_annotations;
mod references;
mod self_through_a_static_call;
mod slot_literal;
mod template_element_return;
mod the_receiver_as_a_type_argument;
mod twig_callables;
mod twig_template_names;
mod twig_variables;
mod two_constructs_are_a_chain;
mod two_tags_for_one_type;
mod unused_imports;
mod variable_types;
mod variadic_template;
mod which_sentinel_a_branch_admitted;
mod workspace_search;
mod writes_in_a_branch;

use std::fmt::Write as _;

use expect_test::Expect;
use phrost_harness::{World, with_open_scopes};
use phrost_model::TextSize;
use phrost_sema::{MemberKind, render_member_table};
use phrost_testkit::FixtureSpec;

use crate::{IndexView, SymbolLocation};

/// One thing to ask the view.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Query {
    /// What is under the marker.
    Cursor {
        label: &'static str,
        marker: &'static str,
    },
    /// Where the thing under the marker is declared.
    Goto {
        label: &'static str,
        marker: &'static str,
    },
    /// Every member of a class, flattened.
    Members {
        label: &'static str,
        class: &'static str,
    },
    /// The class a member of a class holds.
    MemberClassOf {
        label: &'static str,
        marker: &'static str,
        class: &'static str,
        member: &'static str,
    },
    /// The class a variable's declaration says it holds.
    VariableClass {
        label: &'static str,
        marker: &'static str,
        variable: &'static str,
    },
    /// The type of the variable under the marker, rendered the way hover shows it.
    ///
    /// A different question from [`Self::VariableClass`], and the one that catches a right answer
    /// spelled wrongly: `static[]`, `string|?int` and a callable with its parameters missing all
    /// name the correct class and still tell a reader something false.
    VariableType {
        label: &'static str,
        marker: &'static str,
    },
    /// Where one member of a class is declared.
    MemberGoto {
        label: &'static str,
        class: &'static str,
        kind: MemberKind,
        name: &'static str,
    },
}

impl Query {
    pub(crate) fn cursor(label: &'static str, marker: &'static str) -> Self {
        Self::Cursor { label, marker }
    }

    pub(crate) fn member_class(
        label: &'static str,
        marker: &'static str,
        class: &'static str,
        member: &'static str,
    ) -> Self {
        Self::MemberClassOf {
            label,
            marker,
            class,
            member,
        }
    }

    pub(crate) fn variable_class(
        label: &'static str,
        marker: &'static str,
        variable: &'static str,
    ) -> Self {
        Self::VariableClass {
            label,
            marker,
            variable,
        }
    }

    pub(crate) const fn variable_type(label: &'static str, marker: &'static str) -> Self {
        Self::VariableType { label, marker }
    }

    pub(crate) fn goto(label: &'static str, marker: &'static str) -> Self {
        Self::Goto { label, marker }
    }

    pub(crate) fn members(label: &'static str, class: &'static str) -> Self {
        Self::Members { label, class }
    }

    pub(crate) fn method_goto(
        label: &'static str,
        class: &'static str,
        name: &'static str,
    ) -> Self {
        Self::MemberGoto {
            label,
            class,
            kind: MemberKind::Method,
            name,
        }
    }

    pub(crate) fn property_goto(
        label: &'static str,
        class: &'static str,
        name: &'static str,
    ) -> Self {
        Self::MemberGoto {
            label,
            class,
            kind: MemberKind::Property,
            name,
        }
    }

    /// The marker a query is anchored at, if it is a position query.
    fn marker(self) -> Option<&'static str> {
        match self {
            Self::Cursor { marker, .. }
            | Self::Goto { marker, .. }
            | Self::VariableClass { marker, .. }
            | Self::VariableType { marker, .. }
            | Self::MemberClassOf { marker, .. } => Some(marker),
            Self::Members { .. } | Self::MemberGoto { .. } => None,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Cursor { label, .. }
            | Self::Goto { label, .. }
            | Self::VariableClass { label, .. }
            | Self::VariableType { label, .. }
            | Self::MemberClassOf { label, .. }
            | Self::Members { label, .. }
            | Self::MemberGoto { label, .. } => label,
        }
    }
}

/// Runs the queries and snapshots the answers.
///
/// Each query goes through the harness's open-versus-closed check, so a cross-file answer that
/// started depending on which buffers are open fails here.
pub(crate) fn check_view(fixture: &str, queries: &[Query], expect: Expect) {
    check_view_with(fixture, queries, |_| {}, expect);
}

/// The same, with the project's framework knowledge stated first.
///
/// A separate entry point rather than a parameter on [`check_view`], so that the tests about what PHP
/// itself says keep reading as tests about PHP.
pub(crate) fn check_view_with(
    fixture: &str,
    queries: &[Query],
    state: impl FnOnce(&mut World),
    expect: Expect,
) {
    let spec = FixtureSpec::parse(fixture);
    let mut world = World::build(&spec);
    state(&mut world);
    check_world(&spec, &world, queries, expect);
}

/// The same, with the rules stated *before* the files are ingested -- the only order in which a
/// fact read out of a body at ingest, a declared table or a dispatch, can exist at all.
pub(crate) fn check_view_ingested_with(
    fixture: &str,
    queries: &[Query],
    state: impl FnOnce(&mut World),
    expect: Expect,
) {
    let spec = FixtureSpec::parse(fixture);
    let world = World::build_with(&spec, state);
    check_world(&spec, &world, queries, expect);
}

pub(crate) fn check_world(spec: &FixtureSpec, world: &World, queries: &[Query], expect: Expect) {
    let mut out = String::new();

    for query in queries {
        let (cursor_file, offset) = match query.marker() {
            Some(marker) => {
                let position = spec.markers().position(marker);
                (Some(world.file_id(&position.path)), Some(position.offset))
            }
            // A class-level question needs no cursor, but still needs a current file so the
            // open-versus-closed check has something to vary.
            None => (world.any_file(), None),
        };

        let answer = with_open_scopes(world, cursor_file, |open| {
            let view = match cursor_file {
                Some(file) => IndexView::in_file(world.skeleton(), open, file),
                None => IndexView::new(world.skeleton(), open),
            };
            render_answer(world, &view, *query, offset)
        });

        let _ = writeln!(out, "{}", query.label());
        out.push_str(&answer);
    }

    spec.markers().assert_all_used();
    expect.assert_eq(&format!("{}\n", out.trim_end()));
}

fn render_answer(
    world: &World,
    view: &IndexView<'_>,
    query: Query,
    offset: Option<TextSize>,
) -> String {
    let mut out = String::new();

    match query {
        Query::Cursor { .. } => match view.cursor_target(offset.expect("cursor query")) {
            Some(target) => {
                let _ = writeln!(out, "- {}", target.label());
            }
            None => {
                let _ = writeln!(out, "- (nothing under the cursor)");
            }
        },
        Query::Goto { .. } => {
            let location = view.declaration_at(offset.expect("goto query"));
            render_location(&mut out, world, view, location.as_ref());
        }
        Query::Members { class, .. } => {
            for line in render_member_table(&view.members(class), world.skeleton().types()).lines()
            {
                let _ = writeln!(out, "- {line}");
            }
        }
        Query::MemberClassOf { class, member, .. } => {
            match view.member_class(class, MemberKind::Property, member) {
                Some(found) => {
                    let _ = writeln!(out, "- {found}");
                }
                None => {
                    let _ = writeln!(out, "- (type not known here)");
                }
            }
        }
        Query::VariableClass { variable, .. } => {
            match view.variable_class(offset.expect("variable query"), variable) {
                Some(class) => {
                    let _ = writeln!(out, "- {class}");
                }
                None => {
                    let _ = writeln!(out, "- (type not known here)");
                }
            }
        }
        Query::VariableType { .. } => {
            match view.variable_at(offset.expect("variable type query")) {
                Some(info) => {
                    let _ = writeln!(out, "- {}", info.ty.as_deref().unwrap_or("(no type)"));
                }
                None => {
                    let _ = writeln!(out, "- (no variable under the cursor)");
                }
            }
        }
        Query::MemberGoto {
            class, kind, name, ..
        } => {
            let location = view.member_location(class, kind, name);
            render_location(&mut out, world, view, location.as_ref());
        }
    }

    out
}

fn render_location(
    out: &mut String,
    world: &World,
    view: &IndexView<'_>,
    location: Option<&SymbolLocation>,
) {
    let Some(location) = location else {
        let _ = writeln!(out, "- (not found)");
        return;
    };

    let container = location
        .container
        .as_ref()
        .map(|container| format!("{container}::"))
        .unwrap_or_default();
    let _ = writeln!(
        out,
        "- {} {container}{} @ {} {}",
        location.kind.as_str(),
        location.fqn,
        view.file_path(location.file).unwrap_or("<unknown>"),
        world.render_range(location.file, location.name_range)
    );
}
