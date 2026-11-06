use phrost_hir::{
    ExprId, FileHir, TypeRefId,
    body::{AssignOp, CallableReceiver, ClassTarget, ExprData},
};
use phrost_model::Name;

/// Where a value came from, as the file itself spells it out.
///
/// Not a type: resolving one needs imports, a catalog, and member tables, none of which a single
/// file settles. This is the part a body can answer — `$x` was assigned from `new Foo()`, or from
/// `$this->repo->find()` — leaving the layer that has a catalog to say what those name.
#[derive(Clone, Debug)]
pub struct ValuePath {
    pub root: PathRoot,
    /// Applied to the root in order, left to right as written.
    pub steps: Vec<PathStep>,
    /// A `?->` was written somewhere in the chain, so the whole expression may be null.
    ///
    /// PHP short-circuits the *rest* of the chain rather than calling on null, so nothing inside
    /// the chain is at risk -- `$a?->b()->c()` never calls `c()` when `$a` is null, and
    /// `$a?->b()['k']` never indexes. What is at risk is whoever receives the expression's value:
    /// `return $a?->b()` out of a `: string` returns null, and `f($a?->b())` passes one.
    ///
    /// Outside the path's identity, deliberately, and this is what the hand-written [`PartialEq`],
    /// [`Eq`] and [`Hash`] below exist for: `$x?->y()` and `$x->y()` reach the same slot, so a
    /// guard written with one spelling has to narrow the other. Only how the value was *written*
    /// differs, and that is what this records.
    pub may_be_absent: bool,
}

/// Two paths are the same path when they reach the same slot. How the chain was written -- with
/// `->` or with `?->` -- is not part of that: `if ($x?->y()) { $x->y()->z(); }` is one value tested
/// and then used, and telling the two spellings apart would lose every guard written that way.
impl PartialEq for ValuePath {
    fn eq(&self, other: &Self) -> bool {
        self.root == other.root && self.steps == other.steps
    }
}

impl Eq for ValuePath {}

impl std::hash::Hash for ValuePath {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.root.hash(state);
        self.steps.hash(state);
    }
}

impl ValuePath {
    pub(crate) fn of(root: PathRoot) -> Self {
        Self {
            root,
            steps: Vec::new(),
            may_be_absent: false,
        }
    }

    /// Whether the path is a string and nothing more -- a literal, or the `string` a cast or two
    /// literals that agreed on it state.
    pub(crate) fn is_a_plain_string(&self) -> bool {
        self.steps.is_empty()
            && matches!(
                self.root,
                PathRoot::StringLiteral(_) | PathRoot::Builtin(phrost_model::BuiltinTy::String)
            )
    }

    /// Whether the path is the null literal and nothing else.
    ///
    /// What a reader looking through a two-operand shape needs: in `$c ? $this->find() : NULL` one
    /// operand carries the whole value and the other only says it may be absent.
    pub fn names_null(&self) -> bool {
        self.steps.is_empty() && self.root == PathRoot::Builtin(phrost_model::BuiltinTy::Null)
    }

    /// Whether a guard can have been written about this path itself rather than about a step of it.
    ///
    /// A path with steps always can: `$this->thing()` is a slot and `if ($this->thing())` narrows
    /// it. A path with none usually cannot -- a literal, an array, a `new` -- and a bare local is
    /// ranked elsewhere, which is what this used to be spelled as. The exception is a call written
    /// with no receiver: `if (wp_get_referer()) { wp_safe_redirect(wp_get_referer()); }` is how a
    /// project with no classes writes the guard a method call gets for free from its receiver, and
    /// the two halves of that `if` are one slot.
    pub fn can_be_guarded(&self) -> bool {
        match &self.root {
            _ if !self.steps.is_empty() => true,
            PathRoot::FunctionReturn {
                every_argument_written,
                ..
            } => *every_argument_written,
            _ => false,
        }
    }
}

/// What a guard that let the value through ruled out.
///
/// Three, because PHP's tests fall into three groups and collapsing any two of them is wrong in a
/// way nothing fails on. The middle one is the reason this is not a bool: `is_numeric(FALSE)` is
/// false, so the sentinel really is gone, and `is_numeric('0')` is true, so the value may still be
/// falsy -- a test that proves neither of the two things a bool could say.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Refused {
    /// `!== null`, `isset($x)` -- null, and nothing else. `isset(FALSE)` is true.
    NullOnly,
    /// `is_numeric($x)`, `is_countable($x)` -- null and `false`, and nothing about emptiness.
    TheSentinels,
    /// `strlen($x) > 0`, `$x != ''` -- the sentinels and the empty string, and not `'0'`, which is
    /// one character long and loosely unequal to `''`. What is left of a string holds something:
    /// a `non-empty-string`.
    TheEmptyString,
    /// `if ($x)` -- every falsy value, of which this layer can name null and `false`. So `if ($row)`
    /// on the `array|false` the standard library returns leaves an `array`, while
    /// `if ($row !== NULL)` leaves the union alone. And `''` and `'0'`, so a string left is a
    /// `non-falsy-string`.
    EveryFalsy,
}

impl Refused {
    /// Whether the guard took the `false` off as well as the null.
    ///
    /// The two sentinels this layer can name travel together for every guard but the strictest, so
    /// the readers that peel them ask this rather than matching three arms each.
    pub fn takes_the_false(self) -> bool {
        matches!(
            self,
            Self::TheSentinels | Self::TheEmptyString | Self::EveryFalsy
        )
    }

    /// What a string the guard left is known to hold, where it said anything about strings.
    pub fn leaves_strings(self) -> Option<phrost_model::BuiltinTy> {
        match self {
            Self::NullOnly | Self::TheSentinels => None,
            Self::TheEmptyString => Some(phrost_model::BuiltinTy::NonEmptyString),
            Self::EveryFalsy => Some(phrost_model::BuiltinTy::NonFalsyString),
        }
    }

    /// The claim both paths of a join make, which is the weaker of the two.
    ///
    /// A truthiness test rules out more than a sentinel test, and that more than a null test -- so
    /// what survives a branch taken one way and skipped the other is whichever said least.
    pub fn weaker_of(self, other: Self) -> Self {
        match (self, other) {
            (Self::NullOnly, _) | (_, Self::NullOnly) => Self::NullOnly,
            (Self::TheSentinels, _) | (_, Self::TheSentinels) => Self::TheSentinels,
            (Self::TheEmptyString, _) | (_, Self::TheEmptyString) => Self::TheEmptyString,
            _ => Self::EveryFalsy,
        }
    }

    /// Whether the guard proved the value *truthy*, which only a truthiness test does.
    ///
    /// Asked by the one reader that needs more than "not a sentinel": a conditional return type
    /// keyed on `non-empty-array` is settled by a guard above the call, and `is_countable([])` is
    /// true -- so reading the middle case as truth would call an empty array non-empty.
    pub fn proves_truth(self) -> bool {
        matches!(self, Self::EveryFalsy)
    }
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum PathRoot {
    /// `(int) $value`, `0`, `''`, `[]` -- a type the source states outright, with no class behind it.
    Builtin(phrost_model::BuiltinTy),
    /// `time()` -- whatever the function declares it returns.
    ///
    /// With what the call wrote, for the reason [`PathStep::MethodReturn`] carries it: a free
    /// function's return type can be conditional on an argument too.
    ///
    /// The name as *written*, and deliberately not the [`phrost_hir::NameRefId`] it was written as.
    /// A reference is per occurrence, so two calls to one function were two roots and therefore two
    /// slots -- and `if (wp_get_referer()) { wp_safe_redirect(wp_get_referer()); }` narrowed
    /// nothing, on the idiom procedural PHP is made of. [`PathStep::MethodReturn`] states this rule
    /// for a method and this half had never learned it, which is the shape §8 calls two
    /// constructors of one rule.
    ///
    /// The anchor travels beside the path because it is half of what the name means: `\strlen()`
    /// and `strlen()` inside a namespace resolve through different rules, and a slot that merged
    /// them would narrow one call by a guard written about another function.
    FunctionReturn {
        name: Name,
        anchor: phrost_hir::NameAnchor,
        arguments: Box<[phrost_hir::body::LiteralArgument]>,
        /// Whether [`Self::FunctionReturn::arguments`] accounts for every argument the call wrote.
        ///
        /// What decides whether this is a *slot*. Only literals travel, so `current($rows)` and
        /// `current($totals)` are two calls carrying the same empty list and comparing equal -- and
        /// a guard written about one would then narrow the other. Worse where one is the other's
        /// argument: `current(current($this->iterations))` narrowed the outer call to what the
        /// guard proved about the inner, and turned a correct `int|false` return into a report.
        ///
        /// So a call whose arguments are not all written out is a value this layer cannot identify,
        /// and it says so rather than merging. What is left is the idiom the slot was wanted for --
        /// `if (wp_get_referer()) { wp_safe_redirect(wp_get_referer()); }` -- plus the calls keyed
        /// by a literal, which are the same slot for the same reason a catalog key is.
        every_argument_written: bool,
        call: CallSource,
    },
    /// `$a + $b` -- an int when both operands are ints, a float when either is, and an array when
    /// `+` was given two arrays.
    ///
    /// The operands as paths rather than as types, because the reader is the layer that can resolve
    /// them: `$count * $rate` says nothing until both variables have been followed to what they
    /// hold.
    ///
    /// With the operator, because two of the five do not follow that rule: `/` on two ints is an
    /// `int|float` -- PHP hands back an int only where the division comes out even -- and `+` is
    /// the one spelling that means anything at all for two arrays.
    Arithmetic {
        operator: phrost_hir::body::ArithmeticOp,
        left: Box<ValuePath>,
        right: Box<ValuePath>,
    },
    /// `$prefix . $name`, `"{$table}_id"`, `$sql .= $where` -- a string built from pieces, which
    /// holds at least what the piece holding the most does: joining strings never empties one, and
    /// never shortens one to `'0'`.
    ///
    /// `holds` is what the syntax already settled -- the literal pieces, read by lowering -- and the
    /// pieces are the ones it could not read, as paths, because only the reader can say what a
    /// variable holds. Built only where that could add something: a string the syntax already knows
    /// is non-falsy is a [`Self::Builtin`].
    Concatenation {
        holds: phrost_model::BuiltinTy,
        pieces: Box<[ValuePath]>,
    },
    /// `++$i` and `--$i`: the target after it moved by one.
    ///
    /// Not `$i + 1`, though it is that for a number. PHP steps nothing else the way it adds: a
    /// bool is left as it was, `null` becomes `1` going up and stays `null` going down, and a
    /// string that spells no number counts like a column name -- `'z'` steps to `'aa'`. Asked of
    /// PHP 8.5. Folded into the arithmetic, every coercion the operators make would have been
    /// claimed for a step that does not make it.
    Stepped(Box<ValuePath>),
    /// `$a ?? $b`, `$c ? $a : $b`, and `match ($k) { ... }` -- one value out of several the code
    /// wrote, and nothing here knows which.
    ///
    /// The operands as paths, for the reason [`Self::Arithmetic`] carries them: which value each one
    /// holds is a question for the layer that can resolve it. What comes out is a union, so what can
    /// be reached through the result is what every operand agrees on.
    ///
    /// Each with its own position, for the reason [`Self::MaybeWritten`] gives each write one: an
    /// operand is *selected* by something, and it means what it means where it is written.
    /// `is_array($x) ? $x[1] : $x` is the case that forces it -- read at one position both operands
    /// are the un-narrowed value, and the union that comes out puts back exactly the possibility
    /// the condition ruled out.
    ///
    /// `??` and the ternary always write exactly two. `match` writes one per arm, minus any arm
    /// whose body is a `throw` -- that arm never produces a value at all, so it is not an operand
    /// and not a junk branch either. Never fewer than two: a lone survivor is read as itself, the
    /// same rule a resolved union already follows once it is down to one member.
    EitherOf {
        operands: Vec<OperandSite>,
        /// What must have been true of every operand but the last for the next one to be chosen.
        unless: Unless,
    },
    /// A container after a loop iterated it by reference and wrote through the reference:
    /// `foreach ($ints as &$item) { $item = (string) $item; }`.
    ///
    /// Each pass's `$item` *is* that slot of `$ints`, so every whole write to it lands in the
    /// container -- and after the loop the elements are what they were or what was written, a
    /// union, because the list may be empty and a pass may write nothing. Which of those writes
    /// change the element is a question about types, so the reader decides it: `$item = $item * 2`
    /// over a `list<int>` changes nothing, and the container is handed back exactly as it was.
    RewrittenElements {
        /// The container as it was where the loop began, read there.
        before: Box<ValuePath>,
        at: phrost_model::TextSize,
        /// What the loop's body wrote to the reference, each where it was written. `None` when one
        /// of the writes is not a value this layer can name -- a destructuring, a call's
        /// out-parameter -- and the elements are then whatever an array holds.
        writes: Option<Vec<OperandSite>>,
    },
    /// A value written in a branch the position is not in, beside whatever it held before.
    ///
    /// An assignment is only the value of a name where control had to go through it. Written in a
    /// branch, it *may* have run -- `if ($c) { $v = new Foo(); } $v->bar()` reaches a `Foo` or
    /// reaches whatever `$v` was, and nothing in the file says which. So this carries both halves:
    /// the writes, and the path to read before the first of them.
    ///
    /// The path is a bare local or a slot, and the two are one root on purpose: `if ($this->cache
    /// === NULL) { $this->cache = $this->build(); } return $this->cache;` is the commonest way a
    /// property is filled in PHP, and it is exactly the local's shape with `$this->cache` for `$v`.
    /// Held as two constructors, the slot half withdrew its claim wherever the local half joined,
    /// and every lazily filled property read as the `?T` its declaration says.
    ///
    /// Not [`Self::EitherOf`], for two reasons it cannot express. Its operands are written in one
    /// expression and so are read at one position, while these come from different branches and
    /// each means what it meant *there* -- `$v = $v->find()` read anywhere else finds itself. And
    /// every one of its operands must resolve or the union is not a union, while here the value
    /// before the branches may not exist at all: a name written in one branch and nowhere else
    /// holds what that branch wrote, and joining it with a nothing would lose the only claim
    /// anyone made.
    MaybeWritten {
        /// Every conditional write in effect, in the order they are written.
        writes: Vec<WriteSite>,
        /// What to read for what the value held if none of the writes ran: the local itself, or the
        /// slot.
        read: Box<ValuePath>,
        /// Where to read it: before the first of the writes, which is the last position none of
        /// them had happened at.
        before: phrost_model::TextSize,
        /// What the path that skipped every branch is known to satisfy.
        otherwise: Otherwise,
    },
    /// A written type: a parameter's hint, or a `catch` clause's.
    Written(TypeRefId),
    /// A method parameter with no type of its own, at this position in the declaration.
    ///
    /// Kept as the question rather than answered here, the way [`crate::Leaving::Ask`] is: what it
    /// holds is whatever the declaration this method *overrides* says about the same position, and
    /// finding that needs the class hierarchy, which nothing at this layer has.
    ///
    /// A lower bound rather than an answer, strictly -- PHP is contravariant on inputs, so an
    /// override may legally accept more than its parent documents. It is worth reading anyway
    /// because of what a method with no type and `{@inheritdoc}` above it is *saying*: on the
    /// reference project, 188 of 202 untyped parameters are exactly that, and one of them --
    /// `preRender(&$values)` over Drupal's `ResultRow[]` -- is the only definition gap left in a
    /// side-by-side run against another server.
    InheritedParameter {
        position: u16,
    },
    /// `...$items` -- a variadic parameter, which holds an array of whatever its type names.
    ///
    /// Its own root because the hint describes one argument and the variable holds all of them:
    /// `__construct(Definition ...$definitions)` gives `$definitions` an array, and reading the
    /// hint as the variable's type made `foreach ($definitions as $one)` yield nothing.
    VariadicValues(TypeRefId),
    /// `Foo::class` -- a string, and which class's string it is.
    ///
    /// The type is a string and that is what every reader but one wants. The exception is `new $name`
    /// and `$name::make()`, where the string is the only thing that says which class -- so the target
    /// travels with it rather than being thrown away at the point it is read.
    ClassName(ClassTarget),
    /// `'App\Repo'` -- a string, and the literal that wrote it.
    ///
    /// [`Self::ClassName`]'s reason, for the spelling PHP had before `::class`: the type is a
    /// string and that is what every reader but one wants, and `$class = '\Countable'; new
    /// $class()` is the one -- the text is the only thing that says which class. Kept as the
    /// expression because the text lives in the buffer, which a body does not hold.
    StringLiteral(ExprId),
    /// `new Foo()` — the class as the instantiation wrote it.
    ///
    /// With the call, for the reason [`PathStep::MethodReturn`] carries one: a class with a
    /// `@template` says what it holds through its constructor, so `new Collection(new Item())` is
    /// the only place anything says what *this* collection holds.
    Instantiated {
        class: ClassTarget,
        call: CallSource,
    },
    /// `[new Row(), new Row()]` -- a list literal, kept as the expression that built it.
    ///
    /// The expression rather than `array`, for [`Self::Closure`]'s reason: what the literal holds is
    /// not a question a body settles -- each element is itself a path, and resolving one needs the
    /// index. `PathRoot::Builtin(Array)` is what this used to be, and it is why `foreach ([new Row()]
    /// as $row)` gave `$row` nothing while the same array in a typed variable gave `Row`.
    ///
    /// Only a literal whose entries are all *unkeyed*. `['a' => 1]` says which key holds what, which
    /// is a shape rather than a list, and reading it as one would answer `int` for its keys -- so it
    /// stays a bare `array`, which is a miss rather than a wrong answer.
    ArrayLiteral(ExprId),
    /// `fn (): Row => ...`, `function () use ($x) { ... }` -- the closure a literal builds.
    ///
    /// The expression rather than a type, because what invoking it gives back is not a question a
    /// body settles: a declared return type is a reference into this file's arena, and an arrow
    /// function that declares none says what it returns only in its body, whose value has to be
    /// resolved like any other. Both need the layer with a catalog.
    ///
    /// Names no class. A `Closure` has members nobody calls, so the one thing read through this is
    /// [`PathStep::Invoked`] -- which is the whole reason a closure is worth reading as a value at
    /// all. Without it `$make = fn (): Query => ...; $make()` says nothing.
    Closure(ExprId),
    /// `strlen(...)`, `$repo->find(...)`, `Repo::find(...)` -- PHP 8.1's first-class callables.
    ///
    /// The value is a `Closure` whose signature is the callee's, and the only thing PHP lets anyone
    /// reach through a closure is what invoking it gives back. So this carries the path the *call*
    /// would have been -- receiver, then the method's return -- and the reader wraps whatever that
    /// resolves to rather than handing it over: `$fn = strlen(...)` is a callable, emphatically not
    /// an `int`.
    ///
    /// A path rather than the callee's name because the receiver is a value like any other:
    /// `$this->repo->find(...)` has to follow `$this->repo` before the method means anything, and
    /// that is exactly what a path does.
    Callable(Box<ValuePath>),
    /// Another local. The caller resolves it the same way, which is where a chain of assignments
    /// gets followed.
    Local(Name),
    /// `$this` or `static` -- the class the value actually has when the code runs.
    ///
    /// Separate from [`Self::SelfClass`] because only this one can be narrowed. A guard proving
    /// `$this instanceof Special` says what the value *is*, which is exactly the question `$this`
    /// and `static` ask and exactly the question `self` does not.
    This,
    /// `self` -- the class the code is written in, whatever the value turns out to be.
    ///
    /// Never narrowed. `self::` inside `if ($this instanceof Special)` still means the declaring
    /// class, and resolving it to the subclass would hide a call to a method that does not exist
    /// where it is written.
    SelfClass,
    Parent,
    /// A class named as a static receiver: `Foo::make()`.
    Class(ClassTarget),
    /// `(object) $value`: an object stays itself, and anything else becomes a `stdClass`.
    ///
    /// The operand as a path, because which of the two it is depends on what the value holds, and
    /// only the layer that resolves it knows -- the cast states `object` and nothing narrower, which
    /// left `(object) ['a' => 1]` a value whose class nobody could name.
    ObjectCast(Box<ValuePath>),
    /// A global or namespaced constant: `PHP_INT_MAX`, `ABSPATH`, `App\LIMIT`.
    ///
    /// The name as written, because which constant it is -- `LIMIT` inside a namespace falls back
    /// to the global one when the namespace declares none -- is the def-map's to say.
    Constant(phrost_hir::NameRefId),
    /// A variable a call may have written into: `foo($bar)` where `foo` takes `Baz &$bar`.
    ///
    /// PHP's one way of returning through a parameter, and the declaration says so in the signature
    /// rather than in a comment -- so unlike most claims here, the runtime enforces it.
    ///
    /// Whether a parameter really is by-reference is the callee's business, written in whatever file
    /// declares it, so this carries only the **name and the position** and the reader walks every
    /// call that passed the name, newest first, taking the first whose parameter really is one.
    ///
    /// It used to record one call -- the last that passed the name, by-reference or not -- and the
    /// reader looked up exactly that one. So an ordinary call written between an out-parameter and
    /// its use erased the type: `fills($found); reads($found); $found` answered nothing, because
    /// `reads` was the recorded call and takes its argument by value. Nothing else bound the name
    /// in either case, so ranking was not what was wrong; the recorded call was simply the wrong
    /// one of two candidates. `IndexView::contract_narrowed` walks the candidates on its own route
    /// and always did, which is the copy of the question that was right.
    ///
    /// Still reached only when nothing else bound the name: a parameter that turns out to be
    /// by-value leaves the variable with no type, and ranking that against an assignment would let
    /// it hide one.
    ByReference {
        /// The variable as a path, which for a bare local is [`Self::Local`].
        ///
        /// A path and not a name because a name cannot be built here -- names are interned and this
        /// layer has no interner -- and because it is exactly what the reader needs: `PassedTo`
        /// matches a call's argument by comparing written paths, so this is the match key rather
        /// than something to resolve.
        subject: Box<ValuePath>,
        at: phrost_model::TextSize,
    },
    /// A callback's parameter, typed by whatever the call site says it receives.
    ///
    /// `array_map(fn ($item) => ..., $items)` says nothing about `$item` in PHP's own type system
    /// and most code writes no docblock either, so the value has a type only if the *call* gives it
    /// one. Two things can, and they are ranked rather than joined -- the first that answers wins,
    /// because they are two claims about one value and not two alternatives it might hold.
    ///
    /// **The callee's declared signature**, when it documents one: `@param callable(Application):
    /// void $callback` on `Container::singleton` is somebody stating outright what the callback
    /// receives, and it answers about *any* position -- `callable(string, int): void` types both
    /// parameters of a closure written `function ($key, $count)`.
    ///
    /// **The element type of a container passed beside it**, when it does not. Every function
    /// taking a callback beside a container applies it to that container's elements --
    /// `array_map`, `array_filter`, `usort`, `array_walk` -- so this is right far more often than
    /// it is written down. It is still a guess: the sibling of `singleton(Foo::class, function
    /// ($app) {})` is a class name and says nothing. So it loses to a declaration.
    ///
    /// Past the first parameter the guess needs one more fact, and it is a fact this layer has:
    /// *where the callback was written*. A callback written first is handed one element of each
    /// container after it -- `array_map($cb, $timestamps, $payments)` -- while one written after a
    /// container is being told about a single container, and its second parameter is a key
    /// (`array_walk`), an element again (`usort`) or a flag's business, which nothing here can tell
    /// apart. So `argument` decides, and the reader states the rule.
    ///
    /// One root and not two, because which source answers is not a question this layer can settle:
    /// resolving the callee's declaration needs a catalog and member tables. Built as two roots,
    /// the declared one would always be *built* -- an enclosing call is always found -- and would
    /// therefore always be preferred, silently taking the container heuristic out of every
    /// `array_map` in the project. So both travel, and the reader ranks them.
    CallbackParameter {
        /// Which of the callback's own parameters is being asked about, in written order.
        parameter: usize,
        /// What was called, for reading the signature it declared: the same
        /// [`crate::narrowing::Callee`] a by-reference parameter and an argument assertion locate
        /// their declaration with.
        callee: Box<crate::narrowing::Callee>,
        /// Which argument of the call the callback itself was written as. Counts the *call's*
        /// arguments, where `parameter` counts the closure's own -- two indices over two lists.
        argument: usize,
        /// Written as `name: value`. `None` for a positional argument, same as
        /// [`Self::AssertedArgument::name`].
        argument_name: Option<Name>,
        /// The call's other arguments, in written order, for the container reading.
        ///
        /// Written order because which side the container is on differs between functions --
        /// `array_map` takes the callback first and `array_filter` second -- and because with more
        /// than one container the order is what pairs them with the callback's parameters:
        /// `array_map($cb, $timestamps, $payments)` hands `$cb` an element of each. Which of these
        /// is a container at all is a question about types, so the reader decides it; see
        /// `IndexView::resolve_path`.
        siblings: Vec<ValuePath>,
        at: phrost_model::TextSize,
        /// What the callback's own parameter declares, when it declares a bare container.
        ///
        /// The answer when the call site settles nothing: `fn (array $carry, ...)` handed to a
        /// reduce says `array`, and neither source at the call can say more about an accumulator.
        declared: Option<phrost_hir::TypeRefId>,
    },
    /// A guard ruled a class out: the else of `if ($x instanceof T)`.
    ///
    /// Names no class of its own. What it leaves is whatever the declaration listed minus this
    /// one, so the reader resolves `of` at `at` and drops the operands that are a `T`.
    NotOfClass {
        of: Box<ValuePath>,
        at: phrost_model::TextSize,
        class: ClassTarget,
        /// Only the class itself was ruled out, not what extends it: `$x::class !== T::class`.
        exactly: bool,
    },
    /// A guard named the alternatives that are left: `if ($x instanceof A || $x instanceof B)`.
    ///
    /// Names no single class, so the reader resolves `of` at `at` and keeps the operands that are one
    /// of these -- which is why it carries the path where a single `instanceof` does not.
    OneOfClasses {
        of: Box<ValuePath>,
        at: phrost_model::TextSize,
        classes: Vec<ClassTarget>,
    },
    /// A guard tested what kind of value it is: `if (is_string($x))`, and the false side of it.
    ///
    /// Names no class, because the test did not -- it says which *sort* of thing the value is, and
    /// what that leaves depends on what the declaration said it could be. So the reader resolves
    /// `of` at `at`, where the guard does not reach, and keeps or drops the operands that are of
    /// this kind.
    OfKind {
        of: Box<ValuePath>,
        at: phrost_model::TextSize,
        kind: phrost_model::BuiltinTy,
        /// `true` inside `if (is_string($x))`, `false` after `if (is_string($x)) { return; }`.
        holds: bool,
        /// Whether the test named the kind or only ruled alternatives of it out. Carried from
        /// [`Proved::Kind`](crate::narrowing::Proved::Kind), where the difference is spelled out.
        settled: bool,
    },
    /// A test that held named no kind, and ruled out the sentinels `refused` names and every kind
    /// listed: `is_numeric($x)`, which leaves no null, `false`, object, array or callable. Read as
    /// [`Self::NotNull`] is, then as [`Self::OfKind`] on the side where the test failed for each
    /// kind -- in one step rather than one per kind.
    NoneOfKinds {
        of: Box<ValuePath>,
        at: phrost_model::TextSize,
        refused: Refused,
        kinds: &'static [phrost_model::BuiltinTy],
    },
    /// `is_string($box->value)` read about `$box`: the operands of a union of classes whose
    /// `value` cannot be of the kind the test said are gone. A filter over what `$box` was, read
    /// where the types are, because which class declares what is the index's question.
    MemberKind {
        of: Box<ValuePath>,
        at: phrost_model::TextSize,
        property: Name,
        kind: phrost_model::BuiltinTy,
        holds: bool,
    },
    /// A key the container was just found to hold: `array_key_exists($key, $map)`,
    /// `isset($map[$key])`. Of the kind the container's keys are -- read where the types are,
    /// because that is a question about the container's declaration.
    KeyOf {
        of: Box<ValuePath>,
        at: phrost_model::TextSize,
        container: Box<ValuePath>,
    },
    /// A value a strict `in_array($x, $list, true)` found: one of the container's elements, so of
    /// the type they are. Read where the types are, because what the elements are is a question
    /// about the container's declaration -- the same split [`Self::KeyOf`] makes for a key.
    ElementOf {
        of: Box<ValuePath>,
        at: phrost_model::TextSize,
        container: Box<ValuePath>,
    },
    /// A value an identity found not to be an array literal of scalars: the else of
    /// `if ($row === [null])`.
    ///
    /// Names no type, because what not being that array rules out depends on what the value is:
    /// only where every other slot already holds the literal's value is the one left the slot that
    /// differs, and then it is not the literal's value. Which slots those are is a question about
    /// the value's type, so the reader resolves `of` at `at` and compares slot by slot.
    NotIdenticalTo {
        of: Box<ValuePath>,
        at: phrost_model::TextSize,
        /// The array literal it was compared with, every entry a scalar literal under a key the
        /// literal settles.
        literal: phrost_hir::ExprId,
    },
    /// A strict identity held against a value whose type is not written beside it: `$status ===
    /// Status::Draft`, and `$order?->status === Status::Draft` read about `$order`. Whichever of
    /// null and `false` that value cannot be, `of` is not either.
    ///
    /// Read where the types are, because whether `Status::Draft` is an enum case or a `const DRAFT
    /// = null` is the declaration's to say. Only the sentinels and nothing about the type, which is
    /// what makes the second spelling sound: a chain cut short by `?->` yields null, so an end that
    /// matched a non-null value proves the receiver was there -- and says nothing else about it.
    SentinelsRefusedBy {
        of: Box<ValuePath>,
        at: phrost_model::TextSize,
        /// What the identity held against, read at `at` like `of`.
        other: Box<ValuePath>,
    },
    /// A guard ruled the empty string out and nothing else: `$x !== ''`, and the else of
    /// `$x === ''`. Every string the value may be holds something there -- a null stays a null,
    /// which is what the strict spelling leaves and the loose one does not. The reader resolves `of`
    /// at `at`, as for [`Self::NotNull`].
    ///
    /// `like` is a literal the value's strings are known to hold at least as much as: the needle of
    /// `str_starts_with($x, '$')`, which no `''` and no `'0'` begins with. Without one, the strings
    /// hold something and nothing more is known.
    NotEmptyString {
        of: Box<ValuePath>,
        at: phrost_model::TextSize,
        like: Option<ExprId>,
    },
    /// A guard ruled null out: `if ($x)`, `if ($x !== null)`, `isset($x)`, and the false side of
    /// each.
    ///
    /// Names no class, because the guard did not: something else already said what the value is and
    /// this only rules possibilities out. So the reader resolves `of` at `at` -- where the guard was
    /// written, and so where it does not reach, which is what makes that safe -- and then takes the
    /// null off what it finds.
    ///
    /// The subject is a whole path rather than a name because `if ($this->application === NULL) {
    /// return; }` is the guard a form class puts at the top of every handler, and the value it is
    /// about is a property.
    NotNull {
        of: Box<ValuePath>,
        at: phrost_model::TextSize,
        /// What the guard ruled out, which is not the same for every guard that lets a value past.
        ///
        /// Here rather than in three roots so that each of them costs the same single resolution
        /// hop. `if ($x)` is the commonest guard in PHP and every hop is charged against
        /// `FOLLOW_LIMIT`.
        refused: Refused,
    },
    /// A guard proved null: `if (!$x)`, `if ($x === null)`, `empty($x)`.
    Null {
        of: Box<ValuePath>,
        at: phrost_model::TextSize,
        /// Whether the test said null exactly.
        ///
        /// `=== null` and `isset` do. Truthiness does not: it says falsy, and only *null* when
        /// nothing else the value could hold is falsy -- which is a question about the type, since
        /// `?Repo` can be falsy only by being null and `?int` can be zero. So this says which claim
        /// was made and the type layer decides whether the weaker one is an answer.
        strictly: bool,
    },
    /// A boolean method call in a guard whose `@assert-if-true` names a class.
    ///
    /// `if ($app->isBulkOrder()) { ... }`. The class is written on the method's declaration, in
    /// whatever file that is, so this layer can name the call and no more.
    ///
    /// The receiver is boxed rather than named, because it is not always a local: `$this->isX()` is
    /// the commonest spelling of this. It is resolved at `at`, where the guard was written -- and so
    /// where it does not reach, which is what makes that safe.
    Asserted {
        receiver: Box<ValuePath>,
        method: Name,
        /// Whether the guarded branch is the one where the call returned true.
        when_true: bool,
        at: phrost_model::TextSize,
    },
    /// A boolean call in a guard whose `@phpstan-assert-if-true`/`-if-false` names one of its own
    /// *arguments*, not the receiver: `if (Validator::isUser($input)) { $input-> }`.
    ///
    /// The mirror of [`Self::Asserted`]: there the method is called *on* the value being narrowed,
    /// so the class comes off the receiver. Here the call is on a *different* class -- a static
    /// call, or an instance call whose receiver is not the narrowed value -- and the narrowed
    /// value is argument `argument` instead. So this carries what was called and which position,
    /// the same two things [`Self::ByReference`] already needs to find a callee's declaration,
    /// beside the narrowed value's own path rather than a receiver.
    ///
    /// `callee` is the same [`crate::narrowing::Callee`] a by-reference parameter uses, which
    /// also names a free function -- but nothing here constructs one of those, and the reason is
    /// no longer only cost. A boolean flag test is how most guards reach a free function
    /// (`$ok = checkIt($x); if ($ok) { ... }`), and every unrecognized call passed through that
    /// chase would pay for a declaration lookup that almost never resolves to anything. Measured
    /// on a real project: one such chase into a standard library call (`stripos`, inside a flag
    /// later tested by an unrelated guard) cost the one extra level of resolution budget that a
    /// `str_replace` two calls downstream needed to narrow its own conditional return -- turning
    /// a correct `string` into the wider `string|array` and tripping a real
    /// `incompatible-argument` finding four lines later.
    ///
    /// And a call written *directly* in the guard, which does not pay that cost, still cannot be
    /// admitted as it stands -- see
    /// `view::tests::argument_assertions::a_plain_function_in_the_guard_is_not_read_yet` for what
    /// it is worth and what it needs. This root resolves `of` at the *guard*, and a root built
    /// for a callee that turns out to carry no assertion therefore answers with what the subject
    /// held before the branch rather than what it holds at the use. Building one per
    /// `if (f($x))` in a project loses precision that nothing asked it to.
    ///
    /// `when_true` is the *branch's* polarity -- true inside `if (call())`, false past its
    /// negation or past a leaving guard -- and nothing more. Whether the tag itself is the
    /// `-if-true` or `-if-false` form is a fact about the declaration, read only at resolution,
    /// where the two are compared. Conflating them here would make `!Validator::isUser($x)` narrow
    /// the wrong branch.
    AssertedArgument {
        of: Box<ValuePath>,
        callee: Box<crate::narrowing::Callee>,
        argument: usize,
        /// The parameter it was written under, for `f(value: $x)`. `None` for a positional
        /// argument, same as [`Self::ByReference::name`].
        name: Option<Name>,
        when_true: bool,
        at: phrost_model::TextSize,
    },
    /// A class an `instanceof` guard proved the value has: `if ($x instanceof Repo) { $x-> }`.
    ///
    /// Its own arm rather than reusing the others, because it is a different claim: the declaration
    /// says what the value may be, and this says what it is *here*. A diagnostic will want to know
    /// which one it was told.
    ///
    /// With the value it is about, because the guard is not the whole answer. A declaration that
    /// named something the class has no relation to still holds -- `assert($mock instanceof Payment)`
    /// on a `MockInterface` leaves a value that is both -- so the reader asks what it was before.
    Narrowed {
        of: Box<ValuePath>,
        at: phrost_model::TextSize,
        class: ClassTarget,
    },
}

impl PathRoot {
    /// The value a *guard* root narrows, and where the guard was written.
    ///
    /// `None` for every other root, which is the whole vocabulary bar nine. A reader that walks
    /// roots by shape needs this or it stops at the first guard: `class_a_path_names` matched
    /// `Local` and `Written` and nothing else, so a `@var class-string<Rule>` with *any* guard
    /// between it and the `new $class` -- `is_string`, `=== NULL`, `class_exists` -- lost its
    /// argument, and the fixture written to pin that it does not lose it passed only because
    /// nothing in it narrowed at all.
    ///
    /// Exhaustive on purpose, listing every other root by name rather than falling through a `_`.
    /// The vocabulary grows, and a guard root added later must not answer "not a guard" quietly --
    /// the same hazard §8 records for `BuiltinTy`, where every match had a catch-all and a new
    /// variant fell down the wrong branch everywhere with nothing failing.
    pub fn narrows(&self) -> Option<(&ValuePath, phrost_model::TextSize)> {
        match self {
            Self::NotOfClass { of, at, .. }
            | Self::OneOfClasses { of, at, .. }
            | Self::OfKind { of, at, .. }
            | Self::NoneOfKinds { of, at, .. }
            | Self::KeyOf { of, at, .. }
            | Self::ElementOf { of, at, .. }
            | Self::NotIdenticalTo { of, at, .. }
            | Self::SentinelsRefusedBy { of, at, .. }
            | Self::MemberKind { of, at, .. }
            | Self::NotNull { of, at, .. }
            | Self::NotEmptyString { of, at, .. }
            | Self::Null { of, at, .. }
            | Self::AssertedArgument { of, at, .. }
            | Self::Narrowed { of, at, .. } => Some((of, *at)),
            Self::Asserted { receiver, at, .. } => Some((receiver, *at)),
            Self::Builtin(..)
            | Self::FunctionReturn { .. }
            | Self::Arithmetic { .. }
            | Self::Concatenation { .. }
            | Self::Stepped(..)
            | Self::EitherOf { .. }
            | Self::MaybeWritten { .. }
            | Self::RewrittenElements { .. }
            | Self::Written(..)
            | Self::InheritedParameter { .. }
            | Self::VariadicValues(..)
            | Self::ClassName(..)
            | Self::StringLiteral(..)
            | Self::Instantiated { .. }
            | Self::ArrayLiteral(..)
            | Self::Closure(..)
            | Self::Callable(..)
            | Self::Local(..)
            | Self::This
            | Self::SelfClass
            | Self::Parent
            | Self::Class(..)
            | Self::ObjectCast(..)
            | Self::Constant(..)
            | Self::ByReference { .. }
            | Self::CallbackParameter { .. } => None,
        }
    }
}

/// What is known about the name on the path that skipped every branch that wrote it.
///
/// Overwhelmingly, code writes a name in a branch to *normalise* it: `if (!$x) { $x = default(); }`
/// and `if (!$x instanceof T) { $x = makeT(); }` are the two spellings, and both say the same thing
/// -- the branch runs on the one path where the value was not already what the code needs, so the
/// path that skipped it is the path where it was. Reading that as an alternative rather than as a
/// completion puts back exactly the possibility the author wrote the branch to remove.
///
/// # Which mirrors are sound, and which are not
///
/// This once read "only the negative direction", and that conflated two families. A branch entered
/// because the value was *truthy* really cannot say the other path is null -- falsy is `0` and `''`
/// too, so the mirror over-claims, and it stays refused.
///
/// An **exact predicate** is different: `is_string($x)` and `$x instanceof T` are true or false and
/// nothing in between, so on the skipped path the negation is precisely what the author wrote.
/// `if (!is_string($body)) { $body = 'null'; }` is how PHP normalises a `string|false`, and reading
/// the skipped path as "unproved" put the `false` back -- nine `incompatible-argument` errors on one
/// small project, every one of them on a line that had just handled the case.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum Otherwise {
    /// Nothing: the branches were not entered on account of this name, so it is whatever it was.
    Unproved,
    /// There is no such path. The writes fill every arm of the construct, so control past it came
    /// through one of them and what the name held before is not one of the answers.
    ///
    /// `if ($c) { $this->x = a(); } else { $this->x = b(); }` is the shape, and it is how a value is
    /// built lazily when the two ways of building it differ -- Symfony's `Router::getGenerator`
    /// writes it. Read as a skipped path, the declared `?T` joins the two and every use of it below
    /// reports a null nothing can produce.
    Impossible,
    /// The name was not null there. `if (!$x)`, `if ($x === NULL)`, `empty($x)`, `!isset($x)`, and
    /// the same four as the left of an `&&`.
    ///
    /// `refused` decides one more operand: a guard that named more than null rules `false` out of
    /// the skipped path as well, which is how the standard library reports "nothing". The same
    /// vocabulary [`PathRoot::NotNull`] carries, for the same reason and read through the same call
    /// -- one concept and not two.
    NotNull { refused: Refused },
    /// The name was not `''` there: `if ($x === '') { $x = 'default'; }`. A null it may have been
    /// stays possible, which is the strict comparison's whole difference from a falsy test.
    NotEmptyString,
    /// The name passed a test that names no kind: `if (!is_numeric($x)) { $x = 0; }`, whose skipped
    /// path is where the value was numeric -- no null, no `false`, no object, array or callable.
    /// [`Self::NotNull`] with the kinds the test rules out beside it; see
    /// [`PathRoot::NoneOfKinds`].
    NoneOfKinds {
        refused: Refused,
        kinds: &'static [phrost_model::BuiltinTy],
    },
    /// The name was of this class there: `if (!$x instanceof T) { $x = ...; }`.
    OfClass(ClassTarget),
    /// And the mirror: `if ($x instanceof Month) { $x = $x->value; }`, whose skipped path is where it
    /// was not one.
    ///
    /// The complement the class direction was missing while the null and kind directions had both of
    /// theirs. It is how a union is normalised -- `int|Month` down to an `int` -- and without it the
    /// join put the class back beside the value the branch replaced it with.
    NotOfClass(ClassTarget),
    /// The name was *not* of this kind there: `if (is_int($x)) { $x = $this->load($x); }`, whose
    /// skipped path is every alternative the declaration listed except the int.
    ///
    /// The complement of a predicate rather than of a class, and needed for the same reason
    /// [`Self::NotNull`] is: without it the value before the branch keeps an alternative the branch
    /// exists to replace, and the join says `Doc|Doc|int` for a value that is a `Doc` either way.
    NotOfKind(phrost_model::BuiltinTy),
    /// The name *was* of this kind there: `if (!is_string($x)) { $x = 'default'; }`, whose skipped
    /// path is the one where it was a string all along.
    ///
    /// The mirror of [`Self::NotOfKind`] and sound for the reason given above: a type predicate has
    /// no third answer, so the negation of the guard is exactly what holds where it did not fire.
    OfKind {
        kind: phrost_model::BuiltinTy,
        /// As on [`PathRoot::OfKind`]: whether the guard named the kind or only ruled some
        /// alternatives of it out.
        settled: bool,
    },
    /// The guard was a call whose docblock says what it proves, and the claim is written on the
    /// callee rather than here: `if (!is_wp_error($thing)) { $thing = new WP_Error(); }` is how
    /// WordPress normalises one, and `@phpstan-assert-if-true WP_Error $thing` is where the class
    /// comes from.
    ///
    /// A whole path rather than a narrowing to apply, because resolving a tag means reading another
    /// file and the claim cannot be stated in this vocabulary. What it points at is the value
    /// before the branch with the assertion on top, which is exactly what the skipped path holds --
    /// so the reader resolves it instead of narrowing what it already has.
    Asserted(Box<ValuePath>),
}

impl Otherwise {
    /// Whether this claim, whatever else it says, says the value was there.
    ///
    /// The meet of two skipped-path claims that differ: an `instanceof` and a type predicate each
    /// imply the value is not null, so two of them that name different things still agree on that
    /// much. `is_null` is the one predicate that does not, which is why the kind is read rather
    /// than the variant.
    pub fn rules_out_null(&self) -> bool {
        match self {
            Self::NotNull { .. } | Self::NoneOfKinds { .. } | Self::OfClass(_) => true,
            Self::OfKind { kind, .. } => *kind != phrost_model::BuiltinTy::Null,
            Self::Unproved
            | Self::Impossible
            | Self::NotEmptyString
            | Self::NotOfClass(_)
            | Self::NotOfKind(_)
            | Self::Asserted(_) => false,
        }
    }
}

/// One write to a name, and where it was written.
///
/// The position travels with the value because a joined value's operands were written in different
/// branches: each one's names mean what they meant there, which is the same rule
/// [`crate::LocalBinding::written_at`] states for a value that stands alone.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct WriteSite {
    pub value: ValuePath,
    pub at: phrost_model::TextSize,
}

/// One operand of a joined value, and where it is written.
///
/// The sibling of [`WriteSite`], and it carries a position for the same reason. A ternary's arms and
/// a `match`'s arms are written in one expression, but each is *selected* by a condition -- so the
/// names inside an operand mean what they mean in the arm that holds them, not what they mean where
/// the join is read. Reading `is_array($x) ? $x[1] : $x` at one position gives the un-narrowed value
/// twice, and the union of that is the possibility the condition was written to remove.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct OperandSite {
    pub value: ValuePath,
    /// Where the operand begins.
    ///
    /// Its start and not its end: this is the first position at which the operand is being
    /// evaluated, so every guard that chose it is already in effect and no write the operand makes
    /// itself has happened yet. Reading its names after it ran would describe a value the path does
    /// not describe.
    pub at: phrost_model::TextSize,
}

impl OperandSite {
    /// The operand as written, or `None` when the expression is not a path.
    ///
    /// A `throw` is `never`, which is what PHP 8 made it by letting it stand where a value goes:
    /// `$repo->find($id) ?? throw new NotFound()` is the left side with its null taken away, and
    /// it takes the operator's own rule to say so -- the null comes off every operand but the
    /// fallback, and a fallback that never arrives adds nothing to the union.
    fn of(hir: &FileHir, expr: ExprId) -> Option<Self> {
        let value = match hir.expr(expr) {
            ExprData::Throw { .. } => {
                ValuePath::of(PathRoot::Builtin(phrost_model::BuiltinTy::Never))
            }
            _ => path_of(hir, expr)?,
        };
        Some(Self {
            value,
            at: hir.expr(expr).range().start(),
        })
    }
}

/// What ruled an operand out, for the operators that choose the next value when it did not survive.
///
/// The whole difference between `??`, `?:` and `match`. `$label ?? ''` cannot produce the null that
/// selected the empty string, and keeping it would make the one thing the author wrote the operator
/// to rule out read as still possible; a `match` arm has no such relationship to the ones after it,
/// each being chosen by a condition of its own.
///
/// And `?:` is not `??` -- it tests *truthiness*, so it rules out every falsy value rather than the
/// null alone. Spelling both as one flag made `tempnam(...) ?: NULL` a `string|false|null`, which is
/// the one thing that operator is written to prevent: `false` is exactly what it replaced.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Unless {
    /// Nothing: each operand is chosen by a condition of its own and holds whatever it holds.
    /// `match`, and the ternary written with both of its branches.
    Nothing,
    /// A null and nothing else -- `??` and `??=`. A `false` survives those.
    Null,
    /// Anything falsy, which is what `?:` tests. The type layer can name two of them, null and
    /// false, which is as far as this goes: a `0` or an `''` is a value no type in the table names.
    Falsy,
}

/// One step applied to what came before it.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum PathStep {
    /// `->name` or `::$name` — the property's declared type.
    Property(Name),
    /// `->name()` or `::name()` — the method's return type.
    ///
    /// A literal argument is part of the step, because for some methods it is part of the *type*:
    /// `getStorage('node')` and `getStorage('user')` return different classes, and
    /// `create(Foo::class)` returns a `Foo`. Carried as the value rather than as a reference to the
    /// expression, so two calls written the same way stay the same slot -- `if ($this->thing()) {
    /// $this->thing()->run(); }` narrows only because they compare equal, and an offset in the step
    /// would have quietly stopped that.
    ///
    /// With its position, because which argument decides is the callee's business: a catalog
    /// contract means the first, and a `class-string<T>` parameter can be anywhere.
    MethodReturn {
        name: Name,
        arguments: Box<[phrost_hir::body::LiteralArgument]>,
        call: CallSource,
    },
    /// `$handler()` -- what invoking the value gives back.
    ///
    /// Its own step rather than a `__invoke` method return, because naming the method would need an
    /// interner and this layer has none. What it means is the same: PHP's own rule is that calling an
    /// object calls its `__invoke`, and the reader is the layer that knows the name.
    Invoked { call: CallSource },
    /// `::NAME` -- what the class constant holds, and for an enum case the enum itself.
    ClassConstant(Name),
    /// What iterating names each slot by: the `$k` of `foreach ($rows as $k => $row)`.
    ///
    /// A step and not a root for the same reason [`Self::Element`] is one: what is iterated is
    /// itself a path, and the key is a second question about the same value.
    IterationKey,
    /// What iterating or indexing yields: `foreach ($items as $item)`, `$items[$id]`.
    ///
    /// A step rather than a root because what is indexed is itself a path -- `$this->repo->all()[0]`
    /// is three steps and then this.
    ///
    /// The key names the slot, when the source named it at all, so that two occurrences can be told
    /// apart -- and a comparison asks for both keys to be present and equal rather than for the steps
    /// to match, because a slot nobody can name is not the same slot as any other.
    Element(Option<ElementKey>),
}

/// How an index named the slot it reaches.
///
/// The two are kept apart on purpose. `$rows[$app]` and `$rows['app']` reach different slots -- one
/// is whatever the variable holds and the other is the string -- and folding them into one name would
/// let a claim about either answer for the other.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum ElementKey {
    /// `$rows[$id]`: the local it was written with.
    Variable(Name),
    /// `$rows['app']`, `$rows[0]`: the key itself, which is what a shape is keyed by.
    Literal(Name),
    /// `$storage[new Item()]`, `$rows[$a + 1]`: a key was written, and nothing here can name it.
    ///
    /// Apart from no key at all, which is how a `foreach` spells its element: the two are different
    /// questions of an object, since a subscript runs `offsetGet` and a loop runs the iterator --
    /// and `SplObjectStorage` answers them with different types.
    Unnamed,
}

/// Whether two paths name the same value, and not merely the same shape.
///
/// Only [`PathStep::Element`] makes this more than equality: an index written with no name to
/// compare is a slot this layer cannot identify, so it is not the same slot as any other -- not even
/// as itself. Everything else is a name or a nothing, and equality is enough.
pub(crate) fn same_slot(left: &ValuePath, right: &ValuePath) -> bool {
    left.steps.len() == right.steps.len() && covers_slot(left, right)
}

/// Whether writing through `written` changes what `subject` reaches.
///
/// The same slot, or one of the values `subject` is reached *through*: `$this->request = ...`
/// replaces what `$this->request->id` names as surely as writing to that field would, and a reader
/// that only compared whole slots would keep believing the field.
///
/// The step comparison is [`same_slot`]'s, and both are this one function for that reason -- an
/// unnamed index is a slot this layer cannot identify, and a rule that held for one length of path
/// and not the other would be two answers to one question.
pub(crate) fn covers_slot(written: &ValuePath, subject: &ValuePath) -> bool {
    if written.root != subject.root || written.steps.len() > subject.steps.len() {
        return false;
    }
    written
        .steps
        .iter()
        .zip(&subject.steps)
        .all(|(a, b)| match (a, b) {
            // An unnamed key is no slot anyone can identify, not even as itself.
            (PathStep::Element(Some(ElementKey::Unnamed)), _)
            | (_, PathStep::Element(Some(ElementKey::Unnamed))) => false,
            (PathStep::Element(Some(one)), PathStep::Element(Some(other))) => one == other,
            (PathStep::Element(_), _) | (_, PathStep::Element(_)) => false,
            _ => a == b,
        })
}

/// Reads an expression as a path, for a caller outside this crate.
///
/// The public door onto [`path_of`], for the one question that starts from an expression rather than
/// from a name: what is being *passed* at a call site.
pub fn value_path_of(hir: &FileHir, expr: ExprId) -> Option<ValuePath> {
    path_of(hir, expr)
}

/// Reads an expression as a path, or `None` when it is not one.
///
/// Walks receivers inward, collecting steps, until it reaches something that names a value: a
/// variable, a `new`, or a class. An expression whose type depends on evaluating it — a literal, an
/// array access, an operator — is not a path, and saying so is what keeps a guess out.
/// The path a bare local is written as, for a caller that has the name and not the expression.
///
/// The one path a reader outside this crate needs to build, and it exists so that none of them
/// builds one: [`IndexView::contract_narrowed`](../../phrost_view/index.html) matches a call's
/// arguments by comparing written paths, so asking it what a *local* was passed to means handing it
/// this. Handing it the path of what the local's binding holds instead matches nothing and returns
/// the value unrefined -- which is what made the hover card disagree with every check about one
/// variable.
pub fn local_path(name: &phrost_model::Name) -> ValuePath {
    ValuePath::of(PathRoot::Local(name.clone()))
}

pub(crate) fn path_of(hir: &FileHir, expr: ExprId) -> Option<ValuePath> {
    let mut steps = Vec::new();
    let mut current = expr;
    let mut may_be_absent = false;

    loop {
        let mut path = match hir.expr(current) {
            ExprData::New { class, .. } => ValuePath::of(PathRoot::Instantiated {
                class: *class,
                call: CallSource::of(current),
            }),
            // The cast decides the type, whatever it was applied to: `(int) $node->id()` is an int.
            ExprData::Cast {
                to: phrost_model::BuiltinTy::Object,
                value,
                ..
            } => ValuePath::of(match path_of(hir, *value) {
                Some(value) => PathRoot::ObjectCast(Box::new(value)),
                None => PathRoot::Builtin(phrost_model::BuiltinTy::Object),
            }),
            // `(string) $x` is `'' . $x`: a string, holding what the value spells -- digits for a
            // number, `'1'` for `true`.
            ExprData::Cast { to, value, .. } if to.is_string() => {
                ValuePath::of(concatenation(hir, *to, &[*value]))
            }
            ExprData::Cast { to, .. } => ValuePath::of(PathRoot::Builtin(*to)),
            ExprData::ConstantAccess { name, .. } => ValuePath::of(PathRoot::Constant(*name)),
            // A literal states its own type, and most assignments in a real project are one.
            ExprData::Literal {
                kind: phrost_hir::body::LiteralKind::String,
                ..
            } => ValuePath::of(PathRoot::StringLiteral(current)),
            ExprData::Literal { kind, .. } => ValuePath::of(PathRoot::Builtin(builtin_of(*kind))),
            // An operator that decides its own result, and the four boolean shapes lowered as
            // themselves. `$a instanceof B` is a bool as surely as `$a === $b` is.
            ExprData::Operator {
                produces, inner, ..
            } => ValuePath::of(concatenation(hir, *produces, hir.expr_list(*inner))),
            ExprData::Logical { .. }
            | ExprData::Instanceof { .. }
            | ExprData::NullTest { .. }
            | ExprData::Comparison { .. }
            | ExprData::Not { .. } => {
                ValuePath::of(PathRoot::Builtin(phrost_model::BuiltinTy::Bool))
            }
            ExprData::Arithmetic {
                operator,
                left,
                right,
                ..
            } => ValuePath::of(PathRoot::Arithmetic {
                operator: *operator,
                left: Box::new(path_of(hir, *left)?),
                right: Box::new(path_of(hir, *right)?),
            }),
            // `-$x` is `0 - $x` to every type question: the operand decides, and zero is an int.
            ExprData::Signed { value, .. } => ValuePath::of(PathRoot::Arithmetic {
                operator: phrost_hir::body::ArithmeticOp::Subtract,
                left: Box::new(ValuePath::of(PathRoot::Builtin(
                    phrost_model::BuiltinTy::Int,
                ))),
                right: Box::new(path_of(hir, *value)?),
            }),
            ExprData::Coalesce { left, right, .. } => ValuePath::of(PathRoot::EitherOf {
                operands: vec![OperandSite::of(hir, *left)?, OperandSite::of(hir, *right)?],
                unless: Unless::Null,
            }),
            ExprData::Conditional {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                // `?:` has no then-branch: PHP reuses the condition's own value, and the operator
                // reached the else because that value was *falsy* -- so neither a null nor a false
                // can come out of it, which is more than `??` rules out and the reason the two are
                // not one flag.
                let (left, unless) = match then_branch {
                    Some(then) => (*then, Unless::Nothing),
                    None => (*condition, Unless::Falsy),
                };
                ValuePath::of(PathRoot::EitherOf {
                    operands: vec![
                        OperandSite::of(hir, left)?,
                        OperandSite::of(hir, *else_branch)?,
                    ],
                    unless,
                })
            }
            // `match ($k) { 1, 2 => $a, default => $b }` -- one arm's body out of several, the
            // same union `??` and the ternary already produce, generalised to as many operands as
            // there are arms. The conditions contribute nothing to the value: `1, 2 => $a` is
            // still just `$a` on the value side, whichever condition matched.
            //
            // A `throw` arm removes itself rather than joining the union as a junk branch --
            // `match (true) { $ok => $repo->find(), default => throw new NotFound() }` is the
            // common guard-chain idiom, and reading the `throw` as a real operand would leave
            // every guarded `match` unioned with a type nobody can observe.
            ExprData::Match { arms, .. } => {
                let mut operands = Vec::new();
                for &arm in hir.match_arm_list(*arms) {
                    let body = hir.match_arm(arm).body;
                    if matches!(hir.expr(body), ExprData::Throw { .. }) {
                        continue;
                    }
                    operands.push(OperandSite::of(hir, body)?);
                }
                match operands.len() {
                    // Every arm throws (or there were none), so nothing here ever produces a
                    // value to type.
                    0 => return None,
                    // One value out of one is that value outright, not a one-operand union
                    // wrapping it. Its own steps have to travel with it rather than being
                    // replaced by the epilogue below, which is written for a root that starts
                    // with none of its own -- so this returns directly instead of falling
                    // through to it.
                    1 => {
                        let mut only = operands.remove(0).value;
                        steps.reverse();
                        only.steps.extend(steps);
                        only.may_be_absent |= may_be_absent;
                        return Some(only);
                    }
                    _ => ValuePath::of(PathRoot::EitherOf {
                        operands,
                        unless: Unless::Nothing,
                    }),
                }
            }
            ExprData::ArrayLiteral { elements, .. } => match describes_nothing(hir, *elements) {
                true => ValuePath::of(PathRoot::Builtin(phrost_model::BuiltinTy::Array)),
                false => ValuePath::of(PathRoot::ArrayLiteral(current)),
            },
            ExprData::FunctionCall {
                name,
                args,
                literal_arguments,
                ..
            } => ValuePath::of(PathRoot::FunctionReturn {
                name: hir.name_ref(*name).path.clone(),
                anchor: hir.name_ref(*name).anchor,
                // Only the arguments that cannot change between two occurrences of the call: what
                // this decides is whether a guard written about one of them narrows the other, and
                // an interpolated argument is read afresh each time. It still tells the two *apart*
                // -- that is [`PathStep::MethodReturn`]'s business and this field is not in it.
                every_argument_written: literal_arguments
                    .iter()
                    .filter(|argument| argument.is_fixed_where_it_is_written())
                    .count()
                    == hir.expr_list(*args).len(),
                arguments: literal_arguments.clone(),
                call: CallSource::of(current),
            }),
            // The closure itself is the value, and it is one worth naming: a variable holding one
            // is how PHP writes a factory, and `$make()` is dead until this says what `$make` is.
            ExprData::Closure { .. } => ValuePath::of(PathRoot::Closure(current)),
            // `strlen(...)` -- a callable handing back what the function returns. The inner path is
            // the call that was not written, which is what makes the outer wrapper load-bearing:
            // without it the variable would hold the return type instead of a callable producing
            // it, and `$fn(1)` would then be a call on an `int`.
            ExprData::CallableFunction { name, .. } => ValuePath::of(PathRoot::Callable(Box::new(
                ValuePath::of(PathRoot::FunctionReturn {
                    name: hir.name_ref(*name).path.clone(),
                    anchor: hir.name_ref(*name).anchor,
                    // `strlen(...)` is the callable and not a call, so there are no arguments to
                    // have left out.
                    every_argument_written: true,
                    arguments: Box::new([]),
                    call: CallSource::of(current),
                }),
            ))),
            // `$repo->find(...)`, `Repo::find(...)`. Same shape, with the receiver walked first --
            // a computed name (`$repo->$name(...)`) names no method and so produces no value.
            ExprData::CallableMember {
                receiver,
                name: Some(name),
                ..
            } => {
                let mut inner = match receiver {
                    CallableReceiver::Value(value) => path_of(hir, *value)?,
                    CallableReceiver::Class(class) => ValuePath::of(class_path_root(*class)),
                };
                inner.steps.push(PathStep::MethodReturn {
                    name: name.clone(),
                    arguments: Box::new([]),
                    call: CallSource::of(current),
                });
                ValuePath::of(PathRoot::Callable(Box::new(inner)))
            }
            // `$handler()`, `($this->handler)()` -- calling a value calls its `__invoke`, and what
            // is called is itself a path.
            ExprData::IndirectCall { callee, .. } => {
                steps.push(PathStep::Invoked {
                    call: CallSource::of(current),
                });
                current = *callee;
                continue;
            }
            ExprData::Variable { name, .. } => ValuePath::of(if name.as_str() == "this" {
                PathRoot::This
            } else {
                PathRoot::Local(name.clone())
            }),
            // A clone is another object of the same class, so it reaches whatever the original
            // does. The copy is a different *object*, which nothing here claims: what travels is
            // the type, and the slot a copy is assigned to is its own.
            ExprData::Cloned { value, .. } => {
                current = *value;
                continue;
            }
            ExprData::PropertyAccess {
                receiver,
                name: Some(name),
                nullsafe,
                ..
            } => {
                may_be_absent |= *nullsafe;
                steps.push(PathStep::Property(name.clone()));
                current = *receiver;
                continue;
            }
            ExprData::MethodCall {
                receiver,
                name: Some(name),
                literal_arguments,
                nullsafe,
                ..
            } => {
                may_be_absent |= *nullsafe;
                steps.push(PathStep::MethodReturn {
                    name: name.clone(),
                    arguments: literal_arguments.clone(),
                    call: CallSource::of(current),
                });
                current = *receiver;
                continue;
            }
            ExprData::StaticPropertyAccess {
                class,
                name: Some(name),
                ..
            } => {
                steps.push(PathStep::Property(name.clone()));
                ValuePath::of(class_path_root(*class))
            }
            ExprData::StaticCall {
                class,
                name: Some(name),
                literal_arguments,
                ..
            } => {
                steps.push(PathStep::MethodReturn {
                    name: name.clone(),
                    arguments: literal_arguments.clone(),
                    call: CallSource::of(current),
                });
                ValuePath::of(class_path_root(*class))
            }
            ExprData::ClassConstAccess {
                class,
                name: Some(name),
                ..
            } => {
                // `Foo::class` is spelled like a constant and is a string, case-insensitively --
                // PHP accepts `Foo::CLASS`. Everything else is a real constant, and for an enum
                // that constant is a case, which is an instance of the enum.
                if name.as_str().eq_ignore_ascii_case("class") {
                    ValuePath::of(PathRoot::ClassName(*class))
                } else {
                    steps.push(PathStep::ClassConstant(name.clone()));
                    ValuePath::of(class_path_root(*class))
                }
            }
            // `/** @var T $a */ $a = ...`: the annotation is what the author said the variable
            // holds, and it is written precisely because the value does not say.
            ExprData::Assign {
                doc_type_ref: Some(annotated),
                ..
            } => ValuePath::of(PathRoot::Written(*annotated)),
            // `$x ??= $fallback` leaves the target holding what it already held, unless that was
            // null. So it is exactly the union `??` builds -- and its left operand is the target
            // itself, read at the target's own position, which is before this write and therefore
            // finds whatever bound the name last.
            //
            // No node is synthesised for that read. The target is written once and lowered once,
            // and giving the union a second copy of it would put the name in every occurrence walk
            // twice -- two identical rename edits at one range, which is not a duplicate so much as
            // a corrupt edit.
            ExprData::Assign {
                operator: AssignOp::Coalesce,
                target,
                value,
                ..
            } => ValuePath::of(PathRoot::EitherOf {
                operands: vec![
                    OperandSite::of(hir, *target)?,
                    OperandSite::of(hir, *value)?,
                ],
                unless: Unless::Null,
            }),
            // `$n .= 'x'`, `$flags &= $mask`: the operator decides the result alone -- and for a
            // string, what it holds is the target's and the appended piece's together, the target
            // read at its own position as `+=` reads it below.
            ExprData::CompoundAssign {
                produces: Some(produces),
                target,
                value,
                ..
            } => ValuePath::of(concatenation(
                hir,
                *produces,
                &[Some(*target), *value]
                    .into_iter()
                    .flatten()
                    .collect::<Vec<_>>(),
            )),
            // `$total += $amount` is `$total + $amount`, over the target's own value -- read at
            // the target, which is before this write, as `??=` reads it.
            ExprData::CompoundAssign {
                target,
                value: Some(value),
                arithmetic: Some(operator),
                ..
            } => ValuePath::of(PathRoot::Arithmetic {
                operator: *operator,
                left: Box::new(path_of(hir, *target)?),
                right: Box::new(path_of(hir, *value)?),
            }),
            // `$i++` and `++$i`, which the tree spells as one write with no right-hand side and
            // tells apart only by where the target sits. The postfix hands back the value before
            // the write, which is the target itself -- a null stays a null -- and the prefix the
            // value after, which is a step and not `$i + 1`: see `PathRoot::Stepped`.
            ExprData::CompoundAssign {
                target,
                value: None,
                arithmetic: None,
                produces: None,
                range,
            } => match hir.expr(*target).range().start() == range.start() {
                true => {
                    current = *target;
                    continue;
                }
                false => ValuePath::of(PathRoot::Stepped(Box::new(path_of(hir, *target)?))),
            },
            // `$a = $b = new Foo()`: the value of the outer assignment is the inner one's.
            ExprData::Assign { value, .. } => {
                current = *value;
                continue;
            }
            // `$rows[0]` is one of whatever `$rows` holds -- the same step a `foreach` takes, and
            // it pays off wherever the base has an element type to give: a `@param array<Item>`,
            // or a method returning `list<Item>`.
            ExprData::Index {
                base, index, key, ..
            } => {
                // A literal key first: it is what a shape is keyed by, and it names the slot at
                // least as well as a variable does.
                let named = key.clone().map(ElementKey::Literal).or_else(|| {
                    index.and_then(|id| match hir.expr(id) {
                        ExprData::Variable { name, .. } => Some(ElementKey::Variable(name.clone())),
                        _ => Some(ElementKey::Unnamed),
                    })
                });
                steps.push(PathStep::Element(named));
                current = *base;
                continue;
            }
            // `$alias = &$target` makes them the same value, so the path is the target's.
            ExprData::Reference { value, .. } => {
                current = *value;
                continue;
            }
            _ => return None,
        };

        // Collected innermost-first while walking receivers inward.
        steps.reverse();
        path.steps = steps;
        path.may_be_absent |= may_be_absent;
        return Some(path);
    }
}

pub(crate) fn class_path_root(target: ClassTarget) -> PathRoot {
    match target {
        ClassTarget::Relative {
            relative: phrost_model::RelativeTy::Parent,
            ..
        } => PathRoot::Parent,
        ClassTarget::Relative {
            relative: phrost_model::RelativeTy::SelfTy,
            ..
        } => PathRoot::SelfClass,
        // `static::` is late static binding: the class the call was made through, which is what a
        // guard on `$this` proves.
        ClassTarget::Relative { .. } => PathRoot::This,
        named => PathRoot::Class(named),
    }
}

/// A member named at a `->` or `::` in a body, and what it was named on.
///
/// The site rather than the declaration: this is what a cursor lands on, and what references and
/// rename will enumerate.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct MemberSite {
    /// Where the receiver's value came from.
    pub receiver: ValuePath,
    pub name: Name,
    pub kind: SiteKind,
    /// Range of the member name alone, which is what goto and rename operate on.
    pub name_range: phrost_model::TextRange,
    /// Range of the receiver as written. Not needed to answer anything -- it is there so a report
    /// can say *which expression* had no type, which is the difference between "118 sites of
    /// `getValue`" and "111 sites of `$x->get('field')`". The first is a number, the second is a task.
    pub receiver_range: phrost_model::TextRange,
    /// Whether the access was written `?->`.
    ///
    /// A fact about the text, so it belongs here rather than being re-derived by scanning backwards
    /// from the name -- which is where a strip-order bug once killed every nullsafe feature. What it
    /// is for: `?->` handles a null receiver itself, so it is the one spelling where a possibly-null
    /// receiver is not a defect.
    pub nullsafe: bool,
    /// Whether the syntax only *suggests* that a member is named here.
    ///
    /// True for one shape: `[Foo::class, 'bar']`, which is a callable and is equally a list of two
    /// class names -- see [`ExprData::CallableMember::presumed`](phrost_hir::body::ExprData). A
    /// reader that improves with a false site keeps it; one that would report a defect because of
    /// it drops it first.
    pub presumed: bool,
    /// Whether the member is *named* here rather than reached: a string a test double's configuring
    /// call passes, which says which method is meant and calls nothing from where it is written.
    /// So it is a reference for every reader that asks where a member is mentioned, and not one
    /// for a reader that asks whether this scope may reach it, or whether the member ever runs.
    pub named: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SiteKind {
    Property,
    Method,
    ClassConstant,
}

/// Whether an expression is the literal `null`.
/// The root an operator that decides its own result stands for: that builtin, or -- for a string
/// the syntax did not already prove non-falsy -- a [`PathRoot::Concatenation`] over the pieces the
/// reader may know more about. A piece that is not a path adds nothing and is left out.
fn concatenation(hir: &FileHir, produces: phrost_model::BuiltinTy, pieces: &[ExprId]) -> PathRoot {
    if !produces.is_string() || produces == phrost_model::BuiltinTy::NonFalsyString {
        return PathRoot::Builtin(produces);
    }
    let pieces = pieces
        .iter()
        .filter_map(|&piece| path_of(hir, piece))
        .collect::<Box<[_]>>();
    match pieces.is_empty() {
        true => PathRoot::Builtin(produces),
        false => PathRoot::Concatenation {
            holds: produces,
            pieces,
        },
    }
}

/// The type a literal is.
fn builtin_of(kind: phrost_hir::body::LiteralKind) -> phrost_model::BuiltinTy {
    use phrost_hir::body::LiteralKind;
    use phrost_model::BuiltinTy;

    match kind {
        LiteralKind::Int => BuiltinTy::Int,
        LiteralKind::Float => BuiltinTy::Float,
        LiteralKind::String => BuiltinTy::String,
        // Two types, not one, and the comment here used to say the opposite -- that telling `true`
        // from `false` was narrowing's question rather than this one. It is both. Narrowing removes
        // `BuiltinTy::False` from a union and cannot remove `Bool`, because a `bool` may still be
        // `true`; so `$x = $c ? thing() : FALSE;` guarded by `if ($x)` kept the whole `Thing|bool`
        // and reported every use below the guard. What a name holds is what narrowing has to work
        // from, and a widened literal is a fact thrown away before the question is asked.
        LiteralKind::True => BuiltinTy::True,
        LiteralKind::False => BuiltinTy::False,
        LiteralKind::Null => BuiltinTy::Null,
    }
}

/// The call expression a step was read from, when it was read from one.
///
/// Carried because the literals are not the whole of what a call says. A contract can say that what
/// comes back is the *element* of what was passed, and an element is a type rather than a literal --
/// so the reader has to resolve the argument, and needs the call to find it. It also says how many
/// arguments were written, which is what decides that a parameter was left out and its default
/// applies -- the whole of what `response()` says.
///
/// Outside a step's identity, deliberately, and that is what the hand-written impls below are for.
/// Two occurrences of the same call are the same value, and only one of the two readers of a call has
/// an expression at all: the lexical scanner that reads a receiver at the cursor exists precisely
/// because there is none. If this told two paths apart, no guard written about a method call would
/// match the call it guards -- which cost 220 resolved member accesses the first time a field here
/// was part of the identity.
#[derive(Clone, Copy, Debug, Default)]
pub struct CallSource(Option<phrost_hir::ExprId>);

impl CallSource {
    /// The call, and `None` for a reader that had no expression to point at.
    pub fn expr(self) -> Option<phrost_hir::ExprId> {
        self.0
    }

    /// For a reader that read a call out of the text rather than out of the tree.
    pub fn unread() -> Self {
        Self(None)
    }

    pub(crate) fn of(expr: phrost_hir::ExprId) -> Self {
        Self(Some(expr))
    }
}

impl PartialEq for CallSource {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl Eq for CallSource {}

impl std::hash::Hash for CallSource {
    fn hash<H: std::hash::Hasher>(&self, _: &mut H) {}
}

/// Whether an array literal describes nothing at all, which is only ever the empty one.
///
/// `[]` says what a bare `array` says, and what it eventually holds is settled by the writes into
/// it rather than by the literal. Everything else keeps its node and is read above: a list's entries
/// are each a path, so their union is what iterating yields, and a keyed literal's entries are each
/// a *slot*, so the keys and what they hold are a record.
///
/// This used to refuse a keyed literal as well, on the ground that `['a' => 1]` is a shape and not a
/// list. True, and it made the shape unreachable: the root was replaced by the bare `array` here,
/// below the only layer that can resolve what a key holds, so the distinction was gone before any
/// reader existed. One node, and which of the two readings applies is decided where the types are.
fn describes_nothing(hir: &FileHir, elements: lspkit_arena::IdSlice<phrost_hir::ExprId>) -> bool {
    hir.expr_list(elements).is_empty()
}
