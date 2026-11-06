//! A catalog: the keys a framework's own configuration registers, and what each names.
//!
//! Its own file because it is its own concern and the only one here that grew: a service id, a route
//! name, an entity type, a render element and a field are five catalogs filled from five different
//! sources, and what they share is one table and one question -- does this catalog hold this key.
//! `ReturnContracts` still owns the table; a child module reaches its private fields, so the split
//! costs no visibility.
//!
//! The near-miss reader lives here for the same reason: it is the third answer this table can give,
//! between "holds it" and "does not", and it reads no other part of the contracts.

use std::{path::Path, sync::Arc};

use phrost_model::FileId;
use rustc_hash::FxHashMap;

use super::ReturnContracts;

/// What one catalog key names.
///
/// Two answers, because two kinds of thing get named by a string in PHP. A service id names a class
/// the index holds. A route name names a block of YAML -- not PHP, not a declaration anything here
/// can resolve, and nothing but somewhere to go and something to say about it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub(super) enum CatalogValue {
    Class(Box<str>),
    Place(Place),
    /// The shape of the value the key holds, for a catalog of configuration values rather than of
    /// classes: a container parameter, whose YAML says it is a list where the method that reads it
    /// can only say `array|bool|string|int|float|\UnitEnum|null`.
    Shape(phrost_model::BuiltinTy),
    /// The key is registered and nothing more is known about it.
    Named,
}

/// Where a key is declared, in a file the index does not hold.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Place {
    pub path: Arc<Path>,
    /// Zero-based, as the protocol counts them.
    ///
    /// A position rather than an offset, because nothing here has the file's text: it is not PHP, so
    /// it is not indexed, and an offset could never be converted later. Recorded where the file was
    /// read instead.
    pub line: u32,
    pub character: u32,
    /// How to describe it in a list: a route's path, a permission's title.
    pub detail: Box<str>,
}

/// One file's claim on a catalog key.
#[derive(Clone, PartialEq, Eq, Debug)]
pub(super) struct Claim {
    pub(super) file: FileId,
    /// The class the key names, for a catalog whose keys name one.
    ///
    /// `None` where a declaration registers a key that names nothing -- a route derived from an
    /// entity type's link template is a route, and answering the entity class here would make
    /// `Url::fromRoute('entity.node.canonical')` return a `Node`, since an argument bound to a
    /// catalog reads the key's class as the call's return.
    pub(super) class: Option<Box<str>>,
    /// A line to show beside the key, for a claim that has one: the path a link template writes.
    pub(super) detail: Option<Box<str>>,
}

/// The shortest key a near miss is looked for at all.
///
/// Four characters and under, a key one edit from another is more likely a different key than a
/// misspelling of it: `node` and `user` are both real entity types and are four edits apart, but
/// `menu` and `meta` are one.
pub const NEAR_MISS_FLOOR: usize = 5;

/// How far a key may be from a registered one and still be read as a misspelling of it.
///
/// One, and the reason is a measurement rather than a preference. This was two for a long key, on
/// the argument that a long id has room for two independent slips -- and on the reference project
/// that produced a **wrong** suggestion: `entity.node.edt_form` is two edits from
/// `entity.node.add_form`, which is the only one of the family the catalog holds, so the report
/// named `add_form` for a typo of `edit_form`. A confidently wrong suggestion is worse than silence,
/// which is what the whole check is gated on avoiding.
///
/// At one, every real typo the project was probed with still reports -- `user.logn`,
/// `app_core.health_chck`, `textfeild` -- because a transposition already costs one edit here; see
/// [`edits_within`]. What is given up is two independent slips in one identifier, which is rarer
/// than a report that points at the wrong key.
pub const NEAR_MISS_EDITS: usize = 1;

/// The edit distance between two strings, when it is within `limit`.
///
/// Damerau-Levenshtein with adjacent transpositions, and the transposition is the reason: swapping
/// two letters is the commonest typo there is and plain Levenshtein charges two edits for it, so
/// `textfeild` would score the same as two unrelated mistakes. Counting it as one lets the bound
/// stay tight enough that `node` and `user` are never confused.
///
/// `None` as soon as the distance cannot be within the bound, which is what makes this affordable
/// over a catalog of a thousand route names: the answer is wanted for a handful of near misses, and
/// every other candidate should cost as little as possible. The caller's length gate rejects most
/// before this is called at all, and the row minimum stops the rest early.
///
/// Bytes rather than characters. A catalog key is a machine identifier in every framework this
/// describes -- a service id, a route name, a field name -- and none is written in anything but
/// ASCII. A multi-byte name would score its own bytes, which overstates the distance and so reports
/// less rather than more.
pub fn edits_within(left: &str, right: &str, limit: usize) -> Option<usize> {
    let (left, right) = (left.as_bytes(), right.as_bytes());
    if left.len().abs_diff(right.len()) > limit {
        return None;
    }

    // Three rows, because a transposition reads the one from two back.
    let mut before: Vec<usize> = Vec::new();
    let mut previous: Vec<usize> = (0..=right.len()).collect();
    let mut current = vec![0usize; right.len() + 1];

    for (row, &l) in left.iter().enumerate() {
        current[0] = row + 1;
        let mut least = current[0];
        for column in 1..=right.len() {
            let r = right[column - 1];
            let mut cost = (previous[column - 1] + usize::from(l != r))
                .min(previous[column] + 1)
                .min(current[column - 1] + 1);
            // `ab` against `ba`: one edit, not two.
            if row > 0 && column > 1 && l == right[column - 2] && left[row - 1] == r {
                cost = cost.min(before[column - 2] + 1);
            }
            current[column] = cost;
            least = least.min(cost);
        }
        // Nothing in this row is still under the bound, so nothing below it can be either.
        if least > limit {
            return None;
        }
        std::mem::swap(&mut before, &mut previous);
        std::mem::swap(&mut previous, &mut current);
        current.resize(right.len() + 1, 0);
    }

    (previous[right.len()] <= limit).then_some(previous[right.len()])
}

impl ReturnContracts {
    /// Records that `file` declares `class` under `key` of `catalog`.
    pub fn claim_key(&mut self, file: FileId, catalog: &str, key: &str, class: &str) {
        self.record(
            catalog,
            key,
            Claim {
                file,
                class: Some(class.into()),
                detail: None,
            },
        );
    }

    /// Records that `file` registers `key` of `catalog` without its naming a class.
    ///
    /// The same table and the same withdrawal, so a re-ingested file takes back exactly what it put
    /// in. What is different is that nothing resolves through it: see [`Claim::class`].
    pub fn claim_derived_key(&mut self, file: FileId, catalog: &str, key: &str, detail: &str) {
        self.record(
            catalog,
            key,
            Claim {
                file,
                class: None,
                detail: (!detail.is_empty()).then(|| detail.into()),
            },
        );
    }

    fn record(&mut self, catalog: &str, key: &str, claim: Claim) {
        let claims = self
            .declared
            .entry(catalog.into())
            .or_default()
            .entry(key.into())
            .or_default();
        if !claims.contains(&claim) {
            claims.push(claim);
        }
    }

    /// Withdraws one claim, and the key with it when nothing else claimed it.
    pub fn withdraw_key(&mut self, file: FileId, catalog: &str, key: &str) {
        let Some(keys) = self.declared.get_mut(catalog) else {
            return;
        };
        let Some(claims) = keys.get_mut(key) else {
            return;
        };
        claims.retain(|claim| claim.file != file);
        if claims.is_empty() {
            keys.remove(key);
        }
    }

    /// Records that a catalog holds every key there is.
    pub fn close_catalog(&mut self, catalog: &str) {
        let catalog: Box<str> = catalog.into();
        if !self.closed.contains(&catalog) {
            self.closed.push(catalog);
        }
    }

    /// Whether a key this catalog does not hold can be said not to exist.
    pub fn is_closed_catalog(&self, catalog: &str) -> bool {
        self.closed.iter().any(|name| &**name == catalog)
    }

    /// Records what one key of one catalog names.
    ///
    /// Later wins, which is what every container this describes does: a module redefining a core
    /// service id replaces it, and the load order decides. Whoever fills the catalog is responsible
    /// for that order.
    pub fn catalog_entry(&mut self, catalog: &str, key: &str, class: Option<&str>) {
        let value = match class {
            Some(class) => CatalogValue::Class(class.into()),
            None => CatalogValue::Named,
        };
        self.catalogs
            .entry(catalog.into())
            .or_default()
            .insert(key.into(), value);
    }

    /// Records that the project's own files declare one key of one catalog -- see
    /// [`Self::own_keys`](super::ReturnContracts).
    pub fn mark_own(&mut self, catalog: &str, key: &str) {
        self.own_keys
            .entry(catalog.into())
            .or_default()
            .insert(key.into());
    }

    fn is_own(&self, catalog: &str, key: &str) -> bool {
        self.own_keys
            .get(catalog)
            .is_some_and(|keys| keys.contains(key))
    }

    /// Records the shape of the value one key of one catalog holds.
    ///
    /// The same table as a class-valued key, for the reason [`Self::catalog_place`] gives: one
    /// lookup answers "is this a key of this catalog" whichever kind of value it has.
    pub fn catalog_shape(&mut self, catalog: &str, key: &str, shape: phrost_model::BuiltinTy) {
        self.catalogs
            .entry(catalog.into())
            .or_default()
            .insert(key.into(), CatalogValue::Shape(shape));
    }

    /// The shape of the value a catalog key holds, for a key that holds a value rather than a class.
    pub fn catalog_shape_of(&self, catalog: &str, key: &str) -> Option<phrost_model::BuiltinTy> {
        match self.catalogs.get(catalog)?.get(key)? {
            CatalogValue::Shape(shape) => Some(*shape),
            CatalogValue::Class(_) | CatalogValue::Place(_) | CatalogValue::Named => None,
        }
    }

    /// Records that one key of one catalog is declared at one place.
    ///
    /// The same table as a class-valued key, so one lookup answers "is this a key of this catalog"
    /// whichever kind it is -- which is the question completion and a report both ask.
    pub fn catalog_place(&mut self, catalog: &str, key: &str, place: Place) {
        self.catalogs
            .entry(catalog.into())
            .or_default()
            .insert(key.into(), CatalogValue::Place(place));
    }

    /// Every key of a catalog whose place is the file at `path`: the theme hooks a template
    /// implements, the names it is loaded by.
    ///
    /// A scan of the one catalog, which is a few hundred entries: asked once per request about a
    /// template, and a reverse table kept for it would be a second map of the same strings.
    pub fn catalog_keys_placed_at(&self, catalog: &str, path: &std::path::Path) -> Vec<&str> {
        let mut found = self
            .catalogs
            .get(catalog)
            .into_iter()
            .flatten()
            .filter_map(|(key, value)| match value {
                CatalogValue::Place(place) if &*place.path == path => Some(&**key),
                _ => None,
            })
            .collect::<Vec<_>>();
        found.sort_unstable();
        found
    }

    /// Where a catalog key is declared, for a key that names a place rather than a class.
    pub fn catalog_placed_at(&self, catalog: &str, key: &str) -> Option<&Place> {
        match self.catalogs.get(catalog)?.get(key)? {
            CatalogValue::Place(place) => Some(place),
            CatalogValue::Class(_) | CatalogValue::Shape(_) | CatalogValue::Named => None,
        }
    }

    /// How to describe a key in a list: the class it names, or what its declaration says about it.
    ///
    /// One method rather than two, because a list of keys does not care which kind it is showing --
    /// it needs a line of text beside each, and both kinds have one.
    pub fn catalog_detail(&self, catalog: &str, key: &str) -> Option<&str> {
        let registered = match self.catalogs.get(catalog).and_then(|keys| keys.get(key)) {
            Some(CatalogValue::Class(class)) => Some(&**class),
            Some(CatalogValue::Place(place)) => Some(&*place.detail),
            // The shape its configuration gives it, which is the whole of what is known about a
            // value: `array` beside `serializer.formats`.
            Some(CatalogValue::Shape(shape)) => Some(shape.as_str()),
            // Registered, and there is nothing to say about it. A caller shows the key alone, which
            // is honest -- inventing a description would be describing what nothing declared.
            Some(CatalogValue::Named) | None => None,
        };
        registered.or_else(|| {
            // A key no bootstrap-time source registered may still have been claimed by a
            // declaration, and a derived one carries the line its own entry wrote: for an entity
            // route, the path of the link template it came from.
            //
            // The smallest rather than the first, because several declarations claim one key and
            // their order is the order their files were indexed in -- which differs between a cold
            // start and one that read a stored index. Four modules implement `notifications_alter`
            // on the reference project, and hover named a different one of them on every restart.
            // There is no rank to prefer one by, so what makes it stable is the text itself.
            self.declared
                .get(catalog)?
                .get(key)?
                .iter()
                .filter_map(|claim| claim.detail.as_deref())
                .min()
        })
    }

    /// Whether a catalog holds a key at all, of either kind.
    pub fn catalog_holds(&self, catalog: &str, key: &str) -> bool {
        self.catalogs
            .get(catalog)
            .is_some_and(|keys| keys.contains_key(key))
            || self
                .declared
                .get(catalog)
                .is_some_and(|keys| keys.contains_key(key))
    }

    /// The registered key a written one is a plausible misspelling of.
    ///
    /// The third answer between "this catalog holds everything, so a miss is an error" and "it does
    /// not, so a miss says nothing". A **typo always nearly hits**: `entity.node.canonicl` is one
    /// edit from `entity.node.canonical`. A key that simply is not there does not -- a field attached
    /// by a bundle, or a project's own `'#type' => 'Float'` DSL borrowing the `#` convention, are far
    /// from every registered key. So this reports the mistake in an *open* catalog without reporting
    /// the absence, which is what kept five of eight catalogs silent.
    ///
    /// The sharp edge of that is which side of the line a key falls on, and it is decided *here* --
    /// by what the catalog holds -- rather than by anything about the report. `entity.node.canonical`
    /// used to be an absence: Drupal generates it from an entity type's link templates and no YAML
    /// declares it, so a typo of it was silent while a typo of a YAML route reported, and the
    /// asymmetry read from outside as a defect in this check. It was a gap in the catalog, and
    /// [`ReturnContracts::claim_derived_key`] closed it.
    ///
    /// One edit, and no more: see [`NEAR_MISS_EDITS`] for the measurement that settled that, which
    /// is about a wrong suggestion rather than a missed one. Below [`NEAR_MISS_FLOOR`] nothing is
    /// compared at all -- at four characters and under, a key one edit from another is more likely a
    /// different key than a misspelling of it.
    ///
    /// `None` when nothing is close, when several keys are equally close -- naming one would be a
    /// guess, and a person shown two suggestions has learned nothing -- and when the catalog holds
    /// the key already, which is the caller's question and not this one.
    pub fn nearest_catalog_key(&self, catalog: &str, key: &str) -> Option<&str> {
        if key.len() < NEAR_MISS_FLOOR {
            return None;
        }

        let mut best: Option<(&str, usize)> = None;
        let mut tied = false;
        for candidate in self
            .catalogs
            .get(catalog)
            .into_iter()
            .flat_map(|keys| keys.keys())
            .chain(
                self.declared
                    .get(catalog)
                    .into_iter()
                    .flat_map(|keys| keys.keys()),
            )
        {
            let candidate = &**candidate;
            // The cheap gate, and it is what makes this affordable over a catalog of a thousand
            // route names: a key whose length differs by more than the bound cannot be within it,
            // and comparing lengths is one subtraction.
            if candidate.len().abs_diff(key.len()) > NEAR_MISS_EDITS {
                continue;
            }
            let Some(distance) = edits_within(key, candidate, NEAR_MISS_EDITS) else {
                continue;
            };
            match best {
                Some((_, held)) if distance > held => {}
                Some((_, held)) if distance == held => tied = true,
                _ => {
                    best = Some((candidate, distance));
                    tied = false;
                }
            }
        }

        match tied {
            true => None,
            false => best.map(|(candidate, _)| candidate),
        }
    }

    /// The class a catalog key names, `None` when the catalog has no such key.
    ///
    /// Keys are compared exactly. A service id and an entity type id are both case-sensitive in the
    /// containers this describes, and folding them would answer for an id that does not exist.
    /// Configuration first, then declarations. A container's own file is the more explicit of the
    /// two, and in practice the two fill different catalogs anyway.
    ///
    /// A key two declarations claim answers *nothing*. Naming one of them would mean ranking, and
    /// ranking by anything the index knows about -- file id, ingest order -- makes the answer depend
    /// on discovery order, which is the invariant everything else here is checked against. Two
    /// classes claiming one entity type id is a fatal error in the framework anyway, so the case
    /// only arises in code that does not run.
    pub fn catalog_class(&self, catalog: &str, key: &str) -> Option<&str> {
        if let Some(CatalogValue::Class(class)) =
            self.catalogs.get(catalog).and_then(|keys| keys.get(key))
        {
            return Some(class);
        }
        // Only the claims that name one. A key registered by a declaration that names nothing --
        // a derived route -- is a key of this catalog and not a class, and must not become one.
        let named = self
            .declared
            .get(catalog)?
            .get(key)?
            .iter()
            .filter_map(|claim| claim.class.as_deref())
            .collect::<Vec<_>>();
        match named.split_first()? {
            (only, []) => Some(only),
            (first, rest) if rest.iter().all(|class| class == first) => Some(first),
            _ => None,
        }
    }

    /// Every key of a catalog that starts with `prefix`, in a stable order.
    ///
    /// For offering them: somebody typing a service id wants the list, and the list is the only thing
    /// in the file that says what is legal there. Sorted, because a completion list a client does not
    /// reorder must not change between keystrokes.
    ///
    /// Matched the way a member is -- see [`phrost_model::matches_loosely`] -- and not by prefix. A
    /// key carries its subject in the middle: `app_core.application.manager` answers to `manager`,
    /// and nobody remembers which of 1 856 ids a project has starts the way that one does.
    /// At most `limit` of them, because an empty prefix matches every service a project has: on a
    /// real Drupal site that is 1804, and building them all cost 6.8 ms at the median and 25 ms at
    /// the worst -- through a budget of 12. The caller says how many it can afford and tells the
    /// client the list is partial.
    pub fn catalog_keys(&self, catalog: &str, prefix: &str, limit: usize) -> Vec<&str> {
        let configured = self
            .catalogs
            .get(catalog)
            .into_iter()
            .flat_map(|keys| keys.keys());
        let declared = self
            .declared
            .get(catalog)
            .into_iter()
            .flat_map(|keys| keys.keys());
        let mut keys = configured
            .chain(declared)
            .map(|key| &**key)
            .filter(|key| phrost_model::matches_loosely(key, prefix))
            .collect::<Vec<_>>();
        // Prefix hits first, then the rest by how well they match. Which is the whole of what makes
        // loose matching liveable here: a service id is `app_core.application.manager` and what a
        // person remembers of it is `manager`, so a key has to be reachable from the middle -- but
        // somebody who *did* type the start must still see what they typed at the top, and a key
        // whose words the letters open beats one they merely appear in.
        //
        // Sorted before truncating, so which ones survive is the same every time: a list that
        // reshuffles between keystrokes is worse than a short one. Alphabetical last, so a tie is
        // broken the same way on every keystroke rather than by whichever order the map gave.
        keys.sort_by(|left, right| {
            let opens = |key: &str| !phrost_model::starts_with_ignoring_case(key, prefix);
            let scored = |key: &str| phrost_model::match_score(key, prefix).unwrap_or(0);
            // The project's own before its dependencies', among keys that match alike -- with
            // nothing typed every key is a prefix hit, and alphabetically core's
            // `Drupal\Component\...` aliases came first of nearly two thousand.
            let theirs = |key: &str| !self.is_own(catalog, key);
            // And an id spelled as a class -- an autowiring alias, `Drupal\...\FooInterface` -- after
            // the ones a person writes by name: an upper-case letter sorts before every lower-case
            // one, so alphabetically the aliases led the list.
            let classlike = |key: &str| key.contains('\\');
            match (opens(left), opens(right)) {
                // Among the prefix hits, alphabetical -- they all match the same way and equally
                // well, and a person scanning a sorted list is what the order is for.
                (false, false) => theirs(left)
                    .cmp(&theirs(right))
                    .then_with(|| classlike(left).cmp(&classlike(right)))
                    .then_with(|| left.cmp(right)),
                (false, true) => std::cmp::Ordering::Less,
                (true, false) => std::cmp::Ordering::Greater,
                // Among the rest, the score is the whole of what distinguishes them, and length
                // settles a tie before the alphabet does: the shorter key is the closer one.
                (true, true) => scored(right)
                    .cmp(&scored(left))
                    .then_with(|| theirs(left).cmp(&theirs(right)))
                    .then_with(|| left.len().cmp(&right.len()))
                    .then_with(|| left.cmp(right)),
            }
        });
        // A key both configuration and a declaration name is one key, offered once. Two spellings of
        // one key score identically and compare equal, so they are still adjacent here.
        keys.dedup();
        keys.truncate(limit);
        keys
    }

    /// How many keys each catalog holds, for the memory report and the startup log.
    pub fn catalog_sizes(&self) -> Vec<(&str, usize)> {
        let mut sizes = FxHashMap::default();
        for (name, entries) in &self.catalogs {
            *sizes.entry(&**name).or_insert(0) += entries.len();
        }
        for (name, entries) in &self.declared {
            *sizes.entry(&**name).or_insert(0) += entries.len();
        }
        for (name, entries) in &self.dispatched {
            *sizes.entry(&**name).or_insert(0) += entries.len();
        }
        let mut sizes = sizes.into_iter().collect::<Vec<_>>();
        sizes.sort();
        sizes
    }

    /// Every declared catalog key and the class claiming it, in a stable order.
    ///
    /// For the index dump, which is what the `incremental == from-scratch` check compares -- a claim
    /// a re-ingested file failed to withdraw is invisible without it, and invalidation bugs are the
    /// ones that stay hidden for hours.
    pub fn declared_keys(&self) -> Vec<(&str, &str, Vec<&str>)> {
        let mut rows = self
            .declared
            .iter()
            .flat_map(|(catalog, keys)| {
                keys.iter().map(move |(key, claims)| {
                    let mut classes = claims
                        .iter()
                        .filter_map(|claim| claim.class.as_deref())
                        .collect::<Vec<_>>();
                    classes.sort_unstable();
                    (&**catalog, &**key, classes)
                })
            })
            .collect::<Vec<_>>();
        rows.sort();
        rows
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The project's own keys first, and an id spelled as a class after the ones written by name --
    /// with nothing typed, where every key matches alike and the order is the whole answer.
    #[test]
    fn the_projects_own_keys_lead_a_list_nothing_narrows() {
        let mut contracts = ReturnContracts::default();
        for key in [
            "Drupal\\Component\\Datetime\\TimeInterface",
            "access_check.default",
            "app.manager",
            "Drupal\\app\\ManagerInterface",
            "app.logger",
            "cache.default",
        ] {
            contracts.catalog_entry("service", key, None);
        }
        for own in ["app.manager", "Drupal\\app\\ManagerInterface", "app.logger"] {
            contracts.mark_own("service", own);
        }

        assert_eq!(
            contracts.catalog_keys("service", "", 10),
            [
                "app.logger",
                "app.manager",
                "Drupal\\app\\ManagerInterface",
                "access_check.default",
                "cache.default",
                "Drupal\\Component\\Datetime\\TimeInterface",
            ]
        );
    }

    /// A typo nearly hits; a key that is simply absent does not.
    ///
    /// The three that must stay silent are the three that kept five catalogs quiet: a runtime-
    /// generated route, a bundle-attached field, and a project's own DSL borrowing the `#`
    /// convention. None of them is near anything registered.
    #[test]
    fn a_near_miss_is_a_typo_and_an_absent_key_is_not() {
        let mut contracts = ReturnContracts::default();
        for key in [
            "entity.node.canonical",
            "app_core.settings",
            "field_item_amount",
            "textfield",
            "node",
            "user",
        ] {
            contracts.catalog_entry("k", key, None);
        }

        // One edit over a long key, and a transposition, which costs two.
        assert_eq!(
            contracts.nearest_catalog_key("k", "entity.node.canonicl"),
            Some("entity.node.canonical")
        );
        assert_eq!(
            contracts.nearest_catalog_key("k", "field_item_amuont"),
            Some("field_item_amount")
        );
        // One edit over a short key.
        assert_eq!(
            contracts.nearest_catalog_key("k", "textfeild"),
            Some("textfield")
        );

        // Two edits is not a near miss, and the measurement that settled that is this shape: on the
        // reference project `entity.node.edt_form` is two edits from `entity.node.add_form`, the only
        // one of that family the catalog holds -- so a two-edit bound named `add_form` for a typo of
        // `edit_form`. A wrong suggestion is worse than silence.
        assert_eq!(
            contracts.nearest_catalog_key("k", "field_loam_amoumt"),
            None
        );

        // Nothing near: a route nobody declared, a field a bundle attached, a DSL word.
        assert_eq!(
            contracts.nearest_catalog_key("k", "view.frontpage.page_1"),
            None
        );
        assert_eq!(
            contracts.nearest_catalog_key("k", "field_settled_info"),
            None
        );
        assert_eq!(contracts.nearest_catalog_key("k", "Float"), None);

        // Under the floor nothing is compared, so two real four-letter keys one edit apart are not
        // read as misspellings of each other.
        assert_eq!(contracts.nearest_catalog_key("k", "nose"), None);
        // And a key the catalog holds is the caller's question, not this one -- but it answers
        // itself at distance zero, which is what makes the caller's gate the only gate.
        assert_eq!(
            contracts.nearest_catalog_key("k", "textfield"),
            Some("textfield")
        );
    }
}
