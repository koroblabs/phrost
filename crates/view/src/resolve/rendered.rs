//! A template's variables, as the calls that render it hand them over.
//!
//! Nothing in a template binds `$post`; `view('posts.show', ['post' => $post])` does, in a
//! controller's body. The index records where each such call is (see `phrost_index::RenderSite`),
//! and whatever analyses the template has the rendering files lowered beside it -- so the value is
//! read where it was written, with that file's imports and that method's locals, and carried back.
//! Several calls rendering one template hand a variable whatever each of them does, which is the
//! union of their answers.

use phrost_hir::{
    ExprId, FileHir, TypeRefData, TypeRefId,
    body::{ArrayElementKind, ExprData},
};
use phrost_model::{BuiltinTy, FileId, TextRange, TextSize};

use crate::{resolve::Held, view::IndexView};

/// How many includes up a template's variables are followed: the levels of renderers lowered beside
/// a template, and no further, since nothing past them is lowered to read.
const INCLUDED_LEVELS: usize = 4;

impl IndexView<'_> {
    /// Whether this view's file is a template some framework reads as the PHP it holds.
    pub fn is_template(&self) -> bool {
        self.current_file()
            .and_then(|file| self.skeleton.file(file))
            .is_some_and(|file| self.skeleton.is_template(file.path()))
    }

    /// Whether this view's file is a template whose dialect wrote the call to `function` -- a
    /// component tag's, an include's -- which no file declares and nobody wrote.
    pub fn template_writes(&self, function: &str) -> bool {
        self.current_file()
            .and_then(|file| self.skeleton.file(file))
            .is_some_and(|file| {
                self.skeleton
                    .template_writes(file.path(), function.trim_start_matches('\\'))
            })
    }

    /// What in this template's structure the PHP made of it cannot carry, read from `text` -- the
    /// template as written, which this view holds only as that PHP. Nothing for a file that is no
    /// template.
    pub fn template_problems(&self, text: &str) -> Vec<(TextRange, String)> {
        let Some(file) = self
            .current_file()
            .and_then(|file| self.skeleton.file(file))
        else {
            return Vec::new();
        };
        self.skeleton
            .template_problems(file.path(), text)
            .into_iter()
            .filter_map(|(at, problem)| {
                let start = TextSize::try_from(at.start).ok()?;
                let end = TextSize::try_from(at.end).ok()?;
                Some((TextRange::new(start, end), problem))
            })
            .collect()
    }

    /// What a variable a template uses and nothing in it binds holds: the union of what every call
    /// rendering the template hands it under that name. Nothing for a variable no call names, or
    /// where no rendering file is lowered beside the template.
    pub(crate) fn template_variable(&self, name: &str, budget: u8) -> Option<Held> {
        if self.skeleton.returns().rendered_templates().is_empty() {
            return None;
        }
        if let Some(known) = self.template_variables.borrow().get(name) {
            return known.clone();
        }
        let found = self
            .template_property(name, true)
            .or_else(|| self.rendered_to(name, budget))
            .or_else(|| self.shared_variable(name))
            .or_else(|| self.template_property(name, false));
        self.template_variables
            .borrow_mut()
            .insert(name.into(), found.clone());
        found
    }

    fn rendered_to(&self, name: &str, budget: u8) -> Option<Held> {
        let mut found: Vec<Held> = Vec::new();
        self.each_renderer(|view, site| {
            let held = if site.keys.iter().any(|key| **key == *name) {
                view.rendered_value(site, name, budget)
            } else if site.inherits {
                // Not in the array, and the include hands over everything where it is: what the
                // including template holds under that name there -- bound in it, or else handed to
                // it a level up.
                match view.bound_at(site.data, name) {
                    Some(_) => view.local_value(site.data, name, budget),
                    None => return true,
                }
            } else {
                return false;
            };
            if let Some(held) = held
                && !found.contains(&held)
            {
                found.push(held);
            }
            false
        });
        match found.len() {
            0 => None,
            1 => found.pop(),
            _ => Some(Self::one_of(found)),
        }
    }

    /// Each call rendering this template, with a view of the file it is written in, nearest first
    /// -- and, past an include that hands over every variable where it is, the calls rendering the
    /// template including it, wherever `visit` answers that what it asked is still to be found a
    /// level up. Only files lowered beside the template are read, and each is climbed past once.
    fn each_renderer(
        &self,
        mut visit: impl FnMut(&IndexView<'_>, &phrost_index::RenderSite) -> bool,
    ) {
        let Some(file) = self.current_file() else {
            return;
        };
        let mut climbed = vec![file];
        let mut asking = vec![file];
        for _ in 0..INCLUDED_LEVELS {
            let mut next = Vec::new();
            for template in asking {
                let Some(name) = self
                    .skeleton
                    .file(template)
                    .and_then(|entry| self.skeleton.template_name(entry.path()))
                else {
                    continue;
                };
                for (renderer, site) in self.skeleton.renderers_of(&name) {
                    if renderer == file || !self.open.contains(renderer) {
                        continue;
                    }
                    let view = IndexView::in_file(self.skeleton, self.open, renderer);
                    if visit(&view, site) && site.inherits && !climbed.contains(&renderer) {
                        climbed.push(renderer);
                        next.push(renderer);
                    }
                }
            }
            if next.is_empty() {
                break;
            }
            asking = next;
        }
    }

    /// Where something in this view's file binds `name`, as seen from `at`.
    ///
    /// Not a call the name is passed to, which binds it only where nothing else does: a template
    /// handing its own `$chapter` to `userCan()` is reading what it was handed, not writing it.
    fn bound_at(&self, at: TextSize, name: &str) -> Option<TextRange> {
        let open = self.current()?;
        match phrost_body::local_declaration(open.hir(), at, name)? {
            (_, phrost_body::BindingKind::Argument) => None,
            (range, _) => Some(range),
        }
    }

    /// The local `name` where `at` is in this view's file, spelled so that any file can read it.
    fn local_value(&self, at: TextSize, name: &str, budget: u8) -> Option<Held> {
        let hir = self.current()?.hir();
        let held = self.resolve_local_held(at, name, budget.saturating_sub(1))?;
        self.portable(at, hir, held)
    }

    /// Every variable a call rendering this template names, and the ones every template has --
    /// what a template can read that nothing in it binds. Empty for a file that is no template.
    pub(crate) fn template_variable_names(&self) -> Vec<Box<str>> {
        if self.skeleton.returns().rendered_templates().is_empty() {
            return Vec::new();
        }
        let Some(path) = self
            .current_file()
            .and_then(|file| self.skeleton.file(file))
            .map(|file| file.path())
        else {
            return Vec::new();
        };
        if !self.skeleton.is_template(path) {
            return Vec::new();
        }
        let mut names: Vec<Box<str>> = Vec::new();
        self.each_renderer(|view, site| {
            for key in &site.keys {
                if !names.contains(key) {
                    names.push(key.clone());
                }
            }
            if !site.inherits {
                return false;
            }
            // And every variable where an include is: what the including template binds there,
            // and what it was handed.
            if let Some(open) = view.current() {
                for local in phrost_body::locals_in_scope(open.hir(), site.data) {
                    if !names.iter().any(|known| **known == *local.as_str()) {
                        names.push(local.as_str().into());
                    }
                }
            }
            true
        });
        for name in self.skeleton.template_shared_names(path) {
            if !names.iter().any(|known| **known == *name) {
                names.push(name.into());
            }
        }
        for name in self.template_property_names() {
            if !names.contains(&name) {
                names.push(name);
            }
        }
        names
    }

    /// A variable every template has, whatever renders it: Laravel's `$errors`.
    fn shared_variable(&self, name: &str) -> Option<Held> {
        let file = self.current_file()?;
        let class = self
            .skeleton
            .template_shared(self.skeleton.file(file)?.path(), name)?;
        Some(Held::Class(self.class_location(class)?.fqn))
    }

    /// Where a call rendering this template names the variable `name`: the key, in the nearest such
    /// call whose file is lowered beside the template -- or, past an include, where the including
    /// template binds it.
    pub(crate) fn template_variable_named_at(&self, name: &str) -> Option<(FileId, TextRange)> {
        if self.skeleton.returns().rendered_templates().is_empty() {
            return None;
        }
        // A property the class rendering the template hands it is that property, whatever a tag
        // passed it.
        if let Some(property) = self.template_property_at(name) {
            return Some((property.file, property.name_range));
        }
        let mut found = None;
        self.each_renderer(|view, site| {
            if found.is_some() {
                return false;
            }
            let Some(renderer) = view.current_file() else {
                return false;
            };
            if site.keys.iter().any(|key| **key == *name) {
                found = view.rendered_key(site, name).map(|range| (renderer, range));
                return false;
            }
            if !site.inherits {
                return false;
            }
            match view.bound_at(site.data, name) {
                Some(range) => {
                    found = Some((renderer, range));
                    false
                }
                None => true,
            }
        });
        found
    }

    /// The key and the value the array of variables beginning at `data` holds under `name`, in this
    /// view's file.
    fn rendered_element(&self, data: TextSize, name: &str) -> Option<(TextRange, ExprId)> {
        let hir = self.current()?.hir();
        let elements = hir.expressions().find_map(|(_, expr)| match expr {
            ExprData::ArrayLiteral { elements, range } if range.start() == data => Some(*elements),
            _ => None,
        })?;
        hir.expr_list(elements)
            .iter()
            .find_map(|&element| match hir.expr(element) {
                ExprData::ArrayElement {
                    kind: ArrayElementKind::LiteralKey { name: key, range },
                    value,
                    ..
                } if key.as_str() == name => Some((*range, *value)),
                _ => None,
            })
    }

    /// The last whole write of key `name` on the local the assignment at `data` builds a render
    /// array on, in the same body: where the key is written, and the value.
    ///
    /// The last, because that is what the array holds when it is handed over; a write into the
    /// value's own keys after it (`$build['#order']['x'] = ...`) is not read, and leaves the value
    /// what the whole write said.
    fn written_element(&self, data: TextSize, name: &str) -> Option<(TextRange, ExprId)> {
        let hir = self.current()?.hir();
        let local = hir.expressions().find_map(|(_, expr)| match expr {
            ExprData::Assign { target, range, .. } if range.start() == data => {
                phrost_index::written_key(hir, *target).map(|(local, _)| local)
            }
            _ => None,
        })?;
        let body = phrost_index::enclosing_body(hir, data)?;
        hir.expressions()
            .filter_map(|(_, expr)| match expr {
                ExprData::Assign {
                    target,
                    value,
                    range,
                    ..
                } if phrost_index::enclosing_body(hir, range.start()) == Some(body)
                    && phrost_index::written_key(hir, *target) == Some((local, name)) =>
                {
                    let ExprData::Index { index, .. } = hir.expr(*target) else {
                        return None;
                    };
                    let key = hir.expr((*index)?).range();
                    Some((range.start(), key, *value))
                }
                _ => None,
            })
            .max_by_key(|(at, ..)| *at)
            .map(|(_, key, value)| (key, value))
    }

    /// Where the call names `name` in this view's file: the array's key, or the string `compact()`
    /// was handed.
    pub(crate) fn rendered_key(
        &self,
        site: &phrost_index::RenderSite,
        name: &str,
    ) -> Option<TextRange> {
        let hir = self.current()?.hir();
        match site.kind {
            phrost_index::RenderKind::Array => Some(self.rendered_element(site.data, name)?.0),
            phrost_index::RenderKind::Writes => Some(self.written_element(site.data, name)?.0),
            phrost_index::RenderKind::Named => Some(Self::named_argument(hir, site.data, name)?.0),
            // The string naming the local, or the variable's name in a call sharing one.
            phrost_index::RenderKind::Compact | phrost_index::RenderKind::Shared => {
                let text = self.current()?.text();
                let args = Self::arguments_at(hir, site.data)?;
                let names = match site.kind {
                    phrost_index::RenderKind::Shared => &args[..args.len().min(1)],
                    _ => &args[..],
                };
                names.iter().find_map(|&argument| {
                    let range = hir.expr(argument).range();
                    let written =
                        text.get(usize::from(range.start()) + 1..usize::from(range.end()) - 1)?;
                    (written == name).then_some(range)
                })
            }
        }
    }

    /// The argument named `name` in the call beginning at `at` -- a component tag as a template
    /// reader writes one -- as where its name is written and its value.
    fn named_argument(hir: &FileHir, at: TextSize, name: &str) -> Option<(TextRange, ExprId)> {
        Self::arguments_at(hir, at)?
            .into_iter()
            .find_map(|argument| match hir.expr(argument) {
                ExprData::Argument {
                    name: Some(written),
                    value,
                    range,
                    ..
                } if written.as_str() == name => Some((
                    TextRange::at(range.start(), TextSize::of(written.as_str())),
                    *value,
                )),
                _ => None,
            })
    }

    /// The arguments of the call beginning at `at`: `compact()`, or a call sharing a
    /// variable with every template.
    ///
    /// The outermost of the calls starting there: `view()->share('locale', $locale)` begins where
    /// the `view()` it is made on begins, and that one hands over nothing.
    fn arguments_at(hir: &FileHir, at: TextSize) -> Option<Vec<ExprId>> {
        hir.expressions()
            .filter_map(|(_, expr)| match expr {
                ExprData::FunctionCall { args, range, .. }
                | ExprData::StaticCall { args, range, .. }
                | ExprData::MethodCall { args, range, .. }
                    if range.start() == at =>
                {
                    Some((range.len(), *args))
                }
                _ => None,
            })
            .max_by_key(|(length, _)| *length)
            .map(|(_, args)| hir.expr_list(args).to_vec())
    }

    /// What the call hands the template under `name`, in this view's file, spelled so that any
    /// file can read it: the entry's value, or for `compact()` the local of that name where the
    /// call is.
    pub(crate) fn rendered_value(
        &self,
        site: &phrost_index::RenderSite,
        name: &str,
        budget: u8,
    ) -> Option<Held> {
        let hir = self.current()?.hir();
        let value = match site.kind {
            phrost_index::RenderKind::Compact => return self.local_value(site.data, name, budget),
            phrost_index::RenderKind::Shared => *Self::arguments_at(hir, site.data)?.get(1)?,
            phrost_index::RenderKind::Array => self.rendered_element(site.data, name)?.1,
            phrost_index::RenderKind::Writes => self.written_element(site.data, name)?.1,
            phrost_index::RenderKind::Named => Self::named_argument(hir, site.data, name)?.1,
        };
        let at = hir.expr(value).range().start();
        let path = phrost_body::value_path_of(hir, value)?;
        let held = self.resolve_path(at, hir, &path, budget.saturating_sub(1))?;
        self.portable(at, hir, held)
    }

    /// `held` with every type written in this file's arena spelled as one any file can read: a
    /// written type is an index into the file that wrote it, and means nothing in a template.
    fn portable(&self, at: TextSize, hir: &FileHir, held: Held) -> Option<Held> {
        let each = |operands: Box<[Held]>| -> Option<Box<[Held]>> {
            operands
                .into_vec()
                .into_iter()
                .map(|operand| self.portable(at, hir, operand))
                .collect()
        };
        Some(match held {
            Held::Written(id) => self.written_portable(at, hir, id)?,
            Held::Nullable(inner) => Held::Nullable(Box::new(self.portable(at, hir, *inner)?)),
            Held::OneOf(operands) => Held::OneOf(each(operands)?),
            Held::AllOf(operands) => Held::AllOf(each(operands)?),
            Held::ListOf(inner) => Held::ListOf(Box::new(self.portable(at, hir, *inner)?)),
            Held::MapOf { key, value } => Held::MapOf {
                key: Box::new(self.portable(at, hir, *key)?),
                value: Box::new(self.portable(at, hir, *value)?),
            },
            Held::Record(fields) => Held::Record(
                fields
                    .into_vec()
                    .into_iter()
                    .map(|(key, value)| Some((key, self.portable(at, hir, value)?)))
                    .collect::<Option<_>>()?,
            ),
            Held::Applied { class, arguments } => Held::Applied {
                class,
                arguments: each(arguments)?,
            },
            // What a closure's signature says is written in this file; a template calling it gets
            // the one fact that survives the trip.
            Held::Callable(_) => Held::Builtin(BuiltinTy::Callable),
            other => other,
        })
    }

    fn written_portable(&self, at: TextSize, hir: &FileHir, id: TypeRefId) -> Option<Held> {
        match hir.type_ref(id) {
            TypeRefData::Builtin(builtin) => Some(Held::Builtin(*builtin)),
            TypeRefData::Named(name) => {
                let written = hir.name_ref(*name);
                let class = self.resolve_written_name(at, written.path.as_str(), written.anchor)?;
                Some(Held::Class(class.fqn))
            }
            TypeRefData::Nullable(inner) => Some(Held::Nullable(Box::new(
                self.written_portable(at, hir, *inner)?,
            ))),
            TypeRefData::Union(operands) => Some(Self::one_of(
                hir.type_ref_list(*operands)
                    .iter()
                    .map(|&operand| self.written_portable(at, hir, operand))
                    .collect::<Option<Vec<_>>>()?,
            )),
            // What a container holds is the half a template iterates, so it is carried rather than
            // reduced to the class it is not: `@param list<Node> $nodes` handed over as `nodes` is a
            // list of nodes in the template too, and the reduction below answered nothing for it.
            TypeRefData::Generic { base, args, .. } => {
                let args = hir
                    .type_ref_list(*args)
                    .iter()
                    .map(|&arg| self.written_portable(at, hir, arg))
                    .collect::<Option<Vec<_>>>()?;
                match hir.type_ref(*base) {
                    TypeRefData::Builtin(_) => match args.as_slice() {
                        [element] => Some(Held::ListOf(Box::new(element.clone()))),
                        [key, value] => Some(Held::MapOf {
                            key: Box::new(key.clone()),
                            value: Box::new(value.clone()),
                        }),
                        _ => None,
                    },
                    _ => {
                        let Held::Class(class) = self.written_portable(at, hir, *base)? else {
                            return None;
                        };
                        Some(Held::Applied {
                            class,
                            arguments: args.into_boxed_slice(),
                        })
                    }
                }
            }
            // A shape's keys are names the file already interned, so it travels as a record.
            TypeRefData::Shape { fields, .. } => Some(Held::Record(
                hir.type_ref_list(*fields)
                    .iter()
                    .map(|&field| match hir.type_ref(field) {
                        TypeRefData::ShapeField { key, ty, .. } => {
                            Some((key.clone(), self.written_portable(at, hir, *ty)?))
                        }
                        _ => None,
                    })
                    .collect::<Option<Box<[_]>>>()?,
            )),
            _ => self
                .class_of_held(at, hir, &Held::Written(id))
                .map(Held::Class),
        }
    }
}
