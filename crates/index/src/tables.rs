//! What a declaration says, in a method's body, about the properties of the kind it declares.
//!
//! Drupal's field types are the shape and the reason: a project declares its own kinds in PHP, and
//! `StoreSalesItem::propertyDefinitions()` is the only statement anywhere that a `store_sales`
//! field holds a `date`, a `rate` and three more. The framework's own kinds cannot arrive this way
//! -- phrost lowers bodies for project code and deliberately not for vendor -- so a provider writes
//! those out and this reads everything the project adds to them.
//!
//! Read at ingest, where the body still is, and stored with the file. A warm start never lowers
//! anything, so a fact derived here and not written down is one that works on the first server of
//! the day and on none after it -- the shape §8 records for `adopt`.
//!
//! Nothing here knows what a field is. Which attribute marks the declaration, which method holds
//! the table, which static calls name a type and what those names mean are all one contract the
//! framework states: [`DeclaredTableRule`].

pub use phrost_hir::AttributeArgument;

use phrost_hir::{
    FileHir,
    body::{ExprData, LiteralKind, StmtData},
};
use phrost_model::BuiltinTy;

use phrost_defmap::FileScope;

use crate::{ProjectSkeleton, returns::ReturnContracts};

/// A declaration whose body states the properties of the kind it declares.
///
/// The index's own spelling of the rule a framework provider states as `DeclaredTable` -- see it
/// for what each field means and why the vocabulary is the framework's rather than this crate's.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DeclaredTableRule {
    pub attribute: Box<str>,
    pub argument: AttributeArgument,
    pub method: Box<str>,
    pub builders: Vec<Box<str>>,
    pub kinds: Vec<(Box<str>, BuiltinTy)>,
    pub catalog: Box<str>,
}

/// One property and what it holds, or nothing where the declaration named a type PHP has no word
/// for -- the property is *there*, which is the fact a shape needs.
pub type TabledProperty = (Box<str>, Option<BuiltinTy>);

/// One table a file's own declaration states.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct FoundTable {
    pub catalog: Box<str>,
    /// The kind the declaration declares, from its attribute.
    pub kind: Box<str>,
    /// The class whose body states it, fully qualified: an instance of that class *is* one value of
    /// the kind, so `$item->enabled` inside or on it reads the same table a field list's key does.
    pub declarer: Box<str>,
    pub properties: Vec<TabledProperty>,
    /// Where each property's key is written, index for index with [`Self::properties`] -- the
    /// `'enabled'` of `$properties['enabled'] = ...`, which is where goto on `$item->enabled` lands.
    /// Kept beside rather than inside because the other source of the same rows, a provider's table
    /// of the framework's own kinds, has no source to point at.
    pub written_at: Vec<phrost_model::TextRange>,
    /// Whether the walk read every statement of the body.
    ///
    /// A statement it could not read leaves this `false` rather than dropping what it did read: the
    /// keys it found are keys the declaration really writes, and a name it holds is worth
    /// answering. What a partial table may not do is become an `array{...}`.
    pub complete: bool,
}

impl lspkit_memsize::MemSize for FoundTable {
    fn heap_size(&self) -> usize {
        self.catalog.len()
            + self.kind.len()
            + self.declarer.len()
            + lspkit_memsize::vec_flat(&self.properties)
            + lspkit_memsize::vec_flat(&self.written_at)
            + self
                .properties
                .iter()
                .map(|(name, _)| name.len())
                .sum::<usize>()
    }
}

impl ProjectSkeleton {
    /// Every property table this file's own declarations state.
    ///
    /// The attribute literal is what says which declaration is one and what kind it declares, and
    /// lowering has already kept it -- gated, which is why a provider adding this rule must put its
    /// attribute in the keyed set too or nothing is found.
    pub(crate) fn collect_declared_tables(
        returns: &ReturnContracts,
        scope: &FileScope,
        text: &str,
        hir: &FileHir,
    ) -> Box<[FoundTable]> {
        let rules = returns.declared_table_rules();
        if rules.is_empty() {
            return Box::default();
        }

        let mut found = Vec::new();
        for literal in scope.attribute_literals() {
            // An entry of an array argument is not the declaration's own id.
            if literal.key.is_some() {
                continue;
            }
            let Some(symbol) = scope.symbols().get(literal.symbol as usize) else {
                continue;
            };
            for rule in rules {
                if !literal
                    .attribute
                    .as_str()
                    .eq_ignore_ascii_case(&rule.attribute)
                    || literal.argument != rule.argument
                {
                    continue;
                }
                let Some(body) = Self::body_of(hir, symbol.name_range, &rule.method) else {
                    continue;
                };
                let (properties, written_at, complete) = Self::read_table(hir, text, rule, body);
                if properties.is_empty() {
                    continue;
                }
                found.push(FoundTable {
                    catalog: rule.catalog.clone(),
                    kind: literal.value.clone(),
                    declarer: symbol.fqn.as_str().into(),
                    properties,
                    written_at,
                    complete,
                });
            }
        }
        found.into_boxed_slice()
    }

    /// The body of the method named `method` on the class declared at `name_range`.
    ///
    /// Joined on the name range, the one field both tiers keep -- the same join every reader that
    /// has a symbol on one side and the HIR on the other makes.
    fn body_of(
        hir: &FileHir,
        name_range: phrost_model::TextRange,
        method: &str,
    ) -> Option<phrost_hir::BodyId> {
        hir.classes()
            .find(|(_, class)| class.name_range == name_range)
            .and_then(|(_, class)| {
                hir.member_list(class.members)
                    .iter()
                    .map(|&id| hir.member(id))
                    .find(|member| member.name.as_str().eq_ignore_ascii_case(method))
                    .and_then(|member| member.body)
            })
    }

    /// The table one body states, and whether every statement in it was read.
    fn read_table(
        hir: &FileHir,
        text: &str,
        rule: &DeclaredTableRule,
        body: phrost_hir::BodyId,
    ) -> (Vec<TabledProperty>, Vec<phrost_model::TextRange>, bool) {
        let mut properties: Vec<TabledProperty> = Vec::new();
        let mut written_at = Vec::new();
        let mut complete = true;

        for &id in hir.stmt_list(hir.body(body).statements) {
            match hir.stmt(id) {
                // `return $properties;` is the shape of every one of these and says nothing about
                // the table, so it is read and contributes nothing.
                StmtData::Return { .. } => {}
                StmtData::Expr { expr, .. } => {
                    match Self::assigned_property(hir, text, rule, *expr) {
                        Some(((name, holds), at)) => {
                            // A key written twice is the later one, which is what PHP does.
                            if let Some(earlier) =
                                properties.iter().position(|(held, _)| **held == *name)
                            {
                                properties.remove(earlier);
                                written_at.remove(earlier);
                            }
                            properties.push((name, holds));
                            written_at.push(at);
                        }
                        // An `unset`, a `parent::propertyDefinitions()`, a value built in a
                        // variable: each is a statement about the table this cannot read, and each
                        // means the keys above are not all of them.
                        None => complete = false,
                    }
                }
                _ => complete = false,
            }
        }
        (properties, written_at, complete)
    }

    /// `$properties['date'] = DataDefinition::create('string')->setLabel(...)` read as a pair.
    fn assigned_property(
        hir: &FileHir,
        text: &str,
        rule: &DeclaredTableRule,
        expr: phrost_hir::ExprId,
    ) -> Option<(TabledProperty, phrost_model::TextRange)> {
        let ExprData::Assign { target, value, .. } = hir.expr(expr) else {
            return None;
        };
        let ExprData::Index {
            key: Some(key),
            index: Some(index),
            ..
        } = hir.expr(*target)
        else {
            return None;
        };
        let named = Self::builder_argument(hir, text, rule, *value)?;
        Some((
            (
                key.as_str().into(),
                rule.kinds
                    .iter()
                    .find(|(spelled, _)| **spelled == *named)
                    .map(|(_, holds)| *holds),
            ),
            Self::inside_quotes(text, hir.expr(*index).range()),
        ))
    }

    /// The name alone, where a key is written as a quoted literal: what rename rewrites and goto
    /// selects is `enabled`, and a range holding the quotes would have a rename drop them.
    fn inside_quotes(text: &str, range: phrost_model::TextRange) -> phrost_model::TextRange {
        let written = text.get(usize::from(range.start())..usize::from(range.end()));
        match written.map(str::as_bytes) {
            Some([open @ (b'\'' | b'"'), .., close]) if open == close => {
                phrost_model::TextRange::new(
                    range.start() + phrost_model::TextSize::from(1),
                    range.end() - phrost_model::TextSize::from(1),
                )
            }
            _ => range,
        }
    }

    /// The name a builder's first argument writes, at the root of whatever was chained onto it.
    ///
    /// `->setLabel()->setRequired()` and however many more: what says the type is the call at the
    /// bottom, and every method above it hands its own receiver back.
    fn builder_argument(
        hir: &FileHir,
        text: &str,
        rule: &DeclaredTableRule,
        value: phrost_hir::ExprId,
    ) -> Option<Box<str>> {
        // Bounded rather than recursive: a chain is a handful of calls, and a budget is what keeps
        // a shape nobody wrote from walking the arena.
        let mut current = value;
        for _ in 0..16u8 {
            match hir.expr(current) {
                ExprData::MethodCall { receiver, .. } => current = *receiver,
                ExprData::StaticCall {
                    class: phrost_hir::body::ClassTarget::Named(name),
                    name: Some(method),
                    args,
                    ..
                } => {
                    let written = hir.name_ref(*name);
                    let short = written.short();
                    if !rule.builders.iter().any(|builder| **builder == *short)
                        || !method.as_str().eq_ignore_ascii_case("create")
                    {
                        return None;
                    }
                    let &first = hir.expr_list(*args).first()?;
                    let ExprData::Literal {
                        kind: LiteralKind::String,
                        range,
                    } = hir.expr(first)
                    else {
                        return None;
                    };
                    let written = text.get(usize::from(range.start())..usize::from(range.end()))?;
                    return Some(written.trim_matches(['\'', '"']).into());
                }
                _ => return None,
            }
        }
        None
    }
}
