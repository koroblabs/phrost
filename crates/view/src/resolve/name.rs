//! A name, written or implied, and the symbol it stands for.
//!
//! PHP's three symbol tables and the file's imports on one side; `self`, `static`, `parent` and
//! `$this` on the other. They are one concern because they are one question asked two ways -- the
//! source either wrote the name or the language supplied it -- and because both are answered from
//! the file's own scope with nothing inferred. What a *type* names is [`super::class_of`]'s
//! question, and what a `new`/`::` target names is [`super::target`]'s.

use phrost_defmap::DeclarationSource;
use phrost_hir::{ImportKind, NameAnchor};
use phrost_model::{Name, TextSize};

use crate::{location::SymbolLocation, resolve::Held, view::IndexView};

impl IndexView<'_> {
    /// Resolves a written class-like name against the current file's namespace and imports.
    ///
    /// The candidates come from the def-map, which knows nothing about what exists; this tries them
    /// in order against the catalog. A name that resolves to nothing returns `None` — not a guess.
    pub fn resolve_written_name(
        &self,
        offset: TextSize,
        path: &str,
        anchor: NameAnchor,
    ) -> Option<SymbolLocation> {
        self.resolve_name(offset, path, anchor, ImportKind::ClassLike)
    }

    /// The same, in whichever of PHP's symbol tables the name belongs to.
    ///
    /// The tables are separate in the language, so they are separate here: a function call and a
    /// class of the same name are different symbols, and resolving one against the other's table
    /// finds a declaration that has nothing to do with what was written.
    pub fn resolve_name(
        &self,
        offset: TextSize,
        path: &str,
        anchor: NameAnchor,
        kind: ImportKind,
    ) -> Option<SymbolLocation> {
        let open = self.current()?;
        let resolution = open.scope().scope_at(offset).resolve(path, anchor, kind);

        resolution.candidates().find_map(|candidate| match kind {
            ImportKind::ClassLike => self.class_location(candidate.fqn()),
            ImportKind::Function => self.function_location(candidate.fqn()),
            ImportKind::Constant => self.constant_location(candidate.fqn()),
        })
    }

    /// The class the offset sits inside, if any.
    ///
    /// Known lexically, with no inference: `$this`, `self`, and `static` all mean the enclosing
    /// class, which is the one receiver whose type is free. The innermost class wins, for a class
    /// declared inside another's method body.
    pub fn enclosing_class(&self, offset: TextSize) -> Option<Name> {
        let open = self.current()?;
        let hir = open.hir();

        hir.classes()
            // The body and not the declaration: an anonymous class writes its constructor
            // arguments between the `class` keyword and the braces, and PHP evaluates them where
            // the `new` is -- see `ClassData::body_range`.
            .filter(|(_, class)| class.body_range.contains_inclusive(offset))
            .min_by_key(|(_, class)| class.body_range.len())
            .and_then(|(id, _)| {
                // The declared symbol, never the name as written: an anonymous class's written
                // name is synthesised and unqualified, and answering with it names a class the
                // catalog has never heard of -- which is a member access that silently resolves to
                // nothing rather than to the class the body is in.
                open.scope()
                    .symbol_of(DeclarationSource::Class(id))
                    .map(|symbol| symbol.fqn.clone())
            })
            // A template has no class around it, and a framework may still run it inside one:
            // Livewire binds its component's template to the component, `$this` and scope both.
            .or_else(|| self.template_class())
    }

    /// Whether `$this` is bound at `offset`: inside a class, and not inside a static method of it.
    ///
    /// Asked by completion, which offers the name. Every other reader of `$this` starts from one
    /// somebody already wrote and answers [`Self::enclosing_class`], so this is the only place the
    /// static case has to be told apart -- and it has to be, because a Drupal service class
    /// declares `create()` static and offering `$this` there suggests a line that fatals.
    ///
    /// The innermost member containing the offset, so a class declared inside a static method's
    /// body answers about its own methods. A closure written `static function` is not modelled:
    /// it does not bind `$this` either, and the flag is not on the body.
    pub fn binds_this(&self, offset: TextSize) -> bool {
        let Some(open) = self.current() else {
            return false;
        };
        let hir = open.hir();
        let Some((_, class)) = hir
            .classes()
            .filter(|(_, class)| class.body_range.contains_inclusive(offset))
            .min_by_key(|(_, class)| class.body_range.len())
        else {
            return self.template_class().is_some();
        };

        hir.member_list(class.members)
            .iter()
            .map(|id| hir.member(*id))
            .filter(|member| member.full_range.contains_inclusive(offset))
            .min_by_key(|member| member.full_range.len())
            // Outside every member -- a property default, or the class body itself -- there is no
            // `$this` to speak of either, so the answer is the same as a static method's.
            .is_some_and(|member| !member.modifiers.is_static())
    }

    /// The class the enclosing class extends, for a `parent` receiver.
    pub fn parent_class(&self, offset: TextSize) -> Option<Name> {
        self.parent_of_enclosing_class(offset)
    }

    /// The class a name written at `offset` resolves to.
    ///
    /// The anchor comes from the name itself rather than being assumed relative, because the callers
    /// hand over text a person wrote and PHP has three spellings of it -- see [`NameAnchor::of`],
    /// which is where assuming one cost every `\Drupal::` completion in a Drupal project.
    pub fn written_class(&self, offset: TextSize, name: &str) -> Option<Name> {
        let (anchor, path) = NameAnchor::of(name);
        self.resolve_written_name(offset, path, anchor)
            .map(|location| location.fqn)
    }

    /// What `$this` holds at `offset`: what a guard proved, else the class a binding callee names,
    /// else the class the code is written in -- or, inside a trait, what the trait's users are.
    ///
    /// One method because every reader asks it -- the path layer for a receiver, hover for the
    /// variable itself, and the text-scanned chain completion reads -- and copies of "unless a
    /// guard proved otherwise" would drift.
    ///
    /// A trait is half a class body, and `@require-implements`, `@require-extends` and `@mixin` are
    /// how it names the other half: psl's date-time traits call `$this->getMonth()` on the
    /// `DateTimeInterface` they require. An intersection rather than members merged into the trait's
    /// table, because every class composing a trait copies that table -- a mixin merged there took
    /// completion p95 from 0.39 ms to 0.72 (see `merge_mixins`) -- and here it costs one lookup per
    /// `$this` in a trait that writes such a tag, and nothing anywhere else.
    ///
    /// A trait that writes none is read through the classes the project composes it into, which is
    /// the whole of what `$this` can be. One host is the answer whole: its table holds everything
    /// the trait put there. Several are a union -- intersected with the trait, so a member the trait
    /// declares is still found once, on the trait. Read through the union alone, every call to one
    /// of the trait's own methods was a call on several receivers, which the argument checks rightly
    /// decline to judge, and `whereNull($key = null)` passing its null on to
    /// `whereStrict(string $key)` went unreported. A tag wins where there is one, because it states
    /// for every user what the hosts the index holds only show for some.
    pub(crate) fn this_held(&self, offset: TextSize) -> Option<Held> {
        // The guard's answer whole: an `instanceof` an interface leaves an intersection, which
        // names no single class, and asked for one it fell back to the class the code is written
        // in -- dropping the very member the guard was written to reach.
        if let Some(narrowed) = self.narrowed_this_held(offset) {
            return Some(narrowed);
        }
        let class = match self.closure_this(offset) {
            // A callee that binds the closure decides, and one whose answer does not resolve still
            // decided: the class the closure is written in is not what runs.
            Some(bound) => {
                let bound = bound?;
                let traits = self.configured_traits(&bound);
                if !traits.is_empty() {
                    let mut all = vec![Held::Class(bound)];
                    all.extend(traits.into_iter().map(Held::Class));
                    return Some(Self::all_of(all));
                }
                bound
            }
            None => self.enclosing_class(offset)?,
        };
        let users = self.what_a_trait_user_is(class.as_str());
        if users.is_empty() {
            let hosts = self.trait_hosts(class.as_str());
            return Some(match &*hosts {
                [] => Held::Class(class),
                [host] => Held::Class(host.clone()),
                several => Self::all_of(vec![
                    Held::Class(class),
                    Self::one_of(several.iter().cloned().map(Held::Class).collect()),
                ]),
            });
        }
        let mut all = Vec::with_capacity(users.len() + 1);
        all.push(Held::Class(class));
        all.extend(users.into_iter().map(Held::Class));
        Some(Self::all_of(all))
    }

    /// Only the guard's answer, for a caller that needs to know whether there was one.
    pub(crate) fn narrowed_this(&self, offset: TextSize) -> Option<Name> {
        let hir = self.current()?.hir();
        let held = self.narrowed_this_held(offset)?;
        self.class_of_held(offset, hir, &held)
    }

    /// What a guard proves `$this` holds at `offset`, before it is asked for a class.
    fn narrowed_this_held(&self, offset: TextSize) -> Option<Held> {
        let hir = self.current()?.hir();
        let narrowed = phrost_body::narrowed_this(
            hir,
            offset,
            &|hir, expr| self.call_never_returns(offset, hir, expr),
            &self.asserting(),
        )?;
        self.resolve_path(offset, hir, &narrowed, crate::view::FOLLOW_LIMIT)
    }

    pub(crate) fn parent_of_enclosing_class(&self, offset: TextSize) -> Option<Name> {
        self.enclosing_class(offset)
            .and_then(|class| self.class_header(class.as_str()))
            .and_then(|header| header.extends.first().cloned())
    }
}
