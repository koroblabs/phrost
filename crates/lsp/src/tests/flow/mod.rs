//! What the server does in sequence, split into one file per scenario.
//!
//! Every test here drives a real `Backend` in process, in order, with state carried between steps —
//! that is what makes a sequencing bug visible instead of "it works sometimes". Splitting by
//! scenario means two people adding unrelated flow tests add two files, not two hunks in the one
//! file every flow test used to share. Fixtures and renderers more than one scenario needs live
//! here so nothing below duplicates them.

mod a_blade_template;
mod a_configuration_buffer_is_checked;
mod a_constructor_reached_by_new;
mod a_handler_named_after_prose;
mod a_livewire_template;
mod a_test_the_runner_catches;
mod a_twig_template;
mod a_twig_variable;
mod a_value_that_answers_anything;
mod accepted_completion;
mod an_id_completed_past_its_dot;
mod an_included_template;
mod call_hierarchy;
mod cancel;
mod code_the_project_does_not_own;
mod config_references;
mod configuration;
mod deprecation_tags;
mod diagnostics;
mod document_lifecycle;
mod file_rename;
mod folding;
mod foreign_languages;
mod goto_links;
mod hover_and_completion;
mod lenses;
mod navigation_shape;
mod panics;
mod project_diagnostics;
mod project_wide_operations;
mod readiness_and_progress;
mod startup;
mod tokens;
mod unused_declarations;
mod workspace_symbol_members;

use serde_json::Value;

/// Two files: one declares a class, the other extends it and carries the cursor.
const TWO_FILES: &str = r#"
//- /src/Base.php
<?php
namespace App;

class Base
{
    public function run(): void {}
}

//- /src/Repo.php
<?php
namespace App;

class Repo extends Ba@parent@se {}
"#;

/// Renders a `WorkspaceEdit`, filename-relative and in a stable order.
///
/// Shared because two things answer with one: renaming a symbol, and renaming the file a class
/// lives in.
pub(crate) fn render_edits(response: &lspkit_runtime::protocol::jsonrpc::Response) -> Vec<String> {
    let value = serde_json::to_value(response.result().expect("a result")).expect("json");
    let mut out = Vec::new();
    for (uri, edits) in value["changes"].as_object().into_iter().flatten() {
        let file = uri.rsplit('/').next().unwrap_or(uri).to_owned();
        for edit in edits.as_array().into_iter().flatten() {
            out.push(format!(
                "{file} {} -> {}",
                render_span(&edit["range"]),
                edit["newText"].as_str().unwrap_or("?")
            ));
        }
    }
    out.sort();
    out
}

pub(crate) fn render_span(range: &Value) -> String {
    format!(
        "{}:{}-{}:{}",
        range["start"]["line"].as_i64().unwrap_or(-1),
        range["start"]["character"].as_i64().unwrap_or(-1),
        range["end"]["line"].as_i64().unwrap_or(-1),
        range["end"]["character"].as_i64().unwrap_or(-1)
    )
}
