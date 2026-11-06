use phrost_model::{FileId, Name, SymbolKind, Visibility};
use phrost_sema::{MemberKind, MemberProvenance};

/// Which top-level symbol kinds a name position can accept.
///
/// A caller already knows this before asking -- `new ` can only take a class, everywhere else a
/// bare name is genuinely any of PHP's three tables -- so it is given to
/// [`crate::IndexView::candidates_with_prefix`] rather than applied after: a position that can
/// only take a class should not spend its cap on functions and constants it will discard.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NameKinds {
    /// Anywhere a bare name can stand except after `new`: a type position, an operand, an
    /// argument, an attribute. All three of PHP's tables are legal, because which one was meant
    /// depends on what follows the cursor and that is not this projection's question.
    Any,
    /// After `new`, where PHP allows constructing a class and nothing else -- not an interface,
    /// a trait, or an enum, all of which the catalog also files under class-like.
    ClassOnly,
    /// In an `implements` list, or after an `interface`'s `extends`, where PHP allows an interface
    /// and nothing else -- not the class that implements it, and not a trait.
    InterfaceOnly,
    /// In a `use` written inside a class body, where PHP allows a trait and nothing else.
    ///
    /// The same keyword as a file-level import and a different language: that one takes any of the
    /// three tables and is anchored at the root, this one takes one kind and resolves the way every
    /// other name in the body does.
    TraitOnly,
    /// Where a type is written: a signature, a property, a `catch`, a docblock tag.
    ///
    /// Class, interface and enum, which are the three class-likes a value can *be*. Not a trait,
    /// which nothing is an instance of -- and above all not a function or a constant, which is what
    /// a type position used to be filled with, `stats_cdf_beta` offered where `string` goes.
    TypeName,
    /// After a `#[`, where PHP allows a class and nothing else.
    ///
    /// Its own variant rather than [`Self::ClassOnly`] because of what it asks the projection to do
    /// beyond filtering: the classes declared `#[Attribute]` are offered in front of every other
    /// tier, from the list the index keeps of them. Not a filter -- PHP refuses a class that is not
    /// one only when something reflects on the attribute, and a class being written before its
    /// declaration is marked is ordinary.
    Attribute,
    /// Where no name the catalog holds can stand at all: an enum's backing type, which is `int` or
    /// `string` and nothing a project can declare.
    Nothing,
}

impl NameKinds {
    /// Whether a candidate of this kind belongs in a list offered at a position of this shape.
    pub fn accepts(self, kind: SymbolKind) -> bool {
        match self {
            Self::Any => true,
            Self::ClassOnly | Self::Attribute => kind == SymbolKind::Class,
            Self::InterfaceOnly => kind == SymbolKind::Interface,
            Self::TraitOnly => kind == SymbolKind::Trait,
            Self::TypeName => matches!(
                kind,
                SymbolKind::Class | SymbolKind::Interface | SymbolKind::Enum
            ),
            Self::Nothing => false,
        }
    }
}

/// A top-level symbol worth offering.
///
/// Only the fully-qualified name is stored; the short one is [`Self::short`], a slice of it. A
/// caller needs both — the short one is what a person reads and typed a prefix of, the qualified one
/// is what has to end up in the file if nothing imports it — but storing both would mean an
/// allocation per candidate, and a one-letter prefix produces thousands.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Candidate {
    pub fqn: Name,
    pub kind: SymbolKind,
    pub file: FileId,
    /// Whether the declaration says it is deprecated, by `@deprecated` or `#[\Deprecated]`.
    ///
    /// On the row rather than fetched when one is highlighted, because it decides how the row is
    /// *drawn* -- an editor strikes a deprecated item through, and a fact nobody can see until they
    /// pick the item is a fact that arrived too late.
    pub deprecated: bool,
}

impl Candidate {
    pub fn short(&self) -> &str {
        phrost_model::short_name(self.fqn.as_str())
    }
}

/// A member worth offering on a receiver.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct MemberCandidate {
    pub name: Name,
    pub kind: MemberKind,
    pub visibility: Visibility,
    pub is_static: bool,
    /// The type alone, which is what the list itself shows. `None` for a declaration that names no
    /// type, and for an enum case, whose type is the enum and is written nowhere.
    pub ty: Option<Box<str>>,
    /// Which class actually declared it, so a caller can say `inherited from`.
    pub origin: MemberProvenance,
    /// Whether calling it takes at least one argument.
    ///
    /// For deciding where a cursor lands after the parentheses are written: inside them when there is
    /// something to type, after them when there is not. Meaningless for anything but a method, and
    /// `false` there.
    pub takes_arguments: bool,
    /// The parameters a call **must** be given, in order and without their `$`.
    ///
    /// For a client that can write a snippet: accepting `send` should leave
    /// `send($body, $urgent)` with each name selected in turn, which is the difference between a
    /// completion that finishes the call and one that finishes the name.
    ///
    /// Only the required ones, and that is the opposite of what [`Self::takes_arguments`] counts. A
    /// placeholder is *text somebody has to deal with*, so a parameter the language says may be
    /// omitted must not become one -- `array_slice(${1:$array}, ${2:$offset}, ${3:$length},
    /// ${4:$preserve_keys})` is four things to delete. Where a caret goes is a different question,
    /// and there a defaulted parameter does count.
    ///
    /// The names rather than the rendered declaration: this is on every row of the list, and the
    /// declaration is what `completionItem/resolve` exists to fetch for the one row being shown.
    pub parameters: Box<[Name]>,
    /// Whether the declaration says it is deprecated -- see [`Candidate::deprecated`].
    pub deprecated: bool,
    /// Whether it is answered through a `@mixin` rather than declared in the hierarchy.
    ///
    /// What decides whether `::` reaches an instance method: PHP sends a name the class does not
    /// declare to `__callStatic`, which is how `User::where()` reaches the query builder -- and an
    /// instance method the class *does* declare is a fatal error through `::` all the same.
    pub forwarded: bool,
    /// Whether the class's `__call` hands the name back to this declaration -- see
    /// `ResolvedMember::opened`. A scope PHP refuses reaches it through `::` as well, by way of
    /// `__callStatic`, which is how `Post::increment('views')` runs.
    pub opened: bool,
}

/// One member's declaration, as the parts a reader is shown.
///
/// Structured rather than rendered, because how a declaration reads is the feature layer's business
/// -- and this crate has no opinion about markdown.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct MemberDeclaration {
    pub visibility: Visibility,
    pub is_static: bool,
    pub kind: MemberKind,
    pub deprecated: bool,
    /// Without the leading keyword: `find(int $id): ?Repo`.
    pub signature: String,
}

/// A method a framework finds under a prefix, offered by the name a call writes.
///
/// `published` for Eloquent's `scopePublished`, on the model and on a builder for it -- and a
/// `#[Scope]` method by its own name, which from outside is protected and reached through the magic
/// method. The row a list offers is the call; the declaration is what a resolve describes.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PrefixedCandidate {
    /// What the call writes: `published`. Text rather than a [`Name`], because nothing interned
    /// it -- no declaration spells it -- and a request cannot intern.
    pub name: Box<str>,
    /// The method that answers it: `scopePublished`, or the marked method's own name.
    pub declared: Name,
    /// The class that declares it -- the model, whatever the receiver was.
    pub model: Name,
    /// Whether the call takes anything past the builder the framework hands the method first.
    pub takes_arguments: bool,
    /// The ones it must be given, in order: the declaration's after that builder, without the
    /// ones that have defaults -- [`MemberCandidate::parameters`]' rule, one parameter in.
    pub parameters: Box<[Name]>,
}

/// A method a framework's `__call` answers by a prefix and a word the model states: Eloquent's
/// `whereTitle` for the `title` a model lists.
///
/// No declaration is behind it, so what describes it is the word: the property the model states.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct FluentCandidate {
    pub name: Box<str>,
    /// The model stating the word.
    pub model: Name,
    /// The word as the model states it: `published_at` for `wherePublishedAt`.
    pub word: Box<str>,
}

impl crate::IndexView<'_> {
    /// The methods a framework's `__call` answers by a prefix on this receiver, matched against
    /// `typed`. The receiver's chain for [`Self::prefixed_candidates`]' reason.
    pub fn fluent_candidates(
        &self,
        offset: phrost_model::TextSize,
        root: crate::ChainRoot<'_>,
        steps: &[crate::ChainStep<'_>],
        class_fqn: &str,
        typed: &str,
    ) -> Vec<FluentCandidate> {
        if self.skeleton.returns().fluent_prefixes().is_empty() {
            return Vec::new();
        }
        let held = || self.chain_held(offset, root, steps);
        self.fluent_methods_answering(class_fqn, held)
            .into_iter()
            .filter(|candidate| phrost_model::matches_loosely(&candidate.name, typed))
            .collect()
    }

    /// The prefixed methods a receiver answers, matched against `typed`.
    ///
    /// The receiver's chain rather than its class, because a builder says which model it is for
    /// only in its type arguments -- the same reason goto on such a call names the model first.
    pub fn prefixed_candidates(
        &self,
        offset: phrost_model::TextSize,
        root: crate::ChainRoot<'_>,
        steps: &[crate::ChainStep<'_>],
        class_fqn: &str,
        typed: &str,
    ) -> Vec<PrefixedCandidate> {
        let held = || self.chain_held(offset, root, steps);
        self.prefixed_methods_answering(class_fqn, held)
            .into_iter()
            .filter(|candidate| phrost_model::matches_loosely(&candidate.name, typed))
            .collect()
    }
}

/// A property a framework says a receiver has, which nothing declares.
///
/// Its own row rather than a [`MemberCandidate`], because there is no member row behind it: the name
/// is a key a table writes or one a binding speaks for, which no lowering interns. Two sources, the
/// two goto and the type of a read already consult -- a kind's table, whose every name holds
/// something different, and a binding, whose every name holds one class.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct UndeclaredProperty {
    pub name: Box<str>,
    /// What reading it gives back, where something says.
    pub ty: Option<Box<str>>,
    /// The class whose own body states it -- where goto lands, and what a resolve looks in. `None`
    /// where nothing in the project writes the name down: a kind only the framework's own table
    /// describes, and a binding.
    pub declarer: Option<Name>,
    /// One of a catalog's keys, every one of which the accessor answers, rather than a name a table
    /// or a binding writes. On an entity that is every field the project has, so it is the row a
    /// list shows after the rest.
    pub keyed: bool,
}

/// A key of a project catalog, offered where the catalog is what a call accepts.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CatalogCandidate {
    /// The id as configuration wrote it -- what gets inserted.
    pub key: Box<str>,
    /// What it names, which is what tells a reader they picked the right one: the class a service
    /// id builds, the URL a route serves, the title a permission is shown under.
    pub detail: Box<str>,
}

impl crate::IndexView<'_> {
    /// The properties a framework says the receiver a list is being built for has, which no
    /// declaration writes: a kind's table first, since it says what each name holds, then the
    /// names a binding speaks for, then -- at most `limit` of them, matched against `typed` -- the
    /// keys of a catalog a binding answers every one of.
    ///
    /// Each name once, nearest first. A name the member table holds is the declaration's, and the
    /// caller has those already.
    pub fn undeclared_properties(
        &self,
        offset: phrost_model::TextSize,
        root: crate::ChainRoot<'_>,
        steps: &[crate::ChainStep<'_>],
        class_fqn: &str,
        typed: &str,
        limit: usize,
    ) -> Vec<UndeclaredProperty> {
        let mut found = self.tabled_properties_of(offset, root, steps, class_fqn);
        for property in self
            .bound_properties_of(class_fqn, typed, limit)
            .into_iter()
            .chain(self.method_backed_properties_of(class_fqn))
            .chain(self.accessor_properties_of(class_fqn))
            .chain(self.stated_properties_of(class_fqn))
        {
            if !found.iter().any(|known| known.name == property.name) {
                found.push(property);
            }
        }
        found
    }
}
