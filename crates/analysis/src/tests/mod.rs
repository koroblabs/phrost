//! Feature tests: several queries against one fixture, one snapshot.
//!
//! One setup covering four or five checks is what keeps the tests dense enough to be worth writing,
//! and it catches the failure a per-feature test cannot: goto jumping one place while hover
//! describes another.
//!
//! Every fixture also runs through the open-versus-closed check, which the harness owns.

mod a_bool_is_not_a_failed_call;
mod a_bool_where_one_half_is_declared;
mod a_call_a_magic_method_answers;
mod a_call_names_a_parameter;
mod a_call_through_a_value;
mod a_call_written_in_another_case;
mod a_callable_pair_over_a_variable;
mod a_callback_a_framework_spells_with_a_prefix;
mod a_chain_that_names_a_catalog_key;
mod a_chain_written_with_a_nullsafe;
mod a_claim_with_words_before_the_name;
mod a_class_a_loader_aliases;
mod a_class_a_value_names;
mod a_class_named_by_a_string;
mod a_class_two_files_declare;
mod a_class_used_under_an_import_alias;
mod a_clone_that_writes_properties;
mod a_closure_bound_to_a_class;
mod a_closure_bound_where_it_is_written;
mod a_closure_signature_names_its_class;
mod a_closure_whose_parameter_the_signature_refuses;
mod a_closure_whose_return_the_signature_refuses;
mod a_coalesce_over_a_docblock_union;
mod a_column_offered_on_a_model;
mod a_computed_member_name;
mod a_constant_built_from_others;
mod a_constant_the_file_tests_for;
mod a_constructor_argument_and_its_template_bound;
mod a_container_parameters_shape;
mod a_contract_spelled_the_way_mockery_and_laravel_do;
mod a_contract_written_where_it_is_declared;
mod a_contracted_false;
mod a_declaration_php_refuses;
mod a_declaration_that_should_not_be_reached_for;
mod a_declared_non_empty_array;
mod a_declared_non_empty_string;
mod a_decode_the_caller_left_lenient;
mod a_docblock_a_comment_stands_under;
mod a_docblock_naming_nothing;
mod a_docblock_the_framework_publishes;
mod a_documented_callbacks_parameter;
mod a_false_argument;
mod a_false_receiver;
mod a_false_returned;
mod a_foreach_over_a_sentinel;
mod a_form_key_the_form_built;
mod a_frameworks_invoked_callback;
mod a_global_a_docblock_types;
mod a_guard_in_a_finished_statement;
mod a_guard_on_a_bare_call;
mod a_guard_that_names_no_kind;
mod a_guard_that_names_several_classes;
mod a_guard_that_rules_out_the_only_kind;
mod a_helper_returning_a_local;
mod a_hex_written_at_the_call;
mod a_hook_a_call_dispatches;
mod a_join_that_keeps_the_null;
mod a_key_a_function_reads;
mod a_key_a_shape_does_not_name;
mod a_key_a_shape_nearly_declares;
mod a_key_no_key_of_its_container_can_be;
mod a_key_that_is_not_the_first_argument;
mod a_key_the_shape_says_may_be_absent;
mod a_keyword_written_with_a_slash;
mod a_kind_guard_on_a_closed_class;
mod a_label_php_refuses;
mod a_literal_written_into_since;
mod a_local_a_call_writes_back;
mod a_local_a_destructuring_binds;
mod a_loop_variable_bound_more_than_once;
mod a_magic_constant_is_a_literal;
mod a_match_arm_past_two_guards;
mod a_match_that_leaves_a_case_out;
mod a_member_checked_before_leaving;
mod a_member_of_the_composing_class;
mod a_member_only_a_template_calls;
mod a_member_reached_on_object;
mod a_method_a_subclass_implements;
mod a_method_a_test_double_is_configured_with;
mod a_method_a_trait_alias_names;
mod a_method_its_call_reopens;
mod a_mixed_hint_a_docblock_names_a_class_beside;
mod a_mixed_reference_takes_away_what_was_known;
mod a_name_declared_twice_or_a_sealed_parent;
mod a_name_the_file_tests_for;
mod a_name_with_nothing_to_refer_to;
mod a_named_argument_against_its_parameter;
mod a_named_argument_signature;
mod a_namespace_of_functions;
mod a_native_type_php_refuses;
mod a_null_test_settled_by_the_type;
mod a_null_the_author_wrote;
mod a_null_the_parameter_takes;
mod a_nullable_value_returned;
mod a_parameter_list_php_refuses;
mod a_parameter_that_repeats_nothing;
mod a_pipe_is_the_call_it_makes;
mod a_presence_test_on_an_element;
mod a_private_member_the_class_never_uses;
mod a_promoted_propertys_own_docblock;
mod a_property_isset_asks_about;
mod a_protected_member_of_a_relative;
mod a_read_only_tag_written;
mod a_receiver_a_call_changed;
mod a_receiver_its_matched_chain_proves_there;
mod a_record_missing_a_required_key;
mod a_resource;
mod a_result_the_declaration_says_to_keep;
mod a_return_decided_by_the_shape_of_args;
mod a_return_decided_by_what_a_string_holds;
mod a_return_that_says_only_array;
mod a_return_the_declaration_refuses;
mod a_scope_called_on_a_builder;
mod a_sentinel_against_a_bound_template;
mod a_sentinel_written_to_a_typed_property;
mod a_service_fetched_by_a_class_the_container_builds;
mod a_signature_that_repeats_nothing;
mod a_slot_filled_where_a_guard_held;
mod a_static_call_only_call_answers;
mod a_static_method_named_by_a_string;
mod a_static_return_of_another_class;
mod a_subscript_of_a_nullable;
mod a_subscript_that_is_a_lookup;
mod a_tag_no_tool_reads;
mod a_tagged_column_and_a_count_in_a_list;
mod a_template_a_trait_declares;
mod a_test_closure_a_suite_configures;
mod a_theme_hook_a_render_array_names;
mod a_throw_a_call_carries_up;
mod a_throw_its_own_catch_handles;
mod a_throw_that_is_a_defect;
mod a_throw_the_docblock_does_not_declare;
mod a_trait_body_checked_as_its_host;
mod a_trait_names_what_composes_it;
mod a_trait_read_through_its_hosts;
mod a_tuple_a_foreach_takes_apart;
mod a_tuple_shapes_element;
mod a_type_argument_no_value_can_be;
mod a_type_naming_no_class;
mod a_type_the_cursor_jumps_to;
mod a_type_the_language_writes_relatively;
mod a_union_of_shapes;
mod a_union_that_holds_mixed;
mod a_union_that_swallows_its_own_precision;
mod a_use_of_what_is_going_away;
mod a_value_a_loop_carries_round;
mod a_value_an_anonymous_class_makes;
mod a_value_built_in_every_arm;
mod a_value_that_may_not_be_callable;
mod a_value_used_as_it_cannot_be;
mod a_var_above_a_loop_header;
mod a_variadic_documented_as_its_array;
mod a_where_a_column_spells;
mod a_wrapper_the_project_bound_itself;
mod a_write_still_being_typed;
mod a_write_that_cannot_be_null;
mod a_write_the_declaration_refuses;
mod action_accessors;
mod action_annotation;
mod action_constant;
mod action_constructor;
mod action_defaulted;
mod action_doc_tags;
mod action_extract;
mod action_fix_all;
mod action_implement;
mod action_inheritdoc;
mod action_inline;
mod action_kinds;
mod action_organize_imports;
mod action_promote;
mod action_unused_import;
mod actions;
mod an_anonymous_classs_arguments;
mod an_argument_against_a_name_that_names_nothing;
mod an_argument_against_a_template;
mod an_argument_the_method_completes;
mod an_argument_written_back;
mod an_array_literal_handed_to_a_template;
mod an_assertion_written_as_a_statement;
mod an_element_written_into_a_typed_parameter;
mod an_enum_has_what_php_gives_it;
mod an_error_control_operator_is_transparent;
mod an_identity_against_a_literal;
mod an_if_both_ways_out_of;
mod an_input_named_by_a_read;
mod an_input_no_signature_declares;
mod an_interpolated_string_is_a_string;
mod an_object_nothing_can_extend;
mod an_object_nothing_declared_relates;
mod an_object_used_as_what_its_class_is_not;
mod an_out_parameter_in_every_position;
mod an_out_parameter_of_a_constructor;
mod an_out_parameter_survives_a_by_value_call;
mod an_output_parameter_of_the_runtime;
mod an_override_attribute_with_nothing_above_it;
mod applied_returns;
mod argument_returns;
mod arguments;
mod assertion_hover;
mod assumed_non_null;
mod attributes;
mod bindings;
mod broken_files;
mod by_reference;
mod call_brackets;
mod call_hierarchy;
mod callbacks;
mod callbacks_in_a_chain;
mod catalog_places;
mod catalog_places_on_a_receiver;
mod catalog_reports;
mod class_of_an_instance;
mod class_strings;
mod closures;
mod completion;
mod completion_after_a_closure_holding_a_comment;
mod completion_after_a_function_its_contract_names;
mod completion_of_a_macro;
mod completion_on_a_class_named_by_a_value;
mod completion_on_a_value_being_replaced;
mod completion_through_a_conditional_class_string_return;
mod composites;
mod conditional_returns;
mod config_diagnostics;
mod constant_values;
mod contracts;
mod diagnostics;
mod dispatched_hooks;
mod doc_types;
mod docblock_members;
mod docblock_rules;
mod docblocks;
mod documented_hooks;
mod documented_hooks_with_a_placeholder;
mod effective_types;
mod either_of;
mod element_arguments;
mod element_writes;
mod emptied_collectors;
mod engine_variables;
mod enums;
mod existence_checks;
mod extra_arguments;
mod features;
mod fluent;
mod forwarded_catalog;
mod forwarded_catalog_reports;
mod functions;
mod generated_catalog_keys;
mod generics;
mod goto_documented_parameter;
mod goto_inheritdoc;
mod goto_instanceof;
mod guards;
mod hints;
mod hover_says_what_a_call_gives_back_here;
mod implementations;
mod imports;
mod inherited_docblocks;
mod instance_arguments;
mod invoking;
mod iteration;
mod key_arguments;
mod keywords;
mod literal_dates;
mod member_access;
mod misnamed_implementations;
mod missing_returns;
mod mixins;
mod named_arguments;
mod narrowing;
mod nesting;
mod non_array_subscript;
mod nullable;
mod operators;
mod override_completion;
mod overrides;
mod placeholders;
mod psr4;
mod read_before_assignment;
mod receiver_agreement;
mod references;
mod references_to_a_catalog_key;
mod references_to_a_method_read_as_a_property;
mod relative;
mod rename_a_documented_parameter;
mod rename_a_parameter_callers_name;
mod rename_a_property_a_tag_documents;
mod rename_a_tabled_property;
mod rename_alias;
mod rename_family;
mod rename_promoted;
mod rename_refusals;
mod semantic_tokens;
mod shape_key_completion;
mod shape_keys;
mod signature;
mod statement_terminator;
mod subscripts;
mod symbols;
mod the_span_a_jump_is_asked_from;
mod two_traits_that_give_one_name;
mod type_navigation;
mod unbound;
mod uncoercible_literals;
mod undefined_functions;
mod unimplemented;
mod unreachable_code;
mod untyped_class_constants;
mod unused;
mod variable_hover;
mod variables;
mod visibility;
mod visibility_errors;
mod widened_param_docs;
mod written_casts;

use std::fmt::Write as _;

use expect_test::Expect;
use phrost_harness::{World, with_open_scopes};
use phrost_model::{FileId, TextSize};
use phrost_testkit::FixtureSpec;
use phrost_view::IndexView;

use crate::{HoverBlock, completions, goto_definition, hover};

/// One thing to ask at a marker.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Query {
    Goto {
        label: &'static str,
        marker: &'static str,
    },
    /// Where goto-**type**-definition lands, which is a different question from [`Self::Goto`]:
    /// one jumps to the declaration of the thing under the cursor and the other to the declaration
    /// of its type.
    GotoType {
        label: &'static str,
        marker: &'static str,
    },
    Hover {
        label: &'static str,
        marker: &'static str,
    },
    Completion {
        label: &'static str,
        marker: &'static str,
    },
    Signature {
        label: &'static str,
        marker: &'static str,
    },
}

impl Query {
    pub(crate) fn goto(label: &'static str, marker: &'static str) -> Self {
        Self::Goto { label, marker }
    }

    pub(crate) fn goto_type(label: &'static str, marker: &'static str) -> Self {
        Self::GotoType { label, marker }
    }

    pub(crate) fn hover(label: &'static str, marker: &'static str) -> Self {
        Self::Hover { label, marker }
    }

    pub(crate) fn completion(label: &'static str, marker: &'static str) -> Self {
        Self::Completion { label, marker }
    }

    pub(crate) fn signature(label: &'static str, marker: &'static str) -> Self {
        Self::Signature { label, marker }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Goto { label, .. }
            | Self::GotoType { label, .. }
            | Self::Hover { label, .. }
            | Self::Completion { label, .. }
            | Self::Signature { label, .. } => label,
        }
    }

    fn marker(self) -> &'static str {
        match self {
            Self::Goto { marker, .. }
            | Self::GotoType { marker, .. }
            | Self::Hover { marker, .. }
            | Self::Completion { marker, .. }
            | Self::Signature { marker, .. } => marker,
        }
    }
}

pub(crate) fn check(fixture: &str, queries: &[Query], expect: Expect) {
    check_with(fixture, queries, |_| {}, expect);
}

/// The same, with the project's framework knowledge stated first.
///
/// A separate entry point rather than a parameter, so the many tests about what PHP itself offers
/// keep reading as tests about PHP.
pub(crate) fn check_with(
    fixture: &str,
    queries: &[Query],
    state: impl FnOnce(&mut World),
    expect: Expect,
) {
    let spec = FixtureSpec::parse(fixture);
    // Before the files, because a fact derived at ingest -- an extension point a call declares --
    // is never re-read: see `World::build_with`.
    let world = World::build_with(&spec, state);
    let mut out = String::new();

    for query in queries {
        let position = spec.markers().position(query.marker());
        let file = world.file_id(&position.path);
        let offset = position.offset;

        let answer = with_open_scopes(&world, Some(file), |open| {
            let view = IndexView::in_file(world.skeleton(), open, file);
            render(&world, &view, *query, offset)
        });

        let _ = writeln!(out, "{}", query.label());
        out.push_str(&answer);
    }

    spec.markers().assert_all_used();
    expect.assert_eq(&format!("{}\n", out.trim_end()));
}

fn render(world: &World, view: &IndexView<'_>, query: Query, offset: TextSize) -> String {
    let mut out = String::new();

    match query {
        Query::Goto { .. } | Query::GotoType { .. } => {
            // Asked first, as the server asks it: a key naming a place is disjoint from a name PHP
            // declares, and a test that skipped this would read a working jump as no answer.
            if let Query::Goto { .. } = query
                && let Some(external) = crate::goto_external(view, offset)
            {
                return format!(
                    "- `{}` @ {} {}:{}\n",
                    external.key,
                    external
                        .path
                        .strip_prefix(world.root())
                        .unwrap_or(&external.path)
                        .display(),
                    external.line + 1,
                    external.character + 1,
                );
            }
            let targets = match query {
                Query::GotoType { .. } => crate::navigation::goto_type_definition(view, offset),
                _ => goto_definition(view, offset),
            };
            if targets.is_empty() {
                return "- (nothing to go to)\n".to_owned();
            }
            for target in targets {
                let container = target
                    .container
                    .as_ref()
                    .map(|container| format!("{container}::"))
                    .unwrap_or_default();
                let _ = writeln!(
                    out,
                    "- {} {container}{} @ {} {} (full {})",
                    target.kind.as_str(),
                    target.name,
                    path_of(world, target.file),
                    world.render_range(target.file, target.selection_range),
                    world.render_range(target.file, target.full_range),
                );
            }
        }
        Query::Completion { .. } => {
            let list = completions(view, offset);
            if list.items.is_empty() {
                return "- (nothing offered)\n".to_owned();
            }
            for item in &list.items {
                let _ = write!(out, "- {} [{}]", item.label, item.kind.as_str());
                if let Some(detail) = &item.detail {
                    let _ = write!(out, "  {detail}");
                }
                // Shown only when it differs from the label, which is exactly when it matters: it
                // means accepting the item writes something other than what was read.
                //
                // On one line with the breaks spelled out, and the cursor marked where it lands. A
                // generated declaration is several lines of PHP, and the thing worth checking about
                // it is that every character is where it should be.
                if let Some(insertion) = &item.insert
                    && insertion.text != item.label
                {
                    let _ = write!(out, "  insert `{}`", marked(insertion));
                }
                // Where the item sits in the list, which this rendering cannot show by order: it
                // prints alphabetically so a snapshot does not move when an unrelated member is
                // added. The decision is this layer's -- the protocol only carries it as a
                // `sortText` -- and a test that showed nothing here would pass while a demoted
                // member sat first.
                if item.demoted {
                    let _ = write!(out, "  shown last");
                }
                // The tag a client draws the row struck through for. Rendered for the reason above:
                // the decision is this layer's, the encoding is the protocol's, and a test that
                // showed nothing here would pass while every row read as current.
                if item.deprecated {
                    let _ = write!(out, "  deprecated");
                }
                // The brackets accepting it writes, and where the cursor lands. Rendered because the
                // decision is this layer's and the encoding is the protocol's: a test here that did
                // not show it would pass while methods completed to a bare name.
                if let Some(call) = &item.call {
                    let between = match call.shape {
                        crate::CallShape::Empty => "",
                        crate::CallShape::Arguments => "|",
                    };
                    // The `;` is rendered here for the same reason the brackets are: it is a
                    // decision this layer makes, and a position that took one it should not have
                    // leaves a buffer that no longer parses.
                    let end = if call.terminates { ";" } else { "" };
                    let _ = write!(out, "  writes `({between}){end}`");
                }
                // The `use` line, rendered because accepting the item writes it: a snapshot that
                // showed only the name would hide half of what the edit does.
                if let Some(import) = &item.import {
                    let _ = write!(
                        out,
                        "  + `{}` at {}",
                        import.text.trim_start_matches('\n'),
                        view.current_file_id().map_or_else(
                            || "?".to_owned(),
                            |file| world
                                .render_range(file, phrost_model::TextRange::empty(import.at))
                        )
                    );
                }
                let _ = writeln!(out);
            }
            // Two reasons and one flag: a list cut short by a limit, and one filtered by what was
            // already typed. Both mean the same thing to a client -- ask again rather than narrow
            // this -- so the line says that rather than guessing which it was.
            if !list.is_complete {
                let _ = writeln!(out, "- ... and the client must ask again");
            }
            // What accepting the first item would leave behind. Rendered because the range is the
            // half of a completion nothing else shows: an item with a perfect label that replaces one
            // character too many produces a file that no longer parses, and every other contract
            // here would still pass.
            //
            // The first item a *client* would offer, which is not `items.first()`: the list above
            // prints alphabetically so a snapshot does not move when an unrelated member is added,
            // and a demoted item sorts before every plain one under that rule while the protocol
            // puts it last. Taking the alphabetical first said `accepting the first leaves
            // $stage->cases` on a line whose own marker read `shown last`, which is a snapshot
            // contradicting itself.
            if let Some(first) = list
                .items
                .iter()
                .find(|item| !item.demoted)
                .or_else(|| list.items.first())
                && let Some(file) = view.current_file_id()
            {
                let text = world.text_of(file);
                let inserted = first
                    .insert
                    .as_ref()
                    .map_or_else(|| first.label.clone(), marked);
                let start = usize::from(list.replace.start());
                let end = usize::from(list.replace.end());
                let line = text[..start].rfind('\n').map_or(0, |at| at + 1);
                let after = text[line..].find('\n').map_or(text.len(), |at| line + at);
                let _ = writeln!(
                    out,
                    "- accepting the first leaves `{}{inserted}{}`",
                    text[line..start].trim_start(),
                    &text[end..after]
                );
            }
        }
        Query::Signature { .. } => match crate::signature_help(view, offset) {
            None => return "- (no signature)\n".to_owned(),
            Some(help) => {
                let _ = writeln!(out, "- {}", help.label);
                // Marked rather than reported as an index, because the point of the feature is
                // which parameter is pointed at, and an index in a snapshot has to be counted by
                // hand against the label above it.
                for (position, parameter) in help.parameters.iter().enumerate() {
                    let mark = if help.active == Some(position) {
                        ">"
                    } else {
                        " "
                    };
                    let _ = writeln!(out, "  {mark} {parameter}");
                }
                if help.active.is_none() {
                    let _ = writeln!(out, "  (no parameter is being filled)");
                }
            }
        },
        Query::Hover { .. } => match hover(view, offset) {
            None => return "- (nothing to describe)\n".to_owned(),
            Some(info) => {
                // Rendered per variant, so a snapshot shows which kind of block each line is.
                // That is the distinction the protocol layer escapes differently, and a snapshot
                // that hid it would let the two be swapped without any test noticing.
                for block in info.blocks {
                    let _ = match block {
                        HoverBlock::Code(code) => writeln!(out, "- code: {code}"),
                        HoverBlock::Documentation(text) => writeln!(out, "- doc: {text}"),
                        HoverBlock::Note(text) => writeln!(out, "- note: {text}"),
                        HoverBlock::Tag {
                            label,
                            ty,
                            subject,
                            description,
                        } => writeln!(
                            out,
                            "- tag: {label}{}{} {description}",
                            subject.map(|s| format!(" {s}")).unwrap_or_default(),
                            ty.map(|t| format!(" {t}")).unwrap_or_default(),
                        ),
                        HoverBlock::Separator => writeln!(out, "- ---"),
                    };
                }
            }
        },
    }

    out
}

fn path_of(world: &World, file: FileId) -> &str {
    world.path_of(file)
}

/// What an accepted item writes, on one line, with the cursor shown where it lands.
///
/// A generated declaration is several lines of PHP, and a snapshot spread over them is a snapshot
/// whose indentation nobody can check by eye -- so the breaks are spelled `\n`, the tabs `\t`, and
/// the cursor is a `|`. All three are countable; a real tab in a snapshot is not even visible.
fn marked(insertion: &crate::completion::Insertion) -> String {
    let text = match insertion.cursor {
        Some(at) => format!("{}|{}", &insertion.text[..at], &insertion.text[at..]),
        None => insertion.text.clone(),
    };
    text.replace('\n', "\\n").replace('\t', "\\t")
}
