//! Canonical text dump of the whole skeleton.
//!
//! Not a debugging convenience. The `incremental == from-scratch` invariant compares an index built
//! by applying edits against one built from the final state, and that comparison is only meaningful
//! if the rendering is canonical — same content, same bytes, no matter how the index got there.
//!
//! Which means: never hash order. Files are ordered by path, catalog entries by table and key, and
//! anything derived from a `HashMap` is sorted before it is written. Content stamps are left out on
//! purpose; they describe the input rather than the derived index, and the invariant test asserts
//! them separately so the dump stays readable.

use std::fmt::Write as _;

use phrost_hir::ImportKind;

use crate::skeleton::{FileSkeleton, ProjectSkeleton};

/// Renders the skeleton.
///
/// `line_index_for` supplies a line map per file so ranges print as line:column. Callers that do
/// not retain text can return `None`, and offsets are printed instead.
pub fn dump_skeleton(
    skeleton: &ProjectSkeleton,
    mut line_index_for: impl FnMut(&FileSkeleton) -> Option<phrost_model::LineIndex>,
) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "state {}", skeleton.state().as_str());

    let mut files = skeleton.files_by_path();
    files.sort_by(|left, right| left.1.path().cmp(right.1.path()));

    for (file, entry) in &files {
        let lines = line_index_for(entry);
        let _ = writeln!(
            out,
            "file {} [{:?}]",
            skeleton.display_path(entry),
            entry.origin()
        );

        // Dumped so `incremental == from-scratch` covers it. A one-bit fact carried through a
        // re-ingest is precisely the sort a stale path gets wrong without anything else changing.
        if entry.strict_types() {
            let _ = writeln!(out, "  declare(strict_types=1)");
        }

        // The same reason, and a stronger one: nothing else the dump prints implies a `@throws`,
        // so a codec that wrote this section and never read it back -- or read it in the wrong
        // order -- would pass `cold == warm` silently. Sorted, because the rows follow the file's
        // declaration order and a canonical dump may not depend on it.
        let mut thrown: Vec<String> = entry
            .throws()
            .iter()
            .map(|row| {
                let declaration = match &row.owner {
                    Some(owner) => format!("{owner}::{}", row.name),
                    None => row.name.to_string(),
                };
                let classes: Vec<&str> =
                    row.classes.iter().map(phrost_model::Name::as_str).collect();
                format!("  {declaration} throws {}", classes.join(" | "))
            })
            .collect();
        thrown.sort();
        for line in thrown {
            let _ = writeln!(out, "{line}");
        }

        // The same reason again: nothing else the dump prints implies a body-read chain, so a codec
        // that wrote this section and never read it back would pass `cold == warm` in silence.
        let mut chains: Vec<String> = entry
            .returned_paths()
            .iter()
            .map(|row| {
                let declaration = match &row.owner {
                    Some(owner) => format!("{owner}::{}", row.name),
                    None => row.name.to_string(),
                };
                let root = row
                    .root
                    .as_ref()
                    .map_or_else(|| "$this".to_owned(), |root| root.as_str().to_owned());
                // The key too, on the step it was handed to, because the codec carries it and
                // `cold == warm` only compares what this prints.
                let key = match &row.key {
                    None => String::new(),
                    Some(crate::ReturnedKey::Text(text)) => format!("('{text}')"),
                    Some(crate::ReturnedKey::Class(class)) => format!("({class}::class)"),
                };
                let steps: Vec<String> = row
                    .steps
                    .iter()
                    .enumerate()
                    .map(|(at, step)| match at {
                        0 => format!("{step}{key}"),
                        _ => step.to_string(),
                    })
                    .collect();
                format!("  {declaration} returns {root}->{}", steps.join("->"))
            })
            .collect();
        chains.sort();
        for line in chains {
            let _ = writeln!(out, "{line}");
        }

        // What renders a template, for the same reason: only a body says it.
        for site in skeleton.renders_of(*file) {
            let _ = writeln!(
                out,
                "  renders {} at {} with {}{}{}{}",
                site.template,
                u32::from(site.data),
                site.keys.join(", "),
                match site.kind {
                    crate::RenderKind::Array => "",
                    crate::RenderKind::Compact => " (compact)",
                    crate::RenderKind::Shared => " (shared)",
                    crate::RenderKind::Named => " (named)",
                    crate::RenderKind::Writes => " (writes)",
                },
                site.class
                    .as_ref()
                    .map(|class| format!(" in {class}"))
                    .unwrap_or_default(),
                if site.inherits { " and everything" } else { "" }
            );
        }

        // What names the class a callee's closures run as, for the same reason.
        for call in skeleton
            .configured_this
            .get(file)
            .map_or(&[][..], |calls| &calls[..])
        {
            let _ = writeln!(
                out,
                "  configures $this as {}{}",
                call.classes
                    .iter()
                    .map(|class| class.as_str())
                    .collect::<Vec<_>>()
                    .join(" & "),
                call.within
                    .as_ref()
                    .map(|within| format!(" in {}", within.join(", ")))
                    .unwrap_or_default()
            );
        }

        // And what a class states in an array it writes, for the same reason: a kept default and a
        // body are gone on a warm start, so this is the codec's to carry and `cold == warm`'s to
        // check.
        for table in skeleton.stated_of(*file) {
            for getter in &table.getters {
                let _ = writeln!(
                    out,
                    "  {}::{}() gets {}",
                    table.declarer,
                    getter.method,
                    skeleton.types().render_qualified(getter.ty)
                );
            }
            for row in &table.rows {
                let value = match &row.value {
                    crate::StatedValue::Listed => "listed".to_owned(),
                    crate::StatedValue::Text(text) => format!("'{text}'"),
                    crate::StatedValue::Class(class) => format!("{class}::class"),
                    crate::StatedValue::Other => "(unread)".to_owned(),
                };
                let _ = writeln!(
                    out,
                    "  {} states ${} as {value} in {} @ {}",
                    table.declarer,
                    row.name,
                    row.source,
                    render_range(lines.as_ref(), row.at)
                );
            }
        }

        // And what a migration does to a table, and which table a model names: a body and a kept
        // default again.
        if let Some(schema) = skeleton.schema_of(*file) {
            for change in &schema.changes {
                let kind = match &change.kind {
                    crate::schema::TableChangeKind::Created => "creates".to_owned(),
                    crate::schema::TableChangeKind::Altered => "alters".to_owned(),
                    crate::schema::TableChangeKind::Dropped => "drops".to_owned(),
                    crate::schema::TableChangeKind::RenamedTo(to) => format!("renames to {to}"),
                };
                let _ = writeln!(out, "  migration {kind} table {}", change.table);
                for written in &change.columns {
                    let change = match &written.change {
                        phrost_model::schema::ColumnChange::Defined {
                            name,
                            holds,
                            nullable,
                            constrained,
                        } => format!(
                            "defines {name}: {}{}{}",
                            if *nullable { "?" } else { "" },
                            holds.as_str(),
                            if *constrained { ", constrained" } else { "" }
                        ),
                        phrost_model::schema::ColumnChange::Constrained(name) => {
                            format!("constrains {name}")
                        }
                        phrost_model::schema::ColumnChange::Dropped(name) => {
                            format!("drops {name}")
                        }
                        phrost_model::schema::ColumnChange::Renamed { from, to } => {
                            format!("renames {from} to {to}")
                        }
                    };
                    let _ = writeln!(
                        out,
                        "    {change} @ {}",
                        render_range(lines.as_ref(), written.at)
                    );
                }
            }
            for named in &schema.named {
                let _ = writeln!(out, "  {} names table {}", named.class, named.table);
            }
        }
        for site in skeleton.constructions_of(*file) {
            let runs = match &site.callable {
                crate::constructions::Callable::Function(name) => format!("{name}()"),
                crate::constructions::Callable::Method { class, method } => {
                    format!("{class}::{method}()")
                }
                crate::constructions::Callable::Unnamed => "(unnamed)".to_owned(),
            };
            let _ = writeln!(
                out,
                "  {} declares {} `{}` @ {:?} running {runs}",
                site.declarer, site.catalog, site.key, site.key_range
            );
        }
        for kept in skeleton.class_defaults_of(*file) {
            let _ = writeln!(
                out,
                "  {}::${} defaults to {}::class",
                kept.declarer, kept.property, kept.class
            );
        }
        for site in skeleton.macros_of(*file) {
            let returns = match (&site.returns, &site.chain) {
                (Some(ty), _) => skeleton.types().render(*ty),
                // `static` for the class the macro is called on, which `$this` and `static::` both
                // mean there; the first step off a named class is a static call.
                (None, Some(chain)) => format!(
                    "{}::{}",
                    chain.root.as_ref().map_or("static", |root| root.as_str()),
                    chain
                        .steps
                        .iter()
                        .map(|step| step.as_str())
                        .collect::<Vec<_>>()
                        .join("->")
                ),
                (None, None) => "nothing stated".to_owned(),
            };
            let _ = writeln!(
                out,
                "  macro {}::{}() returns {returns} @ {}",
                site.class,
                site.name,
                render_range(lines.as_ref(), site.at)
            );
        }

        for symbol in entry.scope().symbols() {
            // Whether the name can be written is dumped for the same reason the strict-types bit is:
            // it is one bit that no rendering would otherwise show, and getting it wrong on a
            // restore turns an anonymous class into something completion offers. Whether the
            // declaration is conditional is the same shape of fact, and decides ranking -- and so
            // is `@ignore`, which decides it one tier lower.
            let _ = writeln!(
                out,
                "  {} {}{}{}{} @ {}",
                symbol.kind.as_str(),
                symbol.fqn,
                if symbol.is_nameable() {
                    ""
                } else {
                    " [unnameable]"
                },
                if symbol.is_conditional() {
                    " [conditional]"
                } else {
                    ""
                },
                if symbol.is_ignored() {
                    " [ignored]"
                } else {
                    ""
                },
                render_range(lines.as_ref(), symbol.name_range)
            );
        }

        // Signatures live here rather than on the symbol line above, which is generic over every
        // kind of declaration. Dumped so that the incremental-versus-scratch check covers them:
        // a stored type is exactly the kind of thing a stale re-ingest would get wrong quietly.
        for function in entry.functions() {
            let Some(symbol) = entry.scope().symbols().get(function.symbol() as usize) else {
                continue;
            };
            let mut notes = stored_notes(
                skeleton.types(),
                function.params(),
                function.reads_own_arguments(),
            );
            // The one modifier a free function carries, and one a signature does not render.
            if function.is_no_discard() {
                notes.push("no-discard".to_owned());
            }
            if function.is_deprecated() {
                notes.push("deprecated".to_owned());
            }
            let _ = writeln!(
                out,
                "  function {}{}",
                function.signature(symbol.fqn.as_str(), skeleton.types()),
                match notes.is_empty() {
                    true => String::new(),
                    false => format!("  {}", notes.join(" ")),
                }
            );
        }

        // A constant's stated type, for the reason a function's signature is dumped above: a stored
        // type a stale re-ingest or a codec that dropped it would get wrong without a word.
        for row in entry.constants() {
            let Some(symbol) = entry.scope().symbols().get(row.symbol() as usize) else {
                continue;
            };
            let _ = writeln!(
                out,
                "  constant {}: {}",
                symbol.fqn,
                render_constant_value(skeleton, row.value())
            );
        }

        // What the file's own calls declare, which no declaration in it states. Rendered per file
        // rather than only in the catalog below, because the two check different things: this is
        // what the file was found to say, and the catalog is what survived being claimed.
        for row in skeleton.dispatches_of(*file) {
            let _ = writeln!(
                out,
                "  dispatches [{}] {} by {}::{} on {} @ {}",
                row.catalog,
                row.call.name,
                row.call.required,
                row.call.method,
                row.call.receiver.as_ref().map_or_else(
                    || "an unidentified receiver".to_owned(),
                    |receiver| format!("{}::{}", receiver.owner, receiver.member)
                ),
                render_range(lines.as_ref(), row.call.name_range)
            );
        }

        for namespace in entry.scope().scopes() {
            for kind in [
                ImportKind::ClassLike,
                ImportKind::Function,
                ImportKind::Constant,
            ] {
                for binding in namespace.imports().table(kind) {
                    let _ = writeln!(
                        out,
                        "  use {} as {}  [{}]",
                        binding.path(),
                        binding.local(),
                        kind.as_str()
                    );
                }
            }
        }

        for class in entry.classes() {
            // Every stored fact about the declaration itself, and not for readability: this dump is
            // the yardstick both `incremental == from-scratch` and `cold == warm` measure against,
            // so a field it does not render is a field neither of them checks.
            let mut words = vec![class.kind().as_str().to_owned()];
            words.extend(class.modifier_words().into_iter().map(str::to_owned));
            // Not one of the modifier words, which are what a header renders: this is an attribute
            // written above the declaration, and it is here because the dump is what the two
            // equality invariants measure.
            if class.is_attribute() {
                words.push("#[Attribute]".to_owned());
            }
            if class.is_deprecated() {
                words.push("deprecated".to_owned());
            }
            if let Some(backing) = class.enum_backing() {
                words.push(format!("backed by {}", skeleton.types().render(backing)));
            }
            let _ = writeln!(out, "  {} is {}", class.fqn(), words.join(" "));

            for adaptation in class.adaptations() {
                let _ = writeln!(
                    out,
                    "  {} adapts {}{} {} @ {}",
                    class.fqn(),
                    adaptation
                        .trait_fqn
                        .as_ref()
                        .map(|fqn| format!("{fqn}::"))
                        .unwrap_or_default(),
                    adaptation.method,
                    adaptation.kind.label(),
                    render_range(lines.as_ref(), adaptation.method_range)
                );
            }

            // Rendered because the invariants compare dumps: a `@template` dropped on the way
            // through the cache, or lost by an incremental re-ingest, would otherwise pass both
            // checks silently -- which is exactly how a missing field slipped through once before.
            for template in class.templates() {
                let _ = writeln!(
                    out,
                    "  {} template {}{}",
                    class.fqn(),
                    template.name,
                    template
                        .bound
                        .map(|id| format!(" of {}", skeleton.types().render(id)))
                        .unwrap_or_default()
                );
            }

            // `@extends Bar<Baz>` and `@mixin Other`: both resolved at ingest, both carried by
            // the codec, and neither implied by anything else here. A class that lost its
            // supertype arguments answers every generic question with the declaration's bound
            // instead of the argument, which is a wrong answer nothing fails on.
            for supplied in class.supertype_arguments() {
                let _ = writeln!(
                    out,
                    "  {} supplies {}",
                    class.fqn(),
                    skeleton.types().render_qualified(*supplied)
                );
            }
            for (label, targets) in [
                ("extends", class.extends()),
                ("implements", class.implements()),
                ("uses", class.uses()),
                ("mixes in", class.mixins()),
            ] {
                for target in targets {
                    let _ = writeln!(
                        out,
                        "  {} {label} {} @ {}",
                        class.fqn(),
                        target.fqn,
                        render_range(lines.as_ref(), target.range)
                    );
                }
            }

            // Own members, before any inheritance -- the flattened view is a separate question and
            // has its own dump.
            for member in class.members() {
                let mut notes = Vec::new();
                if let Some(set) = member.set_visibility {
                    notes.push(format!("{}(set)", set.as_str()));
                }
                notes.extend(member.modifiers.words().into_iter().map(str::to_owned));
                if member.from_docblock {
                    notes.push("from docblock".to_owned());
                }
                // A constant's type read off its value rather than written, which a declaration
                // rendered as text must not show and a stale re-ingest could flip.
                if member.ty_from_value {
                    notes.push("type from value".to_owned());
                }
                // A `@return` refused for naming a template of the declaration's own, which decides
                // whether an overridden contract is carried down and is otherwise invisible: the type
                // slot it leaves is empty either way.
                if member.doc_ty_refused {
                    notes.push("return names its own template".to_owned());
                }
                // `docblock_range` is deliberately not here. It is the one of the six that adds a
                // note to *every* documented member, and a stale one shows up in `name_range` and
                // `full_range` beside it -- both rendered -- because all three come off the same
                // lowering of the same declaration. The four above carry something nothing else
                // rendered implies, which is what earns a note.
                notes.extend(stored_notes(
                    skeleton.types(),
                    &member.params,
                    member.reads_own_arguments,
                ));
                // Rendered so both invariants can see it. Without this, dropping the field from the
                // codec passes every test -- which is the exact failure this project has already
                // had once, with `@template`.
                match member.contract.as_deref() {
                    None => {}
                    Some(phrost_sema::Contract::Receiver { ty, when_true }) => {
                        notes.push(format!(
                            "asserts {} if {}",
                            skeleton.types().render_qualified(*ty),
                            if *when_true { "true" } else { "false" }
                        ));
                    }
                    Some(phrost_sema::Contract::Argument {
                        parameter,
                        ty,
                        negated,
                        when_true,
                        from_class_string,
                    }) => {
                        notes.push(format!(
                            "asserts ${parameter} is {}{}{}",
                            if *negated { "not " } else { "" },
                            skeleton.types().render_qualified(*ty),
                            match when_true {
                                None => String::new(),
                                Some(true) => " if true".to_owned(),
                                Some(false) => " if false".to_owned(),
                            }
                        ));
                        if let Some(position) = from_class_string {
                            notes.push(format!("named by argument {position}"));
                        }
                    }
                    Some(phrost_sema::Contract::ReturnsArgumentClass {
                        parameter,
                        absent,
                        many,
                        beside,
                    }) => {
                        notes.push(format!(
                            "returns {}{} argument {parameter} names{}",
                            match absent {
                                Some(absence) => format!("{} or ", sentinel(*absence)),
                                None => String::new(),
                            },
                            if *many {
                                "a container of the class"
                            } else {
                                "the class"
                            },
                            // The dump shows every stored field, or a warm start can lose one with
                            // nothing failing.
                            match beside {
                                true => ", beside what it declares",
                                false => "",
                            }
                        ));
                    }
                    Some(phrost_sema::Contract::ReturnsOneOf(operands)) => {
                        let branch = branch_note(skeleton);
                        notes.push(format!(
                            "returns one of {}",
                            operands.iter().map(branch).collect::<Vec<_>>().join(" | ")
                        ));
                    }
                    Some(phrost_sema::Contract::ReturnsApplied {
                        class,
                        arguments,
                        many,
                        alternatives,
                    }) => {
                        let branch = branch_note(skeleton);
                        notes.push(format!(
                            "returns {}{} of {}",
                            if *many { "a list of " } else { "" },
                            std::iter::once(class)
                                .chain(alternatives.iter())
                                .map(|class| skeleton.types().render_qualified(*class))
                                .collect::<Vec<_>>()
                                .join(" or "),
                            arguments.iter().map(branch).collect::<Vec<_>>().join(", ")
                        ));
                    }
                    Some(phrost_sema::Contract::ConstantEntry { table, parameter }) => {
                        notes.push(format!(
                            "returns the entry of {} at the key argument {parameter} is",
                            match table {
                                phrost_sema::ConstantTable::Global(names) => names
                                    .iter()
                                    .map(|name| name.as_str())
                                    .collect::<Vec<_>>()
                                    .join(" or "),
                                phrost_sema::ConstantTable::Member { class, name } => format!(
                                    "{}::{name}",
                                    skeleton.types().render_qualified(*class)
                                ),
                            }
                        ));
                    }
                    Some(phrost_sema::Contract::ReceiverOut { class, arguments }) => {
                        let branch = branch_note(skeleton);
                        notes.push(format!(
                            "leaves its receiver {}{}",
                            skeleton.types().render_qualified(*class),
                            match arguments.is_empty() {
                                true => String::new(),
                                false => format!(
                                    " of {}",
                                    arguments.iter().map(branch).collect::<Vec<_>>().join(", ")
                                ),
                            }
                        ));
                    }
                    Some(phrost_sema::Contract::ConditionalReturn {
                        argument,
                        test,
                        then,
                        otherwise,
                        by_default,
                    }) => {
                        let branch = branch_note(skeleton);
                        notes.push(format!(
                            "returns, if argument {argument} is {}, {} and otherwise {}",
                            test_note(skeleton, test),
                            branch(then),
                            branch(otherwise)
                        ));
                        if let Some(holds) = by_default {
                            notes.push(format!("by default {holds}"));
                        }
                    }
                    Some(phrost_sema::Contract::TestChain { cases, otherwise }) => {
                        let branch = branch_note(skeleton);
                        let links: Vec<String> = cases
                            .iter()
                            .map(|case| {
                                format!(
                                    "if argument {} is {}{}, {}{}",
                                    case.argument,
                                    if case.negated { "not " } else { "" },
                                    test_note(skeleton, &case.test),
                                    branch(&case.then),
                                    match case.by_default {
                                        Some(holds) => format!(" (by default {holds})"),
                                        None => String::new(),
                                    }
                                )
                            })
                            .collect();
                        notes.push(format!(
                            "returns, {}, and otherwise {}",
                            links.join("; "),
                            branch(otherwise)
                        ));
                    }
                    Some(phrost_sema::Contract::LiteralCases {
                        argument,
                        cases,
                        otherwise,
                    }) => {
                        let branch = branch_note(skeleton);
                        let cases = cases
                            .iter()
                            .map(|(literal, then)| format!("'{literal}' -> {}", branch(then)))
                            .collect::<Vec<_>>()
                            .join(", ");
                        notes.push(format!(
                            "returns by argument {argument}: {cases}, otherwise {}",
                            branch(otherwise)
                        ));
                    }
                    Some(phrost_sema::Contract::AgreedReturn(branch)) => {
                        notes.push(format!("returns {}", branch_note(skeleton)(branch)));
                    }
                }
                let _ = writeln!(
                    out,
                    "  {}::{} {}  {}{}  @ {} (full {})",
                    class.fqn(),
                    member.kind.as_str(),
                    member.signature(skeleton.types()),
                    member.visibility.as_str(),
                    if notes.is_empty() {
                        String::new()
                    } else {
                        format!(" {}", notes.join(" "))
                    },
                    render_range(lines.as_ref(), member.name_range),
                    render_range(lines.as_ref(), member.full_range)
                );
            }
        }

        // Silence the unused binding when a file declares nothing at all.
        let _ = file;
    }

    let _ = writeln!(out, "catalog");
    for (key, table, declarers) in skeleton.owners().entries() {
        let ranked = skeleton.rank(declarers);
        let targets = ranked
            .iter()
            .filter_map(|declarer| {
                let entry = skeleton.file(declarer.file)?;
                let symbol = entry.scope().symbols().get(declarer.symbol as usize)?;
                Some(format!("{} ({})", skeleton.display_path(entry), symbol.fqn))
            })
            .collect::<Vec<_>>();
        let _ = writeln!(out, "  [{table}] {key} -> {}", targets.join(" | "));
    }

    // The namespaces the catalog holds, kept by the same claims and withdrawals as the tables
    // above -- so an edit that miscounted one shows here as a difference from a fresh build.
    let _ = writeln!(out, "namespaces");
    for (path, spelled, claims) in skeleton.owners().namespace_entries() {
        let _ = writeln!(out, "  {path} -> {spelled} x{claims}");
    }

    // Only the claimed keys, not the configured ones: a configured catalog is read from files
    // outside the index and does not change when one of them does, so it says nothing about
    // invalidation. A claimed key is exactly what does.
    let _ = writeln!(out, "declared keys");
    for (catalog, key, classes) in skeleton.returns().declared_keys() {
        // Spelled out rather than left as a dangling arrow: a key that names nothing is a *kind* of
        // claim -- a route derived from a link template -- and a blank right-hand side reads as a
        // class the index failed to find, which is the opposite of what it says.
        let names = match classes.is_empty() {
            true => "(names nothing)".to_owned(),
            false => classes.join(" | "),
        };
        let _ = writeln!(out, "  [{catalog}] {key} -> {names}");
    }

    // And the keys a call declared, which is the same fact about invalidation asked of a body
    // rather than of a declaration: a dispatch a re-ingested file failed to withdraw is invisible
    // without this line.
    let _ = writeln!(out, "dispatched keys");
    for (catalog, key, calls) in skeleton.returns().dispatched_keys() {
        let sites = calls
            .iter()
            .filter_map(|call| {
                let entry = skeleton.file(call.file)?;
                // Offsets, not lines: this section is not inside the per-file loop and has no line
                // index to hand. What it has to distinguish is two calls in one file, which an
                // offset does.
                Some(format!(
                    "{} {}",
                    skeleton.display_path(entry),
                    render_range(None, call.name_range)
                ))
            })
            .collect::<Vec<_>>();
        let _ = writeln!(out, "  [{catalog}] {key} -> {}", sites.join(" | "));
    }

    // Which files compose each trait, which `install` and `retract` keep beside the rows. Only
    // when there is one: most fixtures compose nothing, and a header with nothing under it in
    // every one of their snapshots says nothing about invalidation.
    let traits = skeleton.trait_users();
    if !traits.is_empty() {
        let _ = writeln!(out, "trait users");
        for (name, files) in traits {
            let paths = files
                .iter()
                .map(|entry| skeleton.display_path(entry))
                .collect::<Vec<_>>();
            let _ = writeln!(out, "  {name} -> {}", paths.join(" | "));
        }
    }

    out
}

/// The stored facts a signature does not print, as notes.
///
/// Five fields of a member or a function row are carried by the codec, derived at lowering, and
/// invisible to `MemberRow::signature` -- so the `incremental == from-scratch` dimension, which
/// compares *this dump*, could not see any of them. That is the failure the note beside the
/// contract above already records, met five more times: a field the dump does not print is a field
/// a stale re-ingest can get wrong quietly, and nothing in the project would notice.
///
/// Rendered here rather than folded into `signature`, because that one is what hover shows a
/// person: a reader wants `find(int $id): ?Repo`, not the four facts a test needs beside it.
fn stored_notes(
    types: &phrost_ty::TypeTable,
    params: &[phrost_sema::ParamRow],
    reads_own_arguments: bool,
) -> Vec<String> {
    let mut notes = Vec::new();
    if reads_own_arguments {
        notes.push("reads own arguments".to_owned());
    }
    for param in params {
        // What `@param-out` says a by-reference parameter leaves behind, which is a different type
        // from the one in the signature.
        if let Some(out) = param.out_ty {
            notes.push(format!(
                "${} -> {}",
                param.name,
                types.render_qualified(out)
            ));
        }
        // What a documented callback is called with. The arguments of the stored
        // `Generic { Callable, .. }` and not the node itself, because the slot is what gives them
        // their meaning -- printed as a type it would read as a return.
        if let Some(called) = param.callback_params_ty {
            let rendered = match types.get(called) {
                phrost_ty::TypeRef::Generic { args, .. } => types
                    .operands(*args)
                    .iter()
                    .map(|&id| types.render_qualified(id))
                    .collect::<Vec<_>>()
                    .join(", "),
                other => format!("{other:?}"),
            };
            notes.push(format!("${} calls({rendered})", param.name));
        }
        if let Some(this) = param.closure_this_ty {
            notes.push(format!(
                "${} runs with $this as {}",
                param.name,
                types.render_qualified(this)
            ));
        }
    }
    notes
}

fn render_range(lines: Option<&phrost_model::LineIndex>, range: phrost_model::TextRange) -> String {
    match lines {
        Some(lines) => {
            let (start, end) = lines.range_line_cols(range);
            format!(
                "{}:{}-{}:{}",
                start.line + 1,
                start.col + 1,
                end.line + 1,
                end.col + 1
            )
        }
        // Offsets are still canonical, just less readable -- used when text was not retained.
        None => format!("{}..{}", u32::from(range.start()), u32::from(range.end())),
    }
}

/// How one branch of a conditional return type reads, for the dumps the index invariants measure
/// against. A closure over the skeleton, because two contracts render branches and rendering them
/// twice would be two chances to render them differently.
fn branch_note(
    skeleton: &crate::ProjectSkeleton,
) -> impl Fn(&phrost_sema::ReturnBranch) -> String + '_ {
    move |branch| render_branch(skeleton, branch)
}

/// [`branch_note`]'s body, named so a branch holding branches can render them.
fn render_branch(skeleton: &crate::ProjectSkeleton, branch: &phrost_sema::ReturnBranch) -> String {
    match branch {
        phrost_sema::ReturnBranch::Ty(ty) => skeleton.types().render_qualified(*ty),
        phrost_sema::ReturnBranch::ArgumentClass(position) => {
            format!("the class argument {position} names")
        }
        phrost_sema::ReturnBranch::ArgumentType {
            parameter,
            absent,
            evaluated,
        } => format!(
            "whatever argument {parameter} is{}{}",
            match evaluated {
                true => ", or what it returns when it is a callable",
                false => "",
            },
            match absent {
                Some(kind) => format!(", or {} when left out", kind.as_str()),
                None => String::new(),
            }
        ),
        phrost_sema::ReturnBranch::ArgumentElement { parameter, absent } => format!(
            "what argument {parameter} holds{}",
            match absent {
                Some(absence) => format!(", or {}", sentinel(*absence)),
                None => String::new(),
            }
        ),
        phrost_sema::ReturnBranch::ArgumentKey { parameter, absent } => format!(
            "what names a slot of argument {parameter}{}",
            match absent {
                Some(absence) => format!(", or {}", sentinel(*absence)),
                None => String::new(),
            }
        ),
        phrost_sema::ReturnBranch::ArgumentElements { parameter } => {
            format!("a container of what argument {parameter} holds")
        }
        phrost_sema::ReturnBranch::ArgumentTypes { parameter } => {
            format!("a container of whatever argument {parameter} is")
        }
        phrost_sema::ReturnBranch::CombinedElements { keys, values } => {
            format!(
                "a container keyed by what argument {keys} holds, of what argument {values} holds"
            )
        }
        phrost_sema::ReturnBranch::ArgumentKeys { parameter } => {
            format!("a container of what names each slot of argument {parameter}")
        }
        phrost_sema::ReturnBranch::ArgumentColumn { parameter, column } => {
            format!("a container of the column argument {column} names in argument {parameter}")
        }
        phrost_sema::ReturnBranch::CallbackResults { parameter } => {
            format!("a container of what the callable at {parameter} returns")
        }
        phrost_sema::ReturnBranch::CallbackResult { parameter } => {
            format!("what the callable at {parameter} returns")
        }
        phrost_sema::ReturnBranch::Conditional(conditional) => format!(
            "(if argument {} is {}, {} and otherwise {}{})",
            conditional.argument,
            test_note(skeleton, &conditional.test),
            render_branch(skeleton, &conditional.then),
            render_branch(skeleton, &conditional.otherwise),
            match conditional.by_default {
                Some(holds) => format!(" by default {holds}"),
                None => String::new(),
            }
        ),
        phrost_sema::ReturnBranch::Unfilled => "what the slot's bound allows".to_owned(),
        phrost_sema::ReturnBranch::Applied { class, arguments } => format!(
            "{} of {}",
            skeleton.types().render_qualified(*class),
            arguments
                .iter()
                .map(|argument| render_branch(skeleton, argument))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

/// Which way a declaration said the value may not be there.
///
/// Spelled out rather than rendered as "or nothing", because the whole point of keeping the two
/// apart is that they need different guards -- a snapshot that cannot tell them apart cannot pin it.
fn sentinel(absence: phrost_model::Absence) -> &'static str {
    match absence {
        phrost_model::Absence::Null => "null",
        phrost_model::Absence::False => "false",
        phrost_model::Absence::NullOrFalse => "null or false",
    }
}

/// A condition test as the dump prints it, for a conditional and for every link of a chain.
fn test_note(skeleton: &ProjectSkeleton, test: &phrost_sema::ConditionTest) -> String {
    match test {
        phrost_sema::ConditionTest::Null => "null".to_owned(),
        phrost_sema::ConditionTest::True => "true".to_owned(),
        phrost_sema::ConditionTest::False => "false".to_owned(),
        phrost_sema::ConditionTest::ClassString(None) => "a class name".to_owned(),
        phrost_sema::ConditionTest::ClassString(Some(bound)) => format!(
            "the name of a {}",
            skeleton.types().render_qualified(*bound)
        ),
        phrost_sema::ConditionTest::Literal(written) => format!("'{written}'"),
        phrost_sema::ConditionTest::OfKind(kind) => format!("a {}", kind.as_str()),
        phrost_sema::ConditionTest::NonEmpty => "not empty".to_owned(),
        phrost_sema::ConditionTest::Empty => "empty".to_owned(),
        phrost_sema::ConditionTest::NonEmptyString => "a non-empty string".to_owned(),
        phrost_sema::ConditionTest::NonFalsyString => "a non-falsy string".to_owned(),
        phrost_sema::ConditionTest::Number(written) => written.to_string(),
        phrost_sema::ConditionTest::OfKindOrInstance { kind, class } => format!(
            "a {} or a {}",
            kind.as_str(),
            skeleton.types().render_qualified(*class)
        ),
        phrost_sema::ConditionTest::Instance(class) => {
            format!("a {}", skeleton.types().render_qualified(*class))
        }
        phrost_sema::ConditionTest::Scalar => "a scalar".to_owned(),
        phrost_sema::ConditionTest::Shape {
            key,
            accepts,
            sealed,
        } => {
            let accepts: Vec<String> = accepts
                .iter()
                .map(|literal| match literal {
                    phrost_sema::ShapeLiteral::String(text) => format!("'{text}'"),
                    phrost_sema::ShapeLiteral::Int(value) => value.to_string(),
                    phrost_sema::ShapeLiteral::True => "true".to_owned(),
                    phrost_sema::ShapeLiteral::False => "false".to_owned(),
                    phrost_sema::ShapeLiteral::Null => "null".to_owned(),
                })
                .collect();
            format!(
                "array{{{key}: {}{}}}",
                accepts.join("|"),
                if *sealed { "" } else { ", ..." }
            )
        }
    }
}

/// A constant's value as the dump shows it: a type, or the parts and the names each one may reach.
fn render_constant_value(
    skeleton: &crate::ProjectSkeleton,
    value: &crate::constant::ConstantValue,
) -> String {
    use crate::constant::ConstantValue;
    match value {
        ConstantValue::Ty(ty) => skeleton.types().render_qualified(*ty),
        ConstantValue::Constant(names) => format!(
            "value of {}",
            names
                .iter()
                .map(phrost_model::Name::as_str)
                .collect::<Vec<_>>()
                .join(" or ")
        ),
        ConstantValue::Arithmetic(operator, operands) => format!(
            "({} {} {})",
            render_constant_value(skeleton, &operands[0]),
            operator.as_str(),
            render_constant_value(skeleton, &operands[1])
        ),
        ConstantValue::Bitwise(operands) => format!(
            "({} bitwise {})",
            render_constant_value(skeleton, &operands[0]),
            render_constant_value(skeleton, &operands[1])
        ),
    }
}
