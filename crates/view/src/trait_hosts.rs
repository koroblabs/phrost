use std::sync::Arc;

use phrost_model::Name;
use rustc_hash::FxHashSet;

use crate::IndexView;

/// The most classes a trait may be composed into for `$this` inside it to be read as them.
///
/// Every `$this` in the trait's bodies then resolves against each of them, so a trait every value
/// object composes -- Laravel's `Macroable` and `Conditionable` are in forty classes each -- would
/// cost forty member tables per access to say what the trait's own table already says: members
/// those classes share by composing *it*. Past the bound `$this` stays the trait alone, which is
/// what it was before hosts were read at all.
const MOST_HOSTS: usize = 16;

impl IndexView<'_> {
    /// The classes the index composes `trait_fqn` into -- directly, or through another trait that
    /// composes it -- which is everything `$this` inside the trait can be.
    ///
    /// PHP never runs a trait's method on the trait: the body is copied into each class that uses
    /// it, and `$this` is an instance of one of those. Read as the trait alone, `$this->parent` in
    /// Laravel's `InteractsWithPivotTable` reached nothing, though its one host, `BelongsToMany`,
    /// declares it two classes up.
    ///
    /// Empty for anything that is not a trait, for a trait nothing composes, and for one composed
    /// into more than [`MOST_HOSTS`] classes. A host class's own subclasses are not listed: each is
    /// one of its host, which is what a union over hosts already says.
    ///
    /// Read off the skeleton's map from a trait to the files composing it, and memoized for the
    /// life of the view, because a body asks about `$this` once per access.
    pub(crate) fn trait_hosts(&self, trait_fqn: &str) -> Arc<[Name]> {
        let key: Box<str> = trait_fqn.to_ascii_lowercase().into();
        if let Some(known) = self.trait_hosts.borrow().get(&key) {
            return Arc::clone(known);
        }
        let hosts: Arc<[Name]> = match self
            .class_declaration(trait_fqn)
            .is_some_and(|class| class.kind() == phrost_sema::ClassKind::Trait)
        {
            true => self.composing_classes(trait_fqn).into(),
            false => Arc::from([]),
        };
        self.trait_hosts
            .borrow_mut()
            .insert(key, Arc::clone(&hosts));
        hosts
    }

    /// The walk behind [`Self::trait_hosts`]: the files the skeleton says compose each trait of
    /// the frontier, read for the classes in them that do.
    fn composing_classes(&self, trait_fqn: &str) -> Vec<Name> {
        let mut hosts = Vec::new();
        let mut frontier = vec![trait_fqn.to_owned()];
        let mut walked = FxHashSet::from_iter([trait_fqn.to_ascii_lowercase()]);

        while let Some(composed) = frontier.pop() {
            for &file in self.skeleton.files_using_trait(&composed) {
                let Some(entry) = self.skeleton.file(file) else {
                    continue;
                };
                for class in entry.classes() {
                    // The map is keyed by a hash, so a file it lists is checked rather than trusted.
                    if !class
                        .uses()
                        .iter()
                        .any(|used| used.fqn.as_str().eq_ignore_ascii_case(&composed))
                    {
                        continue;
                    }
                    let name = class.fqn();
                    // Against a trait that composes itself, which a half-written file can say.
                    if !walked.insert(name.as_str().to_ascii_lowercase()) {
                        continue;
                    }
                    match class.kind() {
                        phrost_sema::ClassKind::Trait => frontier.push(name.as_str().to_owned()),
                        _ => {
                            hosts.push(name.clone());
                            if hosts.len() > MOST_HOSTS {
                                return Vec::new();
                            }
                        }
                    }
                }
            }
        }
        hosts.sort_by(|left, right| left.as_str().cmp(right.as_str()));
        hosts
    }
}
