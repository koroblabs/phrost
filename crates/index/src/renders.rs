//! Where the project renders a template by name, and the variables it hands it.
//!
//! `view('posts.show', ['post' => $post])` is the only statement anywhere of what `$post` is inside
//! `posts/show.blade.php` -- the template declares nothing, and the call is the declaration. So the
//! call is recorded where it is written: which template it names, where its array of variables
//! begins, and which names that array holds. What each variable *is* is not recorded: it is an
//! expression in the rendering body, which a closed file does not keep, so it is read where a
//! template is analysed, with the rendering file lowered beside it.
//!
//! Recorded from a project's own code only, for the reason [`crate::dispatch`] gives: bodies are
//! lowered for nothing else.

use phrost_defmap::{DeclarationSource, FileScope};
use phrost_hir::{
    FileHir, ImportKind,
    body::{ArrayElementKind, ClassTarget, ExprData, LiteralKind},
};
use phrost_model::{FileId, Name, TextSize};

use crate::{ProjectSkeleton, returns::ReturnContracts};

/// One call rendering a template by name.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct RenderSite {
    /// The template's name as the call wrote it: `posts.show`.
    pub template: Box<str>,
    /// Where the array of variables begins, in the rendering file -- or the `compact()` call.
    pub data: TextSize,
    /// The variables the array names, as written.
    pub keys: Box<[Box<str>]>,
    /// How the variables were handed over, which says where each one's value is.
    pub kind: RenderKind,
    /// The class whose body the call is written in, fully qualified: a framework may make its
    /// instance the template's `$this`.
    pub class: Option<Name>,
    /// Whether the template also has every variable where the call is, whatever the array says:
    /// Blade's `@include`, which hands over `get_defined_vars()`.
    pub inherits: bool,
}

/// How a call hands a template its variables.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RenderKind {
    /// An array literal beginning at the site's `data`: each value is an entry's.
    Array,
    /// `compact('post', ...)` beginning at `data`: each is the local of that name where it is.
    Compact,
    /// A call beginning at `data` handing every template one variable: the value is its second
    /// argument. The site's template is [`SHARED`].
    Shared,
    /// A call beginning at `data` whose named arguments are the variables: a component tag, as a
    /// template reader writes one.
    Named,
    /// Element writes on the local an assignment beginning at `data` writes the template's name
    /// into: `$build['#theme'] = 'x'; $build['#data'] = ...;`, each key's value the last whole write
    /// of it in the same body.
    Writes,
}

/// An array that renders the template it names under `key`, handing it what is under every key
/// beginning with `prefix` but the array's `own`. A framework's rule, restated in this crate's terms.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct RenderArrayRule {
    pub key: Box<str>,
    /// The catalog the template's name is a key of.
    pub catalog: Box<str>,
    pub prefix: Box<str>,
    pub own: Box<[Box<str>]>,
    /// How a template's file name says the name it renders by: the suffix taken off, and the
    /// character the file writes for the one the name has.
    pub file_name: Option<(Box<str>, char, char)>,
}

impl RenderArrayRule {
    /// The name a template file at `path` renders by under this rule's file-name convention.
    pub fn named_by_file(&self, path: &std::path::Path) -> Option<String> {
        let (suffix, written, named) = self.file_name.as_ref()?;
        let stem = path.file_name()?.to_str()?.strip_suffix(&**suffix)?;
        Some(stem.replace(*written, &named.to_string()))
    }
}

/// The name a site handing every template its variables is filed under.
pub const SHARED: &str = "*";

impl ProjectSkeleton {
    /// Every call in `hir` that renders a template a framework's rule names, with a literal name
    /// and a literal array of variables -- or with none, inside a class, which may be what the
    /// template reads, or where the call hands over every variable where it is.
    pub(crate) fn collect_renders(
        returns: &ReturnContracts,
        scope: &FileScope,
        text: &str,
        hir: &FileHir,
    ) -> Box<[RenderSite]> {
        let rules = returns.rendered_templates();
        if rules.is_empty() && returns.render_arrays().is_empty() {
            return Box::default();
        }
        let mut found = Vec::new();
        for (_, expr) in hir.expressions() {
            if let Some(site) = Self::render_array_site(returns, scope, text, hir, expr) {
                found.push(site);
                continue;
            }
            if let Some(site) = Self::render_writes_site(returns, scope, text, hir, expr) {
                found.push(site);
                continue;
            }
            if let Some(site) = Self::shared_site(returns, scope, text, hir, expr) {
                found.push(site);
                continue;
            }
            if let Some(site) = Self::component_site(returns, text, hir, expr) {
                found.push(site);
                continue;
            }
            let ExprData::FunctionCall { name, args, .. } = expr else {
                continue;
            };
            let written = hir.name_ref(*name).path.as_str();
            let Some(&(_, name_at, data_at, inherits)) = rules
                .iter()
                .find(|(function, ..)| function.eq_ignore_ascii_case(written))
            else {
                continue;
            };
            let args = hir.expr_list(*args);
            let Some(template) =
                args.get(usize::from(name_at))
                    .and_then(|&argument| match hir.expr(argument) {
                        ExprData::Literal {
                            kind: LiteralKind::String,
                            range,
                        } => crate::dispatch::unquoted(text, *range),
                        _ => None,
                    })
            else {
                continue;
            };
            let class = Self::enclosing_class(scope, hir, expr.range().start());
            let (range, keys, kind) =
                match args.get(usize::from(data_at)).map(|&data| hir.expr(data)) {
                    Some(ExprData::ArrayLiteral { elements, range }) => (
                        *range,
                        hir.expr_list(*elements)
                            .iter()
                            .filter_map(|&element| match hir.expr(element) {
                                ExprData::ArrayElement {
                                    kind: ArrayElementKind::LiteralKey { name, .. },
                                    ..
                                } => Some(name.as_str().into()),
                                _ => None,
                            })
                            .collect::<Box<[Box<str>]>>(),
                        RenderKind::Array,
                    ),
                    // `compact('post', 'comments')`: the names of the locals it gathers.
                    Some(ExprData::FunctionCall {
                        name, args, range, ..
                    }) if hir
                        .name_ref(*name)
                        .path
                        .as_str()
                        .eq_ignore_ascii_case("compact") =>
                    {
                        (
                            *range,
                            hir.expr_list(*args)
                                .iter()
                                .filter_map(|&argument| match hir.expr(argument) {
                                    ExprData::Literal {
                                        kind: LiteralKind::String,
                                        range,
                                    } => crate::dispatch::unquoted(text, *range).map(Box::from),
                                    _ => None,
                                })
                                .collect::<Box<[Box<str>]>>(),
                            RenderKind::Compact,
                        )
                    }
                    Some(_) => continue,
                    None => (expr.range(), Box::default(), RenderKind::Array),
                };
            if keys.is_empty() && class.is_none() && !inherits {
                continue;
            }
            found.push(RenderSite {
                template: template.into(),
                data: range.start(),
                keys,
                kind,
                class,
                inherits,
            });
        }
        found.into_boxed_slice()
    }

    /// An array literal naming the template it renders under a rule's key: Drupal's
    /// `['#theme' => 'item_list', '#items' => $items]`.
    ///
    /// The keys are kept as written, prefix and all, because that is what the array holds them
    /// under and what the value is looked up by; which variable each is, is the reader's question.
    fn render_array_site(
        returns: &ReturnContracts,
        scope: &FileScope,
        text: &str,
        hir: &FileHir,
        expr: &ExprData,
    ) -> Option<RenderSite> {
        let rules = returns.render_arrays();
        if rules.is_empty() {
            return None;
        }
        let ExprData::ArrayLiteral { elements, range } = expr else {
            return None;
        };
        let elements = hir.expr_list(*elements);
        rules.iter().find_map(|rule| {
            let template = elements
                .iter()
                .find_map(|&element| match hir.expr(element) {
                    ExprData::ArrayElement {
                        kind: ArrayElementKind::LiteralKey { name, .. },
                        value,
                        ..
                    } if name.as_str() == &*rule.key => match hir.expr(*value) {
                        ExprData::Literal {
                            kind: LiteralKind::String,
                            range,
                        } => crate::dispatch::unquoted(text, *range),
                        _ => None,
                    },
                    _ => None,
                })?;
            let keys = elements
                .iter()
                .filter_map(|&element| match hir.expr(element) {
                    ExprData::ArrayElement {
                        kind: ArrayElementKind::LiteralKey { name, .. },
                        ..
                    } if name.as_str().starts_with(&*rule.prefix)
                        && !rule.own.iter().any(|own| **own == *name.as_str()) =>
                    {
                        Some(name.as_str().into())
                    }
                    _ => None,
                })
                .collect::<Box<[Box<str>]>>();
            (!keys.is_empty()).then(|| RenderSite {
                template: template.into(),
                data: range.start(),
                keys,
                kind: RenderKind::Array,
                class: Self::enclosing_class(scope, hir, range.start()),
                inherits: false,
            })
        })
    }

    /// A render array built one key at a time on a local: `$build['#theme'] = 'x';` and the
    /// `$build['#...'] = ...` writes beside it in the same body. Drupal writes it this way as often
    /// as it writes the literal -- 20 of one project's 86 render arrays, and the ones its generated
    /// documents are rendered by.
    fn render_writes_site(
        returns: &ReturnContracts,
        scope: &FileScope,
        text: &str,
        hir: &FileHir,
        expr: &ExprData,
    ) -> Option<RenderSite> {
        let rules = returns.render_arrays();
        if rules.is_empty() {
            return None;
        }
        let ExprData::Assign {
            target,
            value,
            range,
            ..
        } = expr
        else {
            return None;
        };
        let (local, key) = written_key(hir, *target)?;
        let rule = rules.iter().find(|rule| *rule.key == *key)?;
        let template = match hir.expr(*value) {
            ExprData::Literal {
                kind: LiteralKind::String,
                range,
            } => crate::dispatch::unquoted(text, *range)?,
            _ => return None,
        };
        let body = enclosing_body(hir, range.start())?;
        let mut keys: Vec<Box<str>> = Vec::new();
        for (_, other) in hir.expressions() {
            let ExprData::Assign { target, range, .. } = other else {
                continue;
            };
            // In this body and not in a closure inside it, which has a local of its own.
            if !body.contains_range(*range) || enclosing_body(hir, range.start()) != Some(body) {
                continue;
            }
            let Some((written, key)) = written_key(hir, *target) else {
                continue;
            };
            if written == local
                && key.starts_with(&*rule.prefix)
                && !rule.own.iter().any(|own| **own == *key)
                && !keys.iter().any(|known| **known == *key)
            {
                keys.push(key.into());
            }
        }
        (!keys.is_empty()).then(|| RenderSite {
            template: template.into(),
            data: range.start(),
            keys: keys.into_boxed_slice(),
            kind: RenderKind::Writes,
            class: Self::enclosing_class(scope, hir, range.start()),
            inherits: false,
        })
    }

    /// The class whose body `offset` is in, innermost first.
    fn enclosing_class(scope: &FileScope, hir: &FileHir, offset: TextSize) -> Option<Name> {
        hir.classes()
            .filter(|(_, class)| class.body_range.contains_inclusive(offset))
            .min_by_key(|(_, class)| class.body_range.len())
            .and_then(|(id, _)| scope.symbol_of(DeclarationSource::Class(id)))
            .map(|symbol| symbol.fqn.clone())
    }

    /// A component tag as a template reader writes it: `x(\alert, post: $post)`, rendering
    /// `components.alert` with `$post`. The name is read as written, since it parses as constants.
    fn component_site(
        returns: &ReturnContracts,
        text: &str,
        hir: &FileHir,
        expr: &ExprData,
    ) -> Option<RenderSite> {
        let calls = returns.component_calls();
        if calls.is_empty() {
            return None;
        }
        let ExprData::FunctionCall {
            name, args, range, ..
        } = expr
        else {
            return None;
        };
        let written = hir.name_ref(*name).path.as_str();
        let (_, prefix) = calls
            .iter()
            .find(|(function, _)| function.eq_ignore_ascii_case(written))?;
        let args = hir.expr_list(*args);
        let named = hir.expr(*args.first()?).range();
        // One fully qualified name, the separators respelled so that a keyword reads as part of it:
        // `\forms\text_input` is `forms.text-input`.
        let component = text
            .get(usize::from(named.start())..usize::from(named.end()))?
            .trim()
            .trim_start_matches('\\')
            .replace('\\', ".")
            .replace('_', "-");
        let keys = args
            .iter()
            .filter_map(|&argument| match hir.expr(argument) {
                ExprData::Argument {
                    name: Some(name), ..
                } => Some(Box::<str>::from(name.as_str())),
                _ => None,
            })
            .collect::<Box<[Box<str>]>>();
        Some(RenderSite {
            template: format!("{prefix}{component}").into(),
            data: range.start(),
            keys,
            kind: RenderKind::Named,
            class: None,
            inherits: false,
        })
    }

    /// A static call handing every template a variable: `View::share('dates', $formatter)`, or the
    /// array form `View::share(['dates' => $formatter])`.
    fn shared_site(
        returns: &ReturnContracts,
        scope: &FileScope,
        text: &str,
        hir: &FileHir,
        expr: &ExprData,
    ) -> Option<RenderSite> {
        let rules = returns.shared_template_data();
        if rules.is_empty() {
            return None;
        }
        let (method, args, range) = match expr {
            ExprData::StaticCall {
                name: Some(method),
                args,
                range,
                ..
            }
            | ExprData::MethodCall {
                name: Some(method),
                args,
                range,
                ..
            } => (method, args, range),
            _ => return None,
        };
        let called =
            |rule: &&crate::SharedTemplateRule| rule.method.eq_ignore_ascii_case(method.as_str());
        if !rules.iter().any(|rule| called(&rule)) {
            return None;
        }
        let matched = match expr {
            ExprData::StaticCall {
                class: ClassTarget::Named(class),
                ..
            } => {
                let written = hir.name_ref(*class);
                let resolved = scope.scope_at(written.range.start()).resolve(
                    written.path.as_str(),
                    written.anchor,
                    ImportKind::ClassLike,
                );
                let fqn = resolved.primary().fqn();
                rules
                    .iter()
                    .filter(called)
                    .any(|rule| rule.class.eq_ignore_ascii_case(fqn))
            }
            // `view()->share(...)`: the helper, called with nothing, is the class's instance. Read by
            // the name written, as every rendering function here is.
            ExprData::MethodCall { receiver, .. } => match hir.expr(*receiver) {
                ExprData::FunctionCall {
                    name,
                    args: helper_args,
                    ..
                } if hir.expr_list(*helper_args).is_empty() => {
                    let written = hir.name_ref(*name).path.as_str().trim_start_matches('\\');
                    rules.iter().filter(called).any(|rule| {
                        rule.helper
                            .as_deref()
                            .is_some_and(|helper| helper.eq_ignore_ascii_case(written))
                    })
                }
                _ => false,
            },
            _ => false,
        };
        if !matched {
            return None;
        }
        let args = hir.expr_list(*args);
        match hir.expr(*args.first()?) {
            ExprData::Literal {
                kind: LiteralKind::String,
                range: key,
            } if args.len() >= 2 => Some(RenderSite {
                template: SHARED.into(),
                data: range.start(),
                keys: Box::new([crate::dispatch::unquoted(text, *key)?.into()]),
                kind: RenderKind::Shared,
                class: None,
                inherits: false,
            }),
            ExprData::ArrayLiteral { elements, range } => Some(RenderSite {
                template: SHARED.into(),
                data: range.start(),
                keys: hir
                    .expr_list(*elements)
                    .iter()
                    .filter_map(|&element| match hir.expr(element) {
                        ExprData::ArrayElement {
                            kind: ArrayElementKind::LiteralKey { name, .. },
                            ..
                        } => Some(name.as_str().into()),
                        _ => None,
                    })
                    .collect(),
                kind: RenderKind::Array,
                class: None,
                inherits: false,
            }),
            _ => None,
        }
    }

    /// The calls in `file` that render a template.
    pub fn renders_of(&self, file: FileId) -> &[RenderSite] {
        self.renders.get(&file).map_or(&[], |sites| sites)
    }

    /// Every call that renders the template `name` -- or hands every template its variables -- with
    /// the file it is written in, in file order, so that two runs read the renderers in the same
    /// order.
    pub fn renderers_of(&self, name: &str) -> Vec<(FileId, &RenderSite)> {
        // A component's directory may hold it as `index`, or under its own name again:
        // `<x-card>` is `components/card/index.blade.php` or `components/card/card.blade.php`.
        let short = name.strip_suffix(".index").or_else(|| {
            let (parent, last) = name.rsplit_once('.')?;
            parent.ends_with(&format!(".{last}")).then_some(parent)
        });
        let mut found: Vec<(FileId, &RenderSite)> = self
            .renders
            .iter()
            .flat_map(|(file, sites)| {
                sites
                    .iter()
                    .filter(move |site| {
                        same_template(&site.template, name)
                            || short.is_some_and(|short| same_template(&site.template, short))
                            || *site.template == *SHARED
                    })
                    .map(move |site| (*file, site))
            })
            .collect();
        found.sort_by_key(|(file, site)| (file.as_raw(), site.data));
        found
    }
}

impl ProjectSkeleton {
    /// Every array rendering the template at `path`, with the file it is written in and the rule
    /// it was read by: the names the template is rendered by are the keys of the rule's catalog
    /// whose place is that file.
    ///
    /// One answer for the two who ask -- what to lower beside an open template, and what its
    /// variables are -- so they cannot come to disagree about which files render it.
    pub fn render_arrays_of(
        &self,
        path: &std::path::Path,
    ) -> Vec<(FileId, &RenderSite, &RenderArrayRule)> {
        let mut found = Vec::new();
        let mut seen = vec![path.to_path_buf()];
        self.render_arrays_into(path, &mut found);
        // And what renders a page that includes this template by a name it computes, as the project
        // said -- that page's variables are this template's. A few levels, each page once.
        let mut asking = vec![path.to_path_buf()];
        for _ in 0..CONTEXT_LEVELS {
            let mut next = Vec::new();
            for template in &asking {
                for page in self.twig_context_pages(template) {
                    if seen.contains(&page) {
                        continue;
                    }
                    self.render_arrays_into(&page, &mut found);
                    seen.push(page.clone());
                    next.push(page);
                }
            }
            if next.is_empty() {
                break;
            }
            asking = next;
        }
        found
    }

    /// The pages whose variables the template at `path` sees, by the project's statements: each
    /// whose `templates` matches a name the template is loaded by.
    fn twig_context_pages(&self, path: &std::path::Path) -> Vec<std::path::PathBuf> {
        let returns = self.returns();
        let contexts = returns.twig_contexts();
        if contexts.is_empty() {
            return Vec::new();
        }
        let mut names: Vec<&str> = Vec::new();
        for loader in returns.twig_loaders() {
            names.extend(returns.catalog_keys_placed_at(&loader.templates, path));
            names.extend(returns.catalog_keys_placed_at(&loader.aliases, path));
        }
        contexts
            .iter()
            .filter(|(templates, _)| names.iter().any(|name| glob_matches(templates, name)))
            .filter_map(|(_, from)| {
                returns.twig_loaders().iter().find_map(|loader| {
                    returns
                        .catalog_placed_at(&loader.templates, from)
                        .or_else(|| returns.catalog_placed_at(&loader.aliases, from))
                        .map(|place| place.path.to_path_buf())
                })
            })
            .collect()
    }

    fn render_arrays_into<'s>(
        &'s self,
        path: &std::path::Path,
        found: &mut Vec<(FileId, &'s RenderSite, &'s RenderArrayRule)>,
    ) {
        let returns = self.returns();
        for rule in returns.render_arrays() {
            let mut names: Vec<String> = returns
                .catalog_keys_placed_at(&rule.catalog, path)
                .into_iter()
                .map(str::to_owned)
                .collect();
            if let Some(named) = rule.named_by_file(path)
                && !names.contains(&named)
            {
                names.push(named);
            }
            for name in &names {
                for (file, site) in self.renderers_of(name) {
                    if *site.template != *crate::renders::SHARED
                        && !found
                            .iter()
                            .any(|(known, seen, _): &(FileId, &RenderSite, _)| {
                                *known == file && std::ptr::eq(*seen, site)
                            })
                    {
                        found.push((file, site, rule));
                    }
                }
            }
        }
    }
}

/// How many pages up a template's variables are followed through the project's statements.
const CONTEXT_LEVELS: usize = 4;

/// Whether a template name matches a pattern: `*` is any part of one directory's name, `**` any
/// number of whole directories and what follows them, and every other character itself.
pub(crate) fn glob_matches(pattern: &str, name: &str) -> bool {
    let (pattern, name) = (pattern.as_bytes(), name.as_bytes());
    fn matches(pattern: &[u8], name: &[u8]) -> bool {
        match pattern {
            [] => name.is_empty(),
            [b'*', b'*', rest @ ..] => {
                let rest = rest.strip_prefix(b"/").unwrap_or(rest);
                (0..=name.len()).any(|at| matches(rest, &name[at..]))
            }
            [b'*', rest @ ..] => (0..=name.len())
                .take_while(|&at| at == 0 || name[at - 1] != b'/')
                .any(|at| matches(rest, &name[at..])),
            [first, rest @ ..] => name.first() == Some(first) && matches(rest, &name[1..]),
        }
    }
    matches(pattern, name)
}

/// The local and the literal key an element write writes: `build` and `#theme` in
/// `$build['#theme'] = ...`.
pub fn written_key(hir: &FileHir, target: phrost_hir::ExprId) -> Option<(&str, &str)> {
    let ExprData::Index {
        base,
        key: Some(key),
        ..
    } = hir.expr(target)
    else {
        return None;
    };
    let ExprData::Variable { name, .. } = hir.expr(*base) else {
        return None;
    };
    Some((name.as_str(), key.as_str()))
}

/// The innermost body holding `offset`: a function's, a method's or a closure's.
pub fn enclosing_body(hir: &FileHir, offset: TextSize) -> Option<phrost_model::TextRange> {
    hir.bodies()
        .map(|(_, body)| body.range)
        .filter(|range| range.contains_inclusive(offset))
        .min_by_key(|range| range.len())
}

/// Whether two template names are one, an underscore and a hyphen counting as the same: a component
/// tag's name reaches the index with its hyphens respelled, and a file may be named either way.
fn same_template(left: &str, right: &str) -> bool {
    left.len() == right.len()
        && left.bytes().zip(right.bytes()).all(|(left, right)| {
            left == right || (matches!(left, b'-' | b'_') && matches!(right, b'-' | b'_'))
        })
}

#[cfg(test)]
mod tests {
    use super::glob_matches;

    /// `*` stays inside one directory and `**` crosses any number, so `sections/*` is the sections
    /// themselves and `sections/**` is their parts too.
    #[test]
    fn a_template_pattern_matches_by_directory() {
        let cases = [
            ("@mine/sections/*", "@mine/sections/a.html.twig", true),
            (
                "@mine/sections/*",
                "@mine/sections/parts/b.html.twig",
                false,
            ),
            (
                "@mine/sections/**",
                "@mine/sections/parts/b.html.twig",
                true,
            ),
            ("@mine/sections/**", "@mine/sections/a.html.twig", true),
            (
                "@mine/**/b.html.twig",
                "@mine/sections/parts/b.html.twig",
                true,
            ),
            (
                "@mine/sections/*.html.twig",
                "@mine/sections/a.html.twig",
                true,
            ),
            ("@mine/sections/**", "@mine/page.html.twig", false),
            ("@mine/page.html.twig", "@mine/page.html.twig", true),
        ];
        let wrong: Vec<_> = cases
            .iter()
            .filter(|(pattern, name, expected)| glob_matches(pattern, name) != *expected)
            .collect();
        assert!(wrong.is_empty(), "{wrong:?}");
    }
}
