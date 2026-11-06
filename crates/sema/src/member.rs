use phrost_model::{Name, TextRange, Visibility};
use phrost_ty::{TypeId, TypeTable, compat::Fit};

/// One member as a single file declared it, before any inheritance is considered.
///
/// This is what the index stores per class: enough to answer navigation, completion, hover, and —
/// once there is a type layer — inference, without re-reading the file.
///
/// Types are stored as [`TypeId`]s into the project's [`TypeTable`] rather than as rendered text.
/// Text can only be displayed; an id can also be reasoned about, which is what "what does
/// `$repo->find()` return" needs. The interning is what makes it affordable: a project has hundreds
/// of thousands of members and a few thousand distinct types.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct MemberRow {
    pub name: Name,
    pub kind: MemberKind,
    pub visibility: Visibility,
    /// PHP 8.4 asymmetric visibility: the `private(set)` half.
    pub set_visibility: Option<Visibility>,
    pub modifiers: MemberModifiers,
    /// Return type for a method, declared type for a property, constant, or case.
    ///
    /// `None` means unannotated natively; [`Self::doc_ty`] may still say what it is.
    pub ty: Option<TypeId>,
    /// The same, from a docblock, and only when [`Self::ty`] is absent.
    ///
    /// Two fields because which one said it matters: a native declaration is what the runtime
    /// enforces, a docblock is a claim about code that predates it. Use [`Self::effective_ty`] when
    /// the question is only "what is this".
    pub doc_ty: Option<TypeId>,
    /// Whether [`Self::doc_ty`] came from the body rather than from a docblock.
    ///
    /// See [`phrost_hir::MemberData::type_from_body`]: an override inherits the declaration it
    /// overrides, and an inherited claim beats what this file's own body happens to hand back.
    pub doc_ty_from_body: bool,
    /// Whether the docblock wrote a type here that was refused for naming one of the declaration's
    /// own templates. See [`phrost_hir::MemberData::doc_type_refused`].
    pub doc_ty_refused: bool,
    /// Whether [`Self::ty`] was read off the value rather than written in the declaration.
    ///
    /// PHP only let a constant carry a type in 8.3, so nearly every constant in a project has none
    /// and the value is the only thing that says what it is. Taking it is what makes
    /// `NodeInterface::PUBLISHED` an `int` instead of nothing -- but it is *not* what the author
    /// wrote, so a declaration rendered as text must leave it out. Printing `const int PUBLISHED`
    /// over source that says `const PUBLISHED` puts a type in a card that promises to show what is
    /// there, and offers a reader on PHP 8.1 something their runtime will reject.
    pub ty_from_value: bool,
    /// Whether the body reads arguments the signature does not name.
    ///
    /// `func_get_args()` / `func_num_args()`, which make [`Self::params`] a lower bound rather than
    /// a contract: PHP raises nothing when a caller supplies more, and the extra values come back
    /// through the call rather than through a name. Laravel's
    /// `PendingRequest::throwIf($condition)` reads `func_get_args()[1]`, and its own tests pass two
    /// arguments to it on purpose.
    ///
    /// A fact about the *body*, kept here because this is the only side of the boundary that can
    /// hold one: a closed file's statements are never retained, so nothing above can look. Read by
    /// `IndexView::callee_takes_more_than_it_names` and nothing else.
    pub reads_own_arguments: bool,
    /// Parameters, for a method. Empty for every other kind.
    pub params: Box<[ParamRow]>,
    /// Range of the name alone — the goto target, kept for every file including vendor so that
    /// navigation never has to fall back to guessing.
    pub name_range: TextRange,
    pub full_range: TextRange,
    /// Where the docblock is, if there is one.
    ///
    /// The range rather than the text. Docblock text for every member of every vendor class is tens
    /// of megabytes on a real project, so it is read on demand from the range instead.
    pub docblock_range: Option<TextRange>,
    /// What this method's docblock promises beyond its signature.
    ///
    /// Stored rather than read on demand, because it is asked at a guard -- which is on the path a
    /// keystroke takes -- and reading it from the range would be a disk read per lookup.
    ///
    /// Boxed because it is vanishingly rare -- seven methods in the project this was measured on,
    /// out of a quarter of a million rows -- and the widest arm is three times the size of a
    /// pointer. Inline, the empty option cost 4 MB of resident memory on a real project.
    pub contract: Option<Box<Contract>>,
    /// Declared by an `@method` or `@property` tag rather than by PHP.
    ///
    /// The member is real -- it answers at runtime -- but its declaration is a claim rather than
    /// code, and a reader deserves to be told that when goto lands inside a comment.
    pub from_docblock: bool,
}

/// What a declaration promises beyond what its signature says.
///
/// One slot for all of them, and it is the reason they share one: each is vanishingly rare and the
/// slot is already a pointer, so a second field would cost every one of a quarter of a million rows
/// eight bytes to hold nothing. A declaration carrying two of these at once is one method in a real
/// tree, and lowering ranks its return contract first -- see [`Self::ReceiverOut`].
///
/// The resolved half of [`phrost_hir::ContractData`], with the class already looked up against the
/// declaring file's imports.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Contract {
    /// `@assert-if-true T $this`.
    Receiver {
        ty: TypeId,
        /// `true` for `@assert-if-true`. Which return value proves it.
        when_true: bool,
    },
    /// `@assert T $param`, by parameter name -- the caller matches it to an argument.
    Argument {
        parameter: Name,
        ty: TypeId,
        /// `!T` -- the value is *not* a `T` once the call returns.
        negated: bool,
        /// `None` for an unconditional `@assert`. `Some` names the return value that proves it.
        when_true: Option<bool>,
        /// Which argument names the asserted class, when [`Self::Argument::ty`] is a name the
        /// declaration binds from a `class-string<T>` parameter rather than a class of its own.
        from_class_string: Option<u16>,
    },
    /// `@return T` where `T` is bound by a `class-string<T>` parameter.
    ///
    /// A factory: `create(Foo::class)` returns a `Foo` and `create(Bar::class)` a `Bar`, and the
    /// declaration can only say which argument decides. What it returns is written at the call.
    ReturnsArgumentClass {
        /// Which argument names it, counted from zero.
        parameter: u16,
        /// The tag also admitted the value not being there, and which way it said so -- `@return ?T`,
        /// `null|T`, or a `@method` tag's `T|false`. The class is still whichever the call named; see
        /// [`phrost_hir::ContractData::ReturnsArgumentClass`].
        absent: Option<phrost_model::Absence>,
        /// The tag put it in a container -- `@return T[]`. Same source.
        many: bool,
        /// The tag wrote the template beside something else -- `@return MockObject&T`. Same source,
        /// and the reader composes the named class with what the declaration itself returns.
        beside: bool,
    },
    /// `@return Bar<T>` -- a class the declaration names, with arguments only the call can fill.
    ///
    /// The resolved half of [`phrost_hir::ContractData::ReturnsApplied`]. What comes back is an
    /// instance of `class` carrying whatever the branches resolved to, which is the only way a reader
    /// can express `Bar<Widget>` at all: interning one would need the type table open for writing.
    ReturnsApplied {
        class: TypeId,
        /// One per type parameter the tag wrote, in order.
        arguments: Box<[ReturnBranch]>,
        /// Inside a container: a list of those instances rather than one.
        many: bool,
        /// Other classes the same arguments may fill instead. See
        /// [`phrost_hir::ContractData::ReturnsApplied`].
        alternatives: Box<[TypeId]>,
    },
    /// `@return ($p is null ? A : B)` -- a return type the call decides.
    ///
    /// The resolved half of [`phrost_hir::ContractData::ConditionalReturn`], and it carries only the
    /// tests an argument can settle: what a conditional says when nothing settles it is already in
    /// the declared return type, which is the union of its branches.
    ConditionalReturn {
        /// Which argument the test is about, counted from zero.
        argument: u16,
        test: ConditionTest,
        then: ReturnBranch,
        otherwise: ReturnBranch,
        /// Whether the declaration's own default settles the test for a call that writes no argument
        /// there. See [`phrost_hir::ContractData::ConditionalReturn`].
        by_default: Option<bool>,
    },
    /// A chain of string tests on one argument. See [`phrost_hir::ContractData::LiteralCases`].
    LiteralCases {
        argument: u16,
        cases: Box<[(Name, ReturnBranch)]>,
        otherwise: ReturnBranch,
    },
    /// A chain of tests on one argument. See [`phrost_hir::ContractData::TestChain`].
    TestChain {
        cases: Box<[TestCase]>,
        otherwise: ReturnBranch,
    },
    /// `@return ($array is non-empty-array ? V : V|null)` -- a conditional whose test nothing settles
    /// but whose branches agree. See [`phrost_hir::ContractData::AgreedReturn`].
    AgreedReturn(ReturnBranch),
    /// `@return A|B` whose operands are bound by different things -- the resolved half of
    /// [`phrost_hir::ContractData::ReturnsOneOf`]. One operand may need the call and another the
    /// receiver, and every one has to answer.
    ReturnsOneOf(Box<[ReturnBranch]>),
    /// `@phpstan-self-out self<U>` -- what the receiver holds once the call has returned. The
    /// resolved half of [`phrost_hir::ContractData::ReceiverOut`]; `class` may be relative, and
    /// means the declaring class or the one the call was made on as any stored type does.
    ReceiverOut {
        class: TypeId,
        /// One per type parameter the tag wrote, in order. Empty for a class written bare.
        arguments: Box<[ReturnBranch]>,
    },
    /// `@return ID_TABLE[T]` -- the entry of a constant's table at the key the call passed. The
    /// resolved half of [`phrost_hir::ContractData::ConstantEntry`].
    ConstantEntry {
        table: ConstantTable,
        parameter: u16,
    },
}

/// The constant a [`Contract::ConstantEntry`] indexes.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ConstantTable {
    /// The names PHP would try, in its order -- the current namespace, then the global one for an
    /// unqualified name.
    Global(Box<[Name]>),
    /// A class constant. `class` may be relative, as on any stored type.
    Member { class: TypeId, name: Name },
}

/// What a conditional return type compares its subject against.
///
/// See [`phrost_hir::ContractData::ConditionalReturn`] for why this is the whole vocabulary.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ConditionTest {
    Null,
    True,
    False,
    /// `"foo"`, compared against the string the call wrote.
    Literal(Name),
    /// `class-string<T>`, true when the argument names a class.
    ClassString(Option<phrost_ty::TypeId>),
    /// `string`, `array`, `int` -- what sort of value the argument is. Bare only; see
    /// [`phrost_hir::ConditionTestData::OfKind`].
    OfKind(phrost_model::BuiltinTy),
    /// `4`, compared against the number a call wrote. See [`phrost_hir::ConditionTestData::Number`].
    Number(i64),
    /// `non-empty-array`, `non-empty-list`. See [`phrost_hir::ConditionTestData::NonEmpty`].
    NonEmpty,
    /// `list{}`, `array{}`. See [`phrost_hir::ConditionTestData::Empty`].
    Empty,
    /// See [`phrost_hir::ConditionTestData::NonEmptyString`].
    NonEmptyString,
    /// See [`phrost_hir::ConditionTestData::NonFalsyString`].
    NonFalsyString,
    /// `array{ fields: 'ids', ... }`. See [`phrost_hir::ConditionTestData::Shape`].
    Shape {
        key: Name,
        accepts: Box<[ShapeLiteral]>,
        sealed: bool,
    },
    /// `Arrayable|array`. See [`phrost_hir::ConditionTestData::OfKindOrInstance`].
    OfKindOrInstance {
        kind: phrost_model::BuiltinTy,
        class: phrost_ty::TypeId,
    },
    /// `\Closure`. See [`phrost_hir::ConditionTestData::Instance`].
    Instance(phrost_ty::TypeId),
    /// `scalar`. See [`phrost_hir::ConditionTestData::Scalar`].
    Scalar,
}

/// One link of a [`Contract::TestChain`]. See [`phrost_hir::TestCaseData`].
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TestCase {
    pub argument: u16,
    pub test: ConditionTest,
    /// See [`phrost_hir::TestCaseData::negated`].
    pub negated: bool,
    pub then: ReturnBranch,
    pub by_default: Option<bool>,
}

/// A literal a [`ConditionTest::Shape`] accepts. See [`phrost_hir::ShapeLiteralData`].
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ShapeLiteral {
    String(Name),
    Int(i64),
    True,
    False,
    Null,
}

/// One branch of a conditional return type.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ReturnBranch {
    Ty(TypeId),
    /// A `@template` bound by a `class-string<T>` parameter: which class it is, is written at the
    /// call. The same claim [`Contract::ReturnsArgumentClass`] makes, for one branch of two.
    ArgumentClass(u16),
    /// A `@template` bound by a parameter's own type: whatever was passed is what comes back.
    ArgumentType {
        parameter: u16,
        /// What the declaration's own default states, for a call that leaves the argument out. See
        /// [`phrost_hir::ReturnBranchData::ArgumentType`].
        absent: Option<phrost_model::BuiltinTy>,
        /// Whether a callable passed there is the value it returns. See
        /// [`phrost_hir::ReturnBranchData::ArgumentType`].
        evaluated: bool,
    },
    /// A `@template` bound by the element of a parameter's container type: what the argument holds,
    /// and which way the branch admitted the value being absent.
    ArgumentElement {
        parameter: u16,
        absent: Option<phrost_model::Absence>,
    },
    /// What names one slot of a container parameter -- `array_key_first`. See
    /// `phrost_hir::ReturnBranchData::ArgumentKey`.
    ArgumentKey {
        parameter: u16,
        absent: Option<phrost_model::Absence>,
    },
    /// The same element, back in a container of its own -- `array_values` and its neighbours.
    ArgumentElements {
        parameter: u16,
    },
    /// Whatever one parameter was passed, in a container of its own -- `array_fill`. See
    /// `phrost_hir::ReturnBranchData::ArgumentTypes`.
    ArgumentTypes {
        parameter: u16,
    },
    /// Keyed by one parameter's elements and holding another's -- `array_combine`. See
    /// `phrost_hir::ReturnBranchData::CombinedElements`.
    CombinedElements {
        keys: u16,
        values: u16,
    },
    /// A container of the *keys* of a container parameter -- `array_keys`.
    ///
    /// The one shape [`Self::ArgumentElements`] cannot express: what binds the template is the
    /// parameter's first type argument rather than its last, and what a key *is* follows from how the
    /// container was written -- a `list<Row>` is keyed by `int`. See
    /// `phrost_hir::ReturnBranchData::ArgumentKeys`.
    ArgumentKeys {
        parameter: u16,
    },
    /// A container of one column of a container of rows -- `array_column`. See
    /// `phrost_hir::ReturnBranchData::ArgumentColumn`.
    ArgumentColumn {
        parameter: u16,
        column: u16,
    },
    /// A container of whatever the callable at this position gives back -- `array_map`.
    CallbackResults {
        parameter: u16,
    },
    /// Whatever the callable at this position gives back -- every `reduce`.
    CallbackResult {
        parameter: u16,
    },
    /// A conditional slot the call settles. See [`phrost_hir::ReturnBranchData::Conditional`].
    Conditional(Box<ConditionalBranch>),
    /// A class with arguments as one slot of another. See
    /// [`phrost_hir::ReturnBranchData::Applied`].
    Applied {
        class: TypeId,
        arguments: Box<[ReturnBranch]>,
    },
    /// A branch nothing here can read. See [`phrost_hir::ReturnBranchData::Unfilled`].
    Unfilled,
}

/// The resolved half of [`phrost_hir::ConditionalBranchData`].
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ConditionalBranch {
    pub argument: u16,
    pub test: ConditionTest,
    pub then: ReturnBranch,
    pub otherwise: ReturnBranch,
    pub by_default: Option<bool>,
}

impl lspkit_memsize::MemSize for MemberRow {
    /// The parameter list. Everything else is a name -- a pointer into the interner -- or a `TypeId`.
    fn heap_size(&self) -> usize {
        lspkit_memsize::flat(&self.params)
    }
}

impl MemberRow {
    /// The type this member holds, from whichever declaration said so.
    ///
    /// Native first: it is what the runtime enforces, and a docblock that disagrees with one is a
    /// mistake in the source rather than extra information. The exception is a bare `array` or
    /// `iterable`, which says nothing about its contents -- there the docblock is the missing half of
    /// the same claim, not a competing one. See [`TypeTable::refined`].
    pub fn effective_ty(&self, types: &TypeTable) -> Option<TypeId> {
        match (self.ty, self.doc_ty) {
            (Some(native), Some(doc)) => Some(types.refined(native, doc)),
            (Some(native), None) => Some(native),
            (None, doc) => doc,
        }
    }

    /// The declaration as PHP, without the leading keyword: `find(int $id): ?User`, `array $items`,
    /// `string VERSION`.
    ///
    /// As PHP writes it, type first, because a hover card is rendered inside a `php` fence and a
    /// client highlights what it can parse. `$items: array` reads fine and highlights as nothing.
    ///
    /// Rendered on demand rather than stored. It costs a few hundred nanoseconds for the handful of
    /// members one request describes, and storing it instead would mean carrying a string for every
    /// member of every vendor class — more memory than the structured form it was rendered from.
    pub fn signature(&self, types: &TypeTable) -> String {
        self.signature_returning(types, None)
    }

    /// The same, with a method's return type replaced by one the caller resolved.
    ///
    /// For a declaration that writes none of its own: what it hands back is then whatever the
    /// nearest ancestor that says so declares, and that is what a card describing it should read.
    /// Replaced rather than appended, because this row may already be rendering a type read off its
    /// own body -- which a declaration further up outranks, the same way resolution ranks them.
    pub fn signature_returning(&self, types: &TypeTable, returns: Option<&str>) -> String {
        let mut out = String::new();
        match self.kind {
            MemberKind::Method => {
                out.push_str(self.name.as_str());
                out.push('(');
                for (position, param) in self.params.iter().enumerate() {
                    if position > 0 {
                        out.push_str(", ");
                    }
                    param.write(&mut out, types);
                }
                out.push(')');
            }
            // Type first and then the name, which is the order PHP writes both of these in:
            // `private array $items`, `const string VERSION`.
            MemberKind::Property => {
                self.write_type(&mut out, types);
                out.push('$');
                out.push_str(self.name.as_str());
            }
            MemberKind::ClassConstant => {
                self.write_type(&mut out, types);
                out.push_str(self.name.as_str());
            }
            // A case has no type to write: its type is the enum.
            MemberKind::EnumCase => out.push_str(self.name.as_str()),
        }

        // A method's return type comes after the parameters, so it is the one kind written here.
        if self.kind == MemberKind::Method {
            let rendered = match returns {
                Some(resolved) => Some(resolved.to_owned()),
                None => self.effective_ty(types).map(|ty| types.render(ty)),
            };
            if let Some(rendered) = rendered {
                out.push_str(": ");
                out.push_str(&rendered);
            }
        }
        out
    }

    /// Appends the declared type and a space, when the declaration says one.
    ///
    /// The docblock's type is used when nothing native was written, because that is the only thing
    /// the declaration says -- and a signature with no type at all reads as `mixed`.
    /// The type this member holds, rendered, or `None` when its own declaration names none.
    ///
    /// Its own declaration: no hierarchy walk. A completion list is a hundred members long and this
    /// is asked for each of them, and what the list shows should be what the declaration says.
    pub fn rendered_type(&self, types: &TypeTable) -> Option<String> {
        self.effective_ty(types).map(|ty| types.render(ty))
    }

    fn write_type(&self, out: &mut String, types: &TypeTable) {
        // A type nobody wrote is not part of the declaration. See [`Self::ty_from_value`].
        if self.ty_from_value {
            return;
        }
        if let Some(ty) = self.effective_ty(types) {
            out.push_str(&types.render(ty));
            out.push(' ');
        }
    }
}

/// One declared parameter.
///
/// Carries what a caller needs to be told about it — whether it can be omitted, whether it takes a
/// reference, what type it wants — and not the default's value, which is an expression and belongs
/// with bodies.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ParamRow {
    /// Without the leading `$`, matching how names are stored everywhere else.
    pub name: Name,
    pub ty: Option<TypeId>,
    /// From the enclosing declaration's `@param`, whether or not [`Self::ty`] is written.
    ///
    /// Both, because a `@param` beside a hint is saying one of three things and only a resolved
    /// type can tell which. [`Self::effective_ty`] settles the two that are about what the value
    /// *is*; [`Self::rejects`] settles the third, which is about what the callee will *take*.
    pub doc_ty: Option<TypeId>,
    pub has_default: bool,
    /// The default's value as written, for the one reader that writes it back into a file.
    ///
    /// See `ParamData::default`. A stub for a required method that dropped it would make an
    /// optional parameter required and the class still would not load, so the repair used to skip
    /// such a method instead -- 21 % of the reference project's own interfaces hold one.
    pub default: Option<Box<str>>,
    pub is_variadic: bool,
    pub by_reference: bool,
    /// Constructor property promotion: `__construct(private readonly int $x)`. Present here because
    /// it is part of how the parameter reads, even though lowering also emits a property for it.
    pub promotion: Option<ParamPromotion>,
    /// From `@param-out`: what a by-reference parameter leaves in the caller's variable.
    ///
    /// A third type and not a refinement of the two above, because it is about a different moment.
    /// [`Self::ty`] and [`Self::doc_ty`] say what the callee *takes*; this says what it wrote back,
    /// and the two differ on purpose -- `preg_match`'s third parameter takes `array` and leaves
    /// `array<int, string>`, and `str_replace`'s `$count` takes nothing and leaves an `int`.
    ///
    /// Only ever set for a by-reference parameter, and only read by
    /// `IndexView::written_through_reference`. Last here and last in the cache codec beside it,
    /// which is the rule §8 records: a codec's field order is its read order.
    ///
    /// Cost 1.5 MB of a 92.6 MB index when it was added, because `TypeId` was a bare `u32` and so
    /// `Option<TypeId>` had no niche -- eight bytes on every parameter for a field set on the few
    /// that are by-reference. `TypeId` is a `NonZeroU32` now, which halved this field and the two
    /// above it and took the whole index to 88.9 MB: below where it stood before this field existed.
    pub out_ty: Option<TypeId>,
    /// From a `@param callable(A, B): C`: what the callable this parameter takes will be *called
    /// with*, held as a `Generic { base: Callable, args: [A, B] }`.
    ///
    /// A fourth type, and a slot rather than a refinement for the same reason [`Self::out_ty`] is
    /// one -- it is about a different moment. The three above are about the value passed here;
    /// this is about the values that value will receive.
    ///
    /// The slot is what carries the positional meaning, which is why this needed no new type node.
    /// A `Generic { Callable, [x] }` in [`Self::ty`] means `callable(): x` -- one argument, and it
    /// is the *return*. Here the arguments are the parameters in written order. Two readings of one
    /// interned shape is safe only because neither reader can reach the other's slot: this one is
    /// read by `IndexView::callback_parameter` and nothing else.
    ///
    /// Dense, so a reader can index it: a position that said nothing is a `mixed`. A trailing `...`
    /// is dropped rather than modelled, so `callable(A, B...)` answers nothing past the second
    /// parameter instead of answering `B` -- precision lost, never a wrong answer.
    ///
    /// Set on the few parameters that take a documented callback, and free on the rest for the
    /// reason [`Self::out_ty`] records: `TypeId` is a `NonZeroU32`, so this is four bytes in a niche
    /// rather than eight.
    ///
    /// Last here and last in the cache codec beside it -- §8's rule that a codec's field order is
    /// its read order.
    pub callback_params_ty: Option<TypeId>,
    /// What `$this` is inside a closure passed here -- `@param-closure-this`, read by
    /// `IndexView::closure_this` and nothing else. See
    /// [`phrost_hir::ParamData::closure_this_type_ref`].
    pub closure_this_ty: Option<TypeId>,
    /// Whether the declaration said anything about this parameter that reached here or was refused
    /// on the way.
    ///
    /// Not the same question as [`Self::doc_ty`] being present, and the difference is the whole
    /// reason this exists: a `@param (\Closure($this): TWhenParameter)|TWhenParameter|null $value`
    /// names a template, which interns to nothing, so the type slot is empty at a parameter whose
    /// author documented it thoroughly. An empty slot read as "said nothing" is what let an
    /// interface's coarser `@param bool $value` be carried down over it -- six false
    /// `incompatible-argument` on Laravel's own type tests, every one of them
    /// `Collection::when()`.
    ///
    /// A bool and not a fifth type, because the reader wants only to know whether to leave the
    /// slot alone. Free in memory: `ParamRow` already carries three bools and this one fits the
    /// padding beside them.
    pub documented: bool,
}

impl ParamRow {
    /// The type this parameter takes, from whichever declaration said so.
    ///
    /// Same rule as a member's: native wins, except over a bare container the docblock refines. See
    /// [`TypeTable::refined`].
    pub fn effective_ty(&self, types: &TypeTable) -> Option<TypeId> {
        match (self.ty, self.doc_ty) {
            (Some(native), Some(doc)) => Some(types.refined(native, doc)),
            (Some(native), None) => Some(native),
            (None, doc) => doc,
        }
    }

    /// Whether nothing this parameter accepts admits a value the caller's `fit` judges.
    ///
    /// The judgement is supplied rather than performed here, because the *value* side of a call is
    /// inferred in a layer that can see bodies and this crate cannot. What this owns is the other
    /// side: which types a caller is entitled to pass.
    ///
    /// There are up to two, and the second is the whole reason this is not a bare
    /// [`Self::effective_ty`] check. A **return** type's docblock *refines* — `array` refined by
    /// `list<Item>` tells the caller more precisely what it was handed, which is what
    /// `effective_ty` and [`TypeTable::refined`] compute. A **parameter**'s docblock runs the other
    /// way: it is the callee stating what it undertakes to handle, so a `@param` wider than the
    /// hint *adds* to what may legally arrive. Drupal's
    /// `StatementInterface::fetchAll(?FetchAs $mode = NULL)` documents
    /// `@param \Drupal\Core\Database\Statement\FetchAs|int|null $mode` and says in prose that a
    /// deprecated `\PDO::FETCH_*` constant is accepted — an `int` there is the declaration working
    /// as written, not a defect.
    ///
    /// Offered as alternatives rather than joined into one type, for two reasons. Joining would
    /// mean interning a union, and every `TypeTable` constructor needs `&mut`, which a reader
    /// answering a request does not have. And alternatives are what makes a *narrower* `@param`
    /// correctly change nothing: `@param class-string $name` beside `string $name` adds an
    /// acceptance that `string` already covers, so it can only ever stay silent. That is the
    /// direction to be wrong in — a docblock must not make a call illegal that PHP will run and the
    /// hint permits, or the finding is about a comment rather than about the code.
    ///
    /// Nullability is untouched by any of this. `null_arguments` asks `effective_ty` directly and
    /// is not routed through here, so a `@param ?T` beside a `T` hint still cannot talk a null past
    /// a parameter the runtime will refuse it at.
    pub fn rejects(&self, types: &TypeTable, mut fit: impl FnMut(TypeId) -> Fit) -> bool {
        // An untyped parameter takes anything, so there is nothing to reject it against.
        let Some(declared) = self.effective_ty(types) else {
            return false;
        };
        if fit(declared) != Fit::Rejected {
            return false;
        }
        match self.doc_ty {
            Some(documented) if documented != declared => fit(documented) == Fit::Rejected,
            _ => true,
        }
    }

    /// Appends the parameter as it would be written. Takes the buffer rather than returning a
    /// string because a signature renders a whole list of these.
    pub fn write(&self, out: &mut String, types: &TypeTable) {
        if let Some(promotion) = &self.promotion {
            out.push_str(promotion.visibility.as_str());
            out.push(' ');
            if let Some(set) = promotion.set_visibility {
                out.push_str(set.as_str());
                out.push_str("(set) ");
            }
            if promotion.is_readonly {
                out.push_str("readonly ");
            }
        }
        if let Some(ty) = self.effective_ty(types) {
            out.push_str(&types.render(ty));
            out.push(' ');
        }
        if self.by_reference {
            out.push('&');
        }
        if self.is_variadic {
            out.push_str("...");
        }
        out.push('$');
        out.push_str(self.name.as_str());
        if self.has_default {
            // The value as written when it is kept, and `...` when it is not -- an `@method` tag's
            // parameter has a default with no source behind it. A signature is read by a person in
            // hover and written back by the repair that implements a required method, and only the
            // second needs PHP to accept it.
            match &self.default {
                Some(written) => {
                    out.push_str(" = ");
                    out.push_str(written);
                }
                None => out.push_str(" = ..."),
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ParamPromotion {
    pub visibility: Visibility,
    pub set_visibility: Option<Visibility>,
    pub is_readonly: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub enum MemberKind {
    Method,
    Property,
    ClassConstant,
    EnumCase,
}

impl MemberKind {
    /// Whether a lookup for `other` may return this kind.
    ///
    /// Only constants and enum cases share one, for the reason [`MemberTable::find`] describes.
    ///
    /// [`MemberTable::find`]: crate::MemberTable::find
    pub fn shares_namespace_with(self, other: Self) -> bool {
        self == other
            || matches!(
                (self, other),
                (Self::ClassConstant, Self::EnumCase) | (Self::EnumCase, Self::ClassConstant)
            )
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Method => "method",
            Self::Property => "property",
            Self::ClassConstant => "const",
            Self::EnumCase => "case",
        }
    }

    /// Whether PHP compares this kind's name case-insensitively.
    ///
    /// Methods are folded; property, constant, and case names are exact. Folding all of them would
    /// merge `$name` and `$Name`, which are two different properties.
    pub fn is_case_insensitive(self) -> bool {
        matches!(self, Self::Method)
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
    /// The hooks themselves are on `phrost_hir::MemberData::hooks`; this is what a reader wants without
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
    /// The declaration carries an attribute a framework keyed -- one whose meaning only that
    /// framework knows, like Laravel's `#[Scope]`. One bit and not a name, because the rule that
    /// reads it names the attribute and the class it applies below, and the last byte is free.
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

/// What a property's hooks say about writing it -- see `phrost_hir`'s copy of this vocabulary.
///
/// One field rather than three flags because the four states are exclusive and because a member row
/// is the largest table the index holds: three bools cost a word per member, and a real project has
/// hundreds of thousands of them.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum Hooks {
    #[default]
    None,
    /// One of them is the `set`, so PHP takes the write and runs it.
    Settable,
    /// A `get` alone whose body uses `$this->propName`: the property is backed, holds a slot, and
    /// takes a write like any other.
    BackedGet,
    /// A `get` alone with no body -- `public string $n { get; }` in an interface or an abstract
    /// class. A requirement, and whoever implements it decides whether it holds a slot.
    DeclaredGet,
    /// A `get` alone with a body that never mentions the store: virtual, computed, and what PHP
    /// calls read-only.
    VirtualGet,
}

impl Hooks {
    /// Whether any hooks were written -- which is what makes the property something other than a
    /// plain slot, so `readonly` and the write visibility mean something different on it.
    pub fn are_written(self) -> bool {
        self != Self::None
    }

    /// Whether PHP refuses every write to the property, from inside the declaring class as much as
    /// from outside. Verified against PHP 8.5.10 rather than argued from the RFC.
    pub fn refuse_a_write(self) -> bool {
        self == Self::VirtualGet
    }
}

impl MemberModifiers {
    /// The modifiers present, in the order PHP writes them.
    ///
    /// For anything that renders a declaration, and for the dumps the index invariants measure
    /// against -- a modifier that nothing renders is a modifier nothing checks.
    pub fn words(self) -> Vec<&'static str> {
        [
            (self.is_abstract(), "abstract"),
            (self.is_final(), "final"),
            (self.is_static(), "static"),
            (self.is_readonly(), "readonly"),
            // Each state its own word, because which one it is decides whether a write is legal
            // and a dump that spells them all "hooks" is a dump neither index invariant can tell
            // them apart in.
            (self.hooks == Hooks::Settable, "hooks"),
            (self.hooks == Hooks::BackedGet, "backed-get"),
            (self.hooks == Hooks::DeclaredGet, "declared-get"),
            (self.hooks == Hooks::VirtualGet, "virtual-get"),
            (self.is_no_discard(), "no-discard"),
            // `#[\Override]`, which nothing else a dump prints implies: it is one bit carried
            // through the codec, and the two index equality invariants compare the dump.
            (self.is_override(), "override"),
            // `@deprecated` or `#[\Deprecated]`, one bit for both spellings -- and one more the
            // codec carries that nothing else a dump prints implies.
            (self.is_deprecated(), "deprecated"),
            (self.is_marked(), "marked"),
        ]
        .into_iter()
        .filter_map(|(present, word)| present.then_some(word))
        .collect()
    }
}
