//! Where the current file reaches into a value with `[...]` that holds no elements.
//!
//! The sibling of [`MemberAccessOutcome::NotAnObject`](crate::MemberAccessOutcome::NotAnObject),
//! and the same shape of claim: not an absence of knowledge but proof. `$n['key']` where `$n` is an
//! `int` cannot work on any run, and PHP 8 says so -- `Trying to access array offset on int`.
//!
//! Only the kinds that settle it, which is a shorter list than the member check's. A `string` *is*
//! subscriptable -- `$s[0]` is a character -- and so is anything a class inhabits, because an
//! `ArrayAccess` implementer at any depth is an object here and objects are not reported. `mixed`,
//! `iterable`, `object`, `callable` and a union all leave the question open, and `null` belongs to
//! the null check, which knows how the value got there.

use phrost_body::SubscriptSite;
use phrost_model::{BuiltinTy, Name, TextRange};

use crate::{
    references::SentinelReach,
    view::{FOLLOW_LIMIT, IndexView},
};

/// One `$x['k']` whose container declares that the key may not be there.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct AbsentKey {
    /// The whole `$rows['k']`, which is what a report underlines and what a `??` would wrap.
    pub range: TextRange,
    pub key: Name,
    pub absence: Absence,
}

/// Which of the two declarations said the key may be absent.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Absence {
    /// `array{}` is one of the shapes the container can be, so on that arm no key is there.
    EveryKey,
    /// The shape writes this one key `key?:`.
    ThisKey,
}

/// What one arm of a container's type says about a key.
enum Declares {
    /// A shape that lists the key with no `?`.
    Always,
    /// A shape that writes it `key?:`.
    Optional,
    /// `array{}` -- a shape that lists nothing, so every key is absent on this arm.
    Nothing,
    /// A populated shape that does not list the key, a bare array, or not an array at all. The
    /// first of those is deliberate: a shape is a claim about the keys it lists.
    Silent,
}

/// One `[...]` read from a value that may not be there at all.
///
/// The sibling of [`NonArraySubscript`] and the answer to what its own note promised: that one
/// steps aside for a value that may be null, on the ground that the null "belongs to the null
/// check" -- and the null check walks member *accesses*, so `$row['vin']` on a `?array` belonged to
/// nobody. `$row->vin()` on the same value was reported the whole time, which is the tell: one
/// spelling of one mistake answered and the other did not.
///
/// Both sentinels, for the reason the argument check states: `json_decode($body, true)` hands back
/// `array|false` as surely as another call hands back `?array`, and `$decoded['id']` on it reads an
/// offset off a boolean -- which PHP warns about and evaluates as null, exactly as it does for the
/// null.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct NullSubscript {
    /// The value on the left, which is what a report underlines and what a guard has to go around.
    pub range: TextRange,
    /// The whole `$rows['k']`, which is what a fix has to wrap: `?? null` defaults the *read*, and
    /// putting it on the base alone would default the array instead.
    pub whole: TextRange,
    /// The key as a literal wrote it, so a report can name the slot being read.
    pub key: Option<Name>,
    /// What the value holds, rendered -- `?array{vin: string}`. The evidence for the claim, on the
    /// line the claim sits on.
    pub rendered: Option<String>,
    /// Whether the sentinel arrives on every run rather than on some.
    pub certain: bool,
    /// Which sentinel: [`phrost_model::BuiltinTy::Null`] or [`phrost_model::BuiltinTy::False`].
    pub sentinel: phrost_model::BuiltinTy,
}

/// The shortest key a near miss is looked for at all.
///
/// Lower than the catalog's five, because a shape's keys are short by nature -- `id`, `name`,
/// `type`, `#title` -- where a service id or a route name is a dotted path. What the floor is for is
/// the same: below it, one edit is more likely a different key than a misspelling, and the set being
/// compared against is a handful of declared fields rather than a thousand registered ids, so the
/// chance of an accidental neighbour is smaller at the same length.
const NEAR_MISS_FLOOR: usize = 4;

/// One `[...]` whose key is a near miss of one the shape declares.
///
/// Not "a key the shape does not hold", which is a different and unsafe claim: a docblock shape may
/// be written open (`array{id: int, ...}`), a key this layer cannot read is skipped rather than
/// failing, and either leaves a shape that states *some* of what is there. What separates a mistake
/// from an absence is the same thing it separates for a catalog key -- **a typo nearly hits**.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct MisspeltKey {
    /// The key as written, which is what a report underlines.
    pub range: TextRange,
    pub key: Name,
    /// The key it is one edit from.
    pub nearest: String,
}

/// One `[...]` reading a string at a key no string offset can be.
///
/// `$s['key']` is `Cannot access offset of type string on string` -- a `TypeError`, not the warning
/// a subscripted `int` gives -- and it is the mistake [`holds_no_elements`] leaves out by name.
/// What makes it certain is the *literal*: PHP accepts a numeric string, so `$s['1']` and `$s[1]`
/// both read a character, and a key written as a variable may be either at runtime.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct BadStringOffset {
    /// The key as written, which is what a report underlines.
    pub range: TextRange,
    pub key: Name,
}

/// One `[...]` read from a value no element can be in.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct NonArraySubscript {
    /// The value on the left, which is what has the wrong type.
    pub range: TextRange,
    /// The key as a literal wrote it, for a report that can name the slot.
    pub key: Option<Name>,
    pub kind: BuiltinTy,
}

impl IndexView<'_> {
    /// Every element read in the current file from a value that holds no elements.
    ///
    /// Empty when the file is not open, which is the shape of the two tiers rather than a fallback:
    /// a subscript lives inside a body, and bodies exist for open buffers only.
    pub fn non_array_subscripts(&self) -> Vec<NonArraySubscript> {
        let Some(open) = self.current() else {
            return Vec::new();
        };
        let hir = open.hir();

        phrost_body::subscript_sites(hir)
            .into_iter()
            .filter_map(|site| self.settled(hir, site))
            .collect()
    }

    /// Every `$s['word']` in the current file: a string offset PHP refuses outright.
    ///
    /// Beside [`Self::non_array_subscripts`] rather than inside it, because the two are different
    /// claims with different severities -- that one is PHP's warning about a value that holds no
    /// elements, and this is a `TypeError` that stops the line.
    pub fn bad_string_offsets(&self) -> Vec<BadStringOffset> {
        let Some(open) = self.current() else {
            return Vec::new();
        };
        let hir = open.hir();

        phrost_body::subscript_sites(hir)
            .into_iter()
            .filter_map(|site| {
                if !site.base_is_local {
                    return None;
                }
                let key = site.key.clone()?;
                // A numeric key is a character: `$s['1']` and `$s[1]` both read one. Only a key
                // PHP cannot read as an integer is the error.
                if key.as_str().parse::<i64>().is_ok() {
                    return None;
                }
                let held = self.resolve_path(site.range.start(), hir, &site.base, FOLLOW_LIMIT)?;
                // A sentinel belongs to the sentinel check, the same rule `settled` follows.
                if !matches!(self.null_reach_of(hir, &held), SentinelReach::Impossible)
                    || !matches!(self.false_reach_of(hir, &held), SentinelReach::Impossible)
                {
                    return None;
                }
                (self.kind_of_held(hir, &held) == Some(BuiltinTy::String)).then_some(
                    BadStringOffset {
                        range: site.range,
                        key,
                    },
                )
            })
            .collect()
    }

    /// Every element read in the current file from a value that may be null.
    ///
    /// The same sites [`Self::non_array_subscripts`] walks, which is what keeps the two disjoint:
    /// that one gives up the moment null is possible and this one answers only then. A site the
    /// body layer already excluded -- a write, an `isset`, a `??` -- is in neither, because a
    /// presence test is willing to find nothing there.
    pub fn null_subscripts(&self) -> Vec<NullSubscript> {
        let Some(open) = self.current() else {
            return Vec::new();
        };
        let hir = open.hir();

        phrost_body::subscript_sites(hir)
            .into_iter()
            .filter_map(|site| {
                let held = self.resolve_path(
                    site.range.start(),
                    hir,
                    &site.base,
                    crate::view::FOLLOW_LIMIT,
                )?;
                // `mixed` says `Possible` without meaning it: PHP has no `?mixed`, so a
                // declaration spells `mixed|null` for what is already every type. The same
                // exception `states_non_null` makes, and `not_iterable` beside it.
                if self.kind_of_held(hir, &held) == Some(phrost_model::BuiltinTy::Mixed) {
                    return None;
                }
                // Null first: a value that may be either is one mistake, and the guard a reader
                // writes for it handles both.
                let (sentinel, certain) = [
                    phrost_model::BuiltinTy::Null,
                    phrost_model::BuiltinTy::False,
                ]
                .into_iter()
                .find_map(|sentinel| {
                    let reach = match sentinel {
                        phrost_model::BuiltinTy::Null => self.null_reach_of(hir, &held),
                        _ => self.false_reach_of(hir, &held),
                    };
                    match reach {
                        SentinelReach::Impossible => None,
                        // Certain only for a local, whose every write is in the body that reads it.
                        // A property declared `= null` is written by whoever holds the object -- a
                        // closure, a setter, another file -- so "null on every run" is a claim this
                        // file has not seen enough to make, and a test that fills one inside a
                        // callback was reported for the value the callback had just put there.
                        SentinelReach::Certain => Some((sentinel, site.base_is_local)),
                        SentinelReach::Possible => Some((sentinel, false)),
                    }
                })?;
                Some(NullSubscript {
                    range: site.base_range,
                    whole: site.range,
                    key: site.key,
                    rendered: self.render_held(
                        site.base_range.start(),
                        hir,
                        &held,
                        crate::variables::Naming::Short,
                    ),
                    certain,
                    sentinel,
                })
            })
            .collect()
    }

    /// Every element read in the current file whose key is a near miss of a declared one.
    ///
    /// The same sites the other two walk. A key that *is* declared says nothing here, and neither
    /// does one nothing is close to: an unlisted key may be a key the shape did not bother to
    /// write, and reporting it would argue with every open shape in a project.
    pub fn misspelt_keys(&self) -> Vec<MisspeltKey> {
        let Some(open) = self.current() else {
            return Vec::new();
        };
        let hir = open.hir();

        phrost_body::subscript_sites(hir)
            .into_iter()
            .filter_map(|site| self.misspelt_key(hir, &site))
            .collect()
    }

    fn misspelt_key(
        &self,
        hir: &phrost_hir::FileHir,
        site: &phrost_body::SubscriptSite,
    ) -> Option<MisspeltKey> {
        let key = site.key.clone()?;
        if key.as_str().len() < NEAR_MISS_FLOOR {
            return None;
        }
        let held = self.resolve_path(site.range.start(), hir, &site.base, FOLLOW_LIMIT)?;
        let declared = self.shape_keys(hir, &held)?;
        if declared.iter().any(|known| known == key.as_str()) {
            return None;
        }

        // Nothing when two are equally close: naming one would be a guess, and a reader shown two
        // suggestions has learned nothing. The same rule the catalog's near miss follows.
        let mut best: Option<(String, usize)> = None;
        let mut tied = false;
        for candidate in declared {
            let Some(distance) =
                phrost_index::edits_within(key.as_str(), &candidate, phrost_index::NEAR_MISS_EDITS)
            else {
                continue;
            };
            match &best {
                Some((_, held)) if distance > *held => {}
                Some((_, held)) if distance == *held => tied = true,
                _ => {
                    best = Some((candidate, distance));
                    tied = false;
                }
            }
        }
        let (nearest, _) = best.filter(|_| !tied)?;

        Some(MisspeltKey {
            range: site.range,
            key,
            nearest,
        })
    }

    /// Every `$x['k']` whose container declares that `'k'` may not be there.
    ///
    /// Two declarations say it and nothing else does. `array{}` as one arm of a union lists no keys
    /// at all, so on that arm reading any key is null -- and a project writes `array{...}|array{}`
    /// precisely to mean "the whole thing, or nothing". `array{data?: string}` says the same about
    /// one key. Both are the author stating the absence, which is what separates them from a
    /// populated shape that simply does not mention a key: that is a claim about the keys it lists
    /// and says nothing about the rest, which is the position [`Self::misspelt_keys`] answers under
    /// and which this keeps.
    ///
    /// One finding per key rather than per container, because each key is its own question --
    /// `keyed_subscript_sites` is that rule. A read the code already handles is not one: a write
    /// target, an `isset`, a `??` and the rest are gone before the sites arrive, which is what
    /// makes the check liveable on a project that reads optional keys on purpose.
    pub fn absent_keys(&self) -> Vec<AbsentKey> {
        let Some(open) = self.current() else {
            return Vec::new();
        };
        let hir = open.hir();

        let tested = phrost_body::tested_keys(hir);
        phrost_body::keyed_subscript_sites(hir)
            .into_iter()
            .filter_map(|site| self.absent_key(hir, &site, &tested))
            .collect()
    }

    fn absent_key(
        &self,
        hir: &phrost_hir::FileHir,
        site: &SubscriptSite,
        tested: &[phrost_body::TestedKey],
    ) -> Option<AbsentKey> {
        let key = site.key.clone()?;
        if tested.iter().any(|test| {
            test.within.contains_range(site.range) && test.key == key && test.base == site.base
        }) {
            return None;
        }
        if Self::is_superglobal(&site.base) {
            return None;
        }
        let at = site.range.start();
        let held = self.resolve_path(at, hir, &site.base, FOLLOW_LIMIT)?;
        // A lone shape is one arm; a union is its operands. `union_operands` is what knows all
        // three spellings a union arrives in, which is why the question is asked through it rather
        // than by matching the two type tables here.
        let arms = match self.union_operands(at, hir, &held) {
            Some(operands) => operands.into_iter().map(|operand| operand.held).collect(),
            None => vec![held],
        };
        let said = arms
            .iter()
            .map(|arm| self.declares(hir, arm, key.as_str()))
            .collect::<Vec<_>>();
        // An empty arm only speaks where another arm names the key. `array{a: string}|array{}` is
        // "the whole thing, or nothing" and reading `a` off it is the read this check exists for --
        // while a key *no* arm declares is a shape that may simply be incomplete, which is the
        // position `misspelt-key` answers under and which the empty arm must not overturn.
        let named = said.iter().any(|arm| matches!(arm, Declares::Always));
        said.iter()
            .find_map(|arm| match arm {
                Declares::Nothing if named => Some(Absence::EveryKey),
                Declares::Optional => Some(Absence::ThisKey),
                Declares::Nothing | Declares::Always | Declares::Silent => None,
            })
            .map(|absence| AbsentKey {
                range: site.range,
                key,
                absence,
            })
    }

    /// Whether the value being read is one of PHP's own superglobals.
    ///
    /// Their shapes declare nearly every key optional, and rightly: `$_SERVER['REQUEST_URI']` is
    /// absent on the command line and `$_SERVER['argv']` absent under a web server. But the shape
    /// is the *runtime's* documentation of what it may provide, not a claim the author made -- and
    /// this check reports what an author declared about their own data. Measured before the rule
    /// existed: 105 of WordPress's 117 findings were `REQUEST_URI`, `HTTP_HOST` and their
    /// neighbours, which is what a check nobody would leave on looks like.
    fn is_superglobal(base: &phrost_body::ValuePath) -> bool {
        const SUPERGLOBALS: &[&str] = &[
            "_SERVER", "_GET", "_POST", "_COOKIE", "_FILES", "_ENV", "_REQUEST", "_SESSION",
            "GLOBALS",
        ];
        matches!(&base.root, phrost_body::PathRoot::Local(name)
            if SUPERGLOBALS.contains(&name.as_str()))
    }

    /// What one arm of a container's type says about a key.
    fn declares(
        &self,
        hir: &phrost_hir::FileHir,
        held: &crate::resolve::Held,
        key: &str,
    ) -> Declares {
        let fields: Vec<(String, bool)> = match held {
            crate::resolve::Held::Written(id) => match hir.type_ref(*id) {
                phrost_hir::TypeRefData::Shape { fields, .. } => hir
                    .type_ref_list(*fields)
                    .iter()
                    .filter_map(|field| match hir.type_ref(*field) {
                        phrost_hir::TypeRefData::ShapeField { key, optional, .. } => {
                            Some((key.as_str().to_owned(), *optional))
                        }
                        _ => None,
                    })
                    .collect(),
                _ => return Declares::Silent,
            },
            crate::resolve::Held::Stored { ty, .. } => {
                let types = self.skeleton.types();
                match types.get(*ty) {
                    phrost_ty::TypeRef::Shape { fields, .. } => types
                        .operands(*fields)
                        .to_vec()
                        .iter()
                        .filter_map(|field| match types.get(*field) {
                            phrost_ty::TypeRef::Field { key, optional, .. } => {
                                Some((key.as_str().to_owned(), *optional))
                            }
                            _ => None,
                        })
                        .collect(),
                    _ => return Declares::Silent,
                }
            }
            _ => return Declares::Silent,
        };
        if fields.is_empty() {
            return Declares::Nothing;
        }
        match fields.iter().find(|(name, _)| name == key) {
            Some((_, true)) => Declares::Optional,
            Some((_, false)) => Declares::Always,
            None => Declares::Silent,
        }
    }

    /// The keys a value's type declares, when it declares any.
    ///
    /// `None` for anything that is not a shape: a bare array says nothing about its keys, and a
    /// union of shapes says different things on each side -- so the question "is this key one of
    /// the declared ones" has no single answer to compare against.
    fn shape_keys(
        &self,
        hir: &phrost_hir::FileHir,
        held: &crate::resolve::Held,
    ) -> Option<Vec<String>> {
        match held {
            crate::resolve::Held::Written(id) => match hir.type_ref(*id) {
                phrost_hir::TypeRefData::Shape { fields, .. } => Some(
                    hir.type_ref_list(*fields)
                        .iter()
                        .filter_map(|field| match hir.type_ref(*field) {
                            phrost_hir::TypeRefData::ShapeField { key, .. } => {
                                Some(key.as_str().to_owned())
                            }
                            _ => None,
                        })
                        .collect(),
                ),
                _ => None,
            },
            crate::resolve::Held::Stored { ty, .. } => {
                let types = self.skeleton.types();
                match types.get(*ty) {
                    phrost_ty::TypeRef::Shape { fields, .. } => Some(
                        types
                            .operands(*fields)
                            .iter()
                            .filter_map(|field| match types.get(*field) {
                                phrost_ty::TypeRef::Field { key, .. } => {
                                    Some(key.as_str().to_owned())
                                }
                                _ => None,
                            })
                            .collect(),
                    ),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    /// One site, when its base resolves to something proven to hold no elements.
    fn settled(&self, hir: &phrost_hir::FileHir, site: SubscriptSite) -> Option<NonArraySubscript> {
        // Only a local, for the reason `SubscriptSite::base_is_local` gives: a kind read off an
        // untyped property's initializer is a starting value rather than a claim, and judging one
        // reports the shape WordPress's `public $update = false;` is written in.
        if !site.base_is_local {
            return None;
        }
        let held = self.resolve_path(site.range.start(), hir, &site.base, FOLLOW_LIMIT)?;
        // A value that may be a sentinel belongs to the sentinel check whatever else is true of
        // it: `?array` is a null nobody handled and `array|false` is a call nobody checked, not
        // types that cannot be subscripted, and saying both is two reports for one mistake.
        if !matches!(self.null_reach_of(hir, &held), SentinelReach::Impossible)
            || !matches!(self.false_reach_of(hir, &held), SentinelReach::Impossible)
        {
            return None;
        }

        Some(NonArraySubscript {
            range: site.base_range,
            key: site.key,
            kind: self.kind_of_held(hir, &held).filter(holds_no_elements)?,
        })
    }
}

/// Whether a kind of value is one no element can be read from.
///
/// The variants are named rather than matched with a catch-all, because the vocabulary is closed by
/// convention rather than by the type system: a new `BuiltinTy` compiles clean everywhere, and
/// falling into *this* list would turn every subscript of it into a report.
///
/// `String` is deliberately absent -- `$s[0]` is a character, and `$s['k']` is a separate mistake
/// with a separate message. So are `Void` and `Never`, whose appearance means something has already
/// gone wrong further up and the second report would be about the first one's cause.
///
/// So are `Null` and `False`, which the sentinel check owns: both are what a call hands back to mean
/// *not there* rather than a kind somebody declared, and both are reached by a reader who forgot to
/// look rather than by one who thought the value was an array.
fn holds_no_elements(kind: &BuiltinTy) -> bool {
    matches!(
        kind,
        BuiltinTy::Int | BuiltinTy::Float | BuiltinTy::Bool | BuiltinTy::True
    )
}
