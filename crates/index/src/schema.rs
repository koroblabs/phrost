//! A table's columns as the project's migrations write them, and the table each model's rows are in.
//!
//! Laravel's `Schema::create('books', function (Blueprint $table) { $table->string('name'); })` is
//! the only statement anywhere that a `Book` has a `name`: nothing in PHP declares it, and the
//! model's `__get` reads the row. So the migrations, applied in the order the framework runs them,
//! are what a model's attributes are -- the reading Larastan makes and completion on a model needs.
//!
//! Read at ingest, where the bodies still are, and stored with the file, for the reason every such
//! fact is (see `stated.rs`). What is stored is each file's own changes; the table they add up to is
//! composed from every file's on the first question after one changes, which is a few hundred
//! changes on a real project and settles every model at once.
//!
//! Nothing here knows Laravel. Which facade and methods change a table, which method of a migration
//! runs, and what each call on the builder does are one contract the framework states:
//! [`WrittenSchemaRule`].

use std::path::Path;

use phrost_defmap::{DeclarationSource, FileScope, NamespaceScope};
use phrost_hir::{
    FileHir, ImportKind, StmtId,
    body::{ClassTarget, ExprData, LiteralKind, StmtData},
};
use phrost_model::{
    BuiltinTy, FileId, TextRange,
    schema::{BuilderCall, ColumnChange},
};
use rustc_hash::FxHashMap;

use crate::{ProjectSkeleton, returns::ReturnContracts};

/// The framework seam's `WrittenSchema`, restated below it -- see it for each field.
#[derive(Clone, Debug)]
pub struct WrittenSchemaRule {
    pub facade: Box<str>,
    pub create: Vec<Box<str>>,
    pub alter: Vec<Box<str>>,
    pub drop: Vec<Box<str>>,
    pub rename: Vec<Box<str>>,
    pub runs: Box<str>,
    pub directory: Box<str>,
    pub columns: fn(&BuilderCall) -> Vec<ColumnChange>,
    pub model: Box<str>,
    pub table_property: Box<str>,
    pub default_table: fn(&str) -> String,
}

/// What one file says about the schema.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct FileSchema {
    /// What the file's migration does, in the order it does it.
    pub changes: Vec<TableChange>,
    /// The tables its classes name in the rule's property: `protected $table = 'shelves';`.
    pub named: Vec<NamedTable>,
}

impl FileSchema {
    pub fn is_empty(&self) -> bool {
        self.changes.is_empty() && self.named.is_empty()
    }
}

/// One call on the facade: a table created, changed, dropped or renamed.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TableChange {
    pub table: Box<str>,
    pub kind: TableChangeKind,
    /// What the callback did to its columns, in order. Empty for a drop or a rename.
    pub columns: Vec<WrittenColumn>,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum TableChangeKind {
    Created,
    Altered,
    Dropped,
    RenamedTo(Box<str>),
}

/// One column change and where it is written: the column's name between its quotes, or the call
/// that adds it where the method names it (`timestamps()`).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct WrittenColumn {
    pub change: ColumnChange,
    pub at: TextRange,
}

/// A class naming its table outright.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct NamedTable {
    pub class: Box<str>,
    pub table: Box<str>,
}

/// One column of a table as every migration so far leaves it, and where it was last defined.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Column {
    pub name: Box<str>,
    pub holds: BuiltinTy,
    pub nullable: bool,
    /// A foreign key the database enforces -- see `ColumnChange::Defined::constrained`.
    pub constrained: bool,
    pub file: FileId,
    pub at: TextRange,
}

/// Every table the migrations leave, by name.
#[derive(Debug, Default)]
pub struct Schema {
    tables: FxHashMap<Box<str>, Vec<Column>>,
}

impl Schema {
    pub fn table(&self, name: &str) -> Option<&[Column]> {
        self.tables.get(name).map(Vec::as_slice)
    }

    /// Every table, by name.
    pub fn tables(&self) -> Vec<(&str, &[Column])> {
        let mut all = self
            .tables
            .iter()
            .map(|(name, columns)| (&**name, columns.as_slice()))
            .collect::<Vec<_>>();
        all.sort_by_key(|(name, _)| *name);
        all
    }

    /// Applies one file's changes on top of what the files before it left.
    fn apply(&mut self, file: FileId, changes: &[TableChange]) {
        for change in changes {
            match &change.kind {
                TableChangeKind::Dropped => {
                    self.tables.remove(&change.table);
                    continue;
                }
                TableChangeKind::RenamedTo(to) => {
                    if let Some(columns) = self.tables.remove(&change.table) {
                        self.tables.insert(to.clone(), columns);
                    }
                    continue;
                }
                TableChangeKind::Created => {
                    self.tables.insert(change.table.clone(), Vec::new());
                }
                TableChangeKind::Altered => {}
            }
            let columns = self.tables.entry(change.table.clone()).or_default();
            for written in &change.columns {
                match &written.change {
                    ColumnChange::Defined {
                        name,
                        holds,
                        nullable,
                        constrained,
                    } => {
                        let column = Column {
                            name: name.as_str().into(),
                            holds: *holds,
                            nullable: *nullable,
                            constrained: *constrained,
                            file,
                            at: written.at,
                        };
                        // A column defined again is the one `->change()` redefines.
                        match columns.iter_mut().find(|known| *known.name == **name) {
                            Some(known) => *known = column,
                            None => columns.push(column),
                        }
                    }
                    ColumnChange::Constrained(name) => {
                        if let Some(known) = columns.iter_mut().find(|known| *known.name == **name)
                        {
                            known.constrained = true;
                        }
                    }
                    ColumnChange::Dropped(name) => columns.retain(|known| *known.name != **name),
                    ColumnChange::Renamed { from, to } => {
                        if let Some(known) = columns.iter_mut().find(|known| *known.name == **from)
                        {
                            known.name = to.as_str().into();
                        }
                    }
                }
            }
        }
    }
}

impl lspkit_memsize::MemSize for FileSchema {
    fn heap_size(&self) -> usize {
        let changes = self
            .changes
            .iter()
            .map(|change| {
                change.table.len()
                    + lspkit_memsize::vec_flat(&change.columns)
                    + change
                        .columns
                        .iter()
                        .map(|written| match &written.change {
                            ColumnChange::Defined { name, .. }
                            | ColumnChange::Constrained(name)
                            | ColumnChange::Dropped(name) => name.len(),
                            ColumnChange::Renamed { from, to } => from.len() + to.len(),
                        })
                        .sum::<usize>()
            })
            .sum::<usize>();
        let named = self
            .named
            .iter()
            .map(|named| named.class.len() + named.table.len())
            .sum::<usize>();
        lspkit_memsize::vec_flat(&self.changes)
            + lspkit_memsize::vec_flat(&self.named)
            + changes
            + named
    }
}

impl ProjectSkeleton {
    /// What this file says about the schema: the tables its classes name, and -- for a file under
    /// the rule's directory -- what its migration's running method does to them.
    pub(crate) fn collect_schema(
        returns: &ReturnContracts,
        interner: &mut phrost_model::Interner,
        scope: &FileScope,
        path: &Path,
        text: &str,
        hir: &FileHir,
    ) -> FileSchema {
        let mut found = FileSchema::default();
        let Some(rule) = returns.written_schema().first() else {
            return found;
        };
        for kept in hir.kept_texts() {
            if kept.property.as_str() != &*rule.table_property {
                continue;
            }
            if let Some(symbol) = scope.symbol_of(DeclarationSource::Class(kept.class)) {
                found.named.push(NamedTable {
                    class: symbol.fqn.as_str().into(),
                    table: kept.text.clone(),
                });
            }
        }
        // A test that builds a table in `setUp()` writes the same calls, and is not the schema.
        let under = path
            .components()
            .any(|part| part.as_os_str() == std::ffi::OsStr::new(&*rule.directory));
        if !under {
            return found;
        }
        for (_, class) in hir.classes() {
            let reader = Reader {
                rule,
                hir,
                text,
                namespace: scope.scope_of(class.namespace),
            };
            for &member in hir.member_list(class.members) {
                let data = hir.member(member);
                if data.kind != phrost_hir::MemberKind::Method
                    || !data.name.as_str().eq_ignore_ascii_case(&rule.runs)
                {
                    continue;
                }
                if let Some(body) = data.body {
                    reader.statements(hir.stmt_list(hir.body(body).statements), &mut found.changes);
                }
            }
        }
        // Interned, because a column is a place goto names and a request cannot intern.
        for change in &found.changes {
            for written in &change.columns {
                match &written.change {
                    ColumnChange::Defined { name, .. } => {
                        interner.name(name);
                    }
                    ColumnChange::Renamed { to, .. } => {
                        interner.name(to);
                    }
                    ColumnChange::Constrained(_) | ColumnChange::Dropped(_) => {}
                }
            }
        }
        found
    }

    /// Every table the project's migrations leave, applied in the order the framework runs them:
    /// by file name, which is where each one's timestamp is written.
    pub(crate) fn compose_schema(&self) -> Schema {
        let mut files = self
            .schemas
            .iter()
            .filter(|(_, said)| !said.changes.is_empty())
            .filter_map(|(file, said)| {
                let path = self.file(*file)?.path();
                Some((path.file_name()?.to_owned(), path.to_owned(), *file, said))
            })
            .collect::<Vec<_>>();
        files.sort_by(|a, b| (&a.0, &a.1).cmp(&(&b.0, &b.1)));
        let mut schema = Schema::default();
        for (_, _, file, said) in files {
            schema.apply(file, &said.changes);
        }
        schema
    }
}

/// The walk over one migration method, with what every step of it needs.
struct Reader<'a> {
    rule: &'a WrittenSchemaRule,
    hir: &'a FileHir,
    text: &'a str,
    namespace: &'a NamespaceScope,
}

impl Reader<'_> {
    /// The facade calls among `statements`, in order -- through a block and both arms of an `if`,
    /// since a migration that adds a column only where it is missing still adds it.
    fn statements(&self, statements: &[StmtId], changes: &mut Vec<TableChange>) {
        for &statement in statements {
            match self.hir.stmt(statement) {
                StmtData::Expr { expr, .. } => {
                    if let Some(change) = self.facade_call(*expr) {
                        changes.push(change);
                    }
                }
                StmtData::Block { body, .. } => {
                    self.statements(self.hir.stmt_list(*body), changes);
                }
                StmtData::If {
                    then_branch,
                    else_branch,
                    ..
                } => {
                    self.statements(self.hir.stmt_list(*then_branch), changes);
                    self.statements(self.hir.stmt_list(*else_branch), changes);
                }
                _ => {}
            }
        }
    }

    /// `Schema::create('books', function (Blueprint $table) { ... })` and its siblings.
    fn facade_call(&self, expr: phrost_hir::ExprId) -> Option<TableChange> {
        let ExprData::StaticCall {
            class: ClassTarget::Named(named),
            name: Some(method),
            args,
            ..
        } = self.hir.expr(expr)
        else {
            return None;
        };
        let written = self.hir.name_ref(*named);
        let class = self
            .namespace
            .resolve(written.path.as_str(), written.anchor, ImportKind::ClassLike)
            .primary()
            .fqn()
            .to_owned();
        if !class.eq_ignore_ascii_case(&self.rule.facade) {
            return None;
        }
        let is = |names: &[Box<str>]| {
            names
                .iter()
                .any(|name| method.as_str().eq_ignore_ascii_case(name))
        };
        let args = self.hir.expr_list(*args);
        let table: Box<str> = self.string(*args.first()?)?.into();
        if is(&self.rule.drop) {
            return Some(TableChange {
                table,
                kind: TableChangeKind::Dropped,
                columns: Vec::new(),
            });
        }
        if is(&self.rule.rename) {
            let to = self.string(*args.get(1)?)?;
            return Some(TableChange {
                table,
                kind: TableChangeKind::RenamedTo(to.into()),
                columns: Vec::new(),
            });
        }
        let kind = match (is(&self.rule.create), is(&self.rule.alter)) {
            (true, _) => TableChangeKind::Created,
            (_, true) => TableChangeKind::Altered,
            _ => return None,
        };
        let mut columns = Vec::new();
        if let Some(&callback) = args.get(1)
            && let ExprData::Closure { params, body, .. } = self.hir.expr(self.unwrapped(callback))
            && let Some(&builder) = self.hir.param_list(*params).first()
        {
            let builder = self.hir.param(builder).name.as_str();
            self.builder_calls(
                self.hir.stmt_list(self.hir.body(*body).statements),
                builder,
                &mut columns,
            );
        }
        Some(TableChange {
            table,
            kind,
            columns,
        })
    }

    /// Every statement that is a chain of calls on the builder, as the framework reads it.
    fn builder_calls(&self, statements: &[StmtId], builder: &str, out: &mut Vec<WrittenColumn>) {
        for &statement in statements {
            match self.hir.stmt(statement) {
                // An arrow function's body is lowered to the one `return` it amounts to.
                StmtData::Expr { expr, .. }
                | StmtData::Return {
                    value: Some(expr), ..
                } => self.builder_chain(*expr, builder, out),
                StmtData::Block { body, .. } => {
                    self.builder_calls(self.hir.stmt_list(*body), builder, out);
                }
                StmtData::If {
                    then_branch,
                    else_branch,
                    ..
                } => {
                    self.builder_calls(self.hir.stmt_list(*then_branch), builder, out);
                    self.builder_calls(self.hir.stmt_list(*else_branch), builder, out);
                }
                _ => {}
            }
        }
    }

    /// `$table->string('email')->nullable()`: the call on the builder itself, and what is chained
    /// after it.
    fn builder_chain(&self, expr: phrost_hir::ExprId, builder: &str, out: &mut Vec<WrittenColumn>) {
        // Outermost first, which is the reverse of the order they run in.
        let mut chain = Vec::new();
        let mut at = expr;
        loop {
            let ExprData::MethodCall {
                receiver,
                name: Some(name),
                args,
                name_range,
                ..
            } = self.hir.expr(at)
            else {
                return;
            };
            chain.push((name.as_str(), *args, *name_range));
            match self.hir.expr(*receiver) {
                ExprData::Variable { name, .. } if name.as_str() == builder => break,
                _ => at = *receiver,
            }
        }
        let Some((method, args, method_range)) = chain.pop() else {
            return;
        };
        let literals = self
            .hir
            .expr_list(args)
            .iter()
            .map(|&argument| self.literals(argument))
            .collect::<Vec<_>>();
        let call = BuilderCall {
            method: method.to_owned(),
            arguments: literals
                .iter()
                .map(|written| written.iter().map(|(value, _)| value.clone()).collect())
                .collect(),
            then: chain
                .iter()
                .rev()
                .map(|(name, args, _)| {
                    let flag = self
                        .hir
                        .expr_list(*args)
                        .first()
                        .and_then(|&first| self.bool_literal(first));
                    ((*name).to_owned(), flag)
                })
                .collect(),
        };
        for change in (self.rule.columns)(&call) {
            let named = match &change {
                ColumnChange::Defined { name, .. }
                | ColumnChange::Constrained(name)
                | ColumnChange::Dropped(name) => name,
                ColumnChange::Renamed { to, .. } => to,
            };
            // Where the name is written, when a literal wrote it.
            let at = literals
                .iter()
                .flatten()
                .find(|(value, _)| value == named)
                .map_or(method_range, |(_, range)| *range);
            out.push(WrittenColumn { change, at });
        }
    }

    /// An argument's string literals with where each is written: the literal, every one in an array
    /// of them, or a `::class`, which names the class.
    fn literals(&self, argument: phrost_hir::ExprId) -> Vec<(String, TextRange)> {
        let argument = self.unwrapped(argument);
        match self.hir.expr(argument) {
            ExprData::ArrayLiteral { elements, .. } => self
                .hir
                .expr_list(*elements)
                .iter()
                .filter_map(|&element| self.literal(element))
                .collect(),
            _ => self.literal(argument).into_iter().collect(),
        }
    }

    fn literal(&self, expr: phrost_hir::ExprId) -> Option<(String, TextRange)> {
        match self.hir.expr(expr) {
            ExprData::Literal {
                kind: LiteralKind::String,
                range,
            } => Some((self.unquoted(*range)?, Self::inside(*range))),
            ExprData::ClassConstAccess {
                class: ClassTarget::Named(named),
                name: Some(constant),
                range,
                ..
            } if constant.as_str().eq_ignore_ascii_case("class") => {
                let written = self.hir.name_ref(*named);
                let fqn = self
                    .namespace
                    .resolve(written.path.as_str(), written.anchor, ImportKind::ClassLike)
                    .primary()
                    .fqn()
                    .to_owned();
                Some((fqn, *range))
            }
            _ => None,
        }
    }

    fn string(&self, expr: phrost_hir::ExprId) -> Option<String> {
        match self.hir.expr(self.unwrapped(expr)) {
            ExprData::Literal {
                kind: LiteralKind::String,
                range,
            } => self.unquoted(*range),
            _ => None,
        }
    }

    fn bool_literal(&self, expr: phrost_hir::ExprId) -> Option<bool> {
        match self.hir.expr(self.unwrapped(expr)) {
            ExprData::Literal {
                kind: LiteralKind::True,
                ..
            } => Some(true),
            ExprData::Literal {
                kind: LiteralKind::False,
                ..
            } => Some(false),
            _ => None,
        }
    }

    /// A positional argument is its own expression; a named or spread one is wrapped.
    fn unwrapped(&self, expr: phrost_hir::ExprId) -> phrost_hir::ExprId {
        match self.hir.expr(expr) {
            ExprData::Argument { value, .. } => *value,
            _ => expr,
        }
    }

    /// A string literal's value, for the spellings a column's name is written in: single quotes,
    /// or double quotes with nothing PHP would interpret.
    fn unquoted(&self, range: TextRange) -> Option<String> {
        let written = self
            .text
            .get(usize::from(range.start())..usize::from(range.end()))?;
        if let Some(inner) = written
            .strip_prefix('\'')
            .and_then(|w| w.strip_suffix('\''))
        {
            return Some(inner.replace("\\'", "'").replace("\\\\", "\\"));
        }
        let inner = written.strip_prefix('"')?.strip_suffix('"')?;
        (!inner.contains(['\\', '$'])).then(|| inner.to_owned())
    }

    fn inside(range: TextRange) -> TextRange {
        let one = phrost_model::TextSize::from(1);
        match range.len() >= phrost_model::TextSize::from(2) {
            true => TextRange::new(range.start() + one, range.end() - one),
            false => range,
        }
    }
}
