//! What `$this` is inside a closure the callee binds before calling it.
//!
//! PHP binds a closure written in a method to that method's object, and `Closure::bind` can hand it
//! another. A callee that does the second says so with `@param-closure-this`: Laravel's
//! `Macroable::macro` is `@param-closure-this static $macro`, and every manager's `extend()` is
//! `$this`. So a macro written in a service provider's `boot()` runs as the collection, the
//! request or the builder it was registered on -- and reading it as the provider, which is where it
//! is written, reported every call it makes on its real `$this` as undefined.
//!
//! And the bind written in place, which no declaration can describe because the object is an
//! argument: `Closure::bind(function () { $this->initializeBundles(); }, $kernel, $kernel::class)`
//! is how Symfony's debug commands reach into the kernel, and read as the command it reported the
//! kernel's methods as the command's missing ones.

use phrost_model::{Name, TextSize};

use crate::view::{FOLLOW_LIMIT, IndexView};

impl IndexView<'_> {
    /// The class `$this` is at `offset`, when a closure around it is passed to a callee that binds
    /// it.
    ///
    /// `None` when no callee said anything, which leaves the question to the enclosing class.
    /// `Some(None)` when one did and its answer does not resolve: the enclosing class is then known
    /// to be wrong, and nothing is the honest answer.
    pub(crate) fn closure_this(&self, offset: TextSize) -> Option<Option<Name>> {
        let hir = self.current()?.hir();
        phrost_body::closure_this_at(hir, offset)
            .iter()
            .find_map(|source| match source {
                phrost_body::ClosureThis::Passed(call) => {
                    let declared = self.declaration_called(offset, hir, call, FOLLOW_LIMIT)?;
                    let (ty, called_on) = declared.closure_this(call)?;
                    Some(
                        self.bound_as_this(ty, called_on.cloned())
                            .map(|class| self.configured_this(class)),
                    )
                }
                phrost_body::ClosureThis::Bound { at, value } => {
                    Some(value.as_ref().and_then(|value| {
                        let held = self.resolve_path(*at, hir, value, FOLLOW_LIMIT)?;
                        self.class_of_held(*at, hir, &held)
                    }))
                }
            })
    }

    /// The class a call configuring this file names in place of `declared`, which a framework says
    /// its callees declare and do not bind: Pest's `uses(Tests\TestCase::class)` for the
    /// `TestCase` `test()` names. The first class the calls name below `declared`.
    fn configured_this(&self, declared: Name) -> Name {
        if !self
            .skeleton
            .returns()
            .configured_this()
            .iter()
            .any(|rule| rule.declared.eq_ignore_ascii_case(declared.as_str()))
        {
            return declared;
        }
        self.closure_configuration()
            .iter()
            .find(|class| {
                self.descends_from(
                    class.as_str(),
                    declared.as_str(),
                    crate::resolve::HIERARCHY_DEPTH,
                )
            })
            .cloned()
            .unwrap_or(declared)
    }

    /// The traits the calls configuring this file name beside `class`, when `class` is the one
    /// they configured: the test case Pest generates uses them, so `$this->faker` is `WithFaker`'s
    /// -- and, the trait's body being copied into it, reachable however it is declared.
    pub(crate) fn configured_traits(&self, class: &Name) -> Vec<Name> {
        let configured = self.closure_configuration();
        if !configured.contains(class) {
            return Vec::new();
        }
        configured
            .iter()
            .filter(|named| self.is_trait(named.as_str()))
            .cloned()
            .collect()
    }

    /// The classes the calls configuring this file name, as declared.
    fn closure_configuration(&self) -> &[Name] {
        self.configured_this.get_or_init(|| {
            if self.skeleton.returns().configured_this().is_empty() {
                return Vec::new();
            }
            let Some(file) = self.current_file() else {
                return Vec::new();
            };
            self.skeleton
                .configured_this_for(file)
                .into_iter()
                .filter_map(|class| Some(self.class_location(class.as_str())?.fqn))
                .collect()
        })
    }

    /// The class a `@param-closure-this` type names, with `$this` and `static` meaning the class
    /// the call was made on.
    fn bound_as_this(&self, ty: phrost_ty::TypeId, called_on: Option<Name>) -> Option<Name> {
        match self.skeleton.types().get(ty) {
            phrost_ty::TypeRef::Relative(_) => called_on,
            _ => self.class_of_stored(ty, None),
        }
    }
}
