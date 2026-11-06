//! What a variable a Twig template reads holds, and what each attribute read off it is.
//!
//! `{{ data.supplier_name }}` in a template a `['#theme' => ..., '#data' => [...]]` renders is
//! `$data['supplier_name']` where that array is written, and the PHP engine already knows what
//! that holds. So a variable's value is read where it was handed over -- in the rendering file,
//! lowered beside the template -- and every step after it is Twig's own attribute rule applied with
//! the same engine: a key of an array, else a property of an object, else a method named for it, or
//! its `get`, `is` or `has` getter.
//!
//! The walk runs outside any class, which is where a template is: a member PHP would refuse there
//! is not one Twig can read either, and both answer nothing for it.
//!
//! # Where a variable comes from
//!
//! In the template's own order: a binding the template writes (`for`, `set`, `with`, a macro's
//! arguments, a declared type), then what each array or call rendering the template hands it, then
//! what every template has. A macro sees only its arguments.

use std::{cell::Cell, ops::Range, path::Path};

use phrost_hir::{FileHir, FileHirBuilder};
use phrost_index::RenderSite;
use phrost_model::{BuiltinTy, FileId, Name, TextRange, TextSize, Visibility};
use phrost_sema::MemberKind;
use phrost_twig::{BindingKind, Reading, StepKind, Value};

use crate::{
    location::SymbolLocation,
    resolve::{Held, Subscript},
    variables::Naming,
    view::{FOLLOW_LIMIT, IndexView},
};

/// What a name at one position in a template is.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TwigSymbol {
    /// The name's own bytes in the template.
    pub range: Range<usize>,
    pub kind: TwigSymbolKind,
    /// What it holds, rendered. `None` where nothing says.
    pub ty: Option<Box<str>>,
}

/// Which kind of name it is.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum TwigSymbolKind {
    /// A variable, and where its value comes from.
    Variable { name: Box<str>, origin: TwigOrigin },
    /// The member an attribute reads, and where it is declared.
    Member {
        location: Box<SymbolLocation>,
        kind: MemberKind,
    },
    /// A key of an array.
    Key { name: Box<str> },
    /// A filter, function or test, and what runs it.
    Called(Box<super::TwigCallable>),
}

/// Where a variable's value comes from.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum TwigOrigin {
    /// A tag in the template binds it, at this range.
    Bound(Range<usize>),
    /// What renders the template hands it over: the file, and where the key is written there.
    Rendered { file: FileId, range: TextRange },
    /// A template including this one hands it over: that template, and where in it -- the key of
    /// its `with`, or the binding of the variable it passes on whole.
    Included {
        template: std::path::PathBuf,
        range: Range<usize>,
    },
    /// Every template has it.
    Global,
    /// Nothing anyone can see binds it.
    Unbound,
}

/// One name worth offering where a template is being written.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TwigCandidate {
    pub name: Box<str>,
    /// What it holds, rendered, when anything says.
    pub detail: Option<Box<str>>,
    pub kind: TwigCandidateKind,
}

/// What a candidate is.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TwigCandidateKind {
    Variable,
    Property,
    Method,
    Key,
    Filter,
    Function,
}

impl IndexView<'_> {
    /// What the name at `offset` in the template at `path` is.
    pub fn twig_symbol_at(&self, path: &Path, text: &str, offset: usize) -> Option<TwigSymbol> {
        let typed = Typed::of(self, path, text);
        if let Some(called) = typed.reading.called_at(offset) {
            let kind = super::TwigCallableKind::of(called.kind);
            // `same as` is one test with two words, however they are spaced.
            let name = text[called.name.clone()]
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ");
            let found = self.twig_callable(kind, &name)?;
            return Some(TwigSymbol {
                range: called.name.clone(),
                ty: self
                    .twig_callable_returns(kind, &name)
                    .and_then(|held| typed.render(&held)),
                kind: TwigSymbolKind::Called(Box::new(found)),
            });
        }
        let (chain, step) = typed.reading.chain_at(offset)?;
        let chain_index = typed.index_of(chain);
        match step {
            None => {
                let name = &text[chain.root.clone()];
                let held = typed.root(name, chain.root.start);
                Some(TwigSymbol {
                    range: chain.root.clone(),
                    ty: held.and_then(|held| typed.render(&held)),
                    kind: TwigSymbolKind::Variable {
                        name: name.into(),
                        origin: typed.origin(name, chain.root.start),
                    },
                })
            }
            Some(at) => {
                let on_loop = typed.loop_root(chain_index, at);
                // `loop` is no PHP value: what is read off it is Twig's own and needs no receiver.
                let receiver = match on_loop {
                    true => Held::Builtin(BuiltinTy::Array),
                    false => typed.chain_held(chain_index, at)?,
                };
                let step = &chain.steps[at];
                let reached = typed.step(&receiver, step, on_loop)?;
                let ty = reached.held.as_ref().and_then(|held| typed.render(held));
                let kind = match reached.member {
                    Some((class, kind, name)) => TwigSymbolKind::Member {
                        location: Box::new(self.member_location(class.as_str(), kind, &name)?),
                        kind,
                    },
                    None => TwigSymbolKind::Key {
                        name: text[step.name.clone()].into(),
                    },
                };
                Some(TwigSymbol {
                    range: step.name.clone(),
                    kind,
                    ty,
                })
            }
        }
    }

    /// What could be written at `offset`, and what it would replace: a variable where one is being
    /// written, or what an attribute can read where one is.
    pub fn twig_completions(
        &self,
        path: &Path,
        text: &str,
        offset: usize,
        limit: usize,
    ) -> Option<(Range<usize>, Vec<TwigCandidate>)> {
        let typed = Typed::of(self, path, text);
        // A filter being written: every filter an extension declares.
        if let Some(called) = typed.reading.called_at(offset)
            && called.kind == phrost_twig::CalledKind::Filter
        {
            let mut items = self.twig_callable_names(super::TwigCallableKind::Filter);
            items.truncate(limit);
            return Some((called.name.clone(), items));
        }
        let (chain, step) = typed.reading.chain_at(offset)?;
        let chain_index = typed.index_of(chain);
        let mut items = match step {
            // Where an expression starts: a variable, or a function to call.
            None => {
                let mut items = typed.variables(offset);
                items.extend(self.twig_callable_names(super::TwigCallableKind::Function));
                items
            }
            Some(at) => {
                if typed.loop_root(chain_index, at) {
                    LOOP.iter()
                        .map(|(name, ty)| TwigCandidate {
                            name: (*name).into(),
                            detail: Some(ty.as_str().into()),
                            kind: TwigCandidateKind::Property,
                        })
                        .collect()
                } else {
                    let receiver = typed.chain_held(chain_index, at)?;
                    typed.attributes(&receiver)
                }
            }
        };
        let range = match step {
            None => chain.root.clone(),
            Some(at) => chain.steps[at].name.clone(),
        };
        items.truncate(limit);
        Some((range, items))
    }
}

/// What `loop` holds inside a `for`: Twig's own, the same in every template.
const LOOP: &[(&str, BuiltinTy)] = &[
    ("index", BuiltinTy::Int),
    ("index0", BuiltinTy::Int),
    ("revindex", BuiltinTy::Int),
    ("revindex0", BuiltinTy::Int),
    ("first", BuiltinTy::Bool),
    ("last", BuiltinTy::Bool),
    ("length", BuiltinTy::Int),
    ("parent", BuiltinTy::Array),
];

/// How deep one binding is followed through the bindings its value reads.
const DEPTH: u8 = 16;

/// How many templates up a variable is followed through the ones including each.
const LEVELS: u8 = 4;

/// One template, read, with what renders it.
struct Typed<'v, 'a> {
    view: &'v IndexView<'a>,
    path: &'v Path,
    text: &'v str,
    reading: Reading,
    /// How many includes up from the template asked about this one is.
    level: u8,
    /// The templates rendering this one with their variables, each with the places it does,
    /// read when first asked for.
    includers: std::cell::OnceCell<Vec<Includer<'v, 'a>>>,
    /// Every site rendering this template whose file is lowered beside it, with the prefix its keys
    /// carry.
    sites: Vec<(FileId, &'a RenderSite, Box<str>)>,
    /// A body nothing is written in: the walk is outside every class, as a template is.
    hir: FileHir,
    depth: Cell<u8>,
}

/// A template including the one asked about, and where it does.
struct Includer<'v, 'a> {
    typed: Typed<'v, 'a>,
    /// Indices into its `includes`.
    sites: Vec<usize>,
}

/// What a step reached: the value, and the member when an object's member was read.
struct Reached {
    held: Option<Held>,
    member: Option<(Name, MemberKind, String)>,
}

impl<'v, 'a> Typed<'v, 'a> {
    fn of(view: &'v IndexView<'a>, path: &'v Path, text: &'v str) -> Self {
        Self::at_level(view, path, text, 0)
    }

    fn at_level(view: &'v IndexView<'a>, path: &'v Path, text: &'v str, level: u8) -> Self {
        // Only what is lowered beside the template: a closed file keeps no body to read a value
        // out of.
        let sites = view
            .skeleton
            .render_arrays_of(path)
            .into_iter()
            .filter(|(file, ..)| view.open.contains(*file))
            .map(|(file, site, rule)| (file, site, rule.prefix.clone()))
            .collect();
        Self {
            view,
            path,
            text,
            reading: phrost_twig::read(text),
            level,
            includers: std::cell::OnceCell::new(),
            sites,
            hir: FileHirBuilder::new(FileId::from_raw(u32::MAX)).finish(),
            depth: Cell::new(0),
        }
    }

    fn index_of(&self, chain: &phrost_twig::Chain) -> usize {
        self.reading
            .chains
            .iter()
            .position(|known| std::ptr::eq(known, chain))
            .unwrap_or(0)
    }

    fn render(&self, held: &Held) -> Option<Box<str>> {
        self.view
            .render_held(TextSize::from(0), &self.hir, held, Naming::Short)
            .map(Into::into)
    }

    /// Whether the receiver of step `at` is `loop` as a `for` binds it, which no PHP type is.
    fn loop_root(&self, chain: usize, at: usize) -> bool {
        let chain = &self.reading.chains[chain];
        at == 0
            && self
                .reading
                .binding_of(&self.text[chain.root.clone()], chain.root.start)
                .is_some_and(|binding| binding.kind == BindingKind::Loop)
    }

    /// What variable `name` holds where it is read at `at`.
    fn root(&self, name: &str, at: usize) -> Option<Held> {
        if let Some(binding) = self.reading.binding_of(name, at) {
            return self.bound(&binding.kind);
        }
        if self.reading.is_isolated(at) {
            return None;
        }
        self.rendered(name)
            .or_else(|| self.included(name))
            .or_else(|| self.global(name))
    }

    /// The templates rendering this one with their variables, held beside it as text: each with
    /// the places in it that render this one.
    fn includers(&self) -> &[Includer<'v, 'a>] {
        self.includers.get_or_init(|| {
            if self.level >= LEVELS {
                return Vec::new();
            }
            let skeleton: &'a phrost_index::ProjectSkeleton = self.view.skeleton;
            let open: &'a phrost_index::OpenSet = self.view.open;
            skeleton
                .twig_includers_of(self.path)
                .into_iter()
                .filter_map(|path| {
                    let text = open.template_text(path)?;
                    let typed = Typed::at_level(self.view, path, text, self.level + 1);
                    let sites = phrost_twig::template_names(text)
                        .into_iter()
                        .filter(|name| skeleton.twig_path_of(&name.name) == Some(self.path))
                        .filter_map(|name| {
                            typed.reading.includes.iter().position(|include| {
                                include.range.start <= name.literal.start
                                    && name.literal.end <= include.range.end
                            })
                        })
                        .collect::<Vec<_>>();
                    (!sites.is_empty()).then_some(Includer { typed, sites })
                })
                .collect()
        })
    }

    /// What every template including this one hands it under `name`: the key of a `with`, or --
    /// unless the include says `only` -- the variable of that name where the include is.
    fn included(&self, name: &str) -> Option<Held> {
        let mut found: Vec<Held> = Vec::new();
        for includer in self.includers() {
            for &site in &includer.sites {
                let include = &includer.typed.reading.includes[site];
                let held = match handed(include, name) {
                    Some(value) => includer.typed.value(value),
                    None if !include.only => includer.typed.root(name, include.range.start),
                    None => None,
                };
                if let Some(held) = held
                    && !found.contains(&held)
                {
                    found.push(held);
                }
            }
        }
        match found.len() {
            0 => None,
            1 => found.pop(),
            _ => Some(IndexView::one_of(found)),
        }
    }

    /// Where a template including this one hands it `name`, as an origin.
    fn included_origin(&self, name: &str) -> Option<TwigOrigin> {
        for includer in self.includers() {
            for &site in &includer.sites {
                let include = &includer.typed.reading.includes[site];
                if handed(include, name).is_some() {
                    // The key as the hash writes it, quoted or bare, inside the tag.
                    let tag = &includer.typed.text[include.range.clone()];
                    let at = [
                        format!("'{name}'"),
                        format!("\"{name}\""),
                        format!("{name}:"),
                    ]
                    .iter()
                    .find_map(|spelled| {
                        tag.find(spelled.as_str()).map(|at| {
                            let start =
                                include.range.start + at + usize::from(!spelled.ends_with(':'));
                            start..start + name.len()
                        })
                    })
                    .unwrap_or_else(|| include.range.clone());
                    return Some(TwigOrigin::Included {
                        template: includer.typed.path.to_path_buf(),
                        range: at,
                    });
                }
                if include.only {
                    continue;
                }
                match includer.typed.origin(name, include.range.start) {
                    TwigOrigin::Bound(range) => {
                        return Some(TwigOrigin::Included {
                            template: includer.typed.path.to_path_buf(),
                            range,
                        });
                    }
                    TwigOrigin::Unbound | TwigOrigin::Global => {}
                    found => return Some(found),
                }
            }
        }
        None
    }

    /// Where variable `name` read at `at` gets its value.
    fn origin(&self, name: &str, at: usize) -> TwigOrigin {
        if let Some(binding) = self.reading.binding_of(name, at) {
            return TwigOrigin::Bound(binding.range.clone());
        }
        if self.reading.is_isolated(at) {
            return TwigOrigin::Unbound;
        }
        for (file, site, prefix) in &self.sites {
            let key = format!("{prefix}{name}");
            if site.keys.iter().any(|known| **known == *key) {
                let view = IndexView::in_file(self.view.skeleton, self.view.open, *file);
                if let Some(range) = view.rendered_key(site, &key) {
                    return TwigOrigin::Rendered { file: *file, range };
                }
            }
        }
        if let Some(found) = self.included_origin(name) {
            return found;
        }
        match self.global_type(name) {
            Some(_) => TwigOrigin::Global,
            None => TwigOrigin::Unbound,
        }
    }

    /// What every site rendering the template hands it under `name`, as one value.
    fn rendered(&self, name: &str) -> Option<Held> {
        let mut found: Vec<Held> = Vec::new();
        for (file, site, prefix) in &self.sites {
            let key = format!("{prefix}{name}");
            if !site.keys.iter().any(|known| **known == *key) {
                continue;
            }
            let view = IndexView::in_file(self.view.skeleton, self.view.open, *file);
            if let Some(held) = view.rendered_value(site, &key, FOLLOW_LIMIT)
                && !found.contains(&held)
            {
                found.push(held);
            }
        }
        match found.len() {
            0 => None,
            1 => found.pop(),
            _ => Some(IndexView::one_of(found)),
        }
    }

    fn global_type(&self, name: &str) -> Option<&'a str> {
        self.view
            .skeleton
            .returns()
            .twig_loaders()
            .iter()
            .flat_map(|loader| loader.globals.iter())
            .find(|(global, _)| **global == *name)
            .map(|(_, held)| &**held)
    }

    fn global(&self, name: &str) -> Option<Held> {
        self.written(self.global_type(name)?)
    }

    /// A type as a template or a framework writes it: a builtin, a class, or a list of one.
    ///
    /// Not a docblock type: a shape or a generic is left for the parser that reads those, and
    /// answers nothing here rather than something half-read.
    fn written(&self, written: &str) -> Option<Held> {
        let written = written.trim();
        if let Some(inner) = written.strip_prefix('?') {
            return Some(IndexView::nullable(self.written(inner)?));
        }
        if let Some(element) = written.strip_suffix("[]") {
            return Some(Held::ListOf(Box::new(self.written(element)?)));
        }
        if let Some(builtin) = BuiltinTy::from_name(written) {
            return Some(Held::Builtin(builtin));
        }
        if !written
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'\\')
        {
            return None;
        }
        let class = self.view.class_location(written.trim_start_matches('\\'))?;
        Some(Held::Class(class.fqn))
    }

    /// What a binding the template writes holds.
    fn bound(&self, kind: &BindingKind) -> Option<Held> {
        if self.depth.get() >= DEPTH {
            return None;
        }
        self.depth.set(self.depth.get() + 1);
        let held = match kind {
            BindingKind::Element(value) => {
                let held = self.value(value)?;
                self.view
                    .element_of_held(TextSize::from(0), &self.hir, &held)
            }
            BindingKind::Key(value) => {
                let held = self.value(value)?;
                self.view
                    .iteration_key_of_held(TextSize::from(0), &self.hir, &held)
            }
            BindingKind::Assigned(value) => self.value(value),
            BindingKind::Captured => Some(Held::Builtin(BuiltinTy::String)),
            BindingKind::Declared(written) => self.written(written),
            BindingKind::Loop
            | BindingKind::Parameter
            | BindingKind::Macros
            | BindingKind::Macro => None,
        };
        self.depth.set(self.depth.get() - 1);
        held
    }

    /// What an expression a binding is made from holds.
    fn value(&self, value: &Value) -> Option<Held> {
        match value {
            Value::Chain { chain, filters } => {
                let steps = self.reading.chains.get(*chain)?.steps.len();
                let mut held = self.chain_held(*chain, steps)?;
                for filter in filters {
                    held = self.filtered(held, filter)?;
                }
                Some(held)
            }
            Value::String => Some(Held::Builtin(BuiltinTy::String)),
            Value::Number => Some(Held::Builtin(BuiltinTy::Int)),
            Value::Bool => Some(Held::Builtin(BuiltinTy::Bool)),
            Value::Null => Some(Held::Builtin(BuiltinTy::Null)),
            Value::List(_) | Value::Hash(_) => Some(Held::Builtin(BuiltinTy::Array)),
            Value::Call(function) => self
                .view
                .twig_callable_returns(super::TwigCallableKind::Function, function),
            Value::Filtered { base, filters } => {
                let mut held = self.value(base)?;
                for filter in filters {
                    held = self.filtered(held, filter)?;
                }
                Some(held)
            }
            Value::Unknown => None,
        }
    }

    /// What a chain holds after its first `steps` steps.
    fn chain_held(&self, chain: usize, steps: usize) -> Option<Held> {
        let found = self.reading.chains.get(chain)?;
        let on_loop = steps > 0 && self.loop_root(chain, 0);
        let mut held = match on_loop {
            true => Held::Builtin(BuiltinTy::Array),
            false => self.root(&self.text[found.root.clone()], found.root.start)?,
        };
        for (at, step) in found.steps.iter().take(steps).enumerate() {
            held = self.step(&held, step, on_loop && at == 0)?.held?;
        }
        Some(held)
    }

    /// Reads one step off `held`, the way Twig's `getAttribute` does.
    fn step(&self, held: &Held, step: &phrost_twig::Step, on_loop: bool) -> Option<Reached> {
        let name = &self.text[step.name.clone()];
        if on_loop {
            let ty = LOOP.iter().find(|(known, _)| *known == name)?.1;
            return Some(Reached {
                held: Some(Held::Builtin(ty)),
                member: None,
            });
        }
        let zero = TextSize::from(0);
        let object = self.view.class_of_held(zero, &self.hir, held).is_some();
        match step.kind {
            // An array's key first, then an object's property, then a method named for it.
            StepKind::Attribute if !object => Some(Reached {
                held: self.view.indexed_of_held(
                    zero,
                    &self.hir,
                    held,
                    Some(name),
                    Subscript::Written,
                ),
                member: None,
            }),
            StepKind::Attribute => self.member(held, name, true),
            StepKind::Method => self.member(held, name, false),
            StepKind::Key => Some(Reached {
                held: self.view.indexed_of_held(
                    zero,
                    &self.hir,
                    held,
                    Some(name),
                    Subscript::Written,
                ),
                member: None,
            }),
            StepKind::Index => Some(Reached {
                held: self
                    .view
                    .indexed_of_held(zero, &self.hir, held, None, Subscript::Written),
                member: None,
            }),
        }
    }

    /// The member Twig reads for `name` on an object: the property when `property` allows one, then
    /// the method of that name, then its `get`, `is` and `has` getters -- PHP's names compared as PHP
    /// compares a method's, without case.
    fn member(&self, held: &Held, name: &str, property: bool) -> Option<Reached> {
        if name.is_empty() {
            return None;
        }
        let zero = TextSize::from(0);
        let capitalised = {
            let mut chars = name.chars();
            chars
                .next()
                .map(|first| first.to_uppercase().chain(chars).collect::<String>())
                .unwrap_or_default()
        };
        let mut candidates = Vec::new();
        if property {
            candidates.push((MemberKind::Property, name.to_owned()));
        }
        candidates.push((MemberKind::Method, name.to_owned()));
        for prefix in ["get", "is", "has"] {
            candidates.push((MemberKind::Method, format!("{prefix}{capitalised}")));
        }
        candidates.into_iter().find_map(|(kind, member)| {
            let class = self
                .view
                .owner_of_member(zero, &self.hir, held, kind, &member)?;
            // Which class would own the name is not whether it has it: the single class answers
            // for any name, so the declaration -- or a tag stating it -- is asked for itself.
            self.view.member_location(class.as_str(), kind, &member)?;
            // A member PHP refuses outside the class is one Twig cannot read either -- and one a
            // magic method answers instead says nothing about what it holds.
            if self
                .view
                .reach_of_member(zero, class.as_str(), kind, &member)
                != crate::references::MemberReach::Declared
            {
                return None;
            }
            let reached = self
                .view
                .member_through(zero, &self.hir, held, kind, &member, None);
            Some(Reached {
                held: reached,
                member: Some((class, kind, member)),
            })
        })
    }

    /// What a filter makes of a value, for the filters whose result is known whatever they are
    /// given: the same value, an element of it, its keys, or a scalar.
    ///
    /// Two kinds, asked in this order. The filters whose result is the value or a part of it are
    /// Twig's rule and not what their PHP says: `sort` is declared to return an `array` and keeps
    /// the elements it was given, which only the rule can say. Every other filter is what the PHP
    /// running it returns.
    fn filtered(&self, held: Held, filter: &str) -> Option<Held> {
        let zero = TextSize::from(0);
        Some(match filter {
            "default" | "filter" | "sort" | "reverse" | "slice" | "merge" | "raw" | "escape"
            | "e" | "spaceless" => held,
            "first" | "last" => self.view.element_of_held(zero, &self.hir, &held)?,
            "batch" => Held::ListOf(Box::new(held)),
            _ => self
                .view
                .twig_callable_returns(super::TwigCallableKind::Filter, filter)?,
        })
    }

    /// Every variable that can be read at `offset`, nearest first: what the template binds, what
    /// renders it hands over, and what every template has.
    fn variables(&self, offset: usize) -> Vec<TwigCandidate> {
        let mut found: Vec<TwigCandidate> = Vec::new();
        let add = |found: &mut Vec<TwigCandidate>, name: &str, held: Option<Held>| {
            if name.is_empty() || found.iter().any(|known| &*known.name == name) {
                return;
            }
            found.push(TwigCandidate {
                name: name.into(),
                detail: held.and_then(|held| self.render(&held)),
                kind: TwigCandidateKind::Variable,
            });
        };
        for binding in self.reading.visible_at(offset) {
            add(&mut found, &binding.name, self.bound(&binding.kind));
        }
        if self.reading.is_isolated(offset) {
            return found;
        }
        for (_, site, prefix) in &self.sites {
            for key in &site.keys {
                if let Some(name) = key.strip_prefix(&**prefix) {
                    let held = self.rendered(name);
                    add(&mut found, name, held);
                }
            }
        }
        for includer in self.includers() {
            for &site in &includer.sites {
                let include = &includer.typed.reading.includes[site];
                if let Some(phrost_twig::Value::Hash(entries)) = &include.with {
                    for (name, value) in entries {
                        add(&mut found, name, includer.typed.value(value));
                    }
                }
                if !include.only {
                    for candidate in includer.typed.variables(include.range.start) {
                        if !found.iter().any(|known| known.name == candidate.name) {
                            found.push(candidate);
                        }
                    }
                }
            }
        }
        for loader in self.view.skeleton.returns().twig_loaders() {
            for (name, _) in &loader.globals {
                add(&mut found, name, self.global(name));
            }
        }
        found
    }

    /// What an attribute can read off `held`: an array's keys, or an object's public properties and
    /// methods -- a getter by the name Twig reads it under, `title` for `getTitle()`.
    fn attributes(&self, held: &Held) -> Vec<TwigCandidate> {
        let zero = TextSize::from(0);
        let Some(class) = self.view.class_of_held(zero, &self.hir, held) else {
            return self
                .view
                .keys_of_held(zero, &self.hir, held)
                .into_iter()
                .map(|key| TwigCandidate {
                    name: key.name.as_str().into(),
                    detail: key.ty,
                    kind: TwigCandidateKind::Key,
                })
                .collect();
        };
        let mut found: Vec<TwigCandidate> = Vec::new();
        for member in self
            .view
            .member_candidates(class.as_str(), Visibility::Public)
        {
            if member.is_static || member.name.as_str().starts_with("__") {
                continue;
            }
            let (name, kind) = match member.kind {
                MemberKind::Property => {
                    (member.name.as_str().to_owned(), TwigCandidateKind::Property)
                }
                MemberKind::Method => match getter(member.name.as_str()) {
                    Some(read) => (read, TwigCandidateKind::Property),
                    None => (member.name.as_str().to_owned(), TwigCandidateKind::Method),
                },
                _ => continue,
            };
            if found.iter().any(|known| *known.name == *name) {
                continue;
            }
            found.push(TwigCandidate {
                name: name.into(),
                detail: member.ty,
                kind,
            });
        }
        found
    }
}

/// What an include hands over under `name` besides the includer's own variables: the value of that
/// key of its `with` hash.
fn handed<'r>(include: &'r phrost_twig::Include, name: &str) -> Option<&'r Value> {
    match include.with.as_ref()? {
        Value::Hash(entries) => entries
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value),
        _ => None,
    }
}

/// The name Twig reads a getter under: `title` for `getTitle`, `active` for `isActive`.
fn getter(method: &str) -> Option<String> {
    ["get", "is", "has"].iter().find_map(|prefix| {
        let rest = method.strip_prefix(prefix)?;
        let mut chars = rest.chars();
        let first = chars.next()?;
        first
            .is_ascii_uppercase()
            .then(|| first.to_ascii_lowercase().to_string() + chars.as_str())
    })
}
