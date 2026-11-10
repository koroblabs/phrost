//! Climbing the class graph, and the one bound that makes it safe to.
//!
//! Two walks and the depth they share: whether one class is below another, and the nearest class
//! several of them all are. Its own file because everything else here reads a single declaration
//! and stops, while these two recurse through the catalog -- so the cycle bound, the
//! case-insensitive comparison and the extends-only rule are stated once and in one place.

use phrost_model::{Name, TextSize};

use crate::view::IndexView;

/// How far to climb when asking whether two classes share a hierarchy.
///
/// A bound, not a guess: PHP hierarchies are shallow, and a cycle in a broken file must not be
/// walked forever.
pub(crate) const HIERARCHY_DEPTH: u8 = 16;

impl IndexView<'_> {
    /// How much of `class_fqn` the code at `offset` can reach.
    ///
    /// A question about classes and not about objects, which is PHP's rule and the one easiest to
    /// get backwards. Inside `Repo`, another `Repo` instance's private members are reachable --
    /// `$sibling->own` compiles -- and so are a `Base` instance's protected ones. From anywhere
    /// else, neither is.
    ///
    /// So `$this` and `self` need no special case: they name the enclosing class, and the first
    /// The class the code at `offset` counts as, which is not always the class it is written in.
    ///
    /// `Closure::bind($fn, null, CacheItem::class)` gives the closure that class's scope, and inside
    /// it the item's protected fields are as reachable as they are in its own methods. Symfony's
    /// Cache component is built on it -- every adapter makes its items through such a closure --
    /// and reading the enclosing *declaration* there reported 72 accesses PHP allows.
    fn scope_at(&self, offset: TextSize) -> Option<phrost_model::Name> {
        let hir = self.current()?.hir();
        // The enclosing class where the binding names something this cannot resolve, which is what
        // `Closure::bind($fn, $this, $this::class)` writes: a scope that is *this* class said
        // dynamically. Answering nothing there would make every access public and report the class
        // reaching into itself.
        //
        // A callee that says what `$this` will be binds the scope with it: `macro()` calls
        // `bindTo($this, static::class)`, and a macro reads the protected members of what it was
        // registered on as its own.
        phrost_body::bound_closure_scope(hir, offset)
            .and_then(|target| self.class_of_target(offset, hir, target))
            .or_else(|| self.closure_this(offset).flatten())
            .or_else(|| self.enclosing_class(offset))
    }

    /// comparison covers them. Neither does `parent`, which is an ancestor and lands on protected.
    pub fn visible_from(&self, offset: TextSize, class_fqn: &str) -> phrost_model::Visibility {
        use phrost_model::Visibility;

        let mut widest = Visibility::Public;
        for here in self.scopes_at(offset) {
            match self.visible_from_class(here.as_str(), class_fqn) {
                Visibility::Private => return Visibility::Private,
                Visibility::Protected => widest = Visibility::Protected,
                Visibility::Public => {}
            }
        }
        widest
    }

    /// Whether the code at `offset` runs as `class_fqn` or as a class that descends from it.
    ///
    /// Where it does, the class's own name before `::` is `self` or `parent` spelled out, and an
    /// instance method called through it is called on `$this`: `Base::__construct($id)` in a
    /// subclass, `PclZip::privErrorLog(...)` sixty times in the class WordPress ships. Anywhere
    /// else the same line is a static call to a method that is not static, which PHP 8 refuses.
    pub fn runs_within(&self, offset: TextSize, class_fqn: &str) -> bool {
        self.scopes_at(offset).iter().any(|here| {
            here.as_str().eq_ignore_ascii_case(class_fqn)
                || self.descends_from(here.as_str(), class_fqn, HIERARCHY_DEPTH)
        })
    }

    /// [`Self::visible_from`] for code written in `here`.
    fn visible_from_class(&self, here: &str, class_fqn: &str) -> phrost_model::Visibility {
        use phrost_model::Visibility;

        if here.eq_ignore_ascii_case(class_fqn) {
            return Visibility::Private;
        }
        // Either direction: a class reaches its ancestors' protected members and its descendants'.
        if self.descends_from(here, class_fqn, HIERARCHY_DEPTH)
            || self.descends_from(class_fqn, here, HIERARCHY_DEPTH)
        {
            return Visibility::Protected;
        }
        Visibility::Public
    }

    /// Every class the code at `offset` may run as: the one [`Self::scope_at`] names -- and, for a
    /// trait's body, each class it is composed into, since the body is copied into those and runs as
    /// them. `$this->secret` in a trait reads the private property of the class that uses it, and
    /// read from the trait alone it was an access PHP allows reported as one it refuses.
    ///
    /// The trait stays among them: its own private members are its own, and the classes it names
    /// a host through only add to what it reaches. The classes a tag on the trait names stand in
    /// for its users the way they do for `$this`.
    fn scopes_at(&self, offset: TextSize) -> Vec<Name> {
        let Some(here) = self.scope_at(offset) else {
            return Vec::new();
        };
        let mut users = self.what_a_trait_user_is(here.as_str());
        if users.is_empty() {
            users = self.trait_hosts(here.as_str()).to_vec();
        }
        // And a trait a framework composes into the class the closure runs as.
        users.extend(self.configured_traits(&here));
        users.insert(0, here);
        users
    }

    /// Whether the code at `offset` sits in a class that holds the same member declaration.
    ///
    /// The last of PHP's three protected rules, and the one a two-class comparison cannot express:
    /// what decides access is the class the member was *first* declared in, which may be above both
    /// the scope and the object. `$case->setUp()` written in one PHPUnit subclass, on an instance of
    /// another, is allowed because `setUp` is declared on the `TestCase` they share -- and neither
    /// class descends from the other.
    ///
    /// Asked as "does my own class have this member too", which is the same question one table
    /// lookup away: a member reachable from here under the same name and kind is the same
    /// declaration, inherited down both sides. Two unrelated hierarchies that happen to declare the
    /// same protected name would pass, which PHP refuses -- a miss, and the quiet direction.
    pub(crate) fn scope_shares_member(
        &self,
        offset: TextSize,
        kind: phrost_sema::MemberKind,
        name: &str,
    ) -> bool {
        self.scopes_at(offset)
            .iter()
            .any(|here| self.members(here.as_str()).find(kind, name).is_some())
    }

    /// Whether `class_fqn` is a `Stringable` nothing wrote: PHP 8 gives that interface to anything
    /// declaring `__toString()` -- a class, an interface, a class through its parent or its trait
    /// -- so `is_subclass_of('CustomStringable', 'Stringable')` is true with no `extends` in sight.
    ///
    /// Read off the flattened table so an inherited method counts, and asked only when `ancestor`
    /// *is* `Stringable`: the table is not free, and putting this in every class header made each
    /// hierarchy climb pay for it -- code actions' p95 doubled on the reference project.
    pub(crate) fn stringable_without_saying(&self, class_fqn: &str, ancestor: &str) -> bool {
        ancestor.eq_ignore_ascii_case("Stringable")
            && !class_fqn.eq_ignore_ascii_case("Stringable")
            && self
                .members(class_fqn)
                .find(phrost_sema::MemberKind::Method, "__toString")
                .is_some()
    }

    /// Whether every class, interface and trait above `class_fqn` is one the index holds, so that
    /// [`Self::descends_from`] answering no is a fact rather than a gap. A hierarchy deeper than
    /// `depth` counts as unknown.
    pub(crate) fn ancestry_is_known(&self, class_fqn: &str, depth: u8) -> bool {
        if depth == 0 {
            return false;
        }
        let Some(header) = self.class_header(class_fqn) else {
            return false;
        };
        header
            .extends
            .iter()
            .chain(header.implements.iter())
            .chain(header.uses.iter())
            .all(|above| self.ancestry_is_known(above.as_str(), depth - 1))
    }

    /// Whether `class_fqn` has `ancestor` above it, through extends, implements, or a used trait.
    pub(crate) fn descends_from(&self, class_fqn: &str, ancestor: &str, depth: u8) -> bool {
        if depth == 0 {
            return false;
        }
        let Some(header) = self.class_header(class_fqn) else {
            return false;
        };

        header
            .extends
            .iter()
            .chain(header.implements.iter())
            .chain(header.uses.iter())
            .chain(header.implied.iter())
            .any(|above| {
                above.as_str().eq_ignore_ascii_case(ancestor)
                    || self.descends_from(above.as_str(), ancestor, depth - 1)
            })
    }

    /// The nearest class every one of these is.
    ///
    /// What a union of classes reduces to: `catch (TokenException | \Exception $e)` says the value
    /// is one of the two, so the members that can be reached through it are the ones they have in
    /// common -- and the class that holds those is their nearest common ancestor.
    ///
    /// A miss and not a wrong answer when it finds none, and a miss for the members one arm adds:
    /// what it gives is always something the value really is. Nearest first, so `A|B` where `B`
    /// extends `A` answers `A` rather than something above it.
    pub(super) fn common_ancestor(&self, operands: &[Name]) -> Option<Name> {
        let (first, rest) = operands.split_first()?;
        if rest.is_empty() {
            return Some(first.clone());
        }

        let mut candidate = Some(first.clone());
        for _ in 0..HIERARCHY_DEPTH {
            let current = candidate?;
            let shared = rest.iter().all(|other| {
                other.as_str().eq_ignore_ascii_case(current.as_str())
                    || self.descends_from(other.as_str(), current.as_str(), HIERARCHY_DEPTH)
            });
            if shared {
                return Some(current);
            }
            // Only the class chain, not the interfaces: an interface every operand implements is a
            // real answer, but there may be several and choosing between them would be a guess.
            candidate = self
                .class_header(current.as_str())
                .and_then(|header| header.extends.first().cloned());
        }

        None
    }
}
