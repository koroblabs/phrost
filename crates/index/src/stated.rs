//! What a class states, in an array it writes, about the properties `__get` answers for it.
//!
//! Eloquent's model is the case and the reason: `$casts = ['published_at' => 'datetime']`, a
//! `casts()` method returning the same, and `$fillable = ['title']` are the only statements
//! anywhere that `$post->published_at` is a date and `$post->title` is there at all. Nothing
//! declares either property; the model's `__get` reads the database row.
//!
//! Read at ingest, where the property's kept default and the method's body still are, and stored
//! with the file: a warm start never lowers anything, so a fact derived here and not written down
//! works on the first server of the day and on none after it.
//!
//! What is stored is what was *written* -- the cast's spelling, the class a `::class` names --
//! and not what it means. Which spelling is which type is the framework's vocabulary and is read
//! where the types are, so the store never has to be rebuilt for a change to it. Which property
//! and method names hold such an array is a contract the framework states:
//! [`StatedPropertiesRule`].

use phrost_defmap::{DeclarationSource, FileScope};
use phrost_hir::{
    FileHir, ImportKind, KeptValueData,
    body::{ArrayElementKind, ClassTarget, ExprData, LiteralKind, StmtData},
};
use phrost_model::TextRange;

use crate::{ProjectSkeleton, returns::ReturnContracts};

/// Which names a class writes such arrays under, and what the spellings in them mean. The
/// framework's own `StatedProperties` restated below the seam, for the reason every contract here
/// is -- see it for each field.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct StatedPropertiesRule {
    pub class: Box<str>,
    /// Properties and methods whose array maps a property name to what it holds: `casts`.
    pub typed: Vec<Box<str>>,
    /// Properties whose array lists property names and nothing about them: `fillable`.
    pub listed: Vec<Box<str>>,
    /// Attributes on the class, by last segment, whose string list does the same: `Fillable`.
    pub listed_by_attribute: Vec<Box<str>>,
    pub spellings: Vec<(Box<str>, StatedKind)>,
    pub wrapping: Vec<Box<str>>,
    pub classes: Vec<(Box<str>, StatedKind)>,
    pub custom: Option<(Box<str>, Box<str>)>,
    pub implicit: Vec<(Box<str>, StatedKind)>,
    pub key: Option<StatedKeyRule>,
    pub nullable: bool,
}

/// The framework's `StatedKey`, restated below the seam.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct StatedKeyRule {
    pub name: Box<str>,
    pub kind: StatedKind,
    pub unless: Vec<Box<str>>,
}

/// What a spelling means: a builtin, or a class.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum StatedKind {
    Builtin(phrost_model::BuiltinTy),
    Class(Box<str>),
}

/// One class's statements, merged.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct StatedTable {
    /// The class that wrote them, fully qualified.
    pub declarer: Box<str>,
    /// In the order written, a method's after a property's -- so a later row for one name is the
    /// one PHP's own merge keeps.
    pub rows: Vec<StatedRow>,
    /// What each accessor method's getter returns, where the method's body hands the framework a
    /// closure that says: `headline(): Attribute { return Attribute::make(get: fn (): string =>
    /// ...); }`. The signature says only `Attribute`, and the closure is the one statement of what
    /// `$post->headline` is.
    pub getters: Vec<StatedGetter>,
}

/// One accessor's getter type. See [`StatedTable::getters`].
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct StatedGetter {
    pub method: phrost_model::Name,
    pub ty: phrost_ty::TypeId,
}

/// One property a class states.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct StatedRow {
    /// Interned in the index's own table, because goto hands a location named by it back and a
    /// request cannot intern.
    pub name: phrost_model::Name,
    /// Where the name is written, inside its quotes.
    pub at: TextRange,
    pub value: StatedValue,
    /// What wrote it: `$casts` for a property's default, `casts()` for a method's body. A subclass
    /// redeclaring either replaces the parent's whole array -- a property's default is not merged,
    /// and an override does not call the parent unless it says so -- so the reader needs to know
    /// which of the parent's rows the subclass shadowed.
    pub source: Box<str>,
}

/// What a row says the property holds, as written.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum StatedValue {
    /// Listed, and nothing said about it.
    Listed,
    /// A spelling: `datetime`, `decimal:2`.
    Text(Box<str>),
    /// A class a `::class` names, resolved against the file that wrote it.
    Class(Box<str>),
    /// Something no literal says -- the property is still there.
    Other,
}

impl lspkit_memsize::MemSize for StatedTable {
    fn heap_size(&self) -> usize {
        self.declarer.len()
            + lspkit_memsize::vec_flat(&self.getters)
            + lspkit_memsize::vec_flat(&self.rows)
            + self
                .rows
                .iter()
                .map(|row| {
                    row.source.len()
                        + match &row.value {
                            StatedValue::Text(text) | StatedValue::Class(text) => text.len(),
                            StatedValue::Listed | StatedValue::Other => 0,
                        }
                })
                .sum::<usize>()
    }
}

impl ProjectSkeleton {
    /// Every class in this file that states properties under a name the rules give.
    pub(crate) fn collect_stated(
        returns: &ReturnContracts,
        interner: &mut phrost_model::Interner,
        types: &mut phrost_ty::TypeTable,
        scope: &FileScope,
        text: &str,
        hir: &FileHir,
    ) -> Box<[StatedTable]> {
        let rules = returns.stated_property_rules();
        let accessors = returns.accessors();
        if rules.is_empty() && accessors.is_empty() {
            return Box::default();
        }
        let typed = |name: &str| {
            rules
                .iter()
                .any(|rule| rule.typed.iter().any(|t| &**t == name))
        };
        let listed = |name: &str| {
            rules
                .iter()
                .any(|rule| rule.listed.iter().any(|l| &**l == name))
        };
        let by_attribute = |name: &str| {
            rules.iter().any(|rule| {
                // By the last segment, which is what lowering kept: the attribute is matched the
                // way it is written, and a rule names it in full.
                rule.listed_by_attribute.iter().any(|attribute| {
                    attribute
                        .rsplit('\\')
                        .next()
                        .is_some_and(|short| short.eq_ignore_ascii_case(name))
                })
            })
        };

        let mut found = Vec::new();
        for (id, class) in hir.classes() {
            let namespace = scope.scope_of(class.namespace);
            let resolve = |name_ref: phrost_hir::NameRefId| -> Box<str> {
                let written = hir.name_ref(name_ref);
                namespace
                    .resolve(written.path.as_str(), written.anchor, ImportKind::ClassLike)
                    .primary()
                    .fqn()
                    .into()
            };
            let mut rows = Vec::new();

            // A property's default first: the method is what Laravel merges over it.
            for kept in hir.kept_tables().iter().filter(|kept| kept.class == id) {
                if kept.from_attribute {
                    if !by_attribute(kept.property.as_str()) {
                        continue;
                    }
                    rows.extend(kept.entries.iter().map(|entry| StatedRow {
                        name: interner.name(entry.name.as_str()),
                        at: entry.range,
                        value: StatedValue::Listed,
                        source: format!("#[{}]", kept.property).into(),
                    }));
                    continue;
                }
                let as_typed = typed(kept.property.as_str());
                if !as_typed && !listed(kept.property.as_str()) {
                    continue;
                }
                for entry in &kept.entries {
                    let value = match (&entry.value, as_typed) {
                        (Some(KeptValueData::Text(text)), true) => StatedValue::Text(text.clone()),
                        (Some(KeptValueData::Class(name)), true) => {
                            StatedValue::Class(resolve(*name))
                        }
                        (Some(KeptValueData::Other), true) => StatedValue::Other,
                        (None, false) => StatedValue::Listed,
                        // A list where a map is read, or the other way round: not this shape.
                        _ => continue,
                    };
                    rows.push(StatedRow {
                        name: interner.name(entry.name.as_str()),
                        at: entry.range,
                        value,
                        source: format!("${}", kept.property).into(),
                    });
                }
            }

            for &member in hir.member_list(class.members) {
                let data = hir.member(member);
                if data.kind != phrost_hir::MemberKind::Method || !typed(data.name.as_str()) {
                    continue;
                }
                let Some(body) = data.body else {
                    continue;
                };
                let source: Box<str> = format!("{}()", data.name).into();
                Self::read_returned_array(hir, text, body, &resolve, interner, &source, &mut rows);
            }

            let getters = Self::read_getters(hir, accessors, namespace, interner, types, class);
            if rows.is_empty() && getters.is_empty() {
                continue;
            }
            let Some(symbol) = scope.symbol_of(DeclarationSource::Class(id)) else {
                continue;
            };
            found.push(StatedTable {
                declarer: symbol.fqn.as_str().into(),
                rows,
                getters,
            });
        }
        found.into_boxed_slice()
    }

    /// The literal entries of the array a method's body returns: `return ['at' => 'datetime'];`.
    fn read_returned_array(
        hir: &FileHir,
        text: &str,
        body: phrost_hir::BodyId,
        resolve: &dyn Fn(phrost_hir::NameRefId) -> Box<str>,
        interner: &mut phrost_model::Interner,
        source: &str,
        rows: &mut Vec<StatedRow>,
    ) {
        for &statement in hir.stmt_list(hir.body(body).statements) {
            let StmtData::Return {
                value: Some(value), ..
            } = hir.stmt(statement)
            else {
                continue;
            };
            let ExprData::ArrayLiteral { elements, .. } = hir.expr(*value) else {
                continue;
            };
            for &element in hir.expr_list(*elements) {
                let ExprData::ArrayElement {
                    kind: ArrayElementKind::LiteralKey { name, range },
                    value,
                    ..
                } = hir.expr(element)
                else {
                    continue;
                };
                let value = match hir.expr(*value) {
                    ExprData::Literal {
                        kind: LiteralKind::String,
                        range,
                    } => text
                        .get(usize::from(range.start())..usize::from(range.end()))
                        .map_or(StatedValue::Other, |written| {
                            StatedValue::Text(written.trim_matches(['\'', '"']).into())
                        }),
                    ExprData::ClassConstAccess {
                        class: ClassTarget::Named(named),
                        name: Some(constant),
                        ..
                    } if constant.as_str().eq_ignore_ascii_case("class") => {
                        StatedValue::Class(resolve(*named))
                    }
                    _ => StatedValue::Other,
                };
                rows.push(StatedRow {
                    name: interner.name(name.as_str()),
                    at: Self::name_between_quotes(text, *range),
                    value,
                    source: source.into(),
                });
            }
        }
    }

    /// The getter each accessor method of a class hands the framework, where a closure states it.
    ///
    /// A method whose hint is the rule's attribute class by its last segment, whose body returns a
    /// call on that class -- `::make(get: ...)`, `::get(...)`, `new ...(get: ...)` -- and whose
    /// getter is a closure with a return type. A getter written any other way says nothing here.
    fn read_getters(
        hir: &FileHir,
        accessors: &[crate::AccessorRule],
        namespace: &phrost_defmap::NamespaceScope,
        interner: &mut phrost_model::Interner,
        types: &mut phrost_ty::TypeTable,
        class: &phrost_hir::ClassData,
    ) -> Vec<StatedGetter> {
        let mut getters = Vec::new();
        if accessors.is_empty() {
            return getters;
        }
        let is_attribute = |short: &str| {
            accessors.iter().any(|rule| {
                rule.attribute
                    .rsplit('\\')
                    .next()
                    .is_some_and(|attribute| attribute.eq_ignore_ascii_case(short))
            })
        };
        for &member in hir.member_list(class.members) {
            let data = hir.member(member);
            if data.kind != phrost_hir::MemberKind::Method {
                continue;
            }
            let (Some(hint), Some(body)) = (data.type_ref, data.body) else {
                continue;
            };
            let phrost_hir::TypeRefData::Named(named) = hir.type_ref(hint) else {
                continue;
            };
            if !is_attribute(hir.name_ref(*named).short()) {
                continue;
            }
            let Some(getter) = Self::returned_getter(hir, body) else {
                continue;
            };
            let ty = crate::types::TypeLowerer {
                hir,
                namespace,
                interner,
                types,
            }
            .lower(getter);
            getters.push(StatedGetter {
                method: data.name.clone(),
                ty,
            });
        }
        getters
    }

    /// The return type of the getter closure a body's `return` hands over.
    fn returned_getter(hir: &FileHir, body: phrost_hir::BodyId) -> Option<phrost_hir::TypeRefId> {
        hir.stmt_list(hir.body(body).statements)
            .iter()
            .find_map(|&statement| {
                let StmtData::Return {
                    value: Some(value), ..
                } = hir.stmt(statement)
                else {
                    return None;
                };
                let (args, positional_is_getter) = match hir.expr(*value) {
                    ExprData::StaticCall {
                        name: Some(method),
                        args,
                        ..
                    } => (*args, !method.as_str().eq_ignore_ascii_case("set")),
                    ExprData::New { args, .. } => (*args, true),
                    _ => return None,
                };
                let arguments = hir.expr_list(args);
                let named = arguments
                    .iter()
                    .find_map(|&argument| match hir.expr(argument) {
                        ExprData::Argument {
                            name: Some(name),
                            value,
                            ..
                        } if name.as_str() == "get" => Some(*value),
                        _ => None,
                    });
                let getter = named.or_else(|| {
                    let &first = arguments.first()?;
                    let value = match hir.expr(first) {
                        ExprData::Argument {
                            name: None, value, ..
                        } => *value,
                        ExprData::Argument { .. } => return None,
                        _ => first,
                    };
                    positional_is_getter.then_some(value)
                })?;
                match hir.expr(getter) {
                    ExprData::Closure { return_type, .. } => *return_type,
                    _ => None,
                }
            })
    }

    /// A quoted key's range without its quotes.
    fn name_between_quotes(text: &str, range: TextRange) -> TextRange {
        match text
            .get(usize::from(range.start())..usize::from(range.end()))
            .map(str::as_bytes)
        {
            Some([open @ (b'\'' | b'"'), .., close]) if open == close => TextRange::new(
                range.start() + phrost_model::TextSize::from(1),
                range.end() - phrost_model::TextSize::from(1),
            ),
            _ => range,
        }
    }
}
