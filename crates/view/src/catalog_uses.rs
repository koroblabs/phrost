//! Every string in the file that a call reads as a key into a catalog.
//!
//! The whole-file counterpart of [`IndexView::catalog_key_at`], and a different question from either
//! completion or navigation: those ask what a key *could* be or *is*, and this asks whether it is one
//! at all.
//!
//! Whether a miss is worth reporting is not this projection's decision, so both halves of the answer
//! come back: whether the catalog holds the key, and whether the catalog holds every key there is.
//! Drupal's container is loaded from files that can be read in full, so an id missing from it does
//! not exist; a route name can be generated at runtime and a missing one may still be real.

use phrost_body::CallTarget;
use phrost_model::{Name, TextRange};
use phrost_sema::MemberKind;

use crate::{
    resolve::HIERARCHY_DEPTH,
    view::{FOLLOW_LIMIT, IndexView},
};

/// One string a call reads as a key.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CatalogUse {
    /// The key the call reads, which is the argument plus whatever the method appends to it.
    ///
    /// The same as the text between the quotes for every binding but one. Drupal's `alter('form')`
    /// runs `hook_form_alter`, so the key is `form_alter` while the source says `form` -- and the key
    /// is the right half to name in a report, because it is the thing nothing registers. What must
    /// be in the argument's own spelling is [`Self::nearest`], since that is what a person types.
    pub key: Box<str>,
    /// The literal including its delimiters, which is what a report underlines and what an edit
    /// would replace.
    pub range: TextRange,
    /// The catalog this call reads it as a key into.
    pub catalog: Box<str>,
    /// Whether that catalog holds it.
    pub known: bool,
    /// Whether the catalog claims to hold every key there is, so that a miss means "does not exist"
    /// rather than "not one I can see".
    pub closed: bool,
    /// What should have been written at that position, when the key is not held.
    ///
    /// The third answer, and what lets an *open* catalog say something: a typo always nearly hits a
    /// real key and a key that is simply absent does not. `None` for a key the catalog holds, for one
    /// nothing is close to, and for one that is equally close to two -- see
    /// `ReturnContracts::nearest_catalog_key`.
    ///
    /// In the *argument's* spelling rather than the catalog's, which for a suffixed binding is not
    /// the same string: suggesting `form_alter` at a position where the method appends `_alter` tells
    /// a person to write `alter('form_alter')`, which is the mistake this is meant to catch.
    pub nearest: Option<Box<str>>,
}

impl IndexView<'_> {
    /// Every catalog key written in the current file, in source order.
    pub fn catalog_uses(&self) -> Vec<CatalogUse> {
        let Some(open) = self.current() else {
            return Vec::new();
        };
        let hir = open.hir();
        let text = open.text();
        let contracts = self.skeleton.returns();
        let mut out = Vec::new();

        for call in phrost_body::call_sites(hir) {
            let (receiver, name) = match &call.target {
                CallTarget::Member { receiver, name } => (receiver, name),
                // A function's half of the same binding -- `view('posts.show')`, `trans('auth.failed')`,
                // `app('router')` -- which goto has always read and this did not, so a search, a
                // highlight and a rename at the key answered nothing where goto went to the file.
                CallTarget::Function(written) => {
                    let Some(binding) =
                        contracts.function_argument_binding(hir.name_ref(*written).path.as_str())
                    else {
                        continue;
                    };
                    for (range, written) in written_keys(hir, text, &call.arguments, binding.at) {
                        let known = contracts.catalog_holds(&binding.catalog, written);
                        out.push(CatalogUse {
                            range,
                            catalog: binding.catalog.clone(),
                            known,
                            closed: contracts.is_closed_catalog(&binding.catalog),
                            nearest: (!known)
                                .then(|| meant_instead(contracts, &binding.catalog, written, ""))
                                .flatten(),
                            key: written.into(),
                        });
                    }
                    continue;
                }
                _ => continue,
            };
            // The cheap gate first, as everywhere else here: nearly every call in a file is to a
            // method no framework has bound either way, and that costs one scan of a handful of
            // short names per table.
            let bindings = contracts.argument_bindings(name.as_str());
            let forwards = !contracts.forwarded_keys(name.as_str()).is_empty();
            if bindings.is_empty() && !forwards {
                continue;
            }

            // And the literal before the receiver, which is what keeps this off the diagnostics
            // budget: resolving a receiver walks the hierarchy, and a bound method called with a
            // variable where the key goes has nothing here to be right or wrong about. Any position
            // a rule names will do -- which one it was is settled per binding below.
            let written = bindings
                .iter()
                .map(|binding| binding.at)
                .chain(forwards.then_some(0))
                .any(|at| !written_keys(hir, text, &call.arguments, at).is_empty());
            if !written {
                continue;
            }

            let at = call.receiver_at();
            let Some(class) = self
                .resolve_path(at, hir, receiver, FOLLOW_LIMIT)
                .and_then(|held| self.class_of_held(at, hir, &held))
            else {
                continue;
            };

            for binding in bindings {
                let applies = class.as_str().eq_ignore_ascii_case(&binding.class)
                    || self.descends_from(class.as_str(), &binding.class, HIERARCHY_DEPTH);
                if !applies {
                    continue;
                }
                for (range, written) in written_keys(hir, text, &call.arguments, binding.at) {
                    // The key the *method* reads, which is the argument plus what it appends. Empty
                    // for every binding but Drupal's `alter`, whose argument is a hook name with
                    // `_alter` taken off it.
                    let key = match binding.suffix.is_empty() {
                        true => written.to_owned(),
                        false => format!("{written}{}", binding.suffix),
                    };
                    let known = contracts.catalog_holds(&binding.catalog, &key);
                    out.push(CatalogUse {
                        range,
                        catalog: binding.catalog.clone(),
                        known,
                        closed: contracts.is_closed_catalog(&binding.catalog),
                        // Only for a key that missed. Walking a thousand route names to find the
                        // nearest one is affordable exactly because it is asked about the handful
                        // that did.
                        nearest: (!known)
                            .then(|| {
                                meant_instead(contracts, &binding.catalog, written, &binding.suffix)
                            })
                            .flatten(),
                        key: key.into(),
                    });
                }
            }

            // The forwarded shape reads the *writing* call's leading literal and no other: the id in
            // `getStorage('node')->load($id)` is the first argument of the first call by the rule's
            // own construction, and there is no second-argument spelling of it to support.
            if forwards && let Some((range, key)) = written_key(text, &call.arguments, 0) {
                self.push_forwarded_uses(at, hir, &class, name, key, range, &mut out);
            }
        }

        out.extend(self.element_uses(hir, text, contracts));
        out.sort_by_key(|used| used.range.start());
        out
    }

    /// The catalog key the cursor is on, as the call reading it sees it.
    ///
    /// A whole-file walk for one position, which is affordable because it is asked by a search and
    /// a rename rather than on a keystroke -- and it is the same walk that decides what counts as a
    /// use, so the cursor and the list of uses cannot disagree about whether a string is a key.
    ///
    /// Gated on the cursor being in a string at all, before anything is walked: every search and
    /// rename target is decided here first, and a code lens asks once per declaration in the file --
    /// none of which is in a string, and each of which would otherwise pay for resolving every
    /// bound call's receiver.
    pub fn catalog_use_at(&self, offset: phrost_model::TextSize) -> Option<CatalogUse> {
        self.current()?.hir().string_at(offset)?;
        self.catalog_uses()
            .into_iter()
            .find(|used| used.range.contains_inclusive(offset))
    }

    /// Where a catalog's key is declared, when what declares it is a file the framework read.
    pub fn catalog_place(&self, catalog: &str, key: &str) -> Option<crate::Place> {
        self.skeleton
            .returns()
            .catalog_placed_at(catalog, key)
            .cloned()
    }

    /// The same, for a key written one call before the method that reads it.
    ///
    /// `getStorage('node')->load($id)` is the shape, and the id is a key at the *first* call: what
    /// the second one does with the storage handler decides nothing about whether `'node'` is an
    /// entity type. So this reads the writing call alone, which is also the only way a key written
    /// and never used gets reported at all.
    ///
    /// # What keeps a name from being enough
    ///
    /// `getStorage` is a method any project may declare for something of its own, and reporting
    /// every literal passed to one would be the "sometimes a key" mistake -- against a closed
    /// catalog that turns correct code into an error. So the call has to hand back the class whose
    /// methods forward the key, which its own signature states: Drupal declares
    /// `getStorage(): EntityStorageInterface`, and the rules that read the id are written on that
    /// interface. A `getStorage` returning anything else is somebody else's method.
    #[allow(clippy::too_many_arguments)]
    fn push_forwarded_uses(
        &self,
        at: phrost_model::TextSize,
        hir: &phrost_hir::FileHir,
        class: &Name,
        method: &Name,
        key: &str,
        range: TextRange,
        out: &mut Vec<CatalogUse>,
    ) {
        let contracts = self.skeleton.returns();
        // After the gate and after the literal, because this is a member lookup on a real class and
        // costs the flattened table: only a call already known to be contracted and already known to
        // have written a literal pays for it.
        let Some(returns) = self
            .member_held_on(
                class.as_str(),
                MemberKind::Method,
                method.as_str(),
                None,
                &[],
            )
            .and_then(|held| self.class_of_held(at, hir, &held))
        else {
            return;
        };

        for forwarded in contracts.forwarded_keys(method.as_str()) {
            let applies = returns.as_str().eq_ignore_ascii_case(&forwarded.returns)
                || self.descends_from(returns.as_str(), &forwarded.returns, HIERARCHY_DEPTH);
            if !applies {
                continue;
            }
            // One report per literal per catalog. Several rules name one call -- five methods read
            // what `getStorage` was asked for -- and each of them would otherwise underline the
            // same string again.
            if out
                .iter()
                .any(|used| used.range == range && used.catalog == forwarded.catalog)
            {
                continue;
            }
            let known = contracts.catalog_holds(&forwarded.catalog, key);
            out.push(CatalogUse {
                key: key.into(),
                range,
                catalog: forwarded.catalog.clone(),
                known,
                closed: contracts.is_closed_catalog(&forwarded.catalog),
                nearest: (!known)
                    .then(|| contracts.nearest_catalog_key(&forwarded.catalog, key))
                    .flatten()
                    .map(Box::from),
            });
        }
    }

    /// The same, for a key written as an array entry rather than passed to a call.
    ///
    /// `['#type' => 'textfield']` names a registered element with nothing in the source but the key
    /// beside it, so there is no receiver to resolve and no hierarchy to walk -- the key is the whole
    /// contract.
    fn element_uses(
        &self,
        hir: &phrost_hir::FileHir,
        text: &str,
        contracts: &phrost_index::ReturnContracts,
    ) -> Vec<CatalogUse> {
        // The cheap gate first: a project with no framework has bound no array key, and finding the
        // entries means walking the file's expressions.
        if !contracts.binds_any_array_key() {
            return Vec::new();
        }

        let mut out = Vec::new();
        for element in phrost_body::keyed_elements(hir) {
            let catalogs = contracts.array_key_catalogs(element.key.as_str());
            if catalogs.is_empty() {
                continue;
            }
            // A value that is not a literal is not a key written wrong -- it is a key computed, and
            // there is nothing here to be right or wrong about.
            let Some(key) = unquoted(text, element.value) else {
                continue;
            };
            for catalog in catalogs {
                let known = contracts.catalog_holds(catalog, key);
                out.push(CatalogUse {
                    key: key.into(),
                    range: element.value,
                    catalog: catalog.clone(),
                    known,
                    closed: contracts.is_closed_catalog(catalog),
                    nearest: (!known)
                        .then(|| contracts.nearest_catalog_key(catalog, key))
                        .flatten()
                        .map(Box::from),
                });
            }
        }
        out
    }
}

/// What should have been written at an argument position whose key the catalog does not hold.
///
/// In the argument's own spelling, which is the only spelling a person can act on. For an unsuffixed
/// binding that is just the nearest registered key; a suffixed one needs two more things.
///
/// The suffix has to come *off* the suggestion, and a candidate that does not end with it is
/// declined rather than offered -- a hook one edit from `frm_alter` that is not itself an `_alter`
/// hook cannot be reached by writing anything at that position. This is the direction the
/// hook-name matcher's own note is about: a reader answering *which is it* has to decline where one
/// answering *is this reachable* may guess.
///
/// One consequence is worth stating because it is not obvious: the near-miss floor is applied to the
/// *suffixed* key, so `alter('frm')` is compared at nine characters while the part a person could
/// have misspelt is three. The floor exists because a four-character key one edit from another is
/// more likely a different key -- and what makes that safe here is the decline above: the candidates
/// are only the keys carrying the suffix, which is a far smaller set than the catalog, and the stem
/// is compared against stems. Measured on the reference project, whose one literal `alter` call and
/// whose `alter('form')`, `alter('js')`, `alter('css')` shapes all stay silent.
///
/// And a suffix written twice is caught exactly, with no edit distance at all: `alter('form_alter')`
/// asks for `hook_form_alter_alter`, which nothing will ever be, and the mistake is common enough
/// that a person writes it while knowing the hook's real name. What settles it is that the argument
/// *as written* is a key of the catalog -- so the fix is to take the suffix off, and there is nothing
/// to guess.
fn meant_instead(
    contracts: &phrost_index::ReturnContracts,
    catalog: &str,
    written: &str,
    suffix: &str,
) -> Option<Box<str>> {
    if suffix.is_empty() {
        return contracts
            .nearest_catalog_key(catalog, written)
            .map(Box::from);
    }

    if let Some(stem) = written.strip_suffix(suffix)
        && !stem.is_empty()
        && contracts.catalog_holds(catalog, written)
    {
        return Some(stem.into());
    }

    let key = format!("{written}{suffix}");
    let near = contracts.nearest_catalog_key(catalog, &key)?;
    near.strip_suffix(suffix).map(Box::from)
}

/// The literal written at one argument position, with the range a report would underline.
///
/// `None` when that argument is absent, is written by name or spread -- the same rule the body layer
/// follows when it keeps a call's literals -- or is not a literal at all, which is how an argument
/// that is a variable, a constant or a concatenation is left alone.
pub(crate) fn written_key<'t>(
    text: &'t str,
    arguments: &[phrost_body::ArgumentSite],
    at: u16,
) -> Option<(TextRange, &'t str)> {
    let argument = arguments.get(usize::from(at))?;
    if argument.name.is_some() || argument.spread {
        return None;
    }
    // A named or spread argument *earlier* in the list makes every position after it a guess: PHP
    // lets a name fill any slot and a spread fill several, so counting no longer says which is which.
    if arguments[..usize::from(at)]
        .iter()
        .any(|before| before.name.is_some() || before.spread)
    {
        return None;
    }
    Some((argument.range, unquoted(text, argument.range)?))
}

/// Every key written into one argument: the literal it is, or each literal operand of a
/// conditional it is.
///
/// `get($former ? 'field_a' : 'field_b')` may be given either, so both are uses -- which is what
/// the cursor already read them as, since it counts which argument a string sits in rather than
/// requiring the argument to be the string. Nested conditionals are followed a few deep; an operand
/// that is not a literal is no key, and `?:`'s missing middle is the condition, which is not one
/// either.
fn written_keys<'t>(
    hir: &phrost_hir::FileHir,
    text: &'t str,
    arguments: &[phrost_body::ArgumentSite],
    at: u16,
) -> Vec<(TextRange, &'t str)> {
    if let Some(whole) = written_key(text, arguments, at) {
        return vec![whole];
    }
    let mut keys = Vec::new();
    // The same refusals the whole-argument reading makes, so the two cannot disagree about which
    // argument this is.
    let Some(argument) = arguments.get(usize::from(at)) else {
        return keys;
    };
    if arguments[..=usize::from(at)]
        .iter()
        .any(|before| before.name.is_some() || before.spread)
    {
        return keys;
    }
    let mut pending = vec![(argument.value, 0u8)];
    while let Some((expr, depth)) = pending.pop() {
        match hir.expr(expr) {
            phrost_hir::body::ExprData::Literal { range, .. } => {
                if let Some(written) = unquoted(text, *range) {
                    keys.push((*range, written));
                }
            }
            phrost_hir::body::ExprData::Conditional {
                then_branch,
                else_branch,
                ..
            } if depth < CONDITIONAL_DEPTH => {
                pending.push((*else_branch, depth + 1));
                pending.extend(then_branch.map(|then| (then, depth + 1)));
            }
            _ => {}
        }
    }
    keys.sort_by_key(|(range, _)| range.start());
    keys
}

/// How deep a conditional inside a conditional is followed for the keys it may give.
const CONDITIONAL_DEPTH: u8 = 4;

/// The text of a single-quoted or double-quoted literal, without its delimiters.
///
/// `None` for anything that is not one, which is how an argument that is a variable, a constant or a
/// concatenation is left alone: there is no key written there to be right or wrong about.
fn unquoted(text: &str, range: TextRange) -> Option<&str> {
    let written = text.get(usize::from(range.start())..usize::from(range.end()))?;
    let quote = written.chars().next()?;
    if quote != '\'' && quote != '"' {
        return None;
    }
    let inner = written.strip_prefix(quote)?.strip_suffix(quote)?;
    // An escape means the key is not what is written between the quotes, and working out what it
    // would be is the lexer's job rather than this one's.
    (!inner.contains('\\')).then_some(inner)
}
