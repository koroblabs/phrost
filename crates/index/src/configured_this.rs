//! The calls naming what `$this` is in the closures a callee binds.
//!
//! `test()` declares `@param-closure-this TestCase`, and the class Pest actually binds a test's
//! closure to is configuration: `uses(Tests\TestCase::class)` in the file, or the same with
//! `->in('Feature')` after it in `tests/Pest.php`, for every file below that directory. The call is
//! recorded where it is written -- the classes it names and the directories it is applied to -- and
//! read wherever a closure's `$this` is asked. A body says it and nothing else does, so it travels
//! through the codec with what else the file said.

use phrost_defmap::FileScope;
use phrost_hir::{
    ExprId, FileHir, ImportKind,
    body::{ClassTarget, ExprData, LiteralKind},
};
use phrost_model::{FileId, Interner, Name};
use rustc_hash::FxHashSet;

use crate::{FileSkeleton, ProjectSkeleton, returns::ReturnContracts};

/// A framework's call naming the class its bound closures run as. See the framework seam's
/// `ConfiguredClosureThis`, restated below it for the reason the contracts are.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ConfiguredThisRule {
    /// The class the closures' declarations name.
    pub declared: Box<str>,
    /// The functions beginning such a call.
    pub functions: Box<[Box<str>]>,
    /// The methods along it whose arguments name classes too.
    pub methods: Box<[Box<str>]>,
    /// The method applying it to directories rather than to its own file.
    pub within: Box<str>,
}

/// One such call.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ConfiguredThis {
    /// The classes it names, fully qualified, in the order written: a test case and its traits.
    pub classes: Box<[Name]>,
    /// The directories it is applied to, as written -- from the directory of the file it is in.
    /// `None` for a call configuring its own file.
    pub within: Option<Box<[Box<str>]>>,
}

impl ProjectSkeleton {
    /// Every call in `hir` a framework's rule says names the class its closures run as.
    pub(crate) fn collect_configured_this(
        returns: &ReturnContracts,
        interner: &mut Interner,
        scope: &FileScope,
        text: &str,
        hir: &FileHir,
    ) -> Box<[ConfiguredThis]> {
        let rules = returns.configured_this();
        if rules.is_empty() {
            return Box::default();
        }
        let begins = |expr: &ExprData| match expr {
            ExprData::FunctionCall { name, .. } => {
                let written = hir.name_ref(*name).path.as_str();
                rules.iter().find(|rule| {
                    rule.functions
                        .iter()
                        .any(|function| function.eq_ignore_ascii_case(written))
                })
            }
            _ => None,
        };
        if !hir.expressions().any(|(_, expr)| begins(expr).is_some()) {
            return Box::default();
        }
        // A call another is made on is inside a chain; the end of one is what holds all of it.
        let receivers: FxHashSet<ExprId> = hir
            .expressions()
            .filter_map(|(_, expr)| match expr {
                ExprData::MethodCall { receiver, .. } => Some(*receiver),
                _ => None,
            })
            .collect();
        let mut found = Vec::new();
        for (id, _) in hir.expressions() {
            if receivers.contains(&id) {
                continue;
            }
            let mut methods = Vec::new();
            let mut at = id;
            while let ExprData::MethodCall {
                receiver,
                name: Some(name),
                args,
                ..
            } = hir.expr(at)
            {
                methods.push((name, hir.expr_list(*args)));
                at = *receiver;
            }
            let root = hir.expr(at);
            let Some(rule) = begins(root) else {
                continue;
            };
            let ExprData::FunctionCall { args, .. } = root else {
                continue;
            };
            let mut classes = Vec::new();
            let mut within = None;
            let mut name_classes = |args: &[ExprId], classes: &mut Vec<Name>| {
                for &argument in args {
                    if let Some(class) = Self::class_named(scope, hir, argument) {
                        classes.push(interner.name(&class));
                    }
                }
            };
            name_classes(hir.expr_list(*args), &mut classes);
            // Innermost first, which is the order the chain is written in.
            for (name, args) in methods.into_iter().rev() {
                if name.as_str().eq_ignore_ascii_case(&rule.within) {
                    within = Some(
                        args.iter()
                            .filter_map(|&argument| Self::directory_named(text, hir, argument))
                            .collect::<Box<[Box<str>]>>(),
                    );
                } else if rule
                    .methods
                    .iter()
                    .any(|method| method.eq_ignore_ascii_case(name.as_str()))
                {
                    name_classes(args, &mut classes);
                }
            }
            if !classes.is_empty() {
                found.push(ConfiguredThis {
                    classes: classes.into_boxed_slice(),
                    within,
                });
            }
        }
        found.into_boxed_slice()
    }

    /// The class `Foo::class` names, as the file's imports resolve it.
    fn class_named(scope: &FileScope, hir: &FileHir, argument: ExprId) -> Option<String> {
        let argument = match hir.expr(argument) {
            ExprData::Argument { value, .. } => *value,
            _ => argument,
        };
        let ExprData::ClassConstAccess {
            class: ClassTarget::Named(class),
            name: Some(constant),
            ..
        } = hir.expr(argument)
        else {
            return None;
        };
        if !constant.as_str().eq_ignore_ascii_case("class") {
            return None;
        }
        let written = hir.name_ref(*class);
        let resolved = scope.scope_at(written.range.start()).resolve(
            written.path.as_str(),
            written.anchor,
            ImportKind::ClassLike,
        );
        Some(resolved.primary().fqn().to_owned())
    }

    /// A directory as written: a string, or `__DIR__` for the file's own.
    fn directory_named(text: &str, hir: &FileHir, argument: ExprId) -> Option<Box<str>> {
        let ExprData::Literal {
            kind: LiteralKind::String,
            range,
        } = hir.expr(argument)
        else {
            return None;
        };
        let written = text.get(usize::from(range.start())..usize::from(range.end()))?;
        if written == "__DIR__" {
            return Some(".".into());
        }
        crate::dispatch::unquoted(text, *range).map(Box::from)
    }

    /// The classes the calls configuring `file` name: its own first, in the order written, then
    /// those of every call applied to a directory it is below -- Pest merges them all into the one
    /// test case it generates. Empty where nothing configures it.
    pub fn configured_this_for(&self, file: FileId) -> Vec<Name> {
        let mut classes: Vec<Name> = Vec::new();
        let mut add = |named: &[Name]| {
            for class in named {
                if !classes.contains(class) {
                    classes.push(class.clone());
                }
            }
        };
        for call in self.configured_this.get(&file).into_iter().flatten() {
            if call.within.is_none() {
                add(&call.classes);
            }
        }
        let Some(path) = self.file(file).map(FileSkeleton::path) else {
            return classes;
        };
        let mut configuring: Vec<(&FileId, &ConfiguredThis)> = self
            .configured_this
            .iter()
            .flat_map(|(declarer, calls)| calls.iter().map(move |call| (declarer, call)))
            .filter(|(_, call)| call.within.is_some())
            .collect();
        configuring.sort_by_key(|(declarer, _)| declarer.as_raw());
        for (declarer, call) in configuring {
            let Some(base) = self.file(*declarer).and_then(|entry| entry.path().parent()) else {
                continue;
            };
            if call
                .within
                .iter()
                .flatten()
                .any(|directory| path.starts_with(base.join(&**directory)))
            {
                add(&call.classes);
            }
        }
        classes
    }
}
