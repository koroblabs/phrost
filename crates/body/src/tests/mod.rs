//! Local scope tests, against real PHP but no project.
//!
//! Lowering is the only thing below this, so a failure here is about the scope rules rather than
//! about resolution, the catalog, or an editor.
//!
//! A file per behaviour, so two people adding two unrelated tests are adding two files.

mod a_break_that_leaves_the_loop;
mod a_capture_by_reference;
mod a_class_guard_read_backwards;
mod a_container_filled_elsewhere;
mod a_flag_whose_subject_was_replaced;
mod a_guard_in_the_branch_not_taken;
mod a_guard_that_holds_at_the_read;
mod a_literal_handed_to_a_call;
mod a_loop_in_the_other_arm;
mod a_loop_that_writes_through_a_reference;
mod a_name_two_loops_bind;
mod a_normalising_branch;
mod a_rebinding_inside_a_condition;
mod a_strict_search_finds_an_element;
mod a_test_that_only_rules_null_out;
mod a_write_a_later_one_replaces;
mod a_write_under_two_guards;
mod a_write_whose_guard_says_nothing;
mod an_element_write_in_another_branch;
mod an_elseif_that_leaves_or_writes;
mod an_elseif_whose_test_says_nothing;
mod an_identity_against_a_literal_array;
mod an_identity_with_a_constant;
mod annotations;
mod argument_assertions;
mod array_elements;
mod assignment_sites;
mod assignments;
mod bindings;
mod calls_made_on_a_value;
mod catches;
mod class_tests_written_as_calls;
mod closure_values;
mod early_reads;
mod either;
mod either_of_claims;
mod either_operand_sites;
mod empty_string_guards;
mod facts_oracle;
mod falling_off_the_end;
mod false_guards;
mod false_negates_a_guard;
mod file_scope;
mod first_class_callables;
mod globals;
mod in_array_guards;
mod keyed_elements;
mod leaving;
mod literal_arguments;
mod match_true_guards;
mod narrowing;
mod null_guards;
mod occurrences;
mod parameter_declaration;
mod property_hooks;
mod ranges_inside_a_cut_buffer;
mod scopes;
mod slots_filled_after_a_literal;
mod subscripts;
mod true_guards;
mod unreachable_statements;
mod writes_in_a_branch;
mod writes_that_fill_every_arm;

use expect_test::Expect;
use phrost_hir::{FileHir, TypeRefData};
use phrost_model::{FileId, Interner, TextSize};

use crate::{PathRoot, PathStep, local_binding};

/// One argument as the path carries it, quoted the way it was written.
///
/// Double quotes for an interpolated skeleton, with `${}` where each hole is. The marker itself is
/// U+0001, which a snapshot would carry invisibly -- and what these tests are about is precisely
/// that two calls written with different arguments are two different values, so the rendering has
/// to show the difference rather than hide it in a control character.
fn quoted_argument(literal: &phrost_hir::body::LiteralArgument) -> String {
    match literal.kind {
        phrost_hir::body::ArgumentLiteral::Interpolated => format!(
            "\"{}\"",
            literal
                .value
                .as_str()
                .replace(phrost_hir::body::INTERPOLATION_HOLE, "${}")
        ),
        _ => format!("'{}'", literal.value),
    }
}

/// Renders where `variable` at the `$0` marker in `source` got its value.
fn check(source: &str, variable: &str, expect: Expect) {
    expect.assert_eq(&rendered(source, variable));
}

/// Every name in scope at the `$0` marker, in the order the search offers them.
///
/// The other half of what a local is, and a separate harness because it is a separate search:
/// [`check`] asks what a name *holds*, this asks whether the name exists at all. A binding
/// construct taught to only one of them works wherever the variable also appears as an expression
/// and fails silently where it does not, which is a failure nobody notices.
fn check_names(source: &str, expect: Expect) {
    let offset = TextSize::try_from(source.find("$0").expect("the source marks a position"))
        .expect("fixture fits in u32");
    let text = source.replace("$0", "");

    let mut interner = Interner::new();
    let lowered = phrost_lower::lower_text_with_bodies(
        &mut interner,
        FileId::from_raw(0),
        &text,
        &phrost_lower::KeyedAttributes::default(),
    );
    assert!(
        lowered.errors.is_empty(),
        "fixture does not parse: {:?}",
        lowered.errors
    );

    let names = crate::locals_in_scope(&lowered.hir, offset)
        .iter()
        .map(|name| format!("${name}"))
        .collect::<Vec<_>>();
    expect.assert_eq(&match names.is_empty() {
        true => "<nothing in scope>".to_owned(),
        false => names.join("\n"),
    });
}

/// The same rendering, for a test that compares several sources against one string.
fn rendered(source: &str, variable: &str) -> String {
    // `$0`, matching the fixture format the other harnesses use. A bare `|` would collide with the
    // `||` these tests need to write.
    let offset = TextSize::try_from(source.find("$0").expect("the source marks a position"))
        .expect("fixture fits in u32");
    let text = source.replace("$0", "");

    let mut interner = Interner::new();
    let lowered = phrost_lower::lower_text_with_bodies(
        &mut interner,
        FileId::from_raw(0),
        &text,
        &phrost_lower::KeyedAttributes::default(),
    );
    assert!(
        lowered.errors.is_empty(),
        "fixture does not parse: {:?}",
        lowered.errors
    );

    match local_binding(
        &lowered.hir,
        offset,
        variable,
        &|_, _| false,
        &crate::NoAssertions,
    ) {
        Some(bound) => render(&lowered.hir, &bound.value),
        None => "<not declared here>".to_owned(),
    }
}

/// Renders a value path. Recursive, because a null guard carries the path it is about.
fn render(hir: &FileHir, path: &crate::ValuePath) -> String {
    let mut out = match &path.root {
        PathRoot::Written(id) => written(hir, *id),
        PathRoot::VariadicValues(id) => format!("variadic {}", written(hir, *id)),
        // The position, because that is the whole of what this layer decided -- which parameter the
        // question is about. What the overridden declaration says is the view's.
        PathRoot::InheritedParameter { position } => format!("inherited parameter {position}"),
        PathRoot::Builtin(builtin) => format!("({})", builtin.as_str()),
        PathRoot::StringLiteral(_) => "(string)".to_owned(),
        PathRoot::FunctionReturn { name, .. } => format!("{name}()"),
        PathRoot::Instantiated { class, .. } => {
            format!("new {}", class_target(hir, *class))
        }
        PathRoot::Local(name) => format!("${name}"),
        // How many entries it holds, because that is the whole of what this layer decided: which
        // literals are lists at all. What they *hold* is the view's, and rendering it here would
        // pass whether the entries travelled or not.
        PathRoot::ArrayLiteral(expr) => match hir.expr(*expr) {
            phrost_hir::body::ExprData::ArrayLiteral { elements, .. } => {
                format!("list literal of {}", hir.expr_list(*elements).len())
            }
            _ => String::from("list literal"),
        },
        // The declared return, because it is the only half of a closure this layer hands on -- a
        // rendering that showed just `closure` would pass whether the type travelled or not.
        PathRoot::Closure(expr) => match hir.expr(*expr) {
            phrost_hir::body::ExprData::Closure {
                return_type: Some(id),
                ..
            } => format!("closure(): {}", written(hir, *id)),
            _ => "closure".to_owned(),
        },
        // The inner path is the call that was never made, and rendering it is the whole point:
        // a `callable` with nothing after it would pass whether the callee travelled or not.
        PathRoot::Callable(of) => format!("callable(...): {}", render(hir, of)),
        PathRoot::Arithmetic {
            operator,
            left,
            right,
        } => {
            format!(
                "{} {} {}",
                render(hir, left),
                operator.as_str(),
                render(hir, right)
            )
        }
        PathRoot::Concatenation { holds, pieces } => format!(
            "({}) of {}",
            holds.as_str(),
            pieces
                .iter()
                .map(|piece| render(hir, piece))
                .collect::<Vec<_>>()
                .join(" . ")
        ),
        PathRoot::Stepped(target) => format!("++{}", render(hir, target)),
        PathRoot::EitherOf { operands, unless } => operands
            .iter()
            .map(|operand| render(hir, &operand.value))
            .collect::<Vec<_>>()
            .join(match unless {
                crate::path::Unless::Null => " ?? ",
                crate::path::Unless::Falsy => " ?: ",
                crate::path::Unless::Nothing => " or ",
            }),
        PathRoot::RewrittenElements { before, writes, .. } => format!(
            "the elements of {} before, or {}",
            render(hir, before),
            match writes {
                Some(writes) => writes
                    .iter()
                    .map(|write| render(hir, &write.value))
                    .collect::<Vec<_>>()
                    .join(" or "),
                None => "a write nothing names".to_owned(),
            }
        ),
        // The writes, then what the name held if none of them ran -- which is a lookup rather than
        // a path, so it renders as the question rather than as an answer.
        PathRoot::MaybeWritten {
            writes,
            read,
            otherwise,
            ..
        } => {
            let written = writes
                .iter()
                .map(|write| render(hir, &write.value))
                .collect::<Vec<_>>()
                .join(" or ");
            // No third operand where no path skipped the writes, and the rendering says so by
            // leaving the question out rather than by naming it.
            match otherwise {
                crate::path::Otherwise::Impossible => written,
                _ => format!(
                    "{written} or {} before{}",
                    render(hir, read),
                    match otherwise {
                        crate::path::Otherwise::Unproved | crate::path::Otherwise::Impossible =>
                            String::new(),
                        crate::path::Otherwise::NotNull {
                            refused: crate::path::Refused::NullOnly,
                        } => " minus null".to_owned(),
                        crate::path::Otherwise::NotNull {
                            refused: crate::path::Refused::TheEmptyString,
                        } => " minus null and ''".to_owned(),
                        crate::path::Otherwise::NotNull { .. } => " minus falsehood".to_owned(),
                        crate::path::Otherwise::NotEmptyString => " minus ''".to_owned(),
                        crate::path::Otherwise::NoneOfKinds { kinds, .. } => format!(
                            " minus sentinels and {}",
                            kinds
                                .iter()
                                .map(|kind| kind.as_str())
                                .collect::<Vec<_>>()
                                .join("|")
                        ),
                        crate::path::Otherwise::OfKind { kind, .. } =>
                            format!(" being {}", kind.as_str()),
                        crate::path::Otherwise::OfClass(class) =>
                            format!(" as {}", class_target(hir, *class)),
                        crate::path::Otherwise::NotOfClass(class) =>
                            format!(" minus {}", class_target(hir, *class)),
                        crate::path::Otherwise::NotOfKind(kind) =>
                            format!(" minus {}", kind.as_str()),
                        crate::path::Otherwise::Asserted(path) =>
                            format!(" as {}", render(hir, path)),
                    }
                ),
            }
        }
        PathRoot::Asserted {
            method, when_true, ..
        } => {
            format!("asserted by {method}() if {when_true}")
        }
        PathRoot::ByReference { subject, .. } => {
            format!("written through a call taking {}", render(hir, subject))
        }
        PathRoot::AssertedArgument {
            of,
            callee,
            argument,
            when_true,
            ..
        } => format!(
            "argument {argument} of {}, asserted if {when_true}, was {}",
            callee_label(hir, callee),
            render(hir, of)
        ),
        // Both sources, because both are the root's answer and the reader ranks them: a snapshot
        // showing one could not tell a declared signature that reached the root from one that did
        // not.
        PathRoot::CallbackParameter {
            parameter,
            callee,
            argument,
            siblings,
            ..
        } => {
            let beside = match siblings.is_empty() {
                true => String::new(),
                false => format!(
                    " or the elements of {}",
                    siblings
                        .iter()
                        .map(|sibling| render(hir, sibling))
                        .collect::<Vec<_>>()
                        .join(" or ")
                ),
            };
            format!(
                "parameter {parameter} of the callback passed as argument {argument} of {}{beside}",
                callee_label(hir, callee)
            )
        }
        PathRoot::OneOfClasses { of, classes, .. } => format!(
            "instanceof one of [{}] {}",
            classes
                .iter()
                .map(|class| class_target(hir, *class))
                .collect::<Vec<_>>()
                .join(", "),
            render(hir, of)
        ),
        PathRoot::NotOfClass { of, class, .. } => {
            format!(
                "not instanceof {} {}",
                class_target(hir, *class),
                render(hir, of)
            )
        }
        PathRoot::OfKind {
            of, kind, holds, ..
        } => match holds {
            true => format!("({}) {}", kind.as_str(), render(hir, of)),
            false => format!("not ({}) {}", kind.as_str(), render(hir, of)),
        },
        // The sentinels first, spelled as the kinds they are, so the one line says everything the
        // test ruled out.
        PathRoot::NoneOfKinds {
            of, refused, kinds, ..
        } => format!(
            "none of ({}) {}",
            match refused.takes_the_false() {
                true => ["null", "false"].as_slice(),
                false => ["null"].as_slice(),
            }
            .iter()
            .copied()
            .chain(kinds.iter().map(|kind| kind.as_str()))
            .collect::<Vec<_>>()
            .join("|"),
            render(hir, of)
        ),
        PathRoot::MemberKind {
            of,
            property,
            kind,
            holds,
            ..
        } => format!(
            "{} whose ->{property} is{} {}",
            render(hir, of),
            if *holds { "" } else { " not" },
            kind.as_str()
        ),
        PathRoot::KeyOf { of, container, .. } => {
            format!("{} a key of {}", render(hir, of), render(hir, container))
        }
        PathRoot::ElementOf { of, container, .. } => {
            format!(
                "{} an element of {}",
                render(hir, of),
                render(hir, container)
            )
        }
        PathRoot::SentinelsRefusedBy { of, other, .. } => {
            format!(
                "{} without what {} cannot be",
                render(hir, of),
                render(hir, other)
            )
        }
        // The literal's entries by kind, which is all the rule reads of them.
        PathRoot::NotIdenticalTo { of, literal, .. } => {
            let phrost_hir::body::ExprData::ArrayLiteral { elements, .. } = hir.expr(*literal)
            else {
                unreachable!("an identity is only read against an array literal")
            };
            let entries = hir
                .expr_list(*elements)
                .iter()
                .map(|&element| match hir.expr(element) {
                    phrost_hir::body::ExprData::ArrayElement {
                        kind: phrost_hir::body::ArrayElementKind::LiteralKey { name, .. },
                        value,
                        ..
                    } => format!("{name} => {}", literal_kind(hir, *value)),
                    _ => literal_kind(hir, element).to_owned(),
                })
                .collect::<Vec<_>>();
            format!(
                "{} not identical to [{}]",
                render(hir, of),
                entries.join(", ")
            )
        }
        PathRoot::NotNull {
            of,
            refused: crate::path::Refused::TheEmptyString,
            ..
        } => format!("not null or '' {}", render(hir, of)),
        PathRoot::NotNull { of, .. } => format!("not null {}", render(hir, of)),
        PathRoot::NotEmptyString { of, .. } => format!("not '' {}", render(hir, of)),
        PathRoot::Null { of, strictly, .. } => match strictly {
            true => format!("null, exactly, {}", render(hir, of)),
            false => format!("falsy {}", render(hir, of)),
        },
        PathRoot::This => "$this".to_owned(),
        PathRoot::SelfClass => "self".to_owned(),
        PathRoot::Parent => "<parent>".to_owned(),
        PathRoot::Class(class) => class_target(hir, *class),
        PathRoot::ClassName(class) => format!("{}::class", class_target(hir, *class)),
        PathRoot::ObjectCast(value) => format!("(object) {}", render(hir, value)),
        PathRoot::Constant(name) => format!("const {}", hir.name_ref(*name).path),
        PathRoot::Narrowed { of, class, .. } => {
            format!(
                "instanceof {} {}",
                class_target(hir, *class),
                render(hir, of)
            )
        }
    };
    for step in &path.steps {
        out.push_str(&match step {
            PathStep::Property(name) => format!("->${name}"),
            // The literals are rendered because they are part of what the step means: two calls to
            // the same method with different ids are two different values.
            PathStep::MethodReturn {
                name, arguments, ..
            } => format!(
                "->{name}({})",
                arguments
                    .iter()
                    .map(|literal| match &literal.name {
                        // The name and not the position, because for a named argument the position
                        // is the one thing that means nothing.
                        Some(parameter) => format!("{parameter}: {}", quoted_argument(literal)),
                        None => format!("@{} {}", literal.position, quoted_argument(literal)),
                    })
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            // The key is rendered because it is load-bearing: it is the only thing
            // that tells two slots of the same array apart.
            PathStep::ClassConstant(name) => format!("::{name}"),
            // `$name` for a variable key and `'name'` for a literal one, because they are different
            // slots and a rendering that spelled them alike would hide a wrong answer.
            PathStep::Element(Some(crate::ElementKey::Variable(name))) => {
                format!("[${name}]")
            }
            PathStep::Element(Some(crate::ElementKey::Literal(key))) => {
                format!("['{key}']")
            }
            PathStep::Element(Some(crate::ElementKey::Unnamed)) => "[?]".to_owned(),
            PathStep::Element(None) => "[element]".to_owned(),
            PathStep::Invoked { .. } => "()".to_owned(),
            PathStep::IterationKey => "[key of]".to_owned(),
        });
    }
    out
}

/// A rendering of what a call names, shared by `PathRoot::ByReference` and
/// `PathRoot::AssertedArgument` -- both point at a call rather than at a value.
fn callee_label(hir: &FileHir, callee: &crate::Callee) -> String {
    match callee {
        crate::Callee::Member { name, .. } => format!("{name}()"),
        crate::Callee::Function(reference) => format!("{}()", hir.name_ref(*reference).path),
        crate::Callee::Constructed(class) => format!("new {}", class_target(hir, *class)),
    }
}

/// The kind of scalar literal an expression is, for `PathRoot::NotIdenticalTo`'s entries.
fn literal_kind(hir: &FileHir, expr: phrost_hir::ExprId) -> &'static str {
    match hir.expr(expr) {
        phrost_hir::body::ExprData::Literal { kind, .. } => kind.as_str(),
        _ => "<value>",
    }
}

fn class_target(hir: &FileHir, target: phrost_hir::body::ClassTarget) -> String {
    match target {
        phrost_hir::body::ClassTarget::Named(name) => hir.name_ref(name).path.as_str().to_owned(),
        phrost_hir::body::ClassTarget::Relative { relative, .. } => relative.as_str().to_owned(),
        phrost_hir::body::ClassTarget::Expr(_) => "<value>".to_owned(),
    }
}

/// The type as written, which is all this layer knows -- resolving it needs a catalog.
fn written(hir: &FileHir, id: phrost_hir::TypeRefId) -> String {
    match hir.type_ref(id) {
        TypeRefData::Named(name) => hir.name_ref(*name).path.as_str().to_owned(),
        TypeRefData::Builtin(builtin) => builtin.as_str().to_owned(),
        TypeRefData::Relative(relative) => relative.as_str().to_owned(),
        TypeRefData::Nullable(inner) => format!("?{}", written(hir, *inner)),
        TypeRefData::Generic { base, args, .. } => format!(
            "{}<{}>",
            written(hir, *base),
            hir.type_ref_list(*args)
                .iter()
                .map(|&id| written(hir, id))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        TypeRefData::Union(_) => "<union>".to_owned(),
        TypeRefData::Intersection(_) => "<intersection>".to_owned(),
        TypeRefData::Shape { fields, open, list } => format!(
            "{}{{{}{}}}",
            if *list { "list" } else { "array" },
            hir.type_ref_list(*fields)
                .iter()
                .map(|&id| written(hir, id))
                .collect::<Vec<_>>()
                .join(", "),
            if *open { ", ..." } else { "" }
        ),
        TypeRefData::ObjectShape { fields } => format!(
            "object{{{}}}",
            hir.type_ref_list(*fields)
                .iter()
                .map(|&id| written(hir, id))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        TypeRefData::ShapeField { key, optional, ty } => {
            format!(
                "{key}{}: {}",
                if *optional { "?" } else { "" },
                written(hir, *ty)
            )
        }
    }
}
