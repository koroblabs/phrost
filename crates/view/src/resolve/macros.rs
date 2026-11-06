//! A method a class was given at runtime by a registration in a body: Laravel's
//! `Collection::macro('sortByCollator', fn ...)`. See the index's `macros` for what is kept and the
//! seam's `MacroRegistrar` for which classes take them.

use phrost_model::{FileId, Name};
use phrost_sema::MemberKind;

use crate::{
    location::SymbolLocation,
    resolve::{HIERARCHY_DEPTH, Held, RelativeScope},
    variables::Naming,
    view::IndexView,
};

impl IndexView<'_> {
    /// The registration answering `name` on `class_fqn`: one made on the class or a class above it,
    /// where that class takes registrations at all.
    ///
    /// Asked only where no declaration answered, which on a class nothing registers on ends at
    /// the first question: the index holds the files that register one, and nothing else.
    fn macro_for(
        &self,
        class_fqn: &str,
        name: &str,
    ) -> Option<(FileId, phrost_index::macros::MacroSite)> {
        let mut providers = self.skeleton.returns().macro_providers().peekable();
        providers.peek()?;
        let providers = providers.collect::<Vec<_>>();
        self.skeleton
            .macros_named(name)
            .find(|(_, site)| {
                let registered = site.class.as_str();
                let below = class_fqn.eq_ignore_ascii_case(registered)
                    || self.descends_from(class_fqn, registered, HIERARCHY_DEPTH);
                below
                    && providers.iter().any(|provider| {
                        registered.eq_ignore_ascii_case(provider)
                            || self.descends_from(registered, provider, HIERARCHY_DEPTH)
                    })
            })
            .map(|(file, site)| (file, site.clone()))
    }

    /// What calling the macro `name` on `class_fqn` hands back: what its closure declares, with
    /// `static` the class it was called on, or the chain its one `return` builds from `$this` or
    /// from a class.
    pub(crate) fn macro_held(
        &self,
        class_fqn: &str,
        kind: MemberKind,
        name: &str,
        called_on: Option<&Name>,
    ) -> Option<Held> {
        if kind != MemberKind::Method {
            return None;
        }
        let (_, site) = self.macro_for(class_fqn, name)?;
        let called = match called_on {
            Some(called) => called.clone(),
            None => self.class_location(class_fqn)?.fqn,
        };
        if let Some(ty) = site.returns {
            return Some(Held::Stored {
                ty,
                scope: Some(RelativeScope {
                    called: called.clone(),
                    declaring: called,
                }),
            });
        }
        let chain = site.chain.as_ref()?;
        let mut class = chain.root.clone().unwrap_or(called);
        self.walk_returned_guarded(&mut class, &chain.steps)
    }

    /// What a list of members shows beside the macro: the return its closure writes, or the class
    /// its returned chain comes to.
    pub(crate) fn macro_rendered(&self, class_fqn: &str, name: &str) -> Option<String> {
        let (_, site) = self.macro_for(class_fqn, name)?;
        if let Some(ty) = site.returns {
            return Some(self.skeleton.types().render(ty));
        }
        match self.macro_held(class_fqn, MemberKind::Method, name, None)? {
            Held::Class(class) => Some(Naming::Short.write(class.as_str())),
            Held::Stored { ty, scope } => {
                Some(self.render_stored(ty, scope.as_ref(), Naming::Short))
            }
            _ => None,
        }
    }

    /// Every macro `class_fqn` answers, by name -- for a list of its members. The first registration of
    /// a name wins, as `macro_for` reads it.
    pub(crate) fn macros_answered_by(&self, class_fqn: &str) -> Vec<Name> {
        let mut names: Vec<Name> = Vec::new();
        for site in self.skeleton.macros() {
            if names
                .iter()
                .any(|known| known.as_str().eq_ignore_ascii_case(site.name.as_str()))
            {
                continue;
            }
            if self.macro_for(class_fqn, site.name.as_str()).is_some() {
                names.push(site.name.clone());
            }
        }
        names
    }

    /// Where the macro `name` on `class_fqn` was registered: the name the registration writes.
    pub(crate) fn macro_location(
        &self,
        class_fqn: &str,
        kind: MemberKind,
        name: &str,
    ) -> Option<SymbolLocation> {
        if kind != MemberKind::Method {
            return None;
        }
        let (file, site) = self.macro_for(class_fqn, name)?;
        Some(SymbolLocation {
            file,
            fqn: site.name.clone(),
            kind: phrost_model::SymbolKind::Method,
            name_range: site.at,
            full_range: site.at,
            container: Some(site.class.clone()),
            // A declaration's signature, which a card puts after `public function`: the parameters
            // are the closure's and are not kept, the return is when the closure writes one.
            signature: Some(
                match site.returns {
                    Some(ty) => format!("{}(): {}", site.name, self.skeleton.types().render(ty)),
                    None => format!("{}()", site.name),
                }
                .into(),
            ),
            member_modifiers: Some(crate::location::MemberModifiers {
                visibility: phrost_model::Visibility::Public,
                set_visibility: None,
                is_static: false,
                is_abstract: false,
                is_final: false,
                is_readonly: false,
            }),
            from_docblock: false,
            origin: Some(format!("macro on {}", site.class).into()),
            docblock_range: None,
            deprecated: false,
        })
    }
}
