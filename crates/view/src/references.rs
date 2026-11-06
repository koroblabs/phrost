//! Where the current file names a member, and whether that name resolves.
//!
//! A whole-file projection rather than a position query: everything else here answers "what is at
//! the cursor", and this answers "where are all the places a cursor could be, and what happens at
//! each". It is what anything that must visit every access needs -- reporting an undefined method,
//! listing where a member is used, or measuring how much of a real project resolves at all.

use std::sync::Arc;

use phrost_body::SiteKind;
use phrost_model::{BuiltinTy, Name, TextRange, TextSize};
use phrost_sema::MemberKind;

use crate::{
    resolve::Held,
    view::{FOLLOW_LIMIT, IndexView},
};

/// One member access written in the file, and what the index makes of it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct MemberAccess {
    /// The member name alone -- what a cursor lands on and what a diagnostic underlines.
    pub range: TextRange,
    /// The receiver as written, for a report that has to say which expression had no type.
    pub receiver_range: TextRange,
    pub name: Name,
    pub kind: MemberKind,
    pub outcome: MemberAccessOutcome,
    /// Whether null can arrive at this access.
    pub null_reach: SentinelReach,
    /// Whether `false` can -- `reset()` on an array nothing proved non-empty, an uncompared
    /// `strpos()`.
    ///
    /// Its own field rather than folded into the one above, because the two are not handled the
    /// same way: `?->` short-circuits on null and is a fatal `Call to a member function on false`,
    /// measured on PHP 8.5.10. So a nullsafe access silences one and not the other.
    pub false_reach: SentinelReach,
    /// The member is named by a string here rather than reached -- see `MemberSite::named`.
    pub named: bool,
    /// A property `isset()`, `empty()` or the left of `??` asks about.
    ///
    /// PHP reads one there without a word whether or not it exists or is visible -- `isset($f->x)`
    /// on a class declaring no `$x` is `false`, and on a private one from outside is `false` too,
    /// run against 8.5.10 -- so the access *is* the existence check, and a report about it argues
    /// with the guard. A method call inside is still a call and is evaluated, so this is properties
    /// only.
    pub asked_if_set: bool,
}

/// Whether a sentinel -- a value standing for *not there* -- can arrive at one access.
///
/// `null` is the one almost every reader asks about; `false` is the other, and PHP's standard
/// library hands it back for exactly the same purpose. One enum and one walk, because two would
/// drift and a name saying only "null" would lie at the second caller.
///
/// Beside the outcome rather than inside it, because both answers are true at once and they are
/// wanted by different readers. `$app->getId()` on a `?Application` *resolves* -- the member exists,
/// so goto lands, rename renames, and coverage counts it understood -- and it is also a fatal error
/// on the runs where the value is null. Folding the two into one enum would force every reader to
/// care about the other one's question.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SentinelReach {
    /// It cannot: the type holds no such value, a guard ruled it out, or the access is written
    /// `?->`, which handles null itself.
    Impossible,
    /// It can: the type includes it and nothing here ruled it out.
    Possible,
    /// It always does -- the value is that sentinel and nothing else.
    Certain,
}

impl SentinelReach {
    /// The same answer with certainty taken off it.
    ///
    /// For a sentinel read through a `?T` wrapper: the wrapper is one alternative and what is
    /// inside it is the other, so a `false` that is certain *inside* a `?(Row|false)` is only
    /// possible out here -- the value may be null instead.
    pub fn at_most_possible(self) -> Self {
        match self {
            Self::Certain => Self::Possible,
            reach => reach,
        }
    }
}

/// How far resolving an access got.
///
/// Four outcomes rather than a boolean, because they mean entirely different things and only one of
/// them is a defect in the code being read. Reporting "undefined method" for any of the other three
/// is how a checker earns its reputation for noise.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum MemberAccessOutcome {
    /// The member was found on the receiver's class.
    Resolved { class: Name },
    /// The receiver's type is not known here -- an untyped property, an untyped return, a shape
    /// nothing models yet. Says nothing about whether the member exists.
    UnknownReceiver,
    /// The receiver's type *is* known and it is not an object: `$name->format()` on a string.
    ///
    /// The opposite of [`Self::UnknownReceiver`] and kept apart from it for that reason. One is an
    /// absence of knowledge and the other is proof: PHP raises `Call to a member function on string`
    /// and stops, so this is a branch that cannot work rather than one nothing can be said about.
    ///
    /// Only the kinds that settle it. `object`, `mixed`, `iterable` and `callable` are all types an
    /// object inhabits, and `null` belongs to the null check, which knows how the value got there.
    NotAnObject { kind: BuiltinTy },
    /// The receiver names a class the index does not have, so its members are unknown.
    UnknownClass { class: Name },
    /// The class is indexed, but flattening it hit a parent, trait, or interface that is not --
    /// so the member may well exist and simply be out of reach.
    ///
    /// Separate from [`Self::MissingMember`] because one unindexed base class turns every inherited
    /// member into an apparent defect. A test case extending an unresolved `UnitTestCase` reports
    /// every `assertSame` in it as undefined, which is a gap in the index wearing the costume of a
    /// hundred bugs in the code.
    IncompleteHierarchy { class: Name },
    /// The declaration's own members are not the whole answer, so a name that is not among them may
    /// still be there at runtime.
    ///
    /// Two causes and one consequence. A class with `__call` or `__get` answers names it never
    /// declared -- how much of Drupal's and Laravel's public surface works. And a **trait** is half a
    /// class body: `$this` inside it is whoever composed it, and which class that is, a trait cannot
    /// know. Laravel's `View\Concerns\ManagesStacks` reads `$this->renderedOnce` and calls
    /// `$this->getPushedContent()`, both declared by the `Factory` that uses it.
    ///
    /// An outcome and not a footnote because anything reporting an absent member has to stay quiet
    /// about a class whose members are not knowable from the declaration in hand.
    Dynamic { class: Name },
    /// The class is indexed, its hierarchy is complete, it declares no way to answer unknown names,
    /// and it has no member by that name. The only outcome a diagnostic may speak about.
    MissingMember { class: Name },
    /// The receiver is declared `object`, which is every class at once.
    ///
    /// Between [`Self::UnknownReceiver`] and [`Self::NotAnObject`] and neither of them. The index
    /// has an answer and the answer is that the author wrote "any object": no inference will ever
    /// narrow it, and unlike a `string` receiver nothing here is wrong either.
    ///
    /// Apart from `UnknownReceiver` because the coverage sweep ranks the next piece of work by what
    /// it counts, and 345 accesses off `@param object $application` in one generated directory were
    /// sitting at the top of a list of gaps to close while being no gap at all.
    ///
    /// `mixed` is deliberately *not* here. It reads the same from outside and does not mean the same
    /// thing: a `mixed` is as often a stub whose docblock could say more -- `current()` answered
    /// `mixed` until its `@return` was read properly -- and a bucket named unfixable is the wrong
    /// place to hide that.
    AnyObject,
    /// A property an `object{...}` shape on the receiver names: `->pivot` on what Laravel's
    /// `BelongsToMany` loads. Answered, though by no class's table -- and kept apart from
    /// [`Self::AnyObject`] so that nothing reporting a member `object` does not have reports one the
    /// shape states.
    ShapeProperty,
}

/// How many of a class's direct subtypes *from outside the project* the template-method rule
/// looks at, matching goto-implementation's bound. The project's own are all looked at: a member a
/// subclass declares is the whole question here, and the subclass a person wrote is the one most
/// likely to declare it.
const SUBTYPES_ASKED: usize = 100;

/// How PHP reaches a member an access names.
///
/// Three answers rather than a bool, because the middle one is a whole case and used to be folded
/// into "fine": `$router->prefix('foz')` on Laravel's `Router` names a **protected** method, and PHP
/// sends it to `__call` instead -- which forwards to a registrar and hands back something else
/// entirely. Reading the declaration there answered `string`, and the next step in the chain was
/// then reported as a call on one.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum MemberReach {
    /// The declaration is what runs. Also the answer for a name no table holds, which is a
    /// different question and not this one's.
    Declared,
    /// `__call`/`__get` runs instead, so the declaration says nothing about the value.
    Magic,
    /// Nothing runs: PHP raises an error. The visibility the declaration wrote, for the report.
    Refused(phrost_model::Visibility),
}

impl IndexView<'_> {
    /// The text of a range in the current buffer.
    ///
    /// For an edit that has to know what it is replacing: an offer that assumed a `->` where a `?->`
    /// is written would produce `??->`.
    pub fn text_in(&self, range: TextRange) -> Option<&str> {
        self.current()?
            .text()
            .get(usize::from(range.start())..usize::from(range.end()))
    }

    /// Whether a call's parentheses are already written after `offset`.
    ///
    /// For a completion that would add its own: re-completing the name in `$app->getFoo(1)` must not
    /// leave `getFoo()(1)`. Whitespace is stepped over, because `getFoo ()` is legal and means the
    /// same call.
    pub fn opens_call_after(&self, offset: TextSize) -> bool {
        let Some(open) = self.current() else {
            return false;
        };
        open.text()
            .get(usize::from(offset)..)
            .is_some_and(|rest| rest.trim_start().starts_with('('))
    }

    /// The member operator of an access -- `->`, `?->` or `::` -- as written.
    ///
    /// Found in the gap between the receiver and the member name, which holds nothing else: the
    /// operator, whitespace, and possibly a comment. `$node  ->  getTitle()` is legal PHP, so the
    /// position cannot be derived by arithmetic from either range.
    ///
    /// Asked for rather than carried on [`MemberAccess`], because only an edit needs it and the
    /// whole-file projection is walked on every keystroke.
    pub fn operator_of(&self, access: &MemberAccess) -> Option<TextRange> {
        let text = self.current()?.text();
        let start = usize::from(access.receiver_range.end());
        let end = usize::from(access.range.start());
        let gap = text.get(start..end)?;

        // The last one, because a comment in the gap could contain the characters: `$a /* -> */ ->b`.
        for operator in ["?->", "->", "::"] {
            if let Some(at) = gap.rfind(operator) {
                let from = TextSize::from(u32::try_from(start + at).ok()?);
                return Some(TextRange::at(
                    from,
                    TextSize::from(u32::try_from(operator.len()).ok()?),
                ));
            }
        }
        None
    }

    /// Whether the author wrote a runtime check that the member reached at `at` exists.
    ///
    /// `if (method_exists($this, 'getWarranty')) { $this->getWarranty(); }` is the only way PHP has of
    /// saying "this may be on a subclass", so a report about the call inside is a report about the
    /// guard. Asked by whoever is about to speak, not computed for every access: it walks the scope
    /// chain, and on a large file almost no access needs it.
    ///
    /// The name comes back as a range because it is the check's second argument and only the first is
    /// carried in the tree -- so the comparison happens here, where the buffer is.
    pub fn member_existence_checked(&self, at: phrost_model::TextSize, member: &str) -> bool {
        let Some(open) = self.current() else {
            return false;
        };
        let hir = open.hir();
        let Some(site) = phrost_body::member_site_at(hir, at) else {
            return false;
        };

        phrost_body::existence_checked(hir, &site)
            .into_iter()
            .filter_map(|range| {
                open.text()
                    .get(usize::from(range.start())..usize::from(range.end()))
            })
            .any(|literal| literal.trim_matches(['\'', '"']) == member)
    }

    /// Whether the file asks anywhere whether this class constant is declared.
    ///
    /// `defined('Normalizer::NFKC_CF')` is PHP's only way of asking, and Symfony's
    /// `AbstractUnicodeString` writes exactly that -- the constant is in the intl extension and not
    /// in the polyfill class that stands in for it when the extension is absent, which is precisely
    /// what the guard is about.
    ///
    /// Per file rather than per branch, which is the trade `existence_tests` states and the same
    /// one it makes for `class_exists`: the guarded use is often nowhere near the test.
    pub fn class_constant_existence_checked(&self, class: &str, name: &str) -> bool {
        let Some(open) = self.current() else {
            return false;
        };
        phrost_body::existence_tests(open.hir()).tests_class_constant(class, name)
    }

    /// Every existence test the current file writes. See [`crate::view::IndexView::existence`].
    pub(crate) fn existence_tests(&self) -> Option<Arc<phrost_body::ExistenceTests>> {
        if let Some(built) = self.existence.borrow().as_ref() {
            return Some(Arc::clone(built));
        }
        let built = Arc::new(phrost_body::existence_tests(self.current()?.hir()));
        *self.existence.borrow_mut() = Some(Arc::clone(&built));
        Some(built)
    }

    /// Every property name the current file assigns to, in source order, deduplicated.
    ///
    /// Memoized for the reason [`Self::member_accesses`] is, and asked far more often: every
    /// resolution of a value reached through a path consults it before deciding whether a write
    /// could have settled that slot.
    pub(crate) fn assigned_properties(
        &self,
        hir: &phrost_hir::FileHir,
    ) -> Arc<[phrost_model::Name]> {
        if let Some(built) = self.assigned.borrow().as_ref() {
            return Arc::clone(built);
        }
        let built: Arc<[phrost_model::Name]> = phrost_body::assigned_properties(hir).into();
        *self.assigned.borrow_mut() = Some(Arc::clone(&built));
        built
    }

    /// Every member access written in the current file, in source order, each with its outcome.
    ///
    /// Empty when the file is not open, which is not a fallback but the shape of the two tiers: an
    /// access lives inside a body, and bodies exist only for open buffers.
    /// Every call the current file writes, walked once per view -- see [`IndexView::calls`].
    pub(crate) fn file_calls(&self) -> Arc<[phrost_body::CallSite]> {
        if let Some(built) = self.calls.borrow().as_ref() {
            return Arc::clone(built);
        }
        let built: Arc<[phrost_body::CallSite]> = self
            .current()
            .map(|open| phrost_body::call_sites(open.hir()))
            .unwrap_or_default()
            .into();
        *self.calls.borrow_mut() = Some(Arc::clone(&built));
        built
    }

    pub fn member_accesses(&self) -> Arc<[MemberAccess]> {
        if let Some(built) = self.accesses.borrow().as_ref() {
            return Arc::clone(built);
        }
        let built: Arc<[MemberAccess]> = self.walk_member_accesses().into();
        *self.accesses.borrow_mut() = Some(Arc::clone(&built));
        built
    }

    /// Every member access touching `within`, resolved and nothing else.
    ///
    /// Deliberately not a filter over [`Self::member_accesses`]. Resolving a receiver is a
    /// cross-file question, and doing it for every access in the file is the single most expensive
    /// thing a request can ask for -- so the sites are cut down to the range *before* any of them
    /// is resolved, not after.
    ///
    /// Measured: a code action at a cursor spent 94 % of its 7.3 ms proving there was no access
    /// there, because the only way to ask was to resolve the whole file.
    ///
    /// Touching counts, matching how a request about a cursor is meant: a cursor just after a name
    /// is still on it.
    pub fn member_accesses_in(&self, within: TextRange) -> Vec<MemberAccess> {
        let Some(open) = self.current() else {
            return Vec::new();
        };
        let hir = open.hir();

        let sites = self
            .member_sites(hir)
            .into_iter()
            .filter(|site| touches(site.name_range, within) || touches(site.receiver_range, within))
            .collect::<Vec<_>>();
        if sites.is_empty() {
            return Vec::new();
        }

        // Only now, and only if a property is among them: it is a walk of every expression in the
        // file, and the commonest answer here is one method access at a cursor.
        let suppressed = match sites
            .iter()
            .any(|site| matches!(site.kind, SiteKind::Property))
        {
            true => phrost_body::null_suppressed_ranges(hir),
            false => Vec::new(),
        };

        sites
            .into_iter()
            .map(|site| self.access_of(hir, &suppressed, site))
            .collect()
    }

    /// Every member access in the current file that names `name`, resolved and nothing else.
    ///
    /// The same cut [`Self::member_accesses_in`] makes, by name instead of by position, and for the
    /// same reason: resolving a receiver is a cross-file question, and a search for one member has
    /// no use for the other two hundred accesses a file writes. On a project-wide sweep that is the
    /// difference between resolving every access in every candidate file and resolving the handful
    /// that could possibly be the answer.
    ///
    /// As case-insensitively as PHP is, which is per kind: a **method** may be written in any case
    /// and reaches the same declaration, and a property, a class constant and an enum case may not.
    /// This used to be exact for all four, with a comment arguing that the callers compare the
    /// written name to a declaration's the same way -- which they do not. `member_uses` looks the
    /// declaration up by the *target's* name, so a differently-cased call resolves to it and was
    /// then dropped here, before anything could see it. Measured over four corpora: every
    /// `$this->assertInstanceof()` in Symfony's tests was missing from the references of
    /// `Assert::assertInstanceOf`, and so from the rename that has to rewrite it.
    pub fn member_accesses_named(&self, name: &str, kind: MemberKind) -> Vec<MemberAccess> {
        let Some(open) = self.current() else {
            return Vec::new();
        };
        let hir = open.hir();

        let folds_case = kind == MemberKind::Method;
        let sites = self
            .member_sites(hir)
            .into_iter()
            .filter(|site| match folds_case {
                true => site.name.as_str().eq_ignore_ascii_case(name),
                false => site.name.as_str() == name,
            })
            .collect::<Vec<_>>();
        if sites.is_empty() {
            return Vec::new();
        }

        let suppressed = match sites
            .iter()
            .any(|site| matches!(site.kind, SiteKind::Property))
        {
            true => phrost_body::null_suppressed_ranges(hir),
            false => Vec::new(),
        };

        sites
            .into_iter()
            .map(|site| {
                let presumed = site.presumed;
                (presumed, self.access_of(hir, &suppressed, site))
            })
            .filter(|(presumed, access)| Self::settles_a_presumed_site(*presumed, access))
            .map(|(_, access)| access)
            .collect()
    }

    /// The file's member sites, with the ones only a string names: a test double's configuring
    /// call (`stand_ins`) and a callable pair over a variable (`callable_strings`).
    ///
    /// One place for every walk over member sites, so the whole-file, ranged and by-name readers
    /// cannot disagree about whether a string is an access.
    pub(crate) fn member_sites(&self, hir: &phrost_hir::FileHir) -> Vec<phrost_body::MemberSite> {
        let mut sites = phrost_body::member_sites(hir);
        let named = self.stand_in_sites(hir, &sites);
        sites.extend(named);
        sites.extend(self.pair_sites(hir));
        sites
    }

    fn walk_member_accesses(&self) -> Vec<MemberAccess> {
        let Some(open) = self.current() else {
            return Vec::new();
        };
        let hir = open.hir();

        // Computed once for the file rather than per site: the walk is over every expression, and
        // an access asked about on its own has no surroundings to look at.
        let suppressed = phrost_body::null_suppressed_ranges(hir);

        self.member_sites(hir)
            .into_iter()
            .map(|site| {
                let presumed = site.presumed;
                (presumed, self.access_of(hir, &suppressed, site))
            })
            .filter(|(presumed, access)| Self::settles_a_presumed_site(*presumed, access))
            .map(|(_, access)| access)
            .collect()
    }

    /// Whether a site the syntax only suggested is worth keeping.
    ///
    /// `[Foo::class, 'bar']` names a method and equally lists two class names, and what tells the
    /// two apart is not in the syntax -- it is whether the class has that member. A list of class
    /// names resolves to none, so resolution is the test, and applying it here means every reader
    /// downstream gets a site that either *is* a member access or is not there at all. The
    /// alternative -- a flag every reader has to remember -- is the shape §8 calls two
    /// constructors of one rule.
    fn settles_a_presumed_site(presumed: bool, access: &MemberAccess) -> bool {
        !presumed || matches!(access.outcome, MemberAccessOutcome::Resolved { .. })
    }

    /// One site, resolved. Shared so the whole-file walk and the ranged one cannot answer
    /// differently about the same access.
    fn access_of(
        &self,
        hir: &phrost_hir::FileHir,
        suppressed: &[TextRange],
        site: phrost_body::MemberSite,
    ) -> MemberAccess {
        let kind = match site.kind {
            SiteKind::Property => MemberKind::Property,
            SiteKind::Method => MemberKind::Method,
            SiteKind::ClassConstant => MemberKind::ClassConstant,
        };
        let at = site.name_range.start();
        let held = self.resolve_path(at, hir, &site.receiver, FOLLOW_LIMIT);
        let class = held.as_ref().and_then(|held| {
            self.owner_at_site(at, hir, &site.receiver, held, kind, site.name.as_str())
        });

        // Both spellings of "the null is handled here" are folded in, because the question
        // is about the access and not about the type. `?->` returns null instead of
        // calling; and PHP's isset context reads a *property* off a null without
        // complaining, though not a method -- a call there is evaluated like any other.
        let in_isset = matches!(kind, MemberKind::Property)
            && suppressed
                .iter()
                .any(|range| range.contains_range(site.name_range));
        // And a member a string names dereferences nothing: `[$file, 'getFileUri']` on a `?File`
        // builds an array, and whatever goes wrong goes wrong where it is called.
        let handled = site.nullsafe || in_isset || site.named;
        let null_reach = match (handled, &held) {
            (true, _) | (_, None) => SentinelReach::Impossible,
            (false, Some(held)) => self.null_reach_of(hir, held),
        };
        // The isset half carries over and the `?->` half does not: PHP reads a *property* off a
        // `false` with a warning exactly as it does off a null, and calls a *method* on it with a
        // fatal that `?->` does not catch.
        let false_reach = match (in_isset || site.named, &held) {
            (true, _) | (_, None) => SentinelReach::Impossible,
            (false, Some(held)) => self.false_reach_of(hir, held),
        };

        // A value that may be null belongs to the null check whatever else is true of it:
        // saying both "this may be null" and "this can never have members" about one
        // receiver is two reports for one mistake.
        let settles = matches!(null_reach, SentinelReach::Impossible)
            .then(|| held.as_ref())
            .flatten();

        // Whether the call is written on the class itself: `$this->`, `self::` or `static::`, all
        // three of which reach this layer as one root. What that buys is below, in `outcome_for`.
        let through_this = site.receiver.steps.is_empty()
            && matches!(site.receiver.root, phrost_body::PathRoot::This);
        let outcome = self.outcome_of(class, hir, settles, kind, site.name.as_str(), through_this);
        let outcome = self.refused_statically(at, &site, kind, outcome);
        // Asked only where the single-class answer is a report or a miss, which is what keeps this
        // off the ordinary path: a union walk per access is the shape a request budget cannot carry,
        // and an access that resolved has nothing to gain from it.
        //
        // An intersection names no single class either, so it arrives here as no receiver at all
        // when it was written and as any object when a guard built it -- and it is every one of its
        // operands at once, so what the best of them answers is the answer for it too. `$this` in a
        // trait that `@require-implements` an interface is the trait and the interface: a member
        // neither declares is one the composing class may, which is what the trait alone has always
        // answered.
        let outcome = match &outcome {
            MemberAccessOutcome::MissingMember { .. }
            | MemberAccessOutcome::UnknownReceiver
            | MemberAccessOutcome::AnyObject => {
                match held.as_ref().and_then(|held| {
                    Some(self.union_of_held(at, hir, held))
                        .filter(|classes| classes.len() > 1)
                        .or_else(|| {
                            Some(self.intersection_of_held(at, hir, held))
                                .filter(|classes| classes.len() > 1)
                        })
                }) {
                    Some(classes) => self.best_across(&classes, kind, site.name.as_str()),
                    None => outcome,
                }
            }
            _ => outcome,
        };

        MemberAccess {
            range: site.name_range,
            receiver_range: site.receiver_range,
            outcome,
            name: site.name,
            kind,
            null_reach,
            false_reach,
            named: site.named,
            asked_if_set: in_isset,
        }
    }

    /// The declared visibility of a member reached where PHP would not allow it, when that is what
    /// happened.
    ///
    /// `None` for an access PHP permits -- and for one it permits *by another route*: a class with
    /// `__get` or `__call` answers a name it cannot reach directly, so an inaccessible member there is
    /// a call to the magic method rather than an error. That exemption is the difference between this
    /// check and one that reports every framework class with a magic accessor.
    ///
    /// Asked by whoever is about to speak rather than computed for every access: the member table is
    /// memoized, so this is a lookup, but there is no reason to do it for the thousands of accesses
    /// that are fine.
    pub fn inaccessible_member(
        &self,
        at: phrost_model::TextSize,
        class: &str,
        kind: MemberKind,
        name: &str,
    ) -> Option<phrost_model::Visibility> {
        match self.reach_of_member(at, class, kind, name) {
            MemberReach::Refused(declared) => Some(declared),
            MemberReach::Declared | MemberReach::Magic => None,
        }
    }

    /// Whether PHP sends this access to a magic accessor rather than to the declaration it names.
    ///
    /// The step walker's half of [`Self::reach_of_member`], named for the question it asks so a
    /// reader of that one line does not have to hold the enum in their head.
    pub(crate) fn reaches_through_magic(
        &self,
        at: phrost_model::TextSize,
        class: &str,
        kind: MemberKind,
        name: &str,
    ) -> bool {
        self.reach_of_member(at, class, kind, name) == MemberReach::Magic
    }

    /// How PHP reaches a member written here: through its declaration, through a magic accessor, or
    /// not at all.
    ///
    /// One walk and three answers, because two readers want two different thirds of it and a rule
    /// written twice is the shape §8 names. [`Self::inaccessible_member`] wants the refusal; the
    /// step walker wants the *magic* case, where the declaration is not what runs and so says
    /// nothing about the value.
    pub(crate) fn reach_of_member(
        &self,
        at: phrost_model::TextSize,
        class: &str,
        kind: MemberKind,
        name: &str,
    ) -> MemberReach {
        let table = self.members(class);
        let Some(found) = table.find(kind, name) else {
            return MemberReach::Declared;
        };
        let declared = found.visibility;
        // The cheap exit, and it is nearly every access: a public member -- or one the class's
        // `__call` reopens -- is reached by its declaration whatever the scope, and nothing below
        // needs asking. What is below is a walk of the enclosing scope, which is why this is worth
        // writing out.
        if found.reached_with(phrost_model::Visibility::Public) {
            return MemberReach::Declared;
        }
        // Against the class that *declares* the member, not the one the value was reached through.
        // PHP's rule is about where the member was written: `protected` is reachable from anywhere
        // under that declaration, so two classes with a common ancestor reach each other's --
        // `$case->setUp()` on one PHPUnit subclass from inside another is ordinary, and reading the
        // object's own class instead reported sixteen of them in one Laravel test file. `private`
        // is the same rule one step tighter: it belongs to the class that wrote it, and a parent
        // reaching into a subclass instance still reaches its own field.
        let owner = match &found.provenance {
            phrost_sema::MemberProvenance::Own | phrost_sema::MemberProvenance::Trait(_) => class,
            phrost_sema::MemberProvenance::Inherited(fqn)
            | phrost_sema::MemberProvenance::Interface(fqn) => fqn.as_str(),
        };
        if declared.is_visible_from(self.visible_from(at, owner)) {
            return MemberReach::Declared;
        }
        // The member may be declared above both classes, which no comparison between the two can
        // see. See `scope_shares_member`.
        if declared == phrost_model::Visibility::Protected
            && self.scope_shares_member(at, kind, name)
        {
            return MemberReach::Declared;
        }

        let magic: &[&str] = match kind {
            MemberKind::Method => &["__call", "__callStatic"],
            MemberKind::Property => &["__get", "__set"],
            MemberKind::ClassConstant | MemberKind::EnumCase => &[],
        };
        let answered_anyway = magic
            .iter()
            .any(|name| table.find(MemberKind::Method, name).is_some());

        match answered_anyway {
            true => MemberReach::Magic,
            false => MemberReach::Refused(declared),
        }
    }

    /// Which of the outcomes an access reached.
    fn outcome_of(
        &self,
        class: Option<Name>,
        hir: &phrost_hir::FileHir,
        held: Option<&Held>,
        kind: MemberKind,
        name: &str,
        through_this: bool,
    ) -> MemberAccessOutcome {
        let Some(class) = class else {
            // No class, but the value may still have a type -- and a type that no object inhabits
            // settles the access rather than leaving it open.
            return match held.and_then(|held| self.kind_of_held(hir, held)) {
                Some(kind) if has_no_members(&kind) => MemberAccessOutcome::NotAnObject { kind },
                Some(BuiltinTy::Object)
                    if kind == MemberKind::Property
                        && held.is_some_and(|held| {
                            self.object_shape_property(hir, held, name).is_some()
                        }) =>
                {
                    MemberAccessOutcome::ShapeProperty
                }
                Some(BuiltinTy::Object) => MemberAccessOutcome::AnyObject,
                _ => MemberAccessOutcome::UnknownReceiver,
            };
        };

        self.outcome_for(class, kind, name, through_this)
    }

    /// A static call on a class whose only magic answers *instance* calls.
    ///
    /// `__call` is excused from `undefined-method` because PHP sends it every name the class does
    /// not declare -- for a call on an object. `Foo::nope()` written outside Foo's hierarchy is a
    /// static call, which only `__callStatic` answers, and PHP raises `Call to undefined method
    /// Foo::nope()` for it, run against 8.5.10. Inside the hierarchy the same spelling is a
    /// non-static call on `$this` and `__call` does run, so the class the call is written in is
    /// asked first.
    fn refused_statically(
        &self,
        at: TextSize,
        site: &phrost_body::MemberSite,
        kind: MemberKind,
        outcome: MemberAccessOutcome,
    ) -> MemberAccessOutcome {
        let MemberAccessOutcome::Dynamic { class } = &outcome else {
            return outcome;
        };
        let named_class = site.receiver.steps.is_empty()
            && matches!(
                site.receiver.root,
                phrost_body::PathRoot::Class(phrost_hir::body::ClassTarget::Named(_))
            );
        if kind != MemberKind::Method || !named_class {
            return outcome;
        }
        let table = self.members(class.as_str());
        let only_instance_calls = table.find(MemberKind::Method, "__call").is_some()
            && table.find(MemberKind::Method, "__callStatic").is_none()
            && !self.is_trait(class.as_str())
            && !self.skeleton.answers_any_name(class.as_str());
        let inside_the_hierarchy = self.enclosing_class(at).is_some_and(|here| {
            here.as_str().eq_ignore_ascii_case(class.as_str())
                || self.descends_from(
                    here.as_str(),
                    class.as_str(),
                    crate::resolve::HIERARCHY_DEPTH,
                )
        });
        match only_instance_calls && !inside_the_hierarchy {
            true => MemberAccessOutcome::MissingMember {
                class: class.clone(),
            },
            false => outcome,
        }
    }

    /// Which outcome one named class reaches.
    ///
    /// Ranked, and the order is the whole of it: an answer beats a gap in the index, a gap beats an
    /// excuse the class itself offers, and only what is left is a fact about the code being read.
    /// [`Self::best_across`] reads the same order to settle a union.
    fn outcome_for(
        &self,
        class: Name,
        kind: MemberKind,
        name: &str,
        through_this: bool,
    ) -> MemberAccessOutcome {
        if self.member_location(class.as_str(), kind, name).is_some() {
            return MemberAccessOutcome::Resolved { class };
        }

        // A class the index never saw has no members to be missing from, and one whose hierarchy
        // broke part way has members nobody can enumerate. Only what is left is a fact about the
        // code being read; the rest are gaps in the index, and calling them the same thing is what
        // makes a checker unusable.
        if self.class_location(class.as_str()).is_none() {
            return MemberAccessOutcome::UnknownClass { class };
        }
        let table = self.members(class.as_str());
        if table.is_incomplete() {
            return MemberAccessOutcome::IncompleteHierarchy { class };
        }

        // `__call` and `__get` make any name legal. A class with one of them cannot be checked this
        // way at all, and treating its dynamic members as absent would bury every real finding.
        let magic: &[&str] = match kind {
            MemberKind::Method => &["__call", "__callStatic"],
            MemberKind::Property => &["__get", "__set"],
            // A constant or a case cannot be produced at runtime, so nothing can be hiding one.
            MemberKind::ClassConstant | MemberKind::EnumCase => &[],
        };
        if magic
            .iter()
            .any(|name| table.find(MemberKind::Method, name).is_some())
        {
            return MemberAccessOutcome::Dynamic { class };
        }

        // A trait is half a class body, and the other half is whichever class composed it -- which
        // PHP resolves at runtime and a trait cannot name. So a member missing from the trait is not
        // missing: `$this->name()` inside it is a call on the composing class.
        //
        // Only reachable through `$this`, `self` or `static` inside the trait, because PHP has no
        // way to type a parameter or a property as one -- so this needs no gate on the receiver.
        //
        // The honest answer costs a real check: a typo on `$this->` inside a trait is not caught.
        // Recovering it means analysing the trait once per composing class, which is a scan of every
        // class in the index -- the shape a request budget cannot carry. Measured on Laravel's own
        // source, believing the trait was **992 of its 2 491 findings**, and every
        // `undefined-property` in its top six.
        if self.is_trait(class.as_str()) {
            return MemberAccessOutcome::Dynamic { class };
        }

        // And a class a framework says answers anything. `__call` above is PHP's own way of stating
        // that and is a *class* mechanism; a mocking library hands a caller an **interface**, and
        // one declaring `__call` would be a claim about every implementer. See `dynamic_members`.
        if self.skeleton.answers_any_name(class.as_str()) {
            return MemberAccessOutcome::Dynamic { class };
        }

        // And the template method: a base class calling something its subclasses implement. PHP 4
        // had no `abstract`, so the pattern is written as a plain class with the call in it, and
        // `ftp_base::_exec()` is 62 of WordPress's 68 reports -- every one of them a call that runs,
        // because `$this` there is an `ftp_pure` or an `ftp_sockets`.
        //
        // Only through `$this`, and only where a subtype really declares the name. Both halves are
        // what keep it from being an exemption: a typo has no subtype declaring it and is still
        // reported, and a *variable* typed as the base is a value whose class nothing here saw
        // chosen -- widening to that would silence the check on every extensible class there is.
        //
        // Asked last, which is what makes the scan affordable: it runs only where a report is about
        // to be made, the same arrangement `inaccessible_member` has.
        if through_this && self.a_subtype_declares(class.as_str(), kind, name) {
            return MemberAccessOutcome::Dynamic { class };
        }

        MemberAccessOutcome::MissingMember { class }
    }

    /// Whether some class below this one declares the member.
    ///
    /// Direct subtypes, which is what the class table answers in one pass -- the table is scanned,
    /// not indexed by supertype. The hole that leaves is a declaration two levels down under a
    /// subtype that inherits quietly, and there the report stands, which is where it stood before.
    fn a_subtype_declares(&self, class: &str, kind: MemberKind, name: &str) -> bool {
        self.implementers_of(class, SUBTYPES_ASKED)
            .into_iter()
            .any(|found| self.declares_member(found.location.fqn.as_str(), kind, name))
    }

    /// Whether a name is a trait, whose members a composing class completes.
    pub(crate) fn is_trait(&self, fqn: &str) -> bool {
        self.class_header(fqn)
            .is_some_and(|header| header.kind == phrost_sema::ClassKind::Trait)
    }

    /// Which outcome a union reaches: the best any of its operands does.
    ///
    /// A union is one of several, so the access works if *any* operand can answer it, and a report
    /// is only right when none of them can. Reducing the union to a common ancestor first had this
    /// backwards: `Uc|Ud` extending one base answered `Undefined method UBase::cOnly()` for a call
    /// `Uc` declares -- a false positive naming a class the source never wrote, about a call that
    /// may well run -- and goto was dead at the same site for the same reason. Where the operands
    /// share no ancestor at all, the reduction answered nothing and goto was dead there too.
    ///
    /// Ranked by [`Self::outcome_for`]'s own order, because this is that question asked several
    /// times. The operand that *declares* the member is the one named, which is what makes goto land
    /// somewhere real; and an operand excusing the access -- unindexed, incomplete, or answering
    /// unknown names through `__call` -- silences the report the way it would on its own.
    fn best_across(&self, classes: &[Name], kind: MemberKind, name: &str) -> MemberAccessOutcome {
        let mut best: Option<MemberAccessOutcome> = None;
        for class in classes {
            // A union operand is a value whose class nothing here saw chosen, which is not the
            // `$this` the template-method rule is about.
            let outcome = self.outcome_for(class.clone(), kind, name, false);
            if rank(&outcome) == 0 {
                return outcome;
            }
            if best.as_ref().is_none_or(|held| rank(&outcome) < rank(held)) {
                best = Some(outcome);
            }
        }
        best.unwrap_or(MemberAccessOutcome::UnknownReceiver)
    }
}

/// How good an outcome is, lowest first, for choosing between a union's operands.
///
/// Not derived from the enum's declaration order, which is about what the variants *mean* rather
/// than about which of two answers to keep. The two that cannot come out of `outcome_for` are given
/// the worst rank rather than being made unreachable: a `match` that named them would have to be
/// updated for a variant this never sees.
fn rank(outcome: &MemberAccessOutcome) -> u8 {
    match outcome {
        MemberAccessOutcome::Resolved { .. } | MemberAccessOutcome::ShapeProperty => 0,
        MemberAccessOutcome::UnknownClass { .. } => 1,
        MemberAccessOutcome::IncompleteHierarchy { .. } => 2,
        MemberAccessOutcome::Dynamic { .. } => 3,
        MemberAccessOutcome::MissingMember { .. } => 4,
        MemberAccessOutcome::UnknownReceiver
        | MemberAccessOutcome::AnyObject
        | MemberAccessOutcome::NotAnObject { .. } => 5,
    }
}

/// Whether a kind of value is one no object can be.
///
/// False for every type an object *may* inhabit -- `object`, `mixed`, `iterable`, `callable` -- and
/// for `null`, which the null check owns because it knows how the value got there and this does not.
/// `void` and `never` are left out too: a value declared either is code already going wrong
/// somewhere else, and the second report would be about the first one's cause.
fn has_no_members(kind: &BuiltinTy) -> bool {
    matches!(
        kind,
        BuiltinTy::Int
            | BuiltinTy::Float
            | BuiltinTy::String
            | BuiltinTy::Bool
            | BuiltinTy::True
            | BuiltinTy::False
            | BuiltinTy::Array
    )
}

/// Whether two ranges touch, an empty one included.
///
/// The same reading a code action takes of a cursor: a caret just after a name is still on it, and
/// an empty selection at an offset is inside a range that ends there.
fn touches(subject: TextRange, at: TextRange) -> bool {
    subject.start() <= at.end() && at.start() <= subject.end()
}
