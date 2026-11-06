use lspkit_arena::IdSlice;
use phrost_model::{Name, TextRange, Visibility};

use crate::ids::{
    AttributeLiteralId, BodyId, ClassId, DocblockId, HookId, MemberId, NameRefId, NamespaceId,
    ParamId, TemplateId, TraitAdaptationId, TypeRefId,
};

/// One class, interface, trait, or enum declaration.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ClassData {
    pub name: Name,
    pub kind: ClassKind,
    /// `new class { ... }`, which declares a class nobody can name.
    ///
    /// Lowered like any other so that what is *inside* it exists -- its supertypes, its members,
    /// the locals in its methods -- but it must never claim a fully-qualified name: the name it
    /// carries is synthesised from its position, and a person cannot write it, so offering it or
    /// resolving to it would be offering something that is not there.
    pub is_anonymous: bool,
    pub namespace: Option<NamespaceId>,
    pub modifiers: ClassModifiers,
    /// A class extends at most one; an interface may extend several.
    pub extends: IdSlice<NameRefId>,
    pub implements: IdSlice<NameRefId>,
    /// Traits pulled in by `use`, in source order — which matters, because later `use` statements
    /// can conflict with earlier ones.
    pub uses: IdSlice<NameRefId>,
    /// `insteadof` and `as` clauses. Captured at lowering because the alternative is re-reading
    /// the file when trait flattening finally needs them.
    pub trait_adaptations: IdSlice<TraitAdaptationId>,
    /// `enum Suit: string` — the backing type, if any.
    pub enum_backing: Option<TypeRefId>,
    pub members: IdSlice<MemberId>,
    /// `@template` parameters, in the order written.
    pub templates: IdSlice<TemplateId>,
    /// `@extends Bar<Baz>`, `@implements I<Baz>`, `@use T<Baz>` -- what this declaration supplies
    /// for a supertype's template parameters.
    ///
    /// Lowered as the type the tag wrote, so the base names the supertype and the arguments are the
    /// arguments. A tag with no arguments is kept out: it says nothing the `extends` clause did not.
    ///
    /// Not a supertype in its own right. PHP's `extends` already named it; this only says what it
    /// was given.
    pub supertype_arguments: IdSlice<TypeRefId>,
    /// `@mixin Other` -- a class whose members are reachable through this one though PHP says
    /// nothing about it.
    ///
    /// Kept as the written type, like the supertype arguments, because `@mixin Collection<Item>` is
    /// written too -- and then it is both a supertype and an argument for one.
    pub mixins: IdSlice<TypeRefId>,
    /// Named string literals written in the declaration's attributes.
    ///
    /// Kept because some of them are the only thing that identifies the class to a framework:
    /// `#[ContentEntityType(id: 'node')]` is what says that this class is what `getStorage('node')`
    /// hands back, and no signature anywhere says it. Nothing here knows which attribute matters --
    /// the index asks a framework that.
    pub attribute_literals: IdSlice<AttributeLiteralId>,
    /// Range of the name alone. This is the goto target, and the one range that must survive
    /// compaction and a cache round trip, because "no fallbacks" depends on it being there.
    pub name_range: TextRange,
    /// The whole declaration, for folding and selection ranges.
    pub full_range: TextRange,
    /// The braces and what is between them, which is what decides *which class an offset is in*.
    ///
    /// Not the same question as the declaration's extent, and an anonymous class is where the two
    /// part company: `new class($this->mock(), ...) extends Base { ... }` writes its constructor
    /// arguments between the `class` keyword and the body, and PHP evaluates them in the scope the
    /// `new` is written in -- so `$this` there is the *enclosing* object. Read from `full_range`,
    /// every one of those arguments belonged to the anonymous class, and a `$this->` call in one
    /// reported `Undefined method class@anonymous::...`.
    pub body_range: TextRange,
    pub docblock: Option<DocblockId>,
    /// Whether the declaration is reached only when the code around it runs.
    ///
    /// `if (!function_exists('dump')) { function dump(..) {} }` is how PHP ships a polyfill, and a
    /// declaration inside another function's body is the same kind of fact: neither is hoisted, so
    /// neither exists until execution passes it. Recorded because the two read identically to
    /// everything above here and rank differently -- a guarded declaration means *only if nothing
    /// else declared this*, so it must lose to every unguarded one, the bundled stubs included.
    pub is_conditional: bool,
    /// The docblock marks it `@ignore` -- phpDocumentor's "not part of the API".
    ///
    /// Carried for the same reason and read the same way as [`Self::is_conditional`]: it loses a tie
    /// between two declarers of one name and does nothing at all where there is only one. See
    /// `Lowerer::doc_is_ignored`.
    pub is_ignored: bool,
}

/// Which of the four class-like declarations this is.
///
/// Flat rather than a bool soup because PHP's rules differ per kind in ways that matter:
/// interfaces have no bodies, traits cannot be instantiated, enums have cases and an optional
/// backing type.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum ClassKind {
    Class,
    Interface,
    Trait,
    Enum,
}

impl ClassKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Class => "class",
            Self::Interface => "interface",
            Self::Trait => "trait",
            Self::Enum => "enum",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub struct ClassModifiers {
    pub is_abstract: bool,
    pub is_final: bool,
    /// PHP 8.2 `readonly class`, which makes every property readonly without saying so.
    pub is_readonly: bool,
    /// Whether the declaration carries `#[Attribute]`.
    ///
    /// Not a modifier, and here because this is what travels to the stored class row beside
    /// `is_final` and `is_readonly` -- one more bool in the padding those two already pay for. What
    /// it buys is the one position that can rank on it: `#[` offers the whole catalog, and of the
    /// thirty thousand class-likes a real checkout holds, eighty-nine are what can stand there.
    pub is_attribute: bool,
    /// Whether `@deprecated` or PHP 8.4's `#[\Deprecated]` says so. Set in one place for all four
    /// class-like forms, so a trait cannot be the one that forgets.
    pub is_deprecated: bool,
}

/// One method, property, class constant, or enum case.
///
/// One struct for all four kinds, with the kind-specific fields optional. The alternative — an
/// enum with per-kind payloads — reads better in isolation but forces every consumer that only
/// wants a name, a visibility, and a range to match four ways.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct MemberData {
    pub name: Name,
    pub kind: MemberKind,
    pub owner: ClassId,
    pub visibility: Visibility,
    /// PHP 8.4 asymmetric visibility: `public private(set) int $x`. `None` means writes use
    /// [`Self::visibility`]. Modelled because a wrong answer here reports a property as writable
    /// when it is not, which is a diagnostic people trust.
    pub set_visibility: Option<Visibility>,
    pub modifiers: MemberModifiers,
    /// Method return type, property type, or typed class constant. `None` means not annotated —
    /// which is different from `mixed`, and the type layer treats them differently.
    pub type_ref: Option<TypeRefId>,
    /// The same, from a docblock's `@var` or `@return`, and only when [`Self::type_ref`] is absent.
    ///
    /// Two fields rather than one, because which of them said it matters: a native declaration is
    /// what the runtime enforces, and a docblock is a claim about code that predates it. A
    /// diagnostic treats the two differently even when a lookup does not.
    pub doc_type_ref: Option<TypeRefId>,
    /// Whether [`Self::doc_type_ref`] was read off the body rather than out of a docblock.
    ///
    /// A declaration with neither a hint nor a tag takes its type from what its `return` statements
    /// agree on, and that answer is stored in the same field -- there being nowhere else for it and
    /// no reader that wanted the two apart. One does now: a method **overriding** one that declares
    /// a type inherits that declaration, and PHP's own rule is that the inherited claim is the
    /// contract while the body is an implementation detail of it.
    ///
    /// Drupal's `Select::execute()` is the shape. `SelectInterface::execute()` declares
    /// `@return \Drupal\Core\Database\StatementInterface|null` and the concrete class overrides it
    /// with no docblock at all, so reading the body answered the non-nullable thing the body
    /// happens to hand back -- and every `$query->execute()->fetchAll()` in a project read as safe.
    /// PHPStan reports 97 of them on one real checkout.
    pub type_from_body: bool,
    /// Whether a docblock `@return` was written and refused for naming a template of the
    /// declaration's own -- `@return static<TKey, TMapValue>` beside `@template TMapValue`.
    ///
    /// The member half of [`ParamData::doc_type_refused`], for the same reason: the slot it leaves
    /// is empty, and an empty slot reads as a declaration that said nothing -- so an overridden
    /// method's contract was carried down over the override's own promise.
    pub doc_type_refused: bool,
    /// The classes its docblock's `@throws` tags name.
    ///
    /// Empty for the overwhelming majority of declarations, which is why this is an arena slice and
    /// not a `Vec`: an empty one is two numbers. Kept because a *caller* has to know what a callee
    /// throws and cannot read the callee's docblock -- the index stores ranges rather than text.
    pub throws: IdSlice<TypeRefId>,
    /// The member chain its body hands back, when nothing else says what it returns.
    ///
    /// `None` for almost every declaration: only one whose signature *and* docblock are silent and
    /// whose returned expression names no type gets one. See [`ReturnedPathData`].
    pub returned_path: Option<Box<ReturnedPathData>>,
    /// Methods only; empty for the other kinds.
    pub params: IdSlice<ParamId>,
    /// PHP 8.4 property hooks: the `get` and `set` written after a property's name. Empty for
    /// every other kind, and for a property without them.
    ///
    /// Apart from [`Self::body`] because a hook is a function-like of its own -- `set` takes a
    /// parameter and each hook has its own scope -- and a property can carry two of them.
    pub hooks: IdSlice<HookId>,
    /// The method's body, when bodies were lowered. `None` means abstract or interface-declared if
    /// [`FileHir::has_bodies`](crate::FileHir::has_bodies), and merely unread if not.
    pub body: Option<BodyId>,
    /// Whether the declaration writes a value: `public int $x = 1`, or `case A = 'a'`.
    ///
    /// Recorded rather than read back off the text, which is what the one reader of it used to do
    /// -- a scan for `=` inside the declaration's span, which a comment or a string holding one
    /// makes wrong. `false` for everything that is neither a property nor an enum case.
    pub has_default: bool,
    pub name_range: TextRange,
    pub full_range: TextRange,
    pub docblock: Option<DocblockId>,
    /// What `@assert-if-true` / `@assert-if-false` proves about the receiver when this method
    /// returns.
    pub contract: Option<ContractData>,
    /// Declared by an `@method` or `@property` tag rather than by PHP.
    ///
    /// Kept because it changes what an honest answer looks like: goto lands inside a comment, and
    /// the declaration a person is shown is a claim about code that is generated at runtime rather
    /// than code that exists.
    pub from_docblock: bool,
    /// Whether [`Self::type_ref`] was read off the value rather than written.
    ///
    /// Only a constant has one. See `phrost_sema::MemberRow::ty_from_value` for why the two are
    /// kept apart: a type nobody wrote must not appear in a declaration rendered as text.
    pub ty_from_value: bool,
    /// Whether the body reads arguments the signature does not name.
    ///
    /// `func_get_args()` / `func_num_args()`, which make the parameter list a lower bound rather
    /// than a contract -- PHP raises nothing when a caller supplies more. Recorded here because
    /// this is the last layer that can see a body at all: a closed file's statements are never
    /// retained, and from the parameter table alone such a declaration is indistinguishable from
    /// one that really takes what it names. See `phrost_lower::own_arguments`.
    pub reads_own_arguments: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum MemberKind {
    Method,
    Property,
    ClassConstant,
    EnumCase,
}

impl MemberKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Method => "method",
            Self::Property => "property",
            Self::ClassConstant => "const",
            Self::EnumCase => "case",
        }
    }
}

/// The words a member's declaration carries, one bit each.
///
/// Packed rather than a field per word, and the number is why: `size_of::<MemberRow>()` is 88 with
/// a five-byte modifiers field and 96 with a six-byte one, so the sixth byte is a whole word per
/// member -- 2.2 MB of 104 on a real checkout. A project holds hundreds of thousands of members and
/// there will be more facts to record about them, so the bits live in one `u8` and are read through
/// methods.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub struct MemberModifiers {
    words: u8,
    /// PHP 8.4 property hooks, and what they say about writing the property.
    ///
    /// The hooks themselves are on [`MemberData::hooks`]; this is what a reader wants without
    /// looking them up -- a hooked property is not a plain slot, so `readonly` and the write
    /// visibility mean something different on one, and a *virtual* `get` alone means PHP refuses
    /// every write.
    pub hooks: Hooks,
}

impl MemberModifiers {
    const STATIC: u8 = 1 << 0;
    const ABSTRACT: u8 = 1 << 1;
    const FINAL: u8 = 1 << 2;
    const READONLY: u8 = 1 << 3;
    /// PHP 8.5's `#[\NoDiscard]`: the result is the point of the call, and discarding it is a
    /// warning at runtime. A fact about the declaration rather than about any call, so it is
    /// recorded here and survives the codec -- the declaration a call discards is usually in
    /// another file.
    const NO_DISCARD: u8 = 1 << 4;
    /// PHP 8.3's `#[\Override]`: the declaration claims to replace one above it, and PHP refuses
    /// to load the class when there is none. A fact about the declaration, recorded here and
    /// carried through the codec -- what it has to match is in another file.
    const OVERRIDE: u8 = 1 << 5;
    /// `@deprecated` or PHP 8.4's `#[\Deprecated]`: the declaration still works and should not be
    /// reached for. One bit for both spellings, because they say the same thing and a reader asking
    /// "is this deprecated" is not asking which of them said so -- 797 docblocks in WordPress
    /// against seven attributes in Symfony is the whole distribution.
    const DEPRECATED: u8 = 1 << 6;
    /// The declaration carries an attribute a framework keyed. See the index's own
    /// `MemberModifiers::is_marked`.
    const MARKED: u8 = 1 << 7;

    pub fn is_static(self) -> bool {
        self.words & Self::STATIC != 0
    }

    pub fn is_abstract(self) -> bool {
        self.words & Self::ABSTRACT != 0
    }

    pub fn is_final(self) -> bool {
        self.words & Self::FINAL != 0
    }

    pub fn is_readonly(self) -> bool {
        self.words & Self::READONLY != 0
    }

    pub fn is_no_discard(self) -> bool {
        self.words & Self::NO_DISCARD != 0
    }

    pub fn is_override(self) -> bool {
        self.words & Self::OVERRIDE != 0
    }

    pub fn is_deprecated(self) -> bool {
        self.words & Self::DEPRECATED != 0
    }

    pub fn is_marked(self) -> bool {
        self.words & Self::MARKED != 0
    }

    pub fn set_marked(&mut self, on: bool) {
        self.set(Self::MARKED, on);
    }

    pub fn set_static(&mut self, on: bool) {
        self.set(Self::STATIC, on);
    }

    pub fn set_abstract(&mut self, on: bool) {
        self.set(Self::ABSTRACT, on);
    }

    pub fn set_final(&mut self, on: bool) {
        self.set(Self::FINAL, on);
    }

    pub fn set_readonly(&mut self, on: bool) {
        self.set(Self::READONLY, on);
    }

    pub fn set_no_discard(&mut self, on: bool) {
        self.set(Self::NO_DISCARD, on);
    }

    pub fn set_deprecated(&mut self, on: bool) {
        self.set(Self::DEPRECATED, on);
    }

    pub fn set_override(&mut self, on: bool) {
        self.set(Self::OVERRIDE, on);
    }

    fn set(&mut self, bit: u8, on: bool) {
        match on {
            true => self.words |= bit,
            false => self.words &= !bit,
        }
    }

    /// The bits as one number, for a codec. Paired with [`Self::from_bits`], which is the only way
    /// back in -- so an unused bit cannot arrive from a cache as a word nothing declared.
    pub fn bits(self) -> u8 {
        self.words
    }

    pub fn from_bits(words: u8) -> Self {
        Self {
            words: words
                & (Self::STATIC
                    | Self::ABSTRACT
                    | Self::FINAL
                    | Self::READONLY
                    | Self::NO_DISCARD
                    | Self::OVERRIDE
                    | Self::DEPRECATED),
            hooks: Hooks::None,
        }
    }
}

/// What a property's hooks say about writing it.
///
/// PHP's own distinction and a syntactic one: a property is *backed* if any of its hooks uses
/// `$this->propName`, and virtual otherwise. A backed property holds a slot and takes a write; a
/// virtual one with no `set` is what PHP calls read-only, and says so from inside the declaring
/// class as much as from outside. Verified against PHP 8.5.10.
///
/// One field rather than three flags: the states are exclusive, and a member row is the largest
/// table the index holds.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum Hooks {
    #[default]
    None,
    /// One of them is the `set`, so PHP takes the write and runs it.
    Settable,
    /// A `get` alone over a backing store the hook uses.
    BackedGet,
    /// A `get` alone with no body: a requirement, and whoever implements it decides.
    DeclaredGet,
    /// A `get` alone with a body that never mentions the store -- read-only.
    VirtualGet,
}

impl Hooks {
    /// Whether any hooks were written.
    pub fn are_written(self) -> bool {
        self != Self::None
    }
}

/// One `get` or `set` hook of a property.
///
/// Function-like, and modelled as one for that reason: it has a body, `set` has a parameter, and
/// `$this` inside it is the declaring class. Nothing above this layer can answer about a name
/// written inside a hook unless the hook is a scope, and until it was one, goto, hover, rename and
/// every diagnostic were silent inside every hook body in the project.
///
/// Not a [`MemberData`]: a hook is not a member. It has no name anybody can write at a call site,
/// it never appears in a member table, and giving it one would offer `get` and `set` as methods of
/// every hooked property.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct HookData {
    pub kind: HookKind,
    /// `set(string $value)`, and empty for a `get`.
    ///
    /// A `set` written with no list still takes a value, and that one is synthesized -- see
    /// [`HookKind::Set`].
    pub params: IdSlice<ParamId>,
    /// The hook's body, when bodies were lowered. `None` means an abstract hook -- `get;` in an
    /// interface -- if [`FileHir::has_bodies`](crate::FileHir::has_bodies), and merely unread if
    /// not, exactly as [`MemberData::body`] does.
    pub body: Option<BodyId>,
    /// `&get`, which hands back a reference to the backing value.
    pub returns_by_reference: bool,
    /// Range of the `get` or `set` keyword alone -- the goto target for the hook.
    pub name_range: TextRange,
    pub full_range: TextRange,
}

/// Which of PHP's two property hooks this is.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum HookKind {
    Get,
    /// `set`, whose parameter is implicit when no list is written: `set { $this->raw = $value; }`
    /// binds `$value` with the property's own type, and PHP says so rather than the source.
    Set,
}

impl HookKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Get => "get",
            Self::Set => "set",
        }
    }
}

/// One free function declaration.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct FunctionData {
    pub name: Name,
    pub namespace: Option<NamespaceId>,
    pub params: IdSlice<ParamId>,
    pub return_type: Option<TypeRefId>,
    /// From the docblock's `@return`, and only when [`Self::return_type`] is absent.
    pub doc_return_type: Option<TypeRefId>,
    /// `function &f()` — returns by reference.
    pub returns_by_reference: bool,
    /// PHP 8.5's `#[\NoDiscard]` -- see [`MemberModifiers::is_no_discard`].
    pub is_no_discard: bool,
    /// `@deprecated` or PHP 8.4's `#[\Deprecated]` -- see [`MemberModifiers::is_deprecated`].
    pub is_deprecated: bool,
    /// What its docblock promises beyond its signature. See [`ContractData`].
    ///
    /// A project's own `assertFoo($value)` helper is a free function as often as a static method,
    /// and so is a factory.
    pub contract: Option<ContractData>,
    /// The classes its docblock's `@throws` tags name.
    ///
    /// Empty for the overwhelming majority of declarations, which is why this is an arena slice and
    /// not a `Vec`: an empty one is two numbers. Kept because a *caller* has to know what a callee
    /// throws and cannot read the callee's docblock -- the index stores ranges rather than text.
    pub throws: IdSlice<TypeRefId>,
    /// The member chain its body hands back, when nothing else says what it returns.
    ///
    /// `None` for almost every declaration: only one whose signature *and* docblock are silent and
    /// whose returned expression names no type gets one. See [`ReturnedPathData`].
    pub returned_path: Option<Box<ReturnedPathData>>,
    /// Same rule as [`MemberData::body`].
    pub body: Option<BodyId>,
    pub name_range: TextRange,
    pub full_range: TextRange,
    pub docblock: Option<DocblockId>,
    /// Same rule as [`ClassData::is_conditional`].
    pub is_conditional: bool,
    /// The docblock marks it `@ignore` -- phpDocumentor's "not part of the API".
    ///
    /// Carried for the same reason and read the same way as [`Self::is_conditional`]: it loses a tie
    /// between two declarers of one name and does nothing at all where there is only one. See
    /// `Lowerer::doc_is_ignored`.
    pub is_ignored: bool,
    /// Whether the body reads arguments the signature does not name.
    ///
    /// `func_get_args()` / `func_num_args()`, which make the parameter list a lower bound rather
    /// than a contract -- PHP raises nothing when a caller supplies more. Recorded here because
    /// this is the last layer that can see a body at all: a closed file's statements are never
    /// retained, and from the parameter table alone such a declaration is indistinguishable from
    /// one that really takes what it names. See `phrost_lower::own_arguments`.
    pub reads_own_arguments: bool,
}

/// What a body hands back, described rather than typed.
///
/// For a declaration whose signature and docblock both say nothing and whose returned expression
/// names no type either -- `return $this->getDatabase()->select($table);`. A root and the member
/// names applied to it, which is small enough to store with the declaration and complete enough to
/// resolve later. See `Lowerer::returned_path_from_body`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ReturnedPathData {
    pub root: ReturnRootData,
    /// Member names in written order: `$this->a()->b()` is `[a, b]`. Never empty -- a root with no
    /// steps is a type, and `returned_from_body` already has it.
    pub steps: Box<[Name]>,
    /// The first step's first literal argument, when it wrote one.
    ///
    /// What a catalog contract means by "the argument": a service id, an entity type, a field name.
    /// `return \Drupal::service('utilities.manager');` names its class nowhere in the chain and
    /// entirely in that string, so a chain stored without it describes `Drupal::service()` and not
    /// what this one call gives back.
    ///
    /// Only the first step's. A key deeper in the chain decides a class the walk has already
    /// resolved its way to, and carrying one per step would be a list on every row for the sake of
    /// a shape nobody writes -- the idiom is a one-step chain off a container.
    pub key: Option<ReturnedKeyData>,
}

/// What a returned chain's first call was handed first, when a contract reads it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ReturnedKeyData {
    /// A string literal: a service id, an entity type, a field name -- what a catalog reads.
    Text(Name),
    /// `Foo::class`: what a factory's `class-string<T>` is bound to. A name as written, resolved at
    /// ingest against the file that wrote it, because nobody can resolve it once that file is shut.
    ///
    /// `$kernel = $this->createStub(HttpKernelInterface::class); ... return $kernel;` describes a
    /// stub *of that interface*, and a chain stored without the argument read as a bare `Stub` --
    /// so `$kernel::SUB_REQUEST` on the helper's result was an undefined constant.
    ///
    /// The path and its anchor rather than a recorded reference: the expression it was read from
    /// already references the name, and a second reference at the same place is reported twice by
    /// every check that walks them -- a deprecated class was.
    Class {
        path: Name,
        anchor: crate::NameAnchor,
    },
}

/// Where a returned chain starts.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ReturnRootData {
    /// `$this`, or a `static::`/`self::` call: the class the declaration is in.
    Receiver,
    /// A written class name -- `new Foo()` or `Foo::bar()` -- resolved against the declaring file.
    Named(NameRefId),
}

/// One parameter of a function, method, or property hook.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ParamData {
    /// Without the leading `$`, so it matches how the name is written everywhere else.
    pub name: Name,
    pub type_ref: Option<TypeRefId>,
    /// From the enclosing declaration's `@param`, and only when [`Self::type_ref`] is absent.
    pub doc_type_ref: Option<TypeRefId>,
    /// Whether the docblock said something about this parameter that this representation cannot
    /// hold -- which is not the same as saying nothing.
    ///
    /// One cause and it is the common one: a `@param` naming a template. `@param
    /// (\Closure($this): TWhenParameter)|TWhenParameter|null $value` is refused above, correctly,
    /// because lowering it would put a class called `TWhenParameter` in a tooltip -- and the empty
    /// slot it leaves reads exactly like an undocumented parameter to anybody downstream. The one
    /// reader that must tell the two apart is the rule that carries an overridden declaration's
    /// `@param` onto an override that wrote none: Laravel's `Collection::when()` documents its
    /// parameter as a template on the trait and as `bool` on the interface, and reading the
    /// template as silence carried the `bool` down over it.
    pub doc_type_refused: bool,
    /// Whether a default was written. The default's value is an expression and belongs with
    /// bodies; whether one exists decides whether the parameter is optional, which callers need.
    pub has_default: bool,
    /// The default's value, as it was written.
    ///
    /// `None` when there is none. Kept as text rather than as an expression because the one reader
    /// writes it back into a file: a stub for a method an interface requires has to reproduce the
    /// parameter exactly, and one that drops the default makes an optional parameter required and
    /// the class still does not load. That is why the repair used to *skip* such a method -- 21 % of
    /// the reference project's own interfaces and 30 % of Drupal core's hold one.
    ///
    /// 25 029 of a checkout's 148 917 parameters have one, and their text is 0.1 MB.
    pub default: Option<Box<str>>,
    pub is_variadic: bool,
    pub by_reference: bool,
    /// Constructor property promotion: `__construct(private readonly int $x)` declares a property
    /// as a side effect of declaring a parameter. Lowering also emits a [`MemberData`] for it, so
    /// member lookup does not have to know promotion exists.
    pub promotion: Option<ParamPromotion>,
    pub name_range: TextRange,
    pub full_range: TextRange,
    /// From `@param-out`, which says what a by-reference parameter leaves behind rather than what
    /// it takes.
    ///
    /// Last, and kept last in the cache codec beside it, for the reason §8 records: a codec's field
    /// order is its read order and a struct literal does not say so.
    pub out_type_ref: Option<TypeRefId>,
    /// From a `@param callable(A, B): C`: the types the callable's own *parameters* take, held as a
    /// `Generic { base: Callable, args: [A, B] }`.
    ///
    /// A fourth type per parameter, and a slot of its own for the same reason
    /// [`Self::out_type_ref`] is one: it is about a different moment. The three above say what this
    /// parameter takes and leaves; this says what whatever is passed here will be *called with*.
    ///
    /// The slot is what carries the positional meaning, which is why no new type node was needed.
    /// [`Self::type_ref`] holding a `Generic { Callable, [x] }` means `callable(): x` -- the one
    /// argument is the *return*; here the arguments are the parameters, in written order. Two
    /// spellings of `Generic { Callable, .. }` that mean different things is safe only because
    /// neither reader can see the other's slot.
    ///
    /// A position that said nothing -- no type written, or a type this representation cannot hold
    /// -- is a `mixed`, so the list is dense and a reader can index it. A trailing `...` is dropped
    /// rather than modelled: `callable(A, B...)` stores `[A, B]`, so a closure's third parameter
    /// answers nothing instead of answering `B`. Precision lost, never a wrong answer.
    ///
    /// Last, for the reason above it.
    pub callback_params_type_ref: Option<TypeRefId>,
    /// `@param-closure-this T $callback`: what `$this` is inside a closure passed here.
    ///
    /// The callee binds the closure before calling it -- Laravel's `Macroable::macro` and every
    /// `Manager::extend` -- so the class the closure is *written* in says nothing about its
    /// `$this`. Usually `$this` or `static`, meaning whatever the call was made on.
    pub closure_this_type_ref: Option<TypeRefId>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ParamPromotion {
    pub visibility: Visibility,
    pub set_visibility: Option<Visibility>,
    pub modifiers: MemberModifiers,
    /// Hooks written on the promoted parameter, which belong to the property it declares.
    pub hooks: IdSlice<HookId>,
}

/// One namespace-level `const` declaration.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ConstantData {
    pub name: Name,
    pub namespace: Option<NamespaceId>,
    pub name_range: TextRange,
    pub full_range: TextRange,
    pub docblock: Option<DocblockId>,
    /// Same rule as [`ClassData::is_conditional`].
    pub is_conditional: bool,
    /// The docblock marks it `@ignore` -- phpDocumentor's "not part of the API".
    ///
    /// Carried for the same reason and read the same way as [`Self::is_conditional`]: it loses a tie
    /// between two declarers of one name and does nothing at all where there is only one. See
    /// `Lowerer::doc_is_ignored`.
    pub is_ignored: bool,
    /// Whether `@deprecated` or `#[\Deprecated]` says the constant should not be reached for.
    ///
    /// Both, because PHP 8.5 allows attributes on constants and 8.5.10 reports reading one:
    /// `Constant X is deprecated, gone`. A `define()` takes no attribute, so there the docblock is
    /// the whole of what can say it.
    pub is_deprecated: bool,
    /// What the value states, read the way a class constant's is: `const LIMIT = 5` is an `int`,
    /// `define('ABSPATH', __DIR__ . '/')` a `string`.
    ///
    /// Only what the syntax settles. `const PHP_EOL = PHP_EOL;` -- which is how the stubs write
    /// every constant the engine fills in -- names another constant and says nothing here.
    pub ty: Option<TypeRefId>,
    /// What the value is built from, where [`Self::ty`] has nothing: a value naming another constant,
    /// or arithmetic the syntax does not settle alone. See [`ConstantValueData`].
    pub value: Option<ConstantValueData>,
}

/// What a constant's value is made of, kept so that whoever has the whole index can settle its type.
///
/// `define('HOUR_IN_SECONDS', 60 * MINUTE_IN_SECONDS)` is an `int` because `MINUTE_IN_SECONDS` is
/// one, and which constant that name reaches is a question about every file in the project rather
/// than about this one. So the shape is kept -- which parts, joined by which operators -- and the
/// type is read off it on demand, against the index it is asked in.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ConstantValueData {
    /// A part the syntax settles alone: a literal, a concatenation, an array.
    Settled(TypeRefId),
    /// Another constant, as written and where: the namespace and the imports in force at `at` are
    /// what resolve it. The name is not a reference of its own -- the initialiser's expression
    /// already records one -- so it is kept as text rather than as a [`NameRefId`].
    Constant {
        path: Name,
        anchor: crate::NameAnchor,
        at: phrost_model::TextSize,
    },
    /// `+`, `-`, `*`, `/` or `**` over two parts. A sign is the same thing with a zero beside it.
    Arithmetic {
        operator: crate::body::ArithmeticOp,
        operands: Box<[ConstantValueData; 2]>,
    },
    /// `&`, `|` or `^` over two parts -- an int for two ints and a string for two strings -- and `~x`
    /// as `x & x`, which PHP settles by the same rule.
    Bitwise(Box<[ConstantValueData; 2]>),
}

/// What a method's docblock proves once it has returned.
///
/// `@phpstan-assert-if-true \Drupal\...\BulkOrderApplication $this` on
/// `Application::isBulkOrder()`. A base class asking about its own subtype is how PHP writes
/// what other languages write with pattern matching, and the tag is the only place the answer is
/// written down -- the method body says `return $this instanceof X`, and bodies are not read across
/// files.
///
/// Two forms, because the two tags that get written say different things. One is about the receiver
/// and names a class; the other is about a parameter and only rules null out -- `@phpstan-assert
/// !null $actual` on PHPUnit's `assertNotNull`, which is how every test proves a nullable is there
/// before using it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ContractData {
    /// `@assert-if-true T $this`.
    Receiver {
        ty: TypeRefId,
        /// `true` for `@assert-if-true`. Which return value proves it.
        when_true: bool,
    },
    /// `@assert T $param` -- whatever the caller passed there is a `T` once the call returns.
    ///
    /// The four spellings in one shape, because they differ only in when the claim holds and in
    /// which direction it points. `@assert !null $param` is this with `null` and `negated`, which is
    /// the whole of what PHPUnit's `assertNotNull` promises.
    ///
    /// `@assert =T $x` is still absent: it needs the type argument of a `class-string<T>` parameter,
    /// which is a different job.
    Argument {
        parameter: Name,
        ty: TypeRefId,
        /// `!T` -- the value is *not* a `T` once the call returns.
        negated: bool,
        /// `None` for an unconditional `@assert`, which holds however the call returned. `Some` for
        /// the conditional spellings, naming the return value that proves it.
        when_true: Option<bool>,
        /// Which argument names the asserted class, when [`Self::Argument::ty`] is a name the
        /// declaration binds from a `class-string<T>` parameter rather than a class of its own.
        ///
        /// `PHPUnit::assertInstanceOf($expected, $actual)` is the shape: what `$actual` is proved to
        /// be is written at the *call*, as `Foo::class`, and the declaration only says which
        /// parameter carries it.
        from_class_string: Option<u16>,
    },
    /// `@return T` where `T` is bound by a `class-string<T>` parameter -- a factory, whose result is
    /// decided by the class named at the call.
    ReturnsArgumentClass {
        parameter: u16,
        /// The tag also admitted the value not being there: `@return ?T`, the `null|TNode`
        /// `NodeFinder` writes, or the `T|false` a `@method` tag can say.
        ///
        /// Carried here for the reason [`ReturnBranchData::ArgumentElement`] carries the same field,
        /// and typed the same way for the reason given there: that is where it was written. The
        /// sentinel belongs to the *declaration* and not to the argument -- `class-string<TNode>`
        /// says which class, and nothing about whether one was found -- so reading the class through
        /// it is a swap, exactly as `@return ?T` beside a native hint already is. Refusing the whole
        /// tag over the null was the old behaviour, and it made
        /// `findFirstInstanceOf($stmts, FuncCall::class)` a `?Node`, which reports `$first?->name`
        /// as an undefined property on correct code.
        absent: Option<phrost_model::Absence>,
        /// The tag put it in a container: `@return T[]`, `list<T>`, `array<int, T>`.
        ///
        /// `findInstanceOf` beside `findFirstInstanceOf`, and Doctrine's `findAll()`. The same
        /// distinction a framework rule spells `Forwarded::{Instance, ManyOf}`, and for the same
        /// reason: the declaration says `array` either way and only the tag says what is in it.
        many: bool,
        /// The tag wrote the template beside something else: `@return MockObject&T`.
        ///
        /// Both halves are true at once, and the other one is the declaration's own return type as
        /// well -- PHPUnit's `createMock(string $type): MockObject` says exactly that twice. So what
        /// travels is the *fact* that an intersection was written, and the reader composes the class
        /// the call named with the type the declaration returns. Nothing else can carry it: the
        /// operand is a class this contract has no slot for, and interning a rebuilt intersection
        /// needs the type table open for writing.
        beside: bool,
    },
    /// `@return Bar<T>` -- a class the declaration names, with arguments only the call can fill.
    ///
    /// Doctrine's `getRepository(): EntityRepository<T>` and every `@return Collection<T>` beside a
    /// `@param class-string<T>`. The one shape whose two halves come from different places: the class
    /// is written down and resolves against the file's imports like any other name, and what it holds
    /// is written at the call as `Foo::class`. Neither half is an answer alone -- the class on its own
    /// loses the element, and the element has nothing to hang off -- so this is the only contract that
    /// carries a class *with* arguments.
    ///
    /// Recorded only when a parameter settles at least one argument. `Bar<Baz>` names two real classes
    /// and is a plain return type, which the declaration already carries.
    ReturnsApplied {
        /// The class alone, lowered like any written type so the file's imports apply.
        class: TypeRefId,
        /// What fills each of its type parameters, in the order the tag wrote them.
        ///
        /// A [`ReturnBranchData`] because that is already the vocabulary for "read this off an
        /// argument", and a slot the declaration *did* write out is its `Ty`.
        arguments: Box<[ReturnBranchData]>,
        /// The tag put it in a container: `@return list<ReflectionAttribute<A>>`, which is every
        /// `getAttributes()` in the reflection API. The same field, and the same reading, as
        /// [`Self::ReturnsArgumentClass::many`].
        many: bool,
        /// Other classes the same arguments may fill instead: a union of one argument list over
        /// several classes. Eloquent's `Collection::map()` is `Support\Collection<TKey, TMapValue>|
        /// static<TKey, TMapValue>`, because what it hands back is a collection of models only when
        /// the callback made models. Empty for the common single class.
        alternatives: Box<[TypeRefId]>,
    },
    /// `@return ($p is null ? A : B)` -- a return type the call decides.
    ///
    /// Recorded only when the test is one an argument can settle. `$array is non-empty-array` is not
    /// one: nothing here tracks whether an array has elements, so that conditional is left to the
    /// reduction of its branches, which is a plain type and needs no contract.
    ConditionalReturn {
        /// Which argument the test is about, counted from zero.
        argument: u16,
        test: ConditionTestData,
        then: ReturnBranchData,
        otherwise: ReturnBranchData,
        /// Whether the declaration's own default for that parameter satisfies the test, for a call
        /// that writes no argument there -- which is the whole of what `response()` says.
        ///
        /// Decided here because this is where both halves are in hand: the default is an expression
        /// and belongs with bodies, and comparing it to the test needs the test. `None` when there is
        /// no default or when it is not a literal, and an absent argument then settles nothing.
        by_default: Option<bool>,
    },
    /// `@return ($output is 'ARRAY_A' ? A : ($output is 'ARRAY_N' ? B : C))` -- a chain of string
    /// tests on one argument, each with its own branch, and one for everything else.
    ///
    /// WordPress's getters are written this way: `get_post()` picks an array shape or a `WP_Post` by
    /// `$output`, which defaults to the constant `OBJECT`. One conditional carries one test and two
    /// branches, so a chain of them was a contract with nowhere to go and the function returned
    /// nothing. The default is not stored: it is on the parameter, and a constant's value is only
    /// known where the constant is declared.
    LiteralCases {
        argument: u16,
        cases: Vec<(Name, ReturnBranchData)>,
        otherwise: ReturnBranchData,
    },
    /// `@phpstan-return ($args is array{ count: true, ... } ? int : ($args is array{ fields: 'ids',
    /// ... } ? int[] : WP_Comment[]))` -- a chain of tests of any kind, the first that holds picking
    /// its branch. Each link names its own argument: `get_children()` tests `$args` and then
    /// `$output`.
    ///
    /// The general form of [`Self::LiteralCases`], which stays for the chain the completion text
    /// reader settles from characters. A chain of one conditional is [`Self::ConditionalReturn`];
    /// read as one, `get_comments()`'s nested test became the union of its branches and every
    /// comment `int|WP_Comment`.
    TestChain {
        cases: Vec<TestCaseData>,
        otherwise: ReturnBranchData,
    },
    /// `@return ($array is non-empty-array ? V : V|null)` -- a conditional whose test nothing here
    /// settles, but whose branches name one thing once null is set aside.
    ///
    /// `array_pop` is the shape, and PHP's array API is written this way throughout: what comes back
    /// is the element of what was passed, or nothing when there was nothing to take. Reduced here
    /// rather than at the call, because the reduction needs no argument -- every branch says the
    /// same, which is exactly why it is worth recording.
    AgreedReturn(ReturnBranchData),
    /// `@return A|B` where the operands are bound by *different* things.
    ///
    /// Symfony's `InputBag::get` is the shape: `@return TDefault|TInput`, where `TDefault` is the type
    /// of an argument and `TInput` is what the receiver was written with. Each half has a resolver
    /// that is complete for its own kind and blind to the other's, so a union spanning both resolved
    /// in neither -- one asks for the call and the other for the receiver, and nothing held both.
    ///
    /// Every operand has to answer, for the reason a list literal's every entry does: a union with an
    /// unresolved operand dropped is *narrower than the truth*, and a type narrower than the truth
    /// reports correct code.
    ///
    /// A [`ReturnBranchData`] per operand, which is already the vocabulary for "read this off an
    /// argument" -- and an operand the declaration wrote out, or one the receiver supplies, is its
    /// `Ty`. Recorded only when at least one operand needs a call to settle it; a union of plain
    /// types is a return type the declaration already carries.
    ReturnsOneOf(Box<[ReturnBranchData]>),
    /// `@phpstan-self-out self<U>` -- what the receiver holds once the call has returned.
    ///
    /// The receiver's twin of a `@param-out`, and the same shape as [`Self::ReturnsApplied`] made
    /// about a different value: a class written down -- usually `self` or `static`, which mean the
    /// declaring class and the class the call was made on -- and arguments the call fills. Laravel's
    /// `Collection::transform()` is `@phpstan-this-out static<TKey, TMapValue>`: the keys are the
    /// receiver's own, so that slot is a written `Ty` the reader substitutes, and the values are
    /// what the callback returns.
    ///
    /// Recorded whatever its slots hold, unlike a return: there is no declared type standing beside
    /// it to say the same thing, so a slot written out in full is still the only place it is said.
    ReceiverOut {
        class: TypeRefId,
        arguments: Box<[ReturnBranchData]>,
    },
    /// `@return ID_TABLE[T]` beside `@param T $type` -- the entry of a constant's table at the key
    /// the call passed.
    ///
    /// The constant is named here and its value is in whichever file declares it, so what the entry
    /// holds is read where the whole index is known: the table's own type, indexed by the key the
    /// call wrote -- or the parameter's default, for a call that writes none -- exactly as
    /// `ID_TABLE['immutable']` written in code is. A key nothing names leaves the declared return to
    /// answer.
    ConstantEntry {
        table: ConstantTableData,
        /// The parameter the key template is declared as.
        parameter: u16,
    },
}

/// The constant a [`ContractData::ConstantEntry`] indexes: a global one or a class's.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ConstantTableData {
    /// `ID_TABLE`, as written: the namespace and imports of the declaration resolve it, the way
    /// [`ConstantValueData::Constant`] is resolved.
    Global {
        path: Name,
        anchor: crate::NameAnchor,
    },
    /// `self::TABLE`, `Table::TABLE` -- the class lowered like any written name, so a relative one
    /// means what it means on a return type.
    Member { class: TypeRefId, name: Name },
}

/// What a conditional return type compares its subject against.
///
/// The whole of what a call can settle from an argument. Every other test a docblock can write --
/// `list<V>`, `non-empty-array`, `array<Foo>` -- is about a value's contents rather than about the
/// value, and reading one from a call site would take the argument's own type rather than what was
/// written there.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ConditionTestData {
    Null,
    True,
    False,
    /// `"foo"`, compared against the string the call wrote.
    Literal(Name),
    /// `class-string<T>`, true when the argument names a class -- and, where what it is written
    /// over is a class rather than a template, one of that class or below it.
    ///
    /// Symfony's `FormFactoryInterface::create()` is `($type is class-string<FormFlowTypeInterface>
    /// ? FormFlowInterface : FormInterface)`, and read as "names a class" every `create(TextType::class)`
    /// took the flow branch.
    ClassString(Option<crate::TypeRefId>),
    /// `string`, `array`, `int` -- what sort of value the argument is.
    ///
    /// Bare only. `array<Foo>` is a claim about what the array *contains*, and answering it with
    /// "yes, an array" would take the then-branch on every array there is.
    OfKind(phrost_model::BuiltinTy),
    /// `4`, compared against the number the call wrote -- or against the value of the constant it
    /// wrote, which is how a flag is written and never as the number it stands for.
    Number(i64),
    /// `non-empty-array`, `non-empty-list` -- true when the argument is known to hold something.
    ///
    /// Bare only, for [`Self::OfKind`]'s reason. Settled from what a *guard* proved rather than
    /// from the argument's declared type: an empty array is falsy in PHP, so "not empty" is exactly
    /// "truthy", and `if (empty($rows)) { return; }` above the call is the whole of the evidence.
    NonEmpty,
    /// `list{}` or `array{}`: the argument holds nothing.
    ///
    /// Its own test rather than [`Self::NonEmpty`] with the branches swapped, because the two are
    /// settled from different evidence. Mockery's `shouldReceive(...$methodNames)` is
    /// `($methodNames is list{} ? HigherOrderMessage : Expectation)`, and for a variadic parameter
    /// the call answers it exactly both ways -- whether it wrote anything at that position -- where
    /// "not empty" is only ever proved by a guard.
    Empty,
    /// `non-empty-string`: a string holding anything at all.
    ///
    /// Settled from what the call *wrote*, because no type phrost keeps says it: a string literal
    /// answers it by its text, and a string built by interpolation or concatenation answers yes
    /// when some piece of it was written non-empty. `null` answers no, being no string. A value
    /// read from anywhere else settles nothing.
    NonEmptyString,
    /// `non-falsy-string` (and `truthy-string`): a string that is neither `''` nor `'0'`. Settled
    /// as [`Self::NonEmptyString`] is. WordPress keys `wpdb::get_results()` on it, and every
    /// `"SELECT ..."` a call writes answers yes.
    NonFalsyString,
    /// `array{ fields: 'ids', ... }`: an array holding one key, with one of the literal values
    /// listed. WordPress decides what `get_posts()`, `get_terms()` and a dozen more return this
    /// way, and a call settles it from the array it wrote: nearly every one leaves the key out.
    ///
    /// One key only, which is every such test WordPress writes; a shape naming two keys is a
    /// question about both, which nothing here needs to answer yet.
    Shape {
        key: Name,
        /// What the key's value may be. Literal types, matched as PHP would match them -- `0` is not
        /// `'0'` and neither is `false`.
        accepts: Box<[ShapeLiteralData]>,
        /// Written without `...`: an array holding any other key does not pass.
        sealed: bool,
    },
    /// `Arrayable|array` -- a kind, or an instance of a class. Eloquent's `find()` is the shape:
    /// a list of keys or an `Arrayable` of them gives a collection, and anything else one model.
    ///
    /// Its own test rather than a union of two, because the union is what settles it: an `int`
    /// passes neither half, and that is the answer every `User::find($id)` wants.
    OfKindOrInstance {
        kind: phrost_model::BuiltinTy,
        class: crate::TypeRefId,
    },
    /// `\Closure`, `WP_Post` -- an instance of one class.
    ///
    /// Symfony's config builders are the shape: `validate(?\Closure $closure = null)` hands back the
    /// builder itself when given a closure and an `ExprBuilder` to chain on when given nothing, and
    /// every config tree calls it the second way.
    Instance(crate::TypeRefId),
    /// `scalar` -- an int, a float, a string or a bool, which no single kind names. Symfony's
    /// `resolveValue()` hands back a scalar for a scalar and an array of them for anything else.
    Scalar,
}

/// One link of a [`ContractData::TestChain`]: a test, the branch it picks, and what the parameter's
/// default answers for a call that leaves the argument out -- as on
/// [`ContractData::ConditionalReturn`].
///
/// `negated` picks the branch where the test *fails*: `($x is not T ? A : ...)`, and also
/// `($x is T ? (...) : A)`, which is the same chain with the rest nested on the other side --
/// `wpdb::get_row()` tests its query first and only then, inside the passing branch, its output.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TestCaseData {
    pub argument: u16,
    pub test: ConditionTestData,
    pub negated: bool,
    pub then: ReturnBranchData,
    pub by_default: Option<bool>,
}

/// A literal value a [`ConditionTestData::Shape`] test accepts under its key.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ShapeLiteralData {
    String(Name),
    Int(i64),
    True,
    False,
    Null,
}

/// One branch of a conditional return type.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ReturnBranchData {
    Ty(TypeRefId),
    /// A `@template` the declaration binds from a `class-string<T>` parameter: which class the branch
    /// names is written at the call, exactly as [`ContractData::ReturnsArgumentClass`] describes.
    ArgumentClass(u16),
    /// A `@template` the declaration binds from the *element* of a parameter: `@param array<K, V>
    /// $array` with `@return V`.
    ///
    /// Not a class but a type, and not written at the call either -- what decides it is the type of
    /// the argument, so the reader has to resolve it.
    /// A `@template` the declaration binds from a parameter's own type: `@param T $item` with
    /// `@return T`.
    ///
    /// Whatever was passed is what comes back, which is the plainest generic there is and the one
    /// every collection helper in a project is written as.
    ArgumentType {
        parameter: u16,
        /// What the *declaration's own default* for that parameter states, for a call that writes no
        /// argument there.
        ///
        /// `InputBag::get(string $key, mixed $default = null)` is the shape and the reason: PHP puts
        /// `null` in when the second argument is left out, so `get('token')` returns `?string` where
        /// `get('token', '')` returns `string`. Measured on one real project: **5 calls write the
        /// default and 62 leave it out**, so reading the argument alone answered the smaller half.
        ///
        /// Only what a *literal* default states, which is the only kind a declaration can be read for
        /// -- and `null` is nearly all of them. `None` for a parameter with no default, or one whose
        /// default is an expression, and then an absent argument settles nothing.
        absent: Option<phrost_model::BuiltinTy>,
        /// Whether the parameter also takes a callable producing the value, which the callee calls
        /// -- `TDefault|(\Closure(): TDefault)`. A closure passed there is the value it returns.
        evaluated: bool,
    },
    ArgumentElement {
        parameter: u16,
        /// The branch also admitted a value not being there, and *which way* it said so. Carried on
        /// the branch because that is where it was written: `@return V|null` says one thing about
        /// `V` and one about the branch.
        ///
        /// Which one matters to the guard a reader has to write, so the two are not one flag -- see
        /// [`phrost_model::Absence`].
        absent: Option<phrost_model::Absence>,
    },
    /// What names one slot of a container parameter: `@param array<K, V> $array` with `@return K`
    /// or `@return K|null`.
    ///
    /// `array_key_first` and `array_key_last`, and the single-key twin of [`Self::ArgumentKeys`]:
    /// the key is decided by how the argument was written, never by a template a caller can name, so
    /// the reader asks the question `foreach ($x as $k)` asks. Without it the return was the bare
    /// `K`, which names no class and fell back to the declared `string|int|null`.
    ArgumentKey {
        parameter: u16,
        /// See [`Self::ArgumentElement::absent`].
        absent: Option<phrost_model::Absence>,
    },
    /// The same element, back in a container of its own: `@param array<K, V> $array` with
    /// `@return list<V>`.
    ///
    /// `array_values`, `array_reverse` and `array_slice` all say this and each says it with a
    /// different spelling of the container -- which is why the claim is about the elements and not
    /// about the container: what the branches agree on is what comes out of it.
    ArgumentElements {
        parameter: u16,
    },
    /// Whatever one parameter was passed, in a container of its own: `@param T $value` with
    /// `@return list<T>`.
    ///
    /// `array_fill`, whose four branches are four keyings of one claim -- a container of the value
    /// it was handed. The plural of [`Self::ArgumentType`] the way [`Self::ArgumentElements`] is the
    /// plural of [`Self::ArgumentElement`]: the template is the argument's own type rather than what
    /// it holds, so nothing that reads a container parameter can bind it.
    ///
    /// Keyed by `int` -- a list, or an `array<int, T>` -- which is the only keying the reader's
    /// container states. A branch keyed any other way is not this.
    ArgumentTypes {
        parameter: u16,
    },
    /// A container keyed by the elements of one parameter and holding the elements of another:
    /// `@param array<K> $keys` and `@param array<V> $values` with `@return array<K, V>`.
    ///
    /// `array_combine`, and the shape [`Self::ArgumentElements`] refuses: its key is the *values*
    /// argument's own, which here would claim the list's `int` where the call says `string`.
    CombinedElements {
        keys: u16,
        values: u16,
    },
    /// A container of the *keys* of a container parameter: `@param array<K, V> $array` with
    /// `@return list<K>`.
    ///
    /// `array_keys`, and it is the one shape [`Self::ArgumentElements`] cannot express: what binds
    /// `K` is the first type argument of the parameter, not the last, and PHP decides what a key is
    /// from how the container was written rather than from what is in it -- a `list<Row>` is keyed by
    /// `int` and an `array<string, Row>` by `string`, and neither says so in an argument named `K`.
    /// So the branch carries the parameter and the reader asks the same question `foreach ($x as $k)`
    /// asks.
    ArgumentKeys {
        parameter: u16,
    },
    /// A container of one column of a container of rows: `@param array<array-key, array<K, V>>
    /// $array` beside `@param K $column_key`, with `@return array<array-key, V>`.
    ///
    /// `array_column`. The declaration can say only that the column is some value of some row; the
    /// call says which -- `array_column($rows, 'name')` over `list<array{id: int, name: string}>` is a
    /// list of strings, and over a list of objects it is what that property holds.
    ArgumentColumn {
        /// The rows.
        parameter: u16,
        /// The column's key.
        column: u16,
    },
    /// A container of whatever the callable at this position gives back: `@param (callable(V): U)
    /// $callback` with `@return list<U>`.
    ///
    /// `array_map`, and PHP has no other `map`. The declaration cannot name the result -- it is
    /// written at the call, in the closure -- so the reader reads the closure's own return type.
    CallbackResults {
        parameter: u16,
    },
    /// The same, without the container: `@param callable(I, V): R $callback` with `@return R`.
    ///
    /// Every `reduce` in every collection library. What comes out is what the callback produced, and
    /// again the closure at the call is the only place that type is written.
    CallbackResult {
        parameter: u16,
    },
    /// A conditional written as one slot of a returned class, which the call settles as it would
    /// a whole return: `static<($groupBy is string ? array-key : TGroupKey), ...>` on Laravel's
    /// `Collection::groupBy()`, whose key is any key for a column name and what the callback
    /// returns for a callback.
    Conditional(Box<ConditionalBranchData>),
    /// A class with arguments written as one slot of another: the `static<..., TValue>` each group
    /// of a `groupBy()` is. Its own slots are read the way the outer class's are.
    Applied {
        class: TypeRefId,
        arguments: Box<[ReturnBranchData]>,
    },
    /// A branch of a conditional slot this cannot read -- a test on a template a callback binds,
    /// `TGroupKey is \UnitEnum` -- which the slot's own template bound stands in for. Only inside a
    /// conditional slot, so a contract that reads one branch is not refused over the other.
    Unfilled,
}

/// One conditional slot: the fields [`ContractData::ConditionalReturn`] carries, about a slot
/// rather than a whole return.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ConditionalBranchData {
    pub argument: u16,
    pub test: ConditionTestData,
    pub then: ReturnBranchData,
    pub otherwise: ReturnBranchData,
    pub by_default: Option<bool>,
}

/// One `@template T of Bound` on a class-like declaration.
///
/// PHP has no generics, so this is the only place a container says what it holds. Drupal's
/// `FieldItemListInterface` is `@template T of FieldItemInterface` and nothing else in the language
/// states it -- which is why iterating one used to yield nothing at all.
///
/// The bound, not the argument. Substituting an argument through a chain of `@extends` is a
/// separate and much larger job; the bound is what the declaration promises about every possible
/// argument, so using it offers fewer members than the real type has and never more.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TemplateData {
    pub name: Name,
    /// `of Bound`. Absent when the parameter is unbounded, which says nothing and is stored as
    /// nothing.
    pub bound: Option<TypeRefId>,
    /// Range of the name inside the tag, so it is a real position like every other range here.
    pub name_range: TextRange,
}

/// The array a property's default writes, kept because a framework reads it.
///
/// Lowering keeps no property defaults -- a default is a value, and the index stores declarations --
/// except under the names a framework hands down (`KeyedAttributes::keeping_defaults_of` in
/// lowering). Eloquent's `$casts = ['published_at' => 'datetime']` is the one statement anywhere
/// that `$post->published_at` is a date, and `$fillable = ['title']` that `title` is an attribute.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct KeptTableData {
    /// The class the property is declared in.
    pub class: ClassId,
    /// The property's name -- or, where [`Self::from_attribute`], the attribute's last segment.
    pub property: Name,
    /// Whether this is a list an attribute on the class writes rather than a property's default:
    /// Laravel 12's `#[Fillable(['name', 'email'])]`, which says what `$fillable` used to.
    pub from_attribute: bool,
    pub entries: Box<[KeptEntryData]>,
}

/// The string a property's default writes, kept because a framework reads it.
///
/// [`KeptTableData`]'s sibling for a default that is one string rather than an array: Laravel's
/// `protected $signature = 'mail:send {user} {--queue=}'` is the one statement anywhere of what
/// `$this->argument('user')` holds. Kept under the same names, which a framework hands down.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct KeptTextData {
    /// The class the property is declared in.
    pub class: ClassId,
    pub property: Name,
    /// The literal's value, unquoted.
    pub text: Box<str>,
    /// Where it is written, inside the quotes: what is declared there is declared at a place in it.
    pub range: TextRange,
}

/// A property default lowering was asked to keep that names a class: `protected $model =
/// User::class;`. The name as written, resolved against the file's imports where the index reads it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct KeptClassData {
    /// The class the property is declared in.
    pub class: ClassId,
    pub property: Name,
    pub named: crate::ids::NameRefId,
}

/// An object a method builds to declare a named entry, which lowering was asked to keep: Twig's
/// `new TwigFilter('upper', 'strtoupper')` in an extension's `getFilters()`.
///
/// Kept from bodies that are otherwise not lowered -- a dependency's -- because the entry is
/// declared nowhere else: Twig's own filters are in its `CoreExtension`, under `vendor/`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct KeptConstructionData {
    /// The class declaring the method.
    pub class: ClassId,
    /// The class constructed, as written.
    pub constructed: crate::ids::NameRefId,
    /// The name the first argument writes, and where, inside its quotes.
    pub key: Name,
    pub key_range: TextRange,
    pub callable: KeptCallableData,
}

/// What a kept construction runs, as its second argument writes it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum KeptCallableData {
    /// `'strtoupper'`: a function, by its name as written. Always from the root: a callable string
    /// is not resolved against the file's namespace.
    Function(Name),
    /// `[self::class, 'format']`, `[$this, 'format']`, `[Foo::class, 'format']`: a method, on the
    /// class declaring the construction where `class` is `None`.
    Method {
        class: Option<crate::ids::NameRefId>,
        method: Name,
    },
    /// `'Foo::format'`: a method, its class written in the string and so from the root.
    NamedMethod { class: Name, method: Name },
    /// A closure, `null`, or anything else: nothing to name.
    Unnamed,
}

/// One entry of a [`KeptTableData`].
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct KeptEntryData {
    /// The name the entry is about: the key of `'published_at' => 'datetime'`, or the value of a
    /// list's `'title'`.
    pub name: Name,
    /// Where that name is written, inside its quotes -- what goto lands on and rename rewrites.
    pub range: TextRange,
    /// What the key holds, for a keyed entry. `None` for a list's.
    pub value: Option<KeptValueData>,
}

/// The value a [`KeptEntryData`] writes, as far as a literal says it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum KeptValueData {
    /// A string literal's text, unquoted: `'datetime'`, `'decimal:2'`.
    Text(Box<str>),
    /// `Status::class`, resolved against the file's imports where the index reads it.
    Class(NameRefId),
    /// Anything else -- a call, a constant, a nested array. The entry is still there.
    Other,
}

/// One string-literal argument of an attribute.
///
/// Literals only. An argument that is an expression has no value here to record, and that is silence
/// rather than a guess.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct AttributeLiteralData {
    /// The attribute's name exactly as written -- `ContentEntityType`, or a qualified spelling of it.
    ///
    /// As written rather than resolved: resolving it would mean running the def-map over every
    /// attribute in every file during ingest, and the reader compares the last segment, which is how
    /// an attribute is written in the first place.
    pub attribute: Name,
    /// Which argument of the attribute this is.
    pub argument: AttributeArgument,
    /// What the literal says. For an entry of an array argument, the entry's *value*.
    pub value: Box<str>,
    /// The key this entry was written under, when the argument is an array a framework asked to
    /// read the keys of.
    ///
    /// `None` for an argument written as a bare literal, which is nearly all of them. Only the
    /// arguments a rule names are entered at all -- keeping every array argument's entries would
    /// cost a project thousands of strings for the few any rule reads, which is the same trade the
    /// positional gate makes.
    pub key: Option<Box<str>>,
}

/// Which argument of an attribute a literal was written at.
///
/// Two arms because PHP lets one argument be written two ways and both appear in real code.
/// `#[ContentEntityType(id: 'node')]` names it, so the name is what a contract matches; `#[FormElement('textfield')]`
/// does not, and the only thing left to match is where it sits in the list.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum AttributeArgument {
    /// `id: 'node'` -- the name as the call wrote it.
    Named(Name),
    /// `'textfield'` -- counted from zero over the whole argument list, named and positional alike,
    /// so the number means the same thing a reader of the source would count.
    At(u16),
}

/// One conflict-resolution clause inside `use SomeTrait { ... }`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TraitAdaptationData {
    /// The trait the method is taken from, when written as `A::m`. `None` for a bare `m`, which
    /// is only valid when exactly one used trait declares it.
    pub trait_name: Option<NameRefId>,
    pub method: Name,
    pub method_range: TextRange,
    pub kind: TraitAdaptationKind,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum TraitAdaptationKind {
    /// `A::m insteadof B, C;` — the listed traits lose.
    Precedence { excluded: IdSlice<NameRefId> },
    /// `A::m as [visibility] [alias];` — either or both parts may be present.
    Alias {
        alias: Option<Name>,
        /// Where the alias is written: the place the name comes into being, which a search for it
        /// lists and a rename of it edits.
        alias_range: Option<TextRange>,
        visibility: Option<Visibility>,
    },
}

/// The prefix lowering gives the class an anonymous `new class { ... }` declares.
///
/// One constant rather than a spelling in each reader: the name is synthesised, and every layer
/// that has to tell a written name from a synthesised one asks here.
pub const ANONYMOUS_CLASS_PREFIX: &str = "class@anonymous#";

/// Whether a name is one lowering synthesised for an anonymous class.
///
/// Compared against the short name as well as the qualified one, because the class is claimed in
/// whatever namespace it was written in and both spellings reach readers.
///
/// Without the prefix's trailing `#`, so that a name already run through [`display_class_name`]
/// still answers yes: an outline hands a reader `class@anonymous` and a caller asking this of what
/// the outline gave it was told no. `@` is not a character PHP allows in an identifier, so nothing
/// a project declares can be mistaken for one.
pub fn is_anonymous_class_name(path: &str) -> bool {
    let synthesised = ANONYMOUS_CLASS_PREFIX.trim_end_matches('#');
    path.rsplit('\\')
        .next()
        .is_some_and(|short| short.starts_with(synthesised))
}

/// The name of an anonymous class as a person should read it.
///
/// The synthesised name carries the offset its `class` keyword sits at, which is what makes it
/// unique and deterministic -- and which means nothing to a reader. PHP's own errors say
/// `class@anonymous`, so reports and hovers do too; the offset stays in the index, where telling
/// two of them apart is the whole point.
pub fn display_class_name(path: &str) -> &str {
    match path.rfind(ANONYMOUS_CLASS_PREFIX) {
        Some(at) => &path[..at + ANONYMOUS_CLASS_PREFIX.len() - 1],
        None => path,
    }
}
