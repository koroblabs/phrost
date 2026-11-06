//! The allowed-edges matrix for the workspace, and the check that enforces it.
//!
//! Only `phrost-*`, `mago-*` and `lspkit-*` dependencies are policed. Ordinary third-party crates
//! are a per-crate choice; the graph between our own layers, the reach of the PHP parser, and which
//! layers take the base shared with orthra are not.
//!
//! The rules are phrost's and live here. Reading the manifests and matching them is
//! `lspkit_arch::edges`, which also reads what this check used to skip: a dependency renamed with
//! `package =`, one under `[target.'cfg(..)']`, and `[build-dependencies]`.

use lspkit_arch::{
    edges::{Edges, Foreign, Pattern, Rule},
    workspace::Workspace,
};

/// Prefixes of dependency names this check has an opinion about.
const POLICED: &[&str] = &["phrost-", "mago-", "lspkit-"];

/// Crates permitted to name a [`PARSER_CRATES`] type.
const MAY_USE_PARSER: &[&str] = &["phrost-syntax", "phrost-lower", "phrost-syntax-features"];

/// The mago crates whose types make up the parse tree, or appear inside it.
///
/// These are what the parser boundary protects against. mago's tree is arena-bound
/// (`Program<'arena>`) and its shape already changed between the versions this project has used,
/// so any crate that can name one of these types is a crate a parser upgrade can break.
///
/// Other mago crates are not parse-tree-shaped — `mago-composer` is a `composer.json` schema,
/// `mago-formatter` is a formatter — and are granted per crate through the normal allow-list.
const PARSER_CRATES: &[Foreign] = &[
    parse_tree("mago-syntax"),
    parse_tree("mago-syntax-core"),
    parse_tree("mago-phpdoc-syntax"),
    parse_tree("mago-allocator"),
    parse_tree("mago-span"),
    parse_tree("mago-database"),
    parse_tree("mago-word"),
    parse_tree("mago-bytes"),
];

/// One crate of the parse tree, confined to [`MAY_USE_PARSER`] in every kind of dependency.
const fn parse_tree(name: &'static str) -> Foreign {
    Foreign {
        pattern: Pattern::Exact(name),
        owners: MAY_USE_PARSER,
        why: "parse-tree types stay behind the parser boundary. Lower the tree to `phrost-hir` and \
              depend on that instead",
    }
}

/// Test-only dependencies every crate may take, on top of its own rule.
///
/// The fixture harness is not a layer; it describes inputs. Letting it in everywhere is what
/// makes per-layer testing possible at all.
const DEV_EXTRA: &[&str] = &[
    "phrost-testkit",
    // Builds an indexed project from a fixture. Test-only, and the reason it is allowed everywhere
    // is the same as the fixture parser: without it every layer above the index reinvents the same
    // hundred lines of scaffolding, and a shared file that every test edits is a conflict magnet.
    "phrost-harness",
    // A layer's own tests need real PHP as input, and producing it means parsing and lowering.
    // Allowed only as a dev-dependency: a test may reach down to build a fixture, but the crate's
    // own code still may not.
    "phrost-syntax",
    "phrost-lower",
    // And the ids those two are driven with. Without it the pair above is unusable from a crate
    // whose own code has no reason to name a `FileId`.
    "phrost-model",
    // The `incremental == from scratch` sweep, shared with orthra: a test of any layer that keeps
    // state across edits may run it.
    "lspkit-fuzz",
];

/// The declared target graph.
///
/// Crates that do not exist yet are listed so the rule is already in place when they land. A
/// crate present in the workspace but missing here is an error: adding a crate must be a
/// deliberate statement about what it may reach.
///
/// `lspkit-*` crates are not members, so they have no rule here; lspkit checks its own graph. What
/// is policed here is which of phrost's crates may take them.
const RULES: &[Rule] = &[
    // --- foundations: no phrost dependencies, or only the ones below them ---
    // The interner is the only holder of a name's bytes -- every `Name` in the project is an
    // `Arc<str>` pointing here -- so it is the only place that can report them. Measuring it from
    // outside would mean exposing its tables to do it.
    Rule::new("phrost-intern", &["lspkit-memsize"]),
    Rule::new("phrost-profile", &[]),
    // `lspkit-text` is `LineIndex`/`LineCol`, moved out of this crate: shared with orthra through lspkit.
    Rule::new(
        "phrost-model",
        &["lspkit-arena", "lspkit-text", "phrost-intern"],
    ),
    // The type *vocabulary*: resolved type references and the table that interns them. Below the
    // IR layers on purpose -- a member row carries a type, so anything that stores members needs
    // to name one. Inference is not here; it needs to walk a hierarchy, so it lives in
    // `phrost-body` above `phrost-sema`.
    //
    // `lspkit-memsize` for the same reason as the interner: the table owns its nodes and its lookup
    // maps, and the point of the memory report is to show that interning keeps them small.
    Rule::new("phrost-ty", &["phrost-model", "lspkit-memsize"]),
    // The PHP standard library as source text. Depends on nothing of ours: it produces file paths,
    // and what is in those files is the index's problem.
    Rule::new("phrost-stubs", &[]),
    // Twig's syntax, which mago does not know: the one crate that does, as `phrost-syntax` is for
    // PHP. A leaf, because a template's tokens mean nothing without the loader that resolves what
    // they name -- and the loader is configured by a framework, far above.
    Rule::new("phrost-twig", &[]),
    // The fixture syntax is lspkit's, shared with orthra; what the flags mean is phrost's.
    Rule::new("phrost-testkit", &["phrost-model", "lspkit-fixture"]),
    // The structural checks, built on lspkit's engine for them.
    Rule::new("phrost-checks", &["lspkit-arch"]),
    // Test-only scaffolding: builds an indexed project from a fixture. It reaches the index on
    // purpose, which is why nothing may depend on it outside `[dev-dependencies]`.
    Rule::new(
        "phrost-harness",
        &["phrost-model", "phrost-index", "phrost-testkit"],
    ),
    // --- text and project inputs ---
    Rule::new(
        "phrost-vfs",
        &["phrost-model", "phrost-profile", "lspkit-memsize"],
    ),
    Rule::new(
        "phrost-project",
        &[
            // The walk that finds a project's files: lspkit's, shared with orthra.
            "lspkit-walk",
            "phrost-model",
            "phrost-stubs",
            "phrost-vfs",
            "phrost-profile",
            "lspkit-memsize",
            // A composer.json schema, not a parse tree: safe to name directly.
            "mago-composer",
        ],
    ),
    // --- the parser boundary ---
    // `phrost-syntax` is the only crate that knows mago exists; `phrost-lower` is the only one
    // that knows the shape of its tree. Nothing else may name either of them, except the
    // deliberately narrow syntax-only feature crate.
    Rule::new(
        "phrost-syntax",
        &["phrost-model", "phrost-vfs", "phrost-profile", "mago-*"],
    ),
    Rule::new(
        "phrost-lower",
        &[
            "phrost-model",
            "phrost-syntax",
            "phrost-hir",
            "lspkit-arena",
            "phrost-intern",
            "phrost-profile",
            // Walks the parse tree, so it names its node types by definition.
            "mago-*",
        ],
    ),
    Rule::new(
        "phrost-syntax-features",
        &["phrost-model", "phrost-syntax", "phrost-profile", "mago-*"],
    ),
    // --- indexed IR layers ---
    Rule::new(
        "phrost-hir",
        &[
            "phrost-model",
            "lspkit-arena",
            "phrost-intern",
            "lspkit-memsize",
        ],
    ),
    Rule::new(
        "phrost-defmap",
        &[
            "phrost-model",
            "phrost-hir",
            "phrost-project",
            "lspkit-arena",
            "lspkit-memsize",
            "phrost-profile",
        ],
    ),
    Rule::new(
        "phrost-sema",
        &[
            "phrost-model",
            "phrost-hir",
            "phrost-defmap",
            "phrost-ty",
            "lspkit-arena",
            "lspkit-memsize",
            "phrost-profile",
        ],
    ),
    Rule::new(
        "phrost-body",
        &[
            "phrost-model",
            "phrost-hir",
            "phrost-defmap",
            "phrost-sema",
            "phrost-ty",
            "lspkit-arena",
            "lspkit-memsize",
            "phrost-profile",
        ],
    ),
    // --- the store, and the one API features may see ---
    Rule::new(
        "phrost-index",
        &[
            "phrost-model",
            "phrost-vfs",
            "phrost-project",
            "phrost-lower",
            "phrost-hir",
            "phrost-defmap",
            "phrost-sema",
            "phrost-ty",
            "phrost-body",
            "lspkit-arena",
            "phrost-intern",
            "lspkit-memsize",
            "phrost-profile",
        ],
    ),
    Rule::new(
        "phrost-view",
        &[
            "phrost-model",
            "phrost-index",
            // A Twig buffer is answered here the way a configuration buffer is -- names read from
            // the text, catalogs saying what each is -- and finding the names takes Twig's syntax.
            "phrost-twig",
            "phrost-hir",
            "phrost-defmap",
            "phrost-sema",
            "phrost-ty",
            "phrost-body",
            "lspkit-memsize",
            "phrost-profile",
        ],
    ),
    // --- features ---
    // Note what is absent: no store crate, no parser, no VFS. A feature that needs a fact it
    // cannot get from `phrost-view` is telling you the view is missing a projection.
    Rule::new(
        "phrost-framework",
        &[
            "phrost-model",
            "phrost-view",
            "phrost-vfs",
            "phrost-profile",
        ],
    ),
    Rule::new(
        "phrost-analysis",
        &[
            "phrost-model",
            "phrost-view",
            "phrost-framework",
            "phrost-syntax-features",
            "phrost-profile",
        ],
    ),
    Rule::new(
        "phrost-diagnostics",
        &[
            "phrost-model",
            "phrost-view",
            "phrost-framework",
            "phrost-profile",
        ],
    ),
    // --- protocol and tooling ---
    Rule::new(
        "phrost-lsp",
        &[
            "phrost-model",
            "phrost-stubs",
            "phrost-vfs",
            "phrost-project",
            "phrost-index",
            "phrost-view",
            "phrost-analysis",
            "phrost-diagnostics",
            "phrost-framework",
            "phrost-syntax-features",
            "phrost-profile",
            "lspkit-memsize",
            // Panic containment, write poisoning and the buffered transport: what makes this a
            // server that survives a bug in a handler and a client that reads slowly.
            "lspkit-runtime",
            // The one crate allowed `unsafe`, for two foreign calls that hand freed memory back to
            // the operating system. Only the server and the CLI may take it, which is what keeps
            // the exception one file long.
            "lspkit-heap",
            // Dev-dependency only: the flow-test harness is shared with orthra through lspkit.
            "lspkit-flow",
        ],
    ),
    // The CLI is developer tooling: `analyze`, `perf`, `compare-lsp`, `dump-index`. It is the one
    // crate allowed to reach anywhere, because inspecting a layer is its whole purpose -- lspkit's
    // crates included, since it hands memory back after a sweep as the server does.
    Rule::new("phrost-cli", &["phrost-*", "lspkit-*"]),
];

/// The whole policy, as `lspkit_arch` checks it.
const EDGES: Edges<'static> = Edges {
    policed: POLICED,
    rules: RULES,
    dev_everywhere: DEV_EXTRA,
    foreign: PARSER_CRATES,
};

/// Checks every workspace member against [`RULES`], and the rules against themselves.
///
/// Not `Edges::stale_rules`: a crate is listed here before it exists, deliberately, so a rule
/// with no member is the plan and not a leftover.
pub fn check() -> Result<(), String> {
    let workspace = Workspace::above(env!("CARGO_MANIFEST_DIR"), 3);
    let mut problems = EDGES.violations(workspace.manifests());
    // Dependencies point downward, so a permission that closes a loop is wrong before anybody
    // builds the edge.
    problems.extend(EDGES.cycles());
    if problems.is_empty() {
        return Ok(());
    }
    problems.sort();
    Err(format!(
        "workspace dependency rules violated (crates/tools/checks/src/dependency_rules.rs):\n  {}\n",
        problems.join("\n  ")
    ))
}

#[cfg(test)]
mod tests {
    use lspkit_arch::workspace::{Dep, DepKind, Manifest};

    use super::*;

    /// Whether the rules refuse `krate` taking `dependency` as a dependency of `kind`.
    fn rejects(krate: &str, dependency: &str, kind: DepKind) -> bool {
        assert!(
            RULES.iter().any(|rule| rule.krate == krate),
            "{krate} has no rule"
        );
        let manifest = Manifest {
            name: krate.to_owned(),
            deps: vec![Dep {
                name: dependency.to_owned(),
                kind,
            }],
        };
        !EDGES.violations([&manifest]).is_empty()
    }

    #[test]
    fn the_parse_tree_is_reachable_only_from_the_boundary_crates() {
        assert!(!rejects("phrost-syntax", "mago-syntax", DepKind::Normal));
        assert!(!rejects(
            "phrost-lower",
            "mago-phpdoc-syntax",
            DepKind::Normal
        ));

        // The whole point of the layering: no feature or store crate may name mago, not even in
        // a test, because a test that reaches past the boundary keeps the boundary from moving.
        for krate in [
            "phrost-analysis",
            "phrost-view",
            "phrost-defmap",
            "phrost-lsp",
        ] {
            assert!(rejects(krate, "mago-syntax", DepKind::Normal), "{krate}");
            assert!(rejects(krate, "mago-syntax", DepKind::Dev), "{krate} dev");
            assert!(
                rejects(krate, "mago-syntax", DepKind::Build),
                "{krate} build"
            );
        }
    }

    #[test]
    fn features_reach_the_index_only_through_the_view() {
        assert!(!rejects("phrost-analysis", "phrost-view", DepKind::Normal));

        for store in [
            "phrost-index",
            "phrost-sema",
            "phrost-ty",
            "phrost-body",
            "phrost-hir",
        ] {
            assert!(
                rejects("phrost-analysis", store, DepKind::Normal),
                "{store}"
            );
        }
    }

    #[test]
    fn the_fixture_harness_is_allowed_everywhere_but_only_as_a_dev_dependency() {
        assert!(!rejects("phrost-analysis", "phrost-testkit", DepKind::Dev));
        assert!(rejects(
            "phrost-analysis",
            "phrost-testkit",
            DepKind::Normal
        ));
        assert!(rejects("phrost-analysis", "phrost-testkit", DepKind::Build));
    }

    #[test]
    fn the_cli_may_reach_any_phrost_crate_but_still_not_the_parse_tree() {
        assert!(!rejects("phrost-cli", "phrost-index", DepKind::Normal));
        assert!(!rejects("phrost-cli", "phrost-syntax", DepKind::Normal));
        assert!(rejects("phrost-cli", "mago-syntax", DepKind::Normal));
    }

    #[test]
    fn a_mago_crate_that_is_not_the_parse_tree_can_be_granted_normally() {
        // `mago-composer` is a composer.json schema. Confining it would mean hand-rolling a
        // second JSON parser for no benefit, so the boundary is drawn around the tree, not the
        // vendor name.
        assert!(!rejects("phrost-project", "mago-composer", DepKind::Normal));
        assert!(rejects("phrost-analysis", "mago-composer", DepKind::Normal));
    }

    #[test]
    fn only_our_own_lspkit_and_mago_dependencies_are_policed() {
        // `phrost-model` may take none of these, so each is refused exactly when it is policed.
        assert!(rejects("phrost-model", "phrost-view", DepKind::Normal));
        assert!(rejects("phrost-model", "mago-composer", DepKind::Normal));
        assert!(rejects("phrost-model", "lspkit-runtime", DepKind::Normal));
        assert!(!rejects("phrost-model", "serde", DepKind::Normal));
    }

    /// The protocol comes from `lspkit_runtime::protocol`, never from the framework under it, so
    /// that replacing the framework is a change to lspkit and a revision bump here. lspkit holds
    /// every workspace to that without being asked, the server included.
    #[test]
    fn no_crate_names_the_framework_under_the_protocol() {
        for rule in RULES {
            assert!(
                rejects(rule.krate, "tower-lsp-server", DepKind::Normal),
                "{}",
                rule.krate
            );
        }
    }

    /// `CLAUDE.md` §7: nothing may depend on the crate that writes `unsafe` but the server and the
    /// CLI. Written as a rule and not only kept as a habit, since the CLI's own rule reaches every
    /// lspkit crate.
    #[test]
    fn only_the_server_and_the_cli_may_take_the_crate_that_writes_unsafe() {
        for rule in RULES {
            let allowed = !rejects(rule.krate, "lspkit-heap", DepKind::Normal);
            let expected = matches!(rule.krate, "phrost-lsp" | "phrost-cli");
            assert_eq!(allowed, expected, "{}", rule.krate);
        }
    }

    #[test]
    fn every_rule_names_a_distinct_crate() {
        let mut names = RULES.iter().map(|rule| rule.krate).collect::<Vec<_>>();
        names.sort_unstable();
        let count = names.len();
        names.dedup();
        assert_eq!(names.len(), count, "RULES contains a duplicate crate");
    }
}
