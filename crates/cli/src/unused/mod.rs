//! Declarations nothing reaches.
//!
//! A project pass rather than a diagnostic, and the reason is the budget: deciding whether one
//! class is used means asking every file, and a file declares tens of things. That is a
//! millisecond each on a warm index and a fifth of a second per keystroke, against a diagnostics
//! budget of thirty milliseconds. So the expensive, exact question is asked once, here, the way
//! `coverage` asks its own.
//!
//! # What counts as reached
//!
//! Four sources, and leaving any of them out turns this into a report nobody trusts twice.
//!
//! - **PHP**, which is every resolved class-like name the project's own code writes. Resolved
//!   rather than spelled, so two classes of the same short name are two answers.
//! - **Configuration**, which is where a framework's rules say a value names a class -- a
//!   service's `class:`, a route's `_controller`. Read at indexing time; see `ConfigSite`.
//! - **Templates**, which call a method that no PHP file names -- and often under another name
//!   again, since Twig reads `{{ node.title }}` as `getTitle()` where the object has one. Twelve
//!   of the reference project's 124 findings were written in one. See `set_template_words`.
//! - **Discovery**, for the classes a framework finds by scanning rather than being told about.
//!   Drupal's plugin managers walk `src/Plugin/` and instantiate what is there, so nothing names
//!   those and nothing can: 232 of one real project's 246 unreferenced classes are plugins, and a
//!   report without this rule is 94 % noise. A test class is the same statement -- named by
//!   nothing, instantiated by a runner -- and is 142 more.
//!
//! # Members
//!
//! The same question with one difference that is the whole of what keeps it sound: a member is
//! reached by *name*, not by the pair of class and name. An access whose receiver has no type
//! resolves to nothing, and matching the pair would read that as nobody calling the member -- so
//! one `->run()` anywhere keeps every `run` alive. Coarse, and coarse in the direction a report
//! about absence has to be.
//!
//! A member is also left out where a class *above* it declares the same one: an implementation is
//! called through its interface, and `foreach` calls `getIterator` without writing the name. On
//! the reference project that rule alone is 121 findings down to 109.
//!
//! # What it still gets wrong, measured
//!
//! Two of the eighteen it reports on the reference project, and both are found by a *naming*
//! convention rather than by a place: Drupal instantiates `<Module>ServiceProvider` from a
//! module's `src/` root, and Drush finds its commands. Counted before deciding: there is **one**
//! `ServiceProvider` in that whole checkout, which is not a mechanism's worth of use -- so they
//! are named here instead of built around.

use std::{
    collections::{BTreeMap, HashSet},
    path::PathBuf,
    time::Instant,
};

use phrost_lsp::Workspace;
use phrost_model::SourceOrigin;
use phrost_stubs::StubSet;

/// One declaration nothing was found to reach.
struct Unreached {
    display: String,
    line: u32,
    kind: &'static str,
    fqn: String,
}

/// A member the project declares, keyed so two classes' `run` are two entries.
struct DeclaredMember {
    display: String,
    line: u32,
    what: &'static str,
    owner: String,
    name: String,
    kind: phrost_view::MemberKind,
}

/// Indexes a project, then reports the class-likes its own code declares and never reaches.
pub(crate) fn run(root: Option<PathBuf>, limit: Option<usize>) -> Result<(), String> {
    let root = match root {
        Some(root) => root,
        None => {
            std::env::current_dir().map_err(|error| format!("no working directory: {error}"))?
        }
    };

    let stubs =
        StubSet::install().map_err(|error| format!("could not install PHP stubs: {error}"))?;
    let mut workspace = Workspace::new(stubs);
    let root = workspace.set_root(root).to_path_buf();

    let found = workspace.discover();
    let started = Instant::now();
    for batch in found.php.chunks(phrost_index::INGEST_BATCH) {
        workspace.ingest_batch(batch, false);
    }
    // The other half of what reaches a class, and the half a tool is most likely to skip -- see
    // the note in `coverage`.
    workspace.install_config_sites(&found.configuration);
    workspace.install_template_words(&found.templates);
    workspace.mark_ready();
    println!(
        "indexed             {} files in {:.2?}",
        workspace.indexed_file_count(),
        started.elapsed()
    );

    // Said before the report rather than after, because it decides how to read every line of it.
    if !workspace.is_an_application() {
        println!();
        println!("not a composer `project`, so this package has consumers a checkout does not");
        println!("contain: what follows is what its own code does not use of itself, and its");
        println!("public API is most of that rather than dead");
    }

    let mut own: Vec<PathBuf> = found
        .php
        .iter()
        // The project's own by the server's rule, which is the sweep's too: see `Sweep::files`.
        .filter(|(path, origin)| matches!(origin, SourceOrigin::User) && workspace.owns_path(path))
        .map(|(path, _)| path.clone())
        .collect();
    own.sort();
    if let Some(limit) = limit {
        own.truncate(limit);
    }

    let swept = Instant::now();
    let mut reached: HashSet<String> = HashSet::new();
    let mut declared: BTreeMap<String, Unreached> = BTreeMap::new();
    // Member names the project writes anywhere, however it writes them, and the members it
    // declares. By *name* and not by declaring class, which is the whole of what makes this
    // sound: an access whose receiver has no type resolves to nothing, and matching on the pair
    // would read that as nobody calling the member. One `->run()` anywhere keeps every `run`
    // alive, which is coarse and is the direction a report about absence has to be coarse in.
    let mut members: Vec<DeclaredMember> = Vec::new();
    // Gathered first and recorded after the sweep, because judging needs the whole of it: a
    // member is dead only if *nothing* reaches it, and half a project is not an answer.
    let mut workspace_reached: Vec<(phrost_model::FileId, phrost_index::Reaches)> = Vec::new();
    for path in &own {
        let Some(file) = workspace.file_id(path) else {
            continue;
        };
        let display = path
            .strip_prefix(&root)
            .unwrap_or(path)
            .to_string_lossy()
            .into_owned();
        workspace.analysed(file, |view| {
            // The declarations, with somewhere to send a reader.
            let mut here = HashSet::new();
            for entry in phrost_analysis::document_symbols(view) {
                let kind = match entry.kind {
                    phrost_model::SymbolKind::Class => "class",
                    phrost_model::SymbolKind::Interface => "interface",
                    phrost_model::SymbolKind::Trait => "trait",
                    phrost_model::SymbolKind::Enum => "enum",
                    _ => continue,
                };
                here.insert(entry.selection_range);
                let Some(fqn) = qualified(view, &entry.label) else {
                    continue;
                };
                // `new class { ... }` is instantiated where it is written, so it is reached by
                // construction and nobody can name it anyway. The editor's check skips it and this
                // reported one, which is a finding with nothing a reader could do about it.
                if phrost_view::is_anonymous_class_name(&fqn) {
                    continue;
                }
                declared.insert(
                    fqn.to_ascii_lowercase(),
                    Unreached {
                        display: display.clone(),
                        line: view.line_of(entry.selection_range.start()),
                        kind,
                        fqn,
                    },
                );
            }
            // What this file reaches. A declaration's own name is not a use of itself, and it is
            // matched by where it is: a file that declares `Foo` and writes `Foo` below is using
            // it, and the two are the same word.
            for reference in view.name_references() {
                if here.contains(&reference.range) {
                    continue;
                }
                if let Some(resolved) = reference.resolved {
                    reached.insert(resolved.as_str().to_ascii_lowercase());
                }
            }
            // The same record the editor's check reads, filled the same way: an access is a
            // claim on every declaration it satisfies, and one that resolves to nothing is a
            // claim on the bare name. Two assemblies of this rule is how the report and the
            // diagnostic came to disagree once already.
            workspace_reached.push((file, view.reaches()));
            // A file that names a member at runtime says nothing about which of its own it
            // reaches, so none of them can be judged. The editor's check has had this rule all
            // along; without it here the report claimed methods a template pulled into a body
            // calls on `$this`.
            if view.reaches_a_member_by_a_computed_name() {
                return;
            }
            for member in view.declared_members() {
                members.push(DeclaredMember {
                    display: display.clone(),
                    line: view.line_of(member.range.start()),
                    what: match member.kind {
                        // PHP calls a magic method and nobody writes its name, so nothing here
                        // can say whether it is reached.
                        _ if member.name.as_str().starts_with("__") => continue,
                        phrost_view::MemberKind::Method => "method",
                        phrost_view::MemberKind::ClassConstant => "constant",
                        _ => continue,
                    },
                    // Qualified, because that is what a reachability rule is written against:
                    // a namespace segment cannot be matched in a short name, and passing one
                    // made every test method in the project a finding.
                    owner: qualified(view, member.owner.as_str())
                        .unwrap_or_else(|| member.owner.as_str().to_owned()),
                    name: member.name.as_str().to_owned(),
                    kind: member.kind,
                });
            }
        });
    }
    // Everything the configuration names, which is the half no PHP file writes.
    reached.extend(workspace.config_named_classes());
    // And what a framework builds because of what it inherits: a service provider, a Drush
    // command. Asked once for the project rather than per candidate -- see
    // `discovered_by_inheritance`.
    reached.extend(workspace.discovered_by_inheritance());

    for (file, reaches) in workspace_reached {
        workspace.record_reached(file, reaches);
    }
    workspace.mark_reached_complete();

    let mut dead_members: Vec<&DeclaredMember> = members
        .iter()
        .filter(|member| !workspace.member_is_reached(&member.owner, &member.name))
        // Everything that reaches a member without a PHP file writing the call: a route or a
        // container `calls:`, a class a framework finds by scanning, a template calling it under
        // another name again. One question and one assembly of it -- this had two of the three and
        // its own module doc listed all three, which is 46 findings the editor did not show.
        .filter(|member| !workspace.member_is_reached_without_a_call(&member.owner, &member.name))
        // And what a base declares too, which is called through the base and written nowhere.
        .filter(|member| {
            !workspace.member_comes_from_above(&member.owner, member.kind, &member.name)
        })
        .collect();
    dead_members
        .sort_by(|left, right| (&left.display, left.line).cmp(&(&right.display, right.line)));

    let mut out: Vec<&Unreached> = declared
        .iter()
        .filter(|(fqn, _)| !reached.contains(*fqn))
        // Found by scanning rather than named: a Drupal plugin, and nothing to point at.
        .filter(|(_, at)| !workspace.is_discovered(&at.fqn))
        .map(|(_, at)| at)
        .collect();
    out.sort_by(|left, right| (&left.display, left.line).cmp(&(&right.display, right.line)));

    // A dead class's members say nothing more than the class does, so listing them again is a
    // finding per method about one thing a reader deletes once. The editor's check stops at the
    // class for the same reason.
    let dead_classes: HashSet<String> = out.iter().map(|at| at.fqn.to_ascii_lowercase()).collect();
    dead_members.retain(|member| !dead_classes.contains(&member.owner.to_ascii_lowercase()));

    println!(
        "swept               {} files in {:.2?}\n",
        own.len(),
        swept.elapsed()
    );
    for at in &out {
        println!("{}:{}: {} {}", at.display, at.line + 1, at.kind, at.fqn);
    }
    for member in &dead_members {
        println!(
            "{}:{}: {} {}::{}",
            member.display,
            member.line + 1,
            member.what,
            member.owner,
            member.name
        );
    }
    println!(
        "\n{} of {} class-likes and {} of {} members are reached by nothing",
        out.len(),
        declared.len(),
        dead_members.len(),
        members.len()
    );
    Ok(())
}

/// A declaration's fully-qualified name, from the namespace the file declares.
fn qualified(view: &phrost_view::IndexView<'_>, label: &str) -> Option<String> {
    Some(match view.declared_namespace() {
        Some(namespace) => format!("{}\\{}", namespace.name, label),
        None => label.to_owned(),
    })
}
