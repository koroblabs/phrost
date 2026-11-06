//! Lowering snapshot tests.
//!
//! This is the first layer of the per-layer test story: these snapshots describe the HIR and
//! nothing else. When a higher-level feature misbehaves, the question "did the fact survive
//! lowering?" is answered here, by pointing the same fixture at this harness instead of guessing.
//!
//! Snapshots are whole-output, never substring matches. A substring contract would let an extra
//! wrong member through, which is exactly the regression that matters.

mod a_callable_written_as_a_pair;
mod a_docblock_beside_a_nullable_hint;
mod annotation_literals;
mod arrays;
mod attribute_literals;
mod bodies;
mod callback_signature;
mod classes;
mod conditional_declarations;
mod constant_value_types;
mod corpus;
mod declares;
mod defined_constants;
mod doc_see;
mod doc_throws;
mod docblock_members;
mod docblock_templates;
mod docblocks;
mod generators;
mod hash_notation;
mod imports;
mod kept_texts;
mod keyed_containers;
mod list_destructuring;
mod members;
mod null_tests;
mod param_out;
mod partial_application;
mod phpstan_type_alias;
mod pipe_operator;
mod positional_shapes;
mod property_hooks;
mod recovered_annotations;
mod return_from_body;
mod returned_local;
mod sealed_shapes;
mod strings_that_hold_something;
mod types;
mod yield_from_body;

use std::fmt::Write as _;

use expect_test::Expect;
use lspkit_arena::IdSlice;
use phrost_hir::{
    body::{ExprData, StmtData, render_body},
    render::render_file_hir,
};
use phrost_model::{FileId, Interner, LineIndex, TextRange};
use phrost_syntax::ParseSession;
use phrost_testkit::FixtureSpec;

use crate::KeyedAttributes;

/// Lowers one PHP fixture with bodies and snapshots every body it produced.
///
/// A separate harness rather than a flag on [`check_hir`], because the two answer different
/// questions: that one describes what a file declares, this one what a function does. Mixing them
/// would put a hundred lines of statements into every declaration snapshot.
pub(crate) fn check_bodies(fixture: &str, expect: Expect) {
    let spec = FixtureSpec::parse(fixture);
    let file = spec
        .files()
        .iter()
        .find(|file| file.path().ends_with(".php"))
        .expect("the fixture has a PHP file");

    let mut interner = Interner::new();
    let lowered = crate::lower_text_with_bodies(
        &mut interner,
        FileId::from_raw(0),
        file.text(),
        &KeyedAttributes::default(),
    );
    let lines = LineIndex::new(file.text());
    let hir = &lowered.hir;

    let mut out = String::new();
    // File scope first, in source order: a script's statements come before anything it declares.
    if let Some(body) = hir.root_body() {
        assert_ranges_nest(hir, body);
        let rendered = render_body(hir, &lines, body);
        if !rendered.trim().is_empty() {
            let _ = writeln!(out, "<file scope>");
            out.push_str(&indent(&rendered));
        }
    }
    for (_, function) in hir.functions() {
        let _ = writeln!(out, "function {}", function.name);
        match function.body {
            Some(body) => {
                assert_ranges_nest(hir, body);
                out.push_str(&indent(&render_body(hir, &lines, body)));
            }
            None => out.push_str("  <no body>\n"),
        }
    }
    for (_, class) in hir.classes() {
        for &id in hir.member_list(class.members) {
            let member = hir.member(id);
            if member.kind == phrost_hir::MemberKind::Method {
                let _ = writeln!(out, "{}::{}", class.name, member.name);
                match member.body {
                    Some(body) => {
                        assert_ranges_nest(hir, body);
                        out.push_str(&indent(&render_body(hir, &lines, body)));
                    }
                    None => out.push_str("  <no body>\n"),
                }
            }
            // A property hook is a function-like of its own, so it gets its own heading rather than
            // being folded into the property: `$name::set` and `$name::get` are two scopes, and a
            // snapshot that merged them would hide which body a statement is in.
            for &id in hir.hook_list(member.hooks) {
                let hook = hir.hook(id);
                let _ = writeln!(
                    out,
                    "{}::${}::{}({})",
                    class.name,
                    member.name,
                    hook.kind.as_str(),
                    hir.param_list(hook.params)
                        .iter()
                        .map(|&id| format!("${}", hir.param(id).name))
                        .collect::<Vec<_>>()
                        .join(", ")
                );
                match hook.body {
                    Some(body) => {
                        assert_ranges_nest(hir, body);
                        out.push_str(&indent(&render_body(hir, &lines, body)));
                    }
                    None => out.push_str("  <no body>\n"),
                }
            }
        }
    }

    for error in &lowered.errors {
        let _ = writeln!(out, "!! {}", error.message);
    }

    spec.markers().assert_all_used();
    expect.assert_eq(&format!("{}\n", out.trim_end()));
}

/// Every body in the file satisfies "a node's range contains its children's".
///
/// A dimension rather than a test: every fixture that lowers a body checks it, so one new fixture
/// buys the invariant too. It is the one property a position lookup depends on — a node whose range
/// excludes a child means descending by offset walks past the answer — and it is invisible to a
/// snapshot, which shows the ranges without relating them.
fn assert_ranges_nest(hir: &phrost_hir::FileHir, body: phrost_hir::BodyId) {
    fn statements(hir: &phrost_hir::FileHir, list: IdSlice<phrost_hir::StmtId>, outer: TextRange) {
        for &id in hir.stmt_list(list) {
            let data = hir.stmt(id);
            match data {
                StmtData::Expr { expr, range } => {
                    contains(outer, *range, "expression statement");
                    expressions(hir, &[*expr], *range);
                }
                StmtData::Return { value, range } => {
                    contains(outer, *range, "return");
                    expressions(hir, value.as_slice(), *range);
                }
                StmtData::If {
                    condition,
                    then_branch,
                    else_branch,
                    range,
                } => {
                    contains(outer, *range, "if");
                    expressions(hir, &[*condition], *range);
                    statements(hir, *then_branch, *range);
                    statements(hir, *else_branch, *range);
                }
                StmtData::Foreach {
                    subject,
                    key,
                    value,
                    body,
                    doc_type_ref: _,
                    range,
                } => {
                    contains(outer, *range, "foreach");
                    expressions(hir, &[*subject, *value], *range);
                    expressions(hir, key.as_slice(), *range);
                    statements(hir, *body, *range);
                }
                StmtData::While {
                    condition,
                    body,
                    range,
                    ..
                } => {
                    contains(outer, *range, "while");
                    expressions(hir, &[*condition], *range);
                    statements(hir, *body, *range);
                }
                StmtData::For {
                    init,
                    conditions,
                    step,
                    body,
                    range,
                } => {
                    contains(outer, *range, "for");
                    for list in [init, conditions, step] {
                        expressions(hir, hir.expr_list(*list), *range);
                    }
                    statements(hir, *body, *range);
                }
                StmtData::Block { body, range } => {
                    contains(outer, *range, "block");
                    statements(hir, *body, *range);
                }
                StmtData::Try {
                    body,
                    catches,
                    finally,
                    range,
                } => {
                    contains(outer, *range, "try");
                    statements(hir, *body, *range);
                    statements(hir, *finally, *range);
                    for &catch in hir.catch_list(*catches) {
                        let catch = hir.catch(catch);
                        contains(*range, catch.range, "catch");
                        statements(hir, catch.body, catch.range);
                    }
                }
                StmtData::Switch {
                    subject,
                    cases,
                    range,
                } => {
                    contains(outer, *range, "switch");
                    expressions(hir, &[*subject], *range);
                    for &case in hir.switch_case_list(*cases) {
                        let case = hir.switch_case(case);
                        contains(*range, case.range, "switch case");
                        expressions(hir, hir.expr_list(case.conditions), case.range);
                        statements(hir, case.body, case.range);
                    }
                }
                StmtData::Leave { range, .. } => contains(outer, *range, "leave"),
                StmtData::Annotation { range, .. } => contains(outer, *range, "annotation"),
                StmtData::Bind {
                    bindings, range, ..
                } => {
                    contains(outer, *range, "bind");
                    for &binding in hir.expr_list(*bindings) {
                        contains(*range, hir.expr(binding).range(), "bound variable");
                    }
                }
                StmtData::Unmodelled { inner, range, .. } => {
                    contains(outer, *range, "unmodelled statement");
                    expressions(hir, hir.expr_list(*inner), *range);
                }
            }
        }
    }

    fn expressions(hir: &phrost_hir::FileHir, ids: &[phrost_hir::ExprId], outer: TextRange) {
        for &id in ids {
            let data = hir.expr(id);
            contains(outer, data.range(), "expression");
            let inner = data.range();
            match data {
                ExprData::Assign { target, value, .. } => {
                    expressions(hir, &[*target, *value], inner);
                }
                ExprData::New { args, .. } | ExprData::FunctionCall { args, .. } => {
                    expressions(hir, hir.expr_list(*args), inner);
                }
                ExprData::MethodCall { receiver, args, .. } => {
                    expressions(hir, &[*receiver], inner);
                    expressions(hir, hir.expr_list(*args), inner);
                }
                ExprData::PropertyAccess { receiver, .. } => {
                    expressions(hir, &[*receiver], inner);
                }
                ExprData::StaticCall { args, .. } => expressions(hir, hir.expr_list(*args), inner),
                ExprData::IndirectCall { callee, args, .. } => {
                    expressions(hir, &[*callee], inner);
                    expressions(hir, hir.expr_list(*args), inner);
                }
                ExprData::Instanceof { value, .. }
                | ExprData::Not { value, .. }
                | ExprData::Throw { value, .. } => expressions(hir, &[*value], inner),
                ExprData::Logical { left, right, .. } => {
                    expressions(hir, &[*left, *right], inner);
                }
                ExprData::Unmodelled { inner: nested, .. } => {
                    expressions(hir, hir.expr_list(*nested), inner);
                }
                ExprData::Closure { body, .. } => {
                    let data = hir.body(*body);
                    contains(inner, data.range, "closure body");
                    statements(hir, data.statements, data.range);
                }
                ExprData::Match { subject, arms, .. } => {
                    expressions(hir, &[*subject], inner);
                    for &arm in hir.match_arm_list(*arms) {
                        let arm = hir.match_arm(arm);
                        contains(inner, arm.range, "match arm");
                        expressions(hir, hir.expr_list(arm.conditions), arm.range);
                        expressions(hir, &[arm.body], arm.range);
                    }
                }
                _ => {}
            }
        }
    }

    fn contains(outer: TextRange, inner: TextRange, what: &str) {
        assert!(
            outer.contains_range(inner),
            "{what} at {inner:?} is not inside {outer:?}"
        );
    }

    let data = hir.body(body);
    statements(hir, data.statements, data.range);
}

/// Indents a rendered body under the declaration that owns it.
fn indent(text: &str) -> String {
    text.lines()
        .map(|line| format!("  {line}\n"))
        .collect::<String>()
}

/// Lowers every PHP file in the fixture and snapshots the resulting HIR.
///
/// One interner is shared across the fixture's files, matching production: a name that appears in
/// two files is one allocation.
pub(crate) fn check_hir(fixture: &str, expect: Expect) {
    let spec = FixtureSpec::parse(fixture);
    let mut interner = Interner::new();
    let mut out = String::new();

    let php_files = spec
        .files()
        .iter()
        .enumerate()
        .filter(|(_, file)| file.path().ends_with(".php"))
        .collect::<Vec<_>>();
    let multiple = php_files.len() > 1;

    for (index, file) in php_files {
        // A fresh session per file, so the parse arena dies as soon as lowering is done -- the
        // same lifecycle production uses.
        let session = ParseSession::new();
        let parsed = session.parse(file.text());
        let lowered = crate::lower_file(
            &mut interner,
            FileId::from_raw(u32::try_from(index).expect("fixture file count fits in u32")),
            &parsed,
            &KeyedAttributes::default(),
        );

        if multiple {
            let _ = writeln!(out, "// {}", file.path());
        }
        out.push_str(&render_file_hir(&lowered.hir, &LineIndex::new(file.text())));

        // Syntax errors belong in the snapshot: lowering is error-tolerant, and a test needs to
        // show both that a tree came back and what was wrong with it.
        for error in &lowered.errors {
            let _ = writeln!(out, "!! {}", error.message);
        }
    }

    spec.markers().assert_all_used();
    expect.assert_eq(&format!("{}\n", out.trim_end()));
}

/// Every class-like name the file writes, in the order it was recorded.
pub(crate) fn check_name_refs(fixture: &str, expect: Expect) {
    let spec = FixtureSpec::parse(fixture);
    let file = spec
        .files()
        .iter()
        .find(|file| file.path().ends_with(".php"))
        .expect("the fixture has a PHP file");

    let mut interner = Interner::new();
    // With bodies, because a name can be written in one -- a first-class callable is -- and an open
    // file is the only state these features are ever asked about.
    let lowered = crate::lower_text_with_bodies(
        &mut interner,
        FileId::from_raw(0),
        file.text(),
        &KeyedAttributes::default(),
    );
    let lines = LineIndex::new(file.text());

    let mut out = String::new();
    for (_, name_ref) in lowered.hir.name_refs() {
        let start = lines.line_col(name_ref.range.start());
        let end = lines.line_col(name_ref.range.end());
        // Both ends, because where a reference *stops* is a rule in its own right: `@see Foo::bar()`
        // records the class and must not cover the member, or a cursor on `bar()` would goto the
        // class instead of answering nothing.
        let _ = writeln!(
            out,
            "{}  [{:?}] @ {}:{}-{}:{}{}",
            name_ref.path,
            name_ref.kind,
            start.line + 1,
            start.col + 1,
            end.line + 1,
            end.col + 1,
            if name_ref.in_docblock { "  [doc]" } else { "" }
        );
    }
    if out.is_empty() {
        out.push_str("<no name references>\n");
    }

    expect.assert_eq(&out);
}
