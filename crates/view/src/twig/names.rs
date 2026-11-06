//! What a template name written in a Twig template refers to.
//!
//! The second language a framework project is written in besides its configuration, and answered
//! the same way: the buffer is text the index holds nothing about, a reader finds the names in it,
//! and a catalog a framework filled says what each one is. `phrost-twig` knows the syntax; which
//! file `@node/node.html.twig` is was decided by whoever configures the loader, and arrives as the
//! catalogs a [`TwigLoader`](phrost_index::TwigLoader) names.
//!
//! # When a missing name is a mistake
//!
//! The loader reads a namespace's directories and nothing else, so a `.twig` name under a namespace
//! whose directories were walked is a template that exists or one that does not -- and one that does
//! not is an exception while the page renders. Everything else is open, and only a near miss is
//! said: a bare name a framework resolves some other way, a name in a namespace something else
//! registers, a file of another kind, a path that climbs out with `..` or passes through a directory
//! the walk did not enter, and a name the template itself allows to be missing.
//!
//! A name is held when either catalog holds it: the one of names a template is written with, or
//! the one of every other name the loader takes. Drupal registers each namespace at the extension's
//! `templates/` *and* at its root, so `@announcements_feed/templates/x.html.twig` is the same file as
//! `@announcements_feed/x.html.twig` -- core writes both, and reading only the first reported two
//! correct includes as missing.

use phrost_index::{ReturnContracts, TwigLoader};
use phrost_twig::TemplateName;

use crate::{
    configuration::{
        ConfigCandidate, ConfigCandidateKind, ConfigProblem, ConfigReference, ConfigReferent,
    },
    view::IndexView,
};

use super::TWIG;

impl IndexView<'_> {
    /// The template name at `offset` in a template, and what it refers to.
    ///
    /// Nothing for a name no catalog holds and none can rule out: there is nowhere to send a reader
    /// and nothing true to say about it.
    pub(crate) fn twig_reference_at(&self, text: &str, offset: usize) -> Option<ConfigReference> {
        let contracts = self.skeleton.returns();
        let found = phrost_twig::template_names(text)
            .into_iter()
            .find(|found| found.written.contains(&offset) || found.written.end == offset)?;
        let referent = match held_by(contracts, &found.name) {
            Some(catalog) => ConfigReferent::CatalogEntry {
                place: contracts.catalog_placed_at(catalog, &found.name).cloned(),
                detail: contracts
                    .catalog_detail(catalog, &found.name)
                    .unwrap_or("")
                    .into(),
            },
            None => ConfigReferent::Unregistered {
                catalog: proved_missing(contracts, &found)?.templates.clone(),
            },
        };
        Some(ConfigReference {
            name: found.name.into(),
            range: found.written,
            referent,
        })
    }

    /// Every template name in a template that no catalog holds, for the caller to judge.
    ///
    /// Judged by the rule a key written in PHP is, for the reason the configuration reader gives:
    /// two constructors of one rule is how one comes to miss what the other has.
    pub(crate) fn twig_problems(&self, text: &str) -> Vec<ConfigProblem> {
        let contracts = self.skeleton.returns();
        let loaders = contracts.twig_loaders();
        let Some(first) = loaders.first() else {
            return Vec::new();
        };
        phrost_twig::template_names(text)
            .into_iter()
            .filter(|found| held_by(contracts, &found.name).is_none())
            .map(|found| {
                let missing = proved_missing(contracts, &found);
                // The loader the name is closest to a template of, so a near miss is said in the
                // catalog it was a near miss of.
                let (loader, nearest) = loaders
                    .iter()
                    .find_map(|loader| {
                        contracts
                            .nearest_catalog_key(&loader.templates, &found.name)
                            .map(|near| (loader, Some(near)))
                    })
                    .unwrap_or((missing.unwrap_or(first), None));
                ConfigProblem {
                    range: found.written.clone(),
                    used: crate::catalog_uses::CatalogUse {
                        key: found.name.as_str().into(),
                        // Offsets into the text handed in, which is what the caller has too -- the
                        // index holds nothing about this file.
                        range: phrost_model::TextRange::default(),
                        catalog: loader.templates.clone(),
                        known: false,
                        closed: missing.is_some(),
                        nearest: nearest.map(Into::into),
                    },
                }
            })
            .collect()
    }

    /// What could be written where a template name is being written, and what it would replace.
    pub(crate) fn twig_name_completions(
        &self,
        text: &str,
        offset: usize,
        limit: usize,
    ) -> Option<(std::ops::Range<usize>, Vec<ConfigCandidate>)> {
        let contracts = self.skeleton.returns();
        let found = phrost_twig::template_names(text)
            .into_iter()
            .find(|found| found.written.start <= offset && offset <= found.written.end)?;
        let typed = text.get(found.written.start..offset)?;
        let mut items = Vec::new();
        for loader in contracts.twig_loaders() {
            items.extend(
                contracts
                    .catalog_keys(&loader.templates, typed, limit)
                    .into_iter()
                    .map(|key| ConfigCandidate {
                        name: key.into(),
                        detail: contracts
                            .catalog_detail(&loader.templates, key)
                            .unwrap_or("")
                            .into(),
                        kind: ConfigCandidateKind::CatalogKey,
                    }),
            );
        }
        items.truncate(limit);
        Some((found.written, items))
    }
}

/// The catalog holding `name`, when one does: a loader's names first, then its aliases.
fn held_by<'a>(contracts: &'a ReturnContracts, name: &str) -> Option<&'a str> {
    contracts
        .twig_loaders()
        .iter()
        .flat_map(|loader| [&*loader.templates, &*loader.aliases])
        .find(|catalog| contracts.catalog_holds(catalog, name))
}

/// The loader that proves a name no catalog holds is not there, when one can.
///
/// A `.twig` name in a namespace that was walked, written where the template does not allow it to
/// be missing, staying inside the namespace's directories and out of any the walk skipped -- see
/// the module's own notes for why each.
fn proved_missing<'a>(
    contracts: &'a ReturnContracts,
    found: &TemplateName,
) -> Option<&'a TwigLoader> {
    if found.may_be_missing || !found.name.ends_with(TWIG) {
        return None;
    }
    if found.name.split('/').any(|segment| segment == "..") {
        return None;
    }
    let namespace = phrost_twig::namespace_of(&found.name)?;
    contracts.twig_loaders().iter().find(|loader| {
        contracts.catalog_holds(&loader.namespaces, namespace)
            && !loader.passes_unwalked(&found.name)
    })
}
