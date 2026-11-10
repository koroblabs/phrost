//! Completion, split by the question each group asks.
//!
//! Separate files so two people adding unrelated completion behaviour add two files rather than
//! appending to one — the shared test file this used to be was the largest in the project and the
//! only place every completion change had to touch.

mod a_function_calls_result;
mod callable_pairs;
mod catalogs;
mod destructured_locals;
mod doc_tags;
mod docblock_names;
mod docblock_parameters;
mod docblock_spelling;
mod docblock_templates;
mod docblock_types;
mod documented_keys;
mod headers;
mod import_aliases;
mod imports;
mod instantiation;
mod matching;
mod names;
mod namespace_paths;
mod opening_tag;
mod operators;
mod paths_in_strings;
mod qualified_functions;
mod receivers;
mod static_forwarding;
mod tabled_properties;
mod the_enclosing_class_by_name;
mod the_other_two_uses;
mod type_positions;
mod use_paths;
