//! Canonical text rendering of one body.
//!
//! Indented to show nesting, one node per line, every node with its range. Positional because the
//! ranges are the whole point: a body is read to answer "what is at this offset", and a snapshot
//! that hid the ranges would pass while every one of them was wrong.

use std::fmt::Write as _;

use lspkit_arena::IdSlice;
use phrost_model::{LineIndex, TextRange};

use crate::{
    FileHir,
    body::data::{
        ArrayElementKind, CallableReceiver, ClassTarget, ClosureKind, ExprData, LeaveKind,
        LogicalOp, NullTestKind, Passage, StmtData, UnmodelledKind,
    },
    ids::{BodyId, ExprId, StmtId},
};

/// Renders a body as an indented tree.
pub fn render_body(hir: &FileHir, lines: &LineIndex, body: BodyId) -> String {
    let mut out = String::new();
    Renderer { hir, lines }.body(&mut out, body, "");
    out
}

struct Renderer<'a> {
    hir: &'a FileHir,
    lines: &'a LineIndex,
}

impl Renderer<'_> {
    fn body(&self, out: &mut String, id: BodyId, indent: &str) {
        let data = self.hir.body(id);
        // Marked only when set, as every optional piece here is: a word on every body in the
        // project would be noise in every snapshot.
        let _ = writeln!(
            out,
            "{indent}body @ {}{}",
            self.range(data.range),
            if data.is_generator {
                "  [generator]"
            } else {
                ""
            }
        );
        self.statements(out, data.statements, &format!("{indent}  "));
    }

    fn statements(&self, out: &mut String, list: IdSlice<StmtId>, indent: &str) {
        for &id in self.hir.stmt_list(list) {
            self.statement(out, id, indent);
        }
    }

    fn statement(&self, out: &mut String, id: StmtId, indent: &str) {
        let deeper = format!("{indent}  ");
        match self.hir.stmt(id) {
            // Rendered as the expression itself: a wrapper line would say nothing the
            // expression's own line does not.
            StmtData::Expr { expr, .. } => self.expression(out, *expr, indent),
            StmtData::Annotation { name, ty, .. } => {
                let _ = writeln!(
                    out,
                    "{indent}@var ${name} {}",
                    crate::render::render_type_ref(self.hir, *ty)
                );
            }
            StmtData::Bind {
                kind,
                bindings,
                range,
            } => {
                let _ = writeln!(out, "{indent}{} @ {}", kind.as_str(), self.range(*range));
                for &binding in self.hir.expr_list(*bindings) {
                    self.expression(out, binding, &deeper);
                }
            }
            StmtData::Return { value, range } => {
                let _ = writeln!(out, "{indent}return @ {}", self.range(*range));
                if let Some(value) = value {
                    self.expression(out, *value, &deeper);
                }
            }
            StmtData::If {
                condition,
                then_branch,
                else_branch,
                range,
            } => {
                let _ = writeln!(out, "{indent}if @ {}", self.range(*range));
                self.expression(out, *condition, &deeper);
                let _ = writeln!(out, "{deeper}then");
                self.statements(out, *then_branch, &format!("{deeper}  "));
                if !self.hir.stmt_list(*else_branch).is_empty() {
                    let _ = writeln!(out, "{deeper}else");
                    self.statements(out, *else_branch, &format!("{deeper}  "));
                }
            }
            StmtData::Foreach {
                subject,
                key,
                value,
                body,
                doc_type_ref,
                range,
            } => {
                let _ = write!(out, "{indent}foreach");
                if let Some(id) = doc_type_ref {
                    let _ = write!(
                        out,
                        " [@var {}]",
                        crate::render::render_type_ref(self.hir, *id)
                    );
                }
                let _ = writeln!(out, " @ {}", self.range(*range));
                self.expression(out, *subject, &deeper);
                if let Some(key) = key {
                    let _ = writeln!(out, "{deeper}key");
                    self.expression(out, *key, &format!("{deeper}  "));
                }
                let _ = writeln!(out, "{deeper}value");
                self.expression(out, *value, &format!("{deeper}  "));
                self.statements(out, *body, &deeper);
            }
            StmtData::While {
                condition,
                body,
                tested_first,
                range,
            } => {
                // Named apart, because whether the condition holds inside the body depends on it
                // and a snapshot that does not show a field cannot test it.
                let keyword = match tested_first {
                    true => "while",
                    false => "do-while",
                };
                let _ = writeln!(out, "{indent}{keyword} @ {}", self.range(*range));
                self.expression(out, *condition, &deeper);
                self.statements(out, *body, &deeper);
            }
            StmtData::For {
                init,
                conditions,
                step,
                body,
                range,
            } => {
                let _ = writeln!(out, "{indent}for @ {}", self.range(*range));
                for &expr in self.hir.expr_list(*init) {
                    self.expression(out, expr, &deeper);
                }
                for &expr in self.hir.expr_list(*conditions) {
                    self.expression(out, expr, &deeper);
                }
                for &expr in self.hir.expr_list(*step) {
                    self.expression(out, expr, &deeper);
                }
                self.statements(out, *body, &deeper);
            }
            StmtData::Block { body, range } => {
                let _ = writeln!(out, "{indent}block @ {}", self.range(*range));
                self.statements(out, *body, &deeper);
            }
            StmtData::Try {
                body,
                catches,
                finally,
                range,
            } => {
                let _ = writeln!(out, "{indent}try @ {}", self.range(*range));
                self.statements(out, *body, &deeper);
                for &catch in self.hir.catch_list(*catches) {
                    let data = self.hir.catch(catch);
                    let _ = write!(out, "{deeper}catch");
                    if let Some(type_ref) = data.type_ref {
                        let _ = write!(
                            out,
                            " {}",
                            crate::render::render_type_ref(self.hir, type_ref)
                        );
                    }
                    if let Some(variable) = &data.variable {
                        let _ = write!(out, " ${variable}");
                    }
                    let _ = writeln!(out, " @ {}", self.range(data.range));
                    self.statements(out, data.body, &format!("{deeper}  "));
                }
                if !self.hir.stmt_list(*finally).is_empty() {
                    let _ = writeln!(out, "{deeper}finally");
                    self.statements(out, *finally, &format!("{deeper}  "));
                }
            }
            StmtData::Switch {
                subject,
                cases,
                range,
            } => {
                let _ = writeln!(out, "{indent}switch @ {}", self.range(*range));
                self.expression(out, *subject, &deeper);
                for &case in self.hir.switch_case_list(*cases) {
                    let data = self.hir.switch_case(case);
                    let label = match (
                        data.is_default,
                        self.hir.expr_list(data.conditions).is_empty(),
                    ) {
                        (true, true) => "default",
                        (true, false) => "case default",
                        (false, _) => "case",
                    };
                    let _ = writeln!(out, "{deeper}{label} @ {}", self.range(data.range));
                    self.conditions(out, data.conditions, &format!("{deeper}  "));
                    self.statements(out, data.body, &format!("{deeper}  "));
                }
            }
            StmtData::Leave { kind, range } => {
                let word = match kind {
                    LeaveKind::Break => "break",
                    LeaveKind::Continue => "continue",
                };
                let _ = writeln!(out, "{indent}{word} @ {}", self.range(*range));
            }
            StmtData::Unmodelled {
                inner,
                range,
                passage,
                kind,
            } => {
                // The passage is rendered because it is the one thing about an unmodelled statement
                // that any answer turns on: a snapshot showing `?? statement` for both an `echo` and
                // a `goto` is a snapshot that cannot tell the two apart, which is how the flow walk
                // came to treat them the same.
                let word = match passage {
                    Passage::Onward => "onward",
                    Passage::Elsewhere => "elsewhere",
                    Passage::Arrival => "arrival",
                };
                let _ = writeln!(
                    out,
                    "{indent}?? {} ({word}) @ {}",
                    match kind {
                        UnmodelledKind::Declaration => "declaration",
                        UnmodelledKind::Unset => "unset",
                        UnmodelledKind::Statement => "statement",
                    },
                    self.range(*range)
                );
                for &expr in self.hir.expr_list(*inner) {
                    self.expression(out, expr, &deeper);
                }
            }
        }
    }

    fn expression(&self, out: &mut String, id: ExprId, indent: &str) {
        let deeper = format!("{indent}  ");
        match self.hir.expr(id) {
            ExprData::Variable { name, range } => {
                let _ = writeln!(out, "{indent}${name} @ {}", self.range(*range));
            }
            ExprData::ArrayLiteral { elements, range } => {
                let _ = writeln!(out, "{indent}array @ {}", self.range(*range));
                for &element in self.hir.expr_list(*elements) {
                    self.expression(out, element, &deeper);
                }
            }
            ExprData::ArrayElement { kind, value, range } => {
                match kind {
                    // The key's own range on the same line rather than as a child, because there is
                    // no node under it -- and it is in the snapshot because it is data the element
                    // carries, so a snapshot that hid it could pass while every one was wrong.
                    ArrayElementKind::LiteralKey {
                        name,
                        range: key_range,
                    } => {
                        let _ = writeln!(
                            out,
                            "{indent}element {name} @ {} (key @ {})",
                            self.range(*range),
                            self.range(*key_range)
                        );
                        self.expression(out, *value, &deeper);
                    }
                    ArrayElementKind::ComputedKey { key } => {
                        let _ = writeln!(out, "{indent}element @ {}", self.range(*range));
                        let _ = writeln!(out, "{deeper}key");
                        self.expression(out, *key, &format!("{deeper}  "));
                        let _ = writeln!(out, "{deeper}value");
                        self.expression(out, *value, &format!("{deeper}  "));
                    }
                    // No labels: a spread has one child, so nothing could be mistaken for another.
                    ArrayElementKind::Spread => {
                        let _ = writeln!(out, "{indent}element ... @ {}", self.range(*range));
                        self.expression(out, *value, &deeper);
                    }
                }
            }
            ExprData::Index {
                base, index, range, ..
            } => {
                let _ = writeln!(
                    out,
                    "{indent}index{} @ {}",
                    if index.is_some() { "" } else { " []" },
                    self.range(*range)
                );
                self.expression(out, *base, &deeper);
                if let Some(index) = index {
                    self.expression(out, *index, &deeper);
                }
            }
            ExprData::Reference { value, range } => {
                let _ = writeln!(out, "{indent}&ref @ {}", self.range(*range));
                self.expression(out, *value, &deeper);
            }
            ExprData::Cast { to, value, range } => {
                let _ = writeln!(out, "{indent}({}) @ {}", to.as_str(), self.range(*range));
                self.expression(out, *value, &deeper);
            }
            ExprData::Argument {
                name,
                spread,
                value,
                range,
            } => {
                let written = match (name, spread) {
                    (Some(name), true) => format!("{name}: ..."),
                    (Some(name), false) => format!("{name}:"),
                    (None, _) => "...".to_owned(),
                };
                let _ = writeln!(out, "{indent}arg {written} @ {}", self.range(*range));
                self.expression(out, *value, &deeper);
            }
            ExprData::IndirectVariable { range } => {
                let _ = writeln!(out, "{indent}$<computed> @ {}", self.range(*range));
            }
            ExprData::Assign {
                operator,
                target,
                value,
                doc_type_ref,
                range,
            } => {
                let _ = write!(out, "{indent}assign");
                if let Some(id) = doc_type_ref {
                    let _ = write!(
                        out,
                        " [@var {}]",
                        crate::render::render_type_ref(self.hir, *id)
                    );
                }
                let _ = writeln!(out, " @ {}", self.range(*range));
                self.expression(out, *target, &deeper);
                let _ = writeln!(out, "{deeper}{}", operator.as_str());
                self.expression(out, *value, &format!("{deeper}  "));
            }
            ExprData::CompoundAssign {
                target,
                value,
                range,
                ..
            } => {
                // The operator itself is not kept: what a reader of this needs is that the target
                // is written, and which arithmetic the operator did is the type layer's question
                // and it declines to answer it.
                let _ = writeln!(out, "{indent}compound assign @ {}", self.range(*range));
                self.expression(out, *target, &deeper);
                if let Some(value) = value {
                    self.expression(out, *value, &format!("{deeper}  "));
                }
            }
            ExprData::Cloned { value, range } => {
                let _ = writeln!(out, "{indent}clone @ {}", self.range(*range));
                self.expression(out, *value, &deeper);
            }
            ExprData::New { class, args, range } => {
                let _ = writeln!(
                    out,
                    "{indent}new {} @ {}",
                    self.class_target(*class),
                    self.range(*range)
                );
                self.arguments(out, *args, &deeper);
            }
            ExprData::MethodCall {
                receiver,
                name,
                args,
                nullsafe,
                range,
                ..
            } => {
                let arrow = if *nullsafe { "?->" } else { "->" };
                let _ = writeln!(
                    out,
                    "{indent}call {arrow}{}() @ {}",
                    Self::selector(name.as_ref()),
                    self.range(*range)
                );
                let _ = writeln!(out, "{deeper}receiver");
                self.expression(out, *receiver, &format!("{deeper}  "));
                self.arguments(out, *args, &deeper);
            }
            ExprData::PropertyAccess {
                receiver,
                name,
                nullsafe,
                range,
                ..
            } => {
                let arrow = if *nullsafe { "?->" } else { "->" };
                let _ = writeln!(
                    out,
                    "{indent}property {arrow}{} @ {}",
                    Self::selector(name.as_ref()),
                    self.range(*range)
                );
                let _ = writeln!(out, "{deeper}receiver");
                self.expression(out, *receiver, &format!("{deeper}  "));
            }
            ExprData::StaticCall {
                class,
                name,
                args,
                range,
                ..
            } => {
                let _ = writeln!(
                    out,
                    "{indent}call {}::{}() @ {}",
                    self.class_target(*class),
                    Self::selector(name.as_ref()),
                    self.range(*range)
                );
                self.class_target_expr(out, *class, &deeper);
                self.arguments(out, *args, &deeper);
            }
            ExprData::StaticPropertyAccess {
                class, name, range, ..
            } => {
                let _ = writeln!(
                    out,
                    "{indent}static property {}::${} @ {}",
                    self.class_target(*class),
                    Self::selector(name.as_ref()),
                    self.range(*range)
                );
                self.class_target_expr(out, *class, &deeper);
            }
            ExprData::ClassConstAccess {
                class, name, range, ..
            } => {
                let _ = writeln!(
                    out,
                    "{indent}class const {}::{} @ {}",
                    self.class_target(*class),
                    Self::selector(name.as_ref()),
                    self.range(*range)
                );
                self.class_target_expr(out, *class, &deeper);
            }
            // Rendered as `callable`, never as `call`, so a snapshot shows at a glance that the
            // shape which builds a `Closure` did not become the shape that enters the callee.
            ExprData::CallableMember {
                receiver,
                name,
                selector,
                name_range,
                range,
                presumed,
            } => {
                let written = match receiver {
                    CallableReceiver::Value(_) => format!("->{}", Self::selector(name.as_ref())),
                    CallableReceiver::Class(class) => format!(
                        "{}::{}",
                        self.class_target(*class),
                        Self::selector(name.as_ref())
                    ),
                };
                // A snapshot that does not show a field cannot test it, and this one decides
                // which readers see the site at all.
                let written_as = if *presumed { " presumed" } else { "" };
                let _ = writeln!(
                    out,
                    "{indent}callable{written_as} {written}(...) @ {} (name @ {})",
                    self.range(*range),
                    self.range(*name_range)
                );
                match receiver {
                    CallableReceiver::Value(value) => {
                        let _ = writeln!(out, "{deeper}receiver");
                        self.expression(out, *value, &format!("{deeper}  "));
                    }
                    CallableReceiver::Class(class) => {
                        self.class_target_expr(out, *class, &deeper);
                    }
                }
                // The computed name, when there is one. Shown here and not for a call, because a
                // callable writes no arguments: the selector is the only expression this shape can
                // hide, so a snapshot without it would show nothing of a whole subtree.
                if let Some(selector) = selector {
                    let _ = writeln!(out, "{deeper}name");
                    self.expression(out, *selector, &format!("{deeper}  "));
                }
            }
            // `callable`, never `call`, for the reason `CallableMember` renders that way.
            ExprData::CallableFunction { name, range } => {
                let _ = writeln!(
                    out,
                    "{indent}callable {}(...) @ {}",
                    self.hir.name_ref(*name).path,
                    self.range(*range)
                );
            }
            ExprData::FunctionCall {
                name, args, range, ..
            } => {
                let _ = writeln!(
                    out,
                    "{indent}call {}() @ {}",
                    self.hir.name_ref(*name).path,
                    self.range(*range)
                );
                self.arguments(out, *args, &deeper);
            }
            ExprData::IndirectCall {
                callee,
                args,
                range,
            } => {
                let _ = writeln!(out, "{indent}call <value>() @ {}", self.range(*range));
                let _ = writeln!(out, "{deeper}callee");
                self.expression(out, *callee, &format!("{deeper}  "));
                self.arguments(out, *args, &deeper);
            }
            ExprData::ConstantAccess { name, range } => {
                let _ = writeln!(
                    out,
                    "{indent}const {} @ {}",
                    self.hir.name_ref(*name).path,
                    self.range(*range)
                );
            }
            ExprData::Not { value, range } => {
                let _ = writeln!(out, "{indent}! @ {}", self.range(*range));
                self.expression(out, *value, &deeper);
            }
            ExprData::Signed { value, range } => {
                let _ = writeln!(out, "{indent}signed @ {}", self.range(*range));
                self.expression(out, *value, &deeper);
            }
            ExprData::BoolTest {
                value,
                expected,
                range,
            } => {
                let literal = match expected {
                    true => "true",
                    false => "false",
                };
                let _ = writeln!(out, "{indent}=== {literal} @ {}", self.range(*range));
                self.expression(out, *value, &deeper);
            }
            ExprData::NullTest { value, kind, range } => {
                let test = match kind {
                    NullTestKind::Identical => "=== null",
                    NullTestKind::Loose => "== null",
                    NullTestKind::Isset => "isset",
                    NullTestKind::Empty => "empty",
                    NullTestKind::Length => "zero length",
                };
                let _ = writeln!(out, "{indent}{test} @ {}", self.range(*range));
                self.expression(out, *value, &deeper);
            }
            ExprData::Coalesce { left, right, range } => {
                let _ = writeln!(out, "{indent}?? @ {}", self.range(*range));
                self.expression(out, *left, &deeper);
                self.expression(out, *right, &deeper);
            }
            ExprData::Include { path, range } => {
                let _ = writeln!(out, "{indent}include @ {}", self.range(*range));
                self.expression(out, *path, &deeper);
            }
            ExprData::Throw { value, range } => {
                let _ = writeln!(out, "{indent}throw @ {}", self.range(*range));
                self.expression(out, *value, &deeper);
            }
            ExprData::Logical {
                operator,
                left,
                right,
                range,
            } => {
                let word = match operator {
                    LogicalOp::And => "&&",
                    LogicalOp::Or => "||",
                };
                let _ = writeln!(out, "{indent}{word} @ {}", self.range(*range));
                self.expression(out, *left, &deeper);
                self.expression(out, *right, &deeper);
            }
            ExprData::Conditional {
                condition,
                then_branch,
                else_branch,
                range,
            } => {
                let _ = writeln!(out, "{indent}?: @ {}", self.range(*range));
                self.expression(out, *condition, &deeper);
                // The absent then-branch of `?:` is printed as nothing rather than as a
                // placeholder: two builds must not be able to disagree about what goes there.
                if let Some(then_branch) = then_branch {
                    self.expression(out, *then_branch, &deeper);
                }
                self.expression(out, *else_branch, &deeper);
            }
            ExprData::Instanceof {
                value,
                class,
                range,
            } => {
                let _ = writeln!(
                    out,
                    "{indent}instanceof {} @ {}",
                    self.class_target(*class),
                    self.range(*range)
                );
                self.expression(out, *value, &deeper);
            }
            ExprData::Closure {
                kind,
                params,
                captures,
                return_type,
                body,
                range,
            } => {
                let keyword = match kind {
                    ClosureKind::Closure => "closure",
                    ClosureKind::StaticClosure => "static closure",
                    ClosureKind::ArrowFunction => "fn",
                    ClosureKind::StaticArrowFunction => "static fn",
                };
                let returns = return_type.map_or_else(String::new, |ty| {
                    format!(": {}", crate::render::render_type_ref(self.hir, ty))
                });
                let _ = writeln!(
                    out,
                    "{indent}{keyword}({}){returns} @ {}",
                    crate::render::render_params(self.hir, *params),
                    self.range(*range)
                );
                for &capture in self.hir.expr_list(*captures) {
                    let _ = write!(out, "{deeper}use ");
                    let mut captured = String::new();
                    self.expression(&mut captured, capture, "");
                    out.push_str(&captured);
                }
                self.body(out, *body, &deeper);
            }
            ExprData::Match {
                subject,
                arms,
                range,
            } => {
                let _ = writeln!(out, "{indent}match @ {}", self.range(*range));
                self.expression(out, *subject, &deeper);
                for &arm in self.hir.match_arm_list(*arms) {
                    let data = self.hir.match_arm(arm);
                    let label = if self.hir.expr_list(data.conditions).is_empty() {
                        "default"
                    } else {
                        "arm"
                    };
                    let _ = writeln!(out, "{deeper}{label} @ {}", self.range(data.range));
                    self.conditions(out, data.conditions, &format!("{deeper}  "));
                    let _ = writeln!(out, "{deeper}  =>");
                    self.expression(out, data.body, &format!("{deeper}    "));
                }
            }
            ExprData::Literal { kind, range } => {
                let _ = writeln!(out, "{indent}{} @ {}", kind.as_str(), self.range(*range));
            }
            ExprData::Operator {
                produces,
                inner,
                range,
            } => {
                let _ = writeln!(
                    out,
                    "{indent}operator -> {} @ {}",
                    produces.as_str(),
                    self.range(*range)
                );
                for &expr in self.hir.expr_list(*inner) {
                    self.expression(out, expr, &deeper);
                }
            }
            ExprData::Comparison {
                operator,
                left,
                right,
                range,
            } => {
                let _ = writeln!(
                    out,
                    "{indent}compare {} @ {}",
                    operator.as_str(),
                    self.range(*range)
                );
                self.expression(out, *left, &deeper);
                self.expression(out, *right, &deeper);
            }
            ExprData::Arithmetic {
                operator,
                left,
                right,
                range,
            } => {
                let _ = writeln!(
                    out,
                    "{indent}arithmetic {} @ {}",
                    operator.as_str(),
                    self.range(*range)
                );
                self.expression(out, *left, &deeper);
                self.expression(out, *right, &deeper);
            }
            ExprData::Terminate { inner, range } => {
                let _ = writeln!(out, "{indent}terminate @ {}", self.range(*range));
                for &expr in self.hir.expr_list(*inner) {
                    self.expression(out, expr, &deeper);
                }
            }
            ExprData::Unmodelled { inner, range } => {
                let _ = writeln!(out, "{indent}?? expression @ {}", self.range(*range));
                for &expr in self.hir.expr_list(*inner) {
                    self.expression(out, expr, &deeper);
                }
            }
        }
    }

    fn arguments(&self, out: &mut String, args: IdSlice<ExprId>, indent: &str) {
        let args = self.hir.expr_list(args);
        if args.is_empty() {
            return;
        }
        let _ = writeln!(out, "{indent}args");
        for &arg in args {
            self.expression(out, arg, &format!("{indent}  "));
        }
    }

    /// The comparison values of a `case` or `match` arm, when it has any to show.
    ///
    /// A separate heading from [`Self::arguments`] rather than reusing it: without one, a
    /// snapshot could not tell where the conditions end and the arm's body begins.
    fn conditions(&self, out: &mut String, list: IdSlice<ExprId>, indent: &str) {
        let list = self.hir.expr_list(list);
        if list.is_empty() {
            return;
        }
        let _ = writeln!(out, "{indent}values");
        for &id in list {
            self.expression(out, id, &format!("{indent}  "));
        }
    }

    /// A class written as a value has an expression to show; a written name does not.
    fn class_target_expr(&self, out: &mut String, target: ClassTarget, indent: &str) {
        if let ClassTarget::Expr(expr) = target {
            let _ = writeln!(out, "{indent}class");
            self.expression(out, expr, &format!("{indent}  "));
        }
    }

    fn class_target(&self, target: ClassTarget) -> String {
        match target {
            ClassTarget::Named(name) => self.hir.name_ref(name).path.as_str().to_owned(),
            ClassTarget::Relative { relative, .. } => relative.as_str().to_owned(),
            ClassTarget::Expr(_) => "<value>".to_owned(),
        }
    }

    /// A member name, or a marker when the name is computed at runtime.
    fn selector(name: Option<&phrost_model::Name>) -> String {
        name.map_or_else(|| "<computed>".to_owned(), |name| name.as_str().to_owned())
    }

    fn range(&self, range: TextRange) -> String {
        let (start, end) = self.lines.range_line_cols(range);
        format!(
            "{}:{}-{}:{}",
            start.line + 1,
            start.col + 1,
            end.line + 1,
            end.col + 1
        )
    }
}
