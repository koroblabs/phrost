//! Where `$this`, `self`, `static` and `parent` are written without anything to mean.
//!
//! Four of PHP's `Error`s that a file states entirely on its own: `Using $this when not in object
//! context`, `Cannot use "self" when no class scope is active`, the same for `static`, and `Cannot
//! use "parent" when current class scope has no parent`. Every one is fatal the moment the line
//! runs, and the mistake behind the first is ordinary -- a method copied and given `static`, or a
//! closure written `static function` for the garbage collector's sake with a `$this` still in it.
//!
//! # A closure is not decided here, and that is the whole of the precision
//!
//! Run against PHP 8.5.10, one line at a time, and the answers are not what the error messages
//! suggest. **Everything inside a closure compiles**, however the closure is written and wherever
//! it sits: `function () { return $this; }` in a *static* method compiles, `static fn () => $this`
//! compiles, `fn () => parent::class` in a class with no parent compiles, and a `self::` closure at
//! the top level compiles. `Closure::bind` is why -- the scope and the object are settled when the
//! closure is bound, not when it is written, and Symfony has 79 `$this` and 24 `parent::` of
//! exactly this shape that all run.
//!
//! So this walk stops at a closure. What is left is certain: a name written *directly* in a body
//! that can never be rebound.
//!
//! * a method not declared `static` has `$this`; one declared `static` does not, and neither does a
//!   free function. The file's **top level** is left alone: a `.php` file `include`d from inside a
//!   method runs in that object's context, which is what every PHP template is -- Symfony's
//!   `error.html.php` opens with `<?= $this->charset ?>` and is correct;
//! * `self` and `static` need a class around the position at all;
//! * `parent` needs one that extends something -- which is a question for a layer with a hierarchy,
//!   so it is handed up rather than decided here.

use phrost_hir::{
    ClassId, FileHir, MemberKind,
    body::{ClassTarget, ExprData},
};
use phrost_model::{RelativeTy, TextRange};

/// One name written where the language gives it nothing to refer to.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct OutOfContext {
    pub range: TextRange,
    pub written: Written,
    /// The class the position sits in, when it sits in one. `None` for a free function or the
    /// file's top level, which is what makes `self` and `static` refuse there.
    pub enclosing: Option<ClassId>,
}

/// Which of the four was written.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Written {
    This,
    SelfTy,
    StaticTy,
    Parent,
}

/// Every `$this` and relative word in the file with no object or class context to mean.
pub fn out_of_context(hir: &FileHir) -> Vec<OutOfContext> {
    let mut found = Vec::new();

    for (class_id, class) in hir.classes() {
        for &member_id in hir.member_list(class.members) {
            let member = hir.member(member_id);
            if member.owner != class_id || member.kind != MemberKind::Method {
                continue;
            }
            let Some(body) = member.body else {
                continue;
            };
            // A method not declared `static` has `$this`; one declared `static` does not.
            let has_this = !member.modifiers.is_static();
            collect(
                hir,
                hir.body(body).statements,
                has_this,
                Some(class_id),
                &mut found,
            );
        }
        // A property hook is an instance context of its own.
        for &member_id in hir.member_list(class.members) {
            let member = hir.member(member_id);
            for &hook in hir.hook_list(member.hooks) {
                if let Some(body) = hir.hook(hook).body {
                    collect(
                        hir,
                        hir.body(body).statements,
                        true,
                        Some(class_id),
                        &mut found,
                    );
                }
            }
        }
    }

    for (_, function) in hir.functions() {
        if let Some(body) = function.body {
            collect(hir, hir.body(body).statements, false, None, &mut found);
        }
    }
    // The file's top level is deliberately not walked for `$this`. A `.php` file `include`d from
    // inside a method runs in that object's context, which is what every PHP template is:
    // Symfony's `error.html.php` writes `<?= $this->charset ?>` at the top of the file and it is
    // correct. `self` and `static` are still reported there, through the class-range test below.
    if let Some(root) = hir.root_body() {
        collect(hir, hir.body(root).statements, true, None, &mut found);
    }

    found.sort_by_key(|one| one.range.start());
    found.dedup();
    found
}

/// Walks one scope's statements, descending into closures with the context they keep.
fn collect(
    hir: &FileHir,
    statements: lspkit_arena::IdSlice<phrost_hir::StmtId>,
    has_this: bool,
    enclosing: Option<ClassId>,
    found: &mut Vec<OutOfContext>,
) {
    crate::walk::visit_statements(hir, statements, &mut |hir, expr| {
        match hir.expr(expr) {
            ExprData::Variable { name, range } if name.as_str() == "this" && !has_this => {
                found.push(OutOfContext {
                    range: *range,
                    written: Written::This,
                    enclosing,
                });
            }
            // Deliberately not descended into: a closure's `$this`, `self` and `parent` are
            // settled when it is bound, so nothing written here is certain. `visit_statements`
            // does not enter a nested body of its own accord, so this arm exists only to say so.
            ExprData::Closure { .. } => {}
            other => {
                if let Some(target) = class_target(other)
                    && let ClassTarget::Relative { relative, range } = target
                {
                    let written = match relative {
                        RelativeTy::SelfTy => Written::SelfTy,
                        RelativeTy::Static => Written::StaticTy,
                        RelativeTy::Parent => Written::Parent,
                    };
                    // `self` and `static` need a class around the position -- asked of the
                    // *ranges* rather than of the walk, because an expression can be written in a
                    // class outside any body: a constant's initialiser holds `self::OTHER` and
                    // Symfony writes 628 of them.
                    let inside_a_class = hir
                        .classes()
                        .any(|(_, class)| class.full_range.contains_inclusive(range.start()));
                    if !inside_a_class || written == Written::Parent {
                        found.push(OutOfContext {
                            range,
                            written,
                            enclosing,
                        });
                    }
                }
            }
        }
    });
}

/// The class a expression names, when it names one through one of PHP's relative words.
fn class_target(data: &ExprData) -> Option<ClassTarget> {
    match data {
        ExprData::StaticPropertyAccess { class, .. }
        | ExprData::StaticCall { class, .. }
        | ExprData::ClassConstAccess { class, .. }
        | ExprData::New { class, .. }
        | ExprData::Instanceof { class, .. } => Some(*class),
        _ => None,
    }
}
