//! Canonical text rendering of a [`FileScope`].
//!
//! Same contract as the HIR renderer: source order, one-based positions, and nothing that could
//! differ between two builds of the same input. Snapshot tests and the
//! `incremental == from-scratch` invariant both depend on that.

use std::fmt::Write as _;

use phrost_hir::ImportKind;
use phrost_model::{LineIndex, namespace_of, short_name};

use crate::scope::FileScope;

/// Renders a file's namespaces, their imports, and the symbols it declares.
pub fn render_file_scope(scope: &FileScope, lines: &LineIndex) -> String {
    let mut out = String::new();

    for namespace in scope.scopes() {
        let header = namespace
            .name()
            .map_or_else(|| "<global>".to_owned(), |name| name.as_str().to_owned());

        // A scope with nothing in it is noise -- except the global one, which is worth showing
        // when it is the only scope so that "no namespace declared" reads as a fact.
        let imports = [
            ImportKind::ClassLike,
            ImportKind::Function,
            ImportKind::Constant,
        ]
        .into_iter()
        .flat_map(|kind| {
            namespace
                .imports()
                .table(kind)
                .iter()
                .map(move |binding| (kind, binding))
        })
        .collect::<Vec<_>>();

        let declares = scope
            .symbols()
            .iter()
            .filter(|symbol| {
                namespace_of(symbol.fqn.as_str()) == namespace.name().map(|name| name.as_str())
            })
            .collect::<Vec<_>>();

        if imports.is_empty() && declares.is_empty() && scope.scopes().len() > 1 {
            continue;
        }

        let _ = writeln!(out, "scope {header}");

        for (kind, binding) in imports {
            let _ = write!(out, "  use {}", binding.path());
            if binding.local().as_str() != short_name(binding.path().as_str()) {
                let _ = write!(out, " as {}", binding.local());
            }
            let _ = writeln!(out, "  [{}]", kind.as_str());
        }

        for symbol in declares {
            let (start, end) = lines.range_line_cols(symbol.name_range);
            let _ = writeln!(
                out,
                "  {} {} @ {}:{}-{}:{}",
                symbol.kind.as_str(),
                symbol.fqn,
                start.line + 1,
                start.col + 1,
                end.line + 1,
                end.col + 1,
            );
        }
    }

    if out.is_empty() {
        return "<no scopes>\n".to_owned();
    }

    out
}
