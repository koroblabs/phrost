use lspkit_arena::IdSlice;
use phrost_model::{BuiltinTy, Name, RelativeTy, TextRange};

use crate::ids::{BodyId, CatchId, ExprId, MatchArmId, NameRefId, ParamId, StmtId, SwitchCaseId};

/// One function-like's body: the statements it runs, and where they are.
///
/// Only lowered for open buffers. A body is an order of magnitude larger than the declaration it
/// belongs to, and nothing cross-file ever needs one — the answer to "where is this declared" and
/// "what members does this class have" is the same whether or not anybody looked inside a method.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct BodyData {
    pub statements: IdSlice<StmtId>,
    /// The braces included, so a position can be tested against the body rather than the
    /// declaration — a cursor between `)` and `{` is in neither.
    pub range: TextRange,
    /// Whether a `yield` is written directly in this body.
    ///
    /// A `yield` anywhere makes the function a generator, and a generator's declared return type
    /// describes the *generator* rather than what `return` produces -- so anything judging a
    /// returned value has to know, or it reports every generator in the project.
    ///
    /// Per body and not per function, which is the whole difficulty: a `yield` inside a closure
    /// makes the closure a generator and leaves the function around it alone.
    pub is_generator: bool,
}

/// One statement.
///
/// Deliberately fewer shapes than PHP has. What is modelled is what carries a variable, a member
/// access, or a branch; everything else is [`StmtData::Unmodelled`] with its range, so the tree is
/// complete and the gaps are visible in a snapshot rather than silently absent.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum StmtData {
    /// A bare expression, with the statement's own range — which includes the `;` the expression
    /// does not. Kept rather than deriving it, so every statement kind has a range and a position
    /// lookup never has a case where it does not.
    Expr { expr: ExprId, range: TextRange },
    Return {
        value: Option<ExprId>,
        range: TextRange,
    },
    /// `elseif` chains are normalized into nested ifs in the else branch: one shape to narrow
    /// against instead of a chain to walk.
    If {
        condition: ExprId,
        then_branch: IdSlice<StmtId>,
        else_branch: IdSlice<StmtId>,
        range: TextRange,
    },
    /// `/** @var User $entity */` written on a line of its own.
    ///
    /// A statement because that is what it behaves like: it says what a variable holds from there on,
    /// in the scope it is written in, and PHP has no block scope for variables so position is the
    /// whole of it. There is no node in the tree for a comment, so this is synthesized.
    ///
    /// The most-used way to type a value in a framework project -- 243 of them in one real Drupal
    /// site -- because it is what an author writes when the inferred type is too general: a hook
    /// receives an `EntityInterface` and the body knows it is a `User`.
    ///
    /// Only named ones. A nameless `@var` is consumed by the assignment or the loop below it.
    Annotation {
        name: Name,
        ty: crate::ids::TypeRefId,
        range: TextRange,
    },
    Foreach {
        subject: ExprId,
        key: Option<ExprId>,
        value: ExprId,
        body: IdSlice<StmtId>,
        /// `/** @var Special $item */` written above the loop, naming what the value variable holds.
        ///
        /// The standard way to type a loop variable, and written precisely because what the subject
        /// says about its elements is too general: `@return array<Basic>` on the method that produced
        /// it, when every element in this loop is a `Special`. Every other PHP tool honours it, and
        /// the same field on an assignment already did.
        doc_type_ref: Option<crate::ids::TypeRefId>,
        range: TextRange,
    },
    /// `while` and `do ... while` both.
    While {
        condition: ExprId,
        body: IdSlice<StmtId>,
        /// Whether the condition is tested before the body runs -- true of `while`, false of
        /// `do ... while`.
        ///
        /// It used to say here that the difference matters to a runtime and not to anything this
        /// answers, and that stopped being true the moment a loop's condition was read as a guard:
        /// `while ($row = next())` proves the row is there *inside the body*, and the same words
        /// written after a `do` prove it only from the second turn onwards -- so believing them on
        /// the first is a claim about code that has not run.
        tested_first: bool,
        range: TextRange,
    },
    /// `for ($i = 0; $i < $n; $i++)`.
    ///
    /// Its own shape rather than a [`Self::While`], because the three clauses hold expressions that
    /// bind: `for ($i = 0; ...)` is where `$i` comes from. Modelled at all because an unmodelled
    /// statement keeps its expressions and not its statements -- so until this existed the whole
    /// *body* of every `for` loop was invisible to every statement walk. A `foreach` written inside
    /// one bound nothing, and a guard written inside one guarded nothing.
    For {
        /// `$i = 0`, and anything else before the first `;`.
        init: IdSlice<ExprId>,
        /// The tests. Only the last decides whether the loop runs again, which nothing here needs.
        conditions: IdSlice<ExprId>,
        /// `$i++`.
        step: IdSlice<ExprId>,
        body: IdSlice<StmtId>,
        range: TextRange,
    },
    Block {
        body: IdSlice<StmtId>,
        range: TextRange,
    },
    Try {
        body: IdSlice<StmtId>,
        catches: IdSlice<CatchId>,
        finally: IdSlice<StmtId>,
        range: TextRange,
    },
    /// `switch ($subject) { case ...: ...; default: ... }`.
    ///
    /// Modelled for the same reason `if` is: flow analysis has to see whether a case leaves, and
    /// whether the whole thing is exhaustive. See [`SwitchCaseData`] for how fallthrough folds.
    Switch {
        subject: ExprId,
        cases: IdSlice<SwitchCaseId>,
        range: TextRange,
    },
    /// `break`, `continue` — a statement that leaves the enclosing flow.
    ///
    /// Modelled because exactly one thing is ever asked of them: whether the code after this point
    /// runs. `return` is separate because its value is a fact too, and `throw` is an expression.
    Leave { kind: LeaveKind, range: TextRange },
    /// `static $x;`, `static $x = 1;`, `global $x;`.
    ///
    /// Not PHP's `declare(strict_types=1)`, which is a different statement entirely. These bring a
    /// name into the function's scope from outside it -- a slot that survives the call, or the
    /// global scope -- and they are the only way a local is introduced without being written to.
    /// Left unmodelled they bound nothing, because the variable they name is not an expression and
    /// so no walk ever reached it.
    Bind {
        kind: BindKind,
        /// A bare [`ExprData::Variable`] for `static $x;`, and an [`ExprData::Assign`] for
        /// `static $x = 1;`. Spelling the second as an assignment is what makes it free: everything
        /// that already reads an assignment gives the initialiser's type with no new arm.
        bindings: IdSlice<ExprId>,
        range: TextRange,
    },
    /// A statement shape that is not modelled — an `echo`, a `global`, a `declare`.
    ///
    /// The expressions written inside it are still lowered and listed here. Without that, an
    /// unmodelled shape would hide everything within it: a method call inside an unmodelled
    /// statement would have no node, so nothing could be clicked, and the gap would look like the
    /// feature being broken rather than the statement being unmodelled.
    Unmodelled {
        inner: IdSlice<ExprId>,
        range: TextRange,
        /// What control does here, which is the one thing about an unmodelled statement that has to
        /// be known even when its meaning is not.
        passage: Passage,
        /// Which of the few unmodelled statements a reader has to tell apart this one is.
        kind: UnmodelledKind,
    },
}

/// The unmodelled statements some reader has to recognise, and everything else.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum UnmodelledKind {
    /// Runs and means nothing any reader asks about: `echo`, `declare`, `goto`.
    Statement,
    /// *Declares* rather than runs: a class, an interface, a trait, an enum, a function, a
    /// top-level `const`, an import.
    ///
    /// Kept because what is written inside a declaration is not code that runs where the statement
    /// is. A property default, a class constant, a parameter default and an attribute's argument are
    /// all constant expressions, and they are lowered -- so goto and hover reach them -- as nested
    /// expressions of the unmodelled statement around them. A reader that mistook that containment
    /// for membership offered to lift one into a variable *above* the declaration, which is a file
    /// PHP refuses.
    Declaration,
    /// `unset(...)`, whose nested expressions are exactly the things it unsets.
    ///
    /// Kept because removing a key is a write to an array: a record that had `tmp` before
    /// `unset($row['tmp'])` does not have it after, and reading the literal alone hands a sealed
    /// `array{...}` a key it refuses.
    Unset,
}

/// What control does at a statement this layer does not otherwise model.
///
/// Everything unmodelled used to be answered the same way and marked unreadable, on the grounds
/// that nothing told an `echo` from a `goto`. Something does: which of PHP's statements it is. The
/// list of statements that reach the unmodelled arm is closed at fourteen, and every one of them
/// falls cleanly into one of these three -- so the distinction is exact rather than a guess, and it
/// belongs where the mago node is still in hand.
///
/// The cost of not having it was not the `echo` after a `return` that it looked like. A dead region
/// *beginning* with one was dropped whole, and a dead region usually begins with a log line.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Passage {
    /// Control walks straight through: `echo`, `unset`, `declare`.
    Onward,
    /// Control leaves for somewhere this layer does not follow: `goto`, `__halt_compiler()`.
    Elsewhere,
    /// Control can arrive here from somewhere else, so nothing after it is dead on account of what
    /// came before. A `goto` label is one; so is every declaration, because PHP hoists a `function`
    /// or a `class` whatever it is written after.
    Arrival,
}

impl StmtData {
    /// Where the statement is. Every kind has one, so a position lookup can descend without
    /// knowing which kind it is looking at.
    pub fn range(&self) -> TextRange {
        match self {
            Self::Expr { range, .. }
            | Self::Return { range, .. }
            | Self::If { range, .. }
            | Self::Foreach { range, .. }
            | Self::For { range, .. }
            | Self::While { range, .. }
            | Self::Block { range, .. }
            | Self::Try { range, .. }
            | Self::Switch { range, .. }
            | Self::Leave { range, .. }
            | Self::Bind { range, .. }
            | Self::Annotation { range, .. }
            | Self::Unmodelled { range, .. } => *range,
        }
    }
}

/// One `catch` clause. The hint is what gives `$e` a type.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CatchData {
    /// The caught type, as written. A multi-catch (`catch (A|B $e)`) arrives as a union.
    pub type_ref: Option<crate::ids::TypeRefId>,
    /// PHP 8 allows `catch (Throwable)` with no variable at all.
    pub variable: Option<Name>,
    pub variable_range: Option<TextRange>,
    pub body: IdSlice<StmtId>,
    pub range: TextRange,
}

/// One `case`/`default` clause of a `switch`, after empty-bodied fallthrough labels are folded
/// into the arm they fall through to.
///
/// `case 1: case 2: echo "one or two"; break;` lowers to one [`SwitchCaseData`] with two
/// `conditions`, not two clauses -- so "does this arm leave" is answered by looking at one body
/// instead of chasing fallthrough across clauses.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SwitchCaseData {
    /// The values compared with `==`. Empty when nothing but a `default:` label fell into this
    /// arm.
    pub conditions: IdSlice<ExprId>,
    /// Whether this arm's fallthrough chain included a bare `default:` label. Tracked separately
    /// from `conditions` being empty, because a `default:` can itself fall through from -- or
    /// into -- labels that carry values: `case 1: default: ...` is one arm that is both.
    pub is_default: bool,
    pub body: IdSlice<StmtId>,
    pub range: TextRange,
}

/// Which spelling of a null test was written.
///
/// Four rather than two flags, because the two questions a reader asks -- how sure the test is, and
/// whether PHP complains about a null on the way to the value -- are both answered by *which*
/// spelling it was, and a pair of booleans could be set to a combination nobody can write.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NullTestKind {
    /// `$x === null`, `$x !== null`. Says null exactly.
    Identical,
    /// `$x == null`, `$x != null`. Says falsy, which for a `?int` includes `0`.
    Loose,
    /// `isset($x)`, wrapped in the `Not` that makes it a presence test. Says null exactly.
    Isset,
    /// `empty($x)`. Says falsy.
    Empty,
    /// `strlen($x) === 0`, lowered as the test it is. Says `''` -- or the null or `false` PHP
    /// measures as nothing -- and, failed, that a string holds something: `'0'` is one character
    /// long, which is the whole difference from [`Self::Empty`].
    Length,
}

impl NullTestKind {
    /// Whether the test named null rather than falsehood.
    pub fn strict(self) -> bool {
        matches!(self, Self::Identical | Self::Isset)
    }

    /// Whether PHP reads the value without complaining about a null part way along it.
    ///
    /// True for the two constructs, which exist to be asked about something that may not be there.
    /// False for the operators: `$a->b === null` still warns when `$a` is null, because the operator
    /// is handed a value that had to be read first.
    pub fn reads_through_null(self) -> bool {
        matches!(self, Self::Isset | Self::Empty)
    }
}

/// One expression.
///
/// Same rule as statements: shapes that carry a name, a receiver, or a type, and
/// [`ExprData::Unmodelled`] for the rest.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ExprData {
    /// `$name`, without the `$`. `$this` is one of these, spelled `this`: PHP scopes it like a
    /// variable, and a separate node would mean two paths for every consumer to handle.
    Variable {
        name: Name,
        range: TextRange,
    },
    /// `[]`, `[1, 2]`, `array(...)`.
    ///
    /// Its own shape rather than a literal, because it has contents: a member access written inside
    /// one has to stay reachable, and `Unmodelled` kept it reachable while losing the one thing an
    /// array literal plainly states, which is that it is an array.
    ArrayLiteral {
        /// One id per element written, in source order. A bare element is its own expression; the
        /// two that say more than that are wrapped -- see [`Self::ArrayElement`].
        elements: IdSlice<ExprId>,
        range: TextRange,
    },
    /// One element of an array literal that says more than the value it holds: `key => value`, or
    /// `...$rest`.
    ///
    /// Only those two, for the reason [`Self::Argument`] wraps only two of its own: a bare element
    /// *is* its expression, and wrapping every one of them would put a node per element into every
    /// list literal in the project for no fact gained.
    ///
    /// Modelled because without it the three shapes could not be told apart. The elements used to
    /// arrive as one flat list from the generic unmodelled walk, so `['a' => new Pen()]` and
    /// `[new Pen(), 'a']` were the same two ids in a different order -- and nothing could say which
    /// half of a pair was the value, which is the half an element type comes from.
    ///
    /// Costs nothing for the element PHP is mostly made of. A literal key holds its text here
    /// instead of in a node of its own, so `'a' => $v` is a wrapper and a value where it used to be
    /// a key and a value -- see [`ArrayElementKind::LiteralKey`].
    ArrayElement {
        kind: ArrayElementKind,
        /// The right of a `=>`, or what a `...` spreads.
        value: ExprId,
        range: TextRange,
    },
    /// `$rows['key']`, `$rows[$i]`, and `$rows[]` in a write position.
    ///
    /// Modelled because it is a *binding*: PHP creates `$rows` when `$rows['key'] = 1` is written,
    /// so leaving it unmodelled means the commonest way to build an array in PHP declares nothing,
    /// and goto on the variable it created finds no declaration.
    Index {
        base: ExprId,
        /// `None` for the append form, `$rows[] = 1`, which names no key.
        index: Option<ExprId>,
        /// The key as a literal writes it: `app` in `$row['app']`, `0` in `$row[0]`.
        ///
        /// Kept because a shape says what *this* key holds, and the index alone is an expression id
        /// with a kind and a range -- the one thing it does not carry is the text. `None` when the
        /// key is anything else, which is a slot this layer cannot name.
        key: Option<Name>,
        range: TextRange,
    },
    /// `&$item`, in `foreach ($rows as &$item)` or `$alias = &$target`.
    ///
    /// Modelled because it wraps the thing being bound rather than replacing it. Left unmodelled,
    /// the loop variable of the commonest way to mutate a collection in place binds nothing.
    Reference {
        value: ExprId,
        range: TextRange,
    },
    /// `(int) $value`, `(string) $value`.
    ///
    /// Modelled because it is the plainest statement of type a PHP file can make: nothing has to be
    /// inferred, the source says what the value is. Left unmodelled, `$id = (int) $node->id()` gives
    /// `$id` no type at all, which is exactly the case a reader most expects to work.
    Cast {
        to: BuiltinTy,
        value: ExprId,
        range: TextRange,
    },
    /// An argument that is more than the expression it passes: `f(id: 1)`, `f(...$rest)`.
    ///
    /// Only those two. A plain positional argument *is* its expression, and wrapping every one of
    /// them would make every walk over an argument list unwrap for nothing.
    ///
    /// Modelled because a feature has to be able to tell them apart: a parameter-name hint over
    /// `id: 1` would be repeating what is written, and a spread stands for however many arguments
    /// are in the array, so it lines up with no single parameter.
    Argument {
        /// Written before the colon, without it.
        name: Option<Name>,
        /// `...$rest`.
        spread: bool,
        value: ExprId,
        range: TextRange,
    },
    /// A variable whose name is itself computed: `$$name`, `${$expr}`. Kept apart from
    /// [`Self::Variable`] because it has no name to look up, and treating it as one would answer
    /// about whatever variable happened to share the spelling.
    IndirectVariable {
        range: TextRange,
    },
    /// `$target = value`, and `$target ??= value`. The other compound assignments are
    /// [`Self::Unmodelled`], because their result type depends on the operator rather than on the
    /// right-hand side.
    ///
    /// `??=` is here because it binds the target exactly as `=` does -- everything that asks where
    /// a name comes from, whether it is read before it is written, or whether a branch wrote it,
    /// wants the same answer for both. Only what the target ends up *holding* differs, and that is
    /// the one reader [`AssignOp`] exists for.
    Assign {
        operator: AssignOp,
        target: ExprId,
        value: ExprId,
        /// From a `/** @var T $target */` written above the statement.
        ///
        /// The idiom for typing a local whose value nothing else describes -- a service container
        /// lookup, a decoded payload -- and on Drupal it is how most locals get a type at all.
        doc_type_ref: Option<crate::ids::TypeRefId>,
        range: TextRange,
    },
    /// `$n += 1`, `$s .= 'x'` -- an assignment whose result the *operator* decides.
    ///
    /// Apart from [`Self::Assign`] on purpose, and not folded into it: what the target ends up
    /// holding is the operator's answer and not the right-hand side's, so a reader that treated the
    /// two alike would type `$n .= 'x'` by whatever `'x'` is. It used to be an [`Self::Unmodelled`]
    /// for that reason, which was right about the type and wrong about the **write**: the target is
    /// a place the value is put, and every reader that asks what writes a name saw a read there.
    /// `Inline $offset` rewrote `$offset += $batch_size;` to `0 += $batch_size;` on a real project,
    /// because one write is what makes a variable safe to inline and this one was invisible.
    CompoundAssign {
        /// What is written to, which is also read: `$n += 1` takes `$n`'s value and puts one back.
        target: ExprId,
        /// `None` for `$i++`, `++$i`, `$i--` and `--$i`, which name no right-hand side: the
        /// operator is the whole of it. They are here rather than unmodelled for the reason the
        /// rest of this shape is -- the target is a place the value is put, and a reader that saw
        /// only a read there offered to inline `$i` and wrote `use (&0)`.
        value: Option<ExprId>,
        /// What the operator makes of the two, when it decides that without reading them: `.=` a
        /// string, the bitwise ones and `%=` an int. `None` for the arithmetic ones, whose result
        /// depends on what the operands are -- see [`Self::CompoundAssign::arithmetic`] -- and for
        /// the increments, which name no right-hand side.
        produces: Option<BuiltinTy>,
        /// `+=`, `-=`, `*=`, `/=`, `**=`: the same operator `$a + $b` lowers to, over the target's own
        /// value and the right-hand side.
        arithmetic: Option<ArithmeticOp>,
        range: TextRange,
    },
    New {
        class: ClassTarget,
        args: IdSlice<ExprId>,
        range: TextRange,
    },
    MethodCall {
        receiver: ExprId,
        /// `None` when the method name is computed: `$obj->$name()`.
        name: Option<Name>,
        /// The expression that computes the name, when it is computed.
        ///
        /// Kept because it holds a variable, and a variable nothing can reach is one a rename would
        /// not change: `$this->{$func}()` after `$func = 'logError'` is written in real code, and
        /// leaving it out makes the rename produce a file that no longer calls anything. The same
        /// reason every unmodelled shape keeps its subtree.
        selector: Option<ExprId>,
        name_range: TextRange,
        args: IdSlice<ExprId>,
        /// The value of the first argument, when it is a plain string literal.
        ///
        /// Here because it is the only literal in a call that decides a *type*: a container reached
        /// as `service('application.manager')` and one reached as `service('messenger')` hold
        /// different classes, and the id is the whole of what says so. The same for
        /// `getStorage('node')`.
        ///
        /// Only the first, and only a string. Interning every string literal instead would cost
        /// twenty times as much for nothing -- measured on two real files, 1093 literals against 62
        /// first arguments -- and no other position has ever decided a type.
        literal_arguments: Box<[LiteralArgument]>,
        nullsafe: bool,
        range: TextRange,
    },
    PropertyAccess {
        receiver: ExprId,
        name: Option<Name>,
        /// See [`Self::MethodCall::selector`].
        selector: Option<ExprId>,
        name_range: TextRange,
        nullsafe: bool,
        range: TextRange,
    },
    StaticCall {
        class: ClassTarget,
        name: Option<Name>,
        /// See [`Self::MethodCall::selector`].
        selector: Option<ExprId>,
        name_range: TextRange,
        args: IdSlice<ExprId>,
        /// See [`Self::MethodCall::literal_arguments`].
        literal_arguments: Box<[LiteralArgument]>,
        range: TextRange,
    },
    StaticPropertyAccess {
        class: ClassTarget,
        name: Option<Name>,
        /// See [`Self::MethodCall::selector`].
        selector: Option<ExprId>,
        name_range: TextRange,
        range: TextRange,
    },
    ClassConstAccess {
        class: ClassTarget,
        /// `Foo::class` included: it is spelled like a constant and its type depends on the class.
        name: Option<Name>,
        /// See [`Self::MethodCall::selector`].
        selector: Option<ExprId>,
        name_range: TextRange,
        range: TextRange,
    },
    /// The method a first-class callable names: `$repo->find(...)`, `Repo::find(...)`.
    ///
    /// Its own shape rather than a call, because it does not call: PHP 8.1's `(...)` builds a
    /// `Closure` and never enters the callee. Reusing [`Self::MethodCall`] would give
    /// `$fn = Repo::find(...)` the return type of `find` -- a confidently wrong type where there is
    /// none -- and would make every argument-count and argument-type check fire on a call written
    /// with no arguments.
    ///
    /// So it carries no argument list and produces no value path, and the projections split on
    /// exactly that: it is a member site, and it is not a call site.
    ///
    /// Only the two member forms; `strlen(...)` is [`Self::CallableFunction`], which has no
    /// receiver to hang a member site on.
    CallableMember {
        receiver: CallableReceiver,
        /// `None` when the method name is computed: `$obj->$name(...)`.
        name: Option<Name>,
        /// See [`Self::MethodCall::selector`].
        selector: Option<ExprId>,
        name_range: TextRange,
        range: TextRange,
        /// Whether the syntax settles that this names a method, or only suggests it.
        ///
        /// `(...)` settles it. A two-element array does not: `[Foo::class, 'bar']` is a callable
        /// *and* a perfectly ordinary list of two class names, and Symfony writes the second --
        /// `setAllowedTypes('active_at', [\DateTimeInterface::class, 'null'])`. Read as a
        /// callable it reports an undefined method on a class that never claimed one.
        ///
        /// So a presumed site is kept for the readers that are improved by a false one and cost
        /// nothing for it -- references and rename match what a name *resolves to*, and a list of
        /// class names resolves to no method at all -- and dropped before the readers a false one
        /// would make lie. `[$this, 'run']` is not presumed: a pair of the object you are inside
        /// and a string has no second reading.
        presumed: bool,
    },
    /// The function a first-class callable names: `strlen(...)`, `App\make(...)`.
    ///
    /// Apart from [`Self::CallableMember`] because a function has no receiver and so no member
    /// site: the name is already a reference in the function table, which is where goto, hover and
    /// find-references look for it. What this adds is the *value* -- a `Closure` handing back
    /// whatever the function returns -- which is a question about the expression rather than about
    /// the name, and which nothing could ask while the shape was unmodelled.
    ///
    /// Not [`Self::FunctionCall`] with an empty argument list, for the reason
    /// [`Self::CallableMember`] is not [`Self::MethodCall`]: nothing is invoked here, so an
    /// argument-count check firing on it would be reporting a call that was never written.
    CallableFunction {
        name: NameRefId,
        range: TextRange,
    },
    /// A call whose callee is a written name: `strlen($s)`, `App\make()`.
    FunctionCall {
        name: NameRefId,
        args: IdSlice<ExprId>,
        /// See [`Self::MethodCall::literal_arguments`]. A free function needs them for the same
        /// reason a method does: `response(null)` and `response('body')` return different classes,
        /// and the only thing that says which is what the call wrote.
        literal_arguments: Box<[LiteralArgument]>,
        range: TextRange,
    },
    /// A call through a value: `$callable()`, `($f)()`, `foo()()`.
    IndirectCall {
        callee: ExprId,
        args: IdSlice<ExprId>,
        range: TextRange,
    },
    /// A bare name used as a value: a global constant, or `true`/`false`/`null` written as one.
    ConstantAccess {
        name: NameRefId,
        range: TextRange,
    },
    /// `$value instanceof Class`. Modelled because it is what narrowing reads.
    Instanceof {
        value: ExprId,
        class: ClassTarget,
        range: TextRange,
    },
    /// `!value`.
    ///
    /// Modelled so a guard can be read backwards: `if (!($x instanceof T)) { return; }` proves what
    /// its negation does, for the code after it.
    Not {
        value: ExprId,
        range: TextRange,
    },
    /// A test for null: `$x === null`, `$x == null`, `isset($x)`, `empty($x)`.
    ///
    /// Always in the affirmative -- "value is null" -- because the negated spellings are the same
    /// claim wrapped in a [`Self::Not`], and reading a guard backwards is machinery that already
    /// exists. `$x !== null` is therefore `Not(NullTest)`, and `isset($x)` is the same tree.
    ///
    /// Modelled because it is how PHP code is actually written. Truthiness (`if ($x)`) narrows for
    /// free out of `Not`, but `if ($this->application === NULL) { return; }` is the guard a form
    /// class puts at the top of every submit handler, and without it every use below reads as
    /// possibly null.
    NullTest {
        value: ExprId,
        kind: NullTestKind,
        range: TextRange,
    },
    /// A strict test for `false`: `$x === false`, and `$x !== false` as a [`Self::Not`] around it.
    ///
    /// Affirmative and negation-wrapped for the same reason [`Self::NullTest`] is, and modelled for
    /// the same reason too: `strpos($h, $n) !== false` is how PHP reports "not found", and so do
    /// `strtotime`, `array_search`, `file_get_contents` and most of the standard library. Without
    /// it every value guarded that way still reads as `T|false` below the guard, and a check on
    /// what is done with it reports the branch the author just ruled out.
    ///
    /// **Strict only.** `$x == false` is a truthiness test -- `0`, `''` and `[]` all satisfy it --
    /// so reading it as "the value is `false`" would narrow away alternatives that are still live.
    /// Loose spellings stay an ordinary operator, and truthiness already narrows through `Not`.
    /// `true` is the same node and not a second one. It is written far less often -- the standard
    /// library has no `true` sentinel -- but a union carrying one is ordinary where a map means
    /// "all of them": `['order' => ['a', 'b'], 'user' => TRUE]` is a real Drupal idiom, and
    /// `TRUE === $map[$k] || in_array($b, $map[$k])` is how it is read. Until the node carried
    /// which boolean was tested, the left half narrowed nothing and the right half was reported
    /// for passing a `true` to a parameter that takes an array -- on code where the `||` has
    /// already ruled it out.
    BoolTest {
        value: ExprId,
        /// Which literal the value was compared against.
        expected: bool,
        range: TextRange,
    },
    /// `$a ?? $b`.
    ///
    /// Modelled for what it does to the left operand rather than for the value it produces: PHP
    /// reads a property chain there without complaining about a null in the middle, so
    /// `$definition->label ?? $default` is the author handling the null and not missing it.
    Coalesce {
        left: ExprId,
        right: ExprId,
        range: TextRange,
    },
    /// `include`, `include_once`, `require`, `require_once`.
    ///
    /// Modelled for what it does to the *scope*: the included file sees every variable of the scope
    /// that included it, and can add its own. So anything asking whether a local is used, or whether
    /// one is defined, has to stop at an include -- the answer is in another file, and which file is
    /// often not knowable.
    Include {
        /// What is being included, still lowered: it is usually a concatenation and can hold a call.
        path: ExprId,
        range: TextRange,
    },
    /// `throw expr`.
    ///
    /// An expression in PHP 8, and modelled for its effect on flow: a statement that throws is a
    /// statement the code after does not follow.
    Throw {
        value: ExprId,
        range: TextRange,
    },
    /// `&&` and `||`, and their `and`/`or` spellings.
    ///
    /// Modelled for one reason: narrowing has to tell them apart. `$x instanceof A && $y` means `$x`
    /// really is an `A` inside the branch; `$x instanceof A || $y` does not, and a walker that only
    /// saw "there is an instanceof in the condition somewhere" would narrow on both.
    Logical {
        operator: LogicalOp,
        left: ExprId,
        right: ExprId,
        range: TextRange,
    },
    /// `$c ? $a : $b`, and `$c ?: $b` where the then-branch is the condition itself.
    ///
    /// Modelled for the same reason `&&` is: a narrowing written in the condition holds in the arm
    /// that the condition selected, and on a report generator that is where half the calls are --
    /// `$isBulk ? round($app->getBulkFee(), 2) : ''`.
    Conditional {
        condition: ExprId,
        /// `None` for `?:`, where PHP reuses the condition's own value.
        then_branch: Option<ExprId>,
        else_branch: ExprId,
        range: TextRange,
    },
    Closure {
        kind: ClosureKind,
        params: IdSlice<ParamId>,
        /// `use ($x)` captures, as variable expressions so each keeps its own range.
        captures: IdSlice<ExprId>,
        /// What it declares it gives back: `fn (Row $r): Item => ...`.
        ///
        /// Kept because a function taking a callback often says nothing about its own result except
        /// that it is made of whatever the callback returns -- `array_map` is the whole of PHP's
        /// answer to `map`, and the closure written at the call is the only place that type appears.
        return_type: Option<crate::ids::TypeRefId>,
        body: BodyId,
        range: TextRange,
    },
    /// `match ($subject) { conditions => value, ... }`.
    ///
    /// Compares with `===`, and an unhandled value throws `UnhandledMatchError` at runtime rather
    /// than falling through -- so unlike a `switch`, a `default` arm (or one whose conditions
    /// cover every case) makes every arm here exhaustive on its own, with no separate flag needed:
    /// see [`MatchArmData::conditions`].
    Match {
        subject: ExprId,
        arms: IdSlice<MatchArmId>,
        range: TextRange,
    },
    /// `clone $row` -- another object of whatever was cloned.
    ///
    /// Its own shape rather than an unmodelled node holding the operand, because what it produces
    /// is a *value* with a type: `$copy = clone $this;` is how PHP writes a with-er, and reading it
    /// as unmodelled left the copy untyped, its members unresolvable and every rule about them --
    /// readonly first among them -- silent.
    Cloned {
        value: ExprId,
        range: TextRange,
    },
    Literal {
        kind: LiteralKind,
        range: TextRange,
    },
    /// An operator whose result the operator itself decides: `.` builds a string, a comparison
    /// answers a bool, a bitwise operator or `%` produces an int.
    ///
    /// The operands are still lowered and listed, exactly as [`Self::Unmodelled`] lists them: an
    /// operator hides calls inside it as readily as any other shape.
    Operator {
        produces: BuiltinTy,
        inner: IdSlice<ExprId>,
        range: TextRange,
    },
    /// `-$x` and `+$x` -- the operand as a number.
    ///
    /// PHP computes both as `$x * -1` and `$x * 1`, so the operand decides the result exactly as it
    /// does for [`Self::Arithmetic`]: an `int` stays one, a `float` stays one, a numeric string is
    /// either. `-1` is this too -- the lexer has no negative literal -- and left unmodelled it made
    /// `$index = -1;` a local of no type at all.
    Signed {
        value: ExprId,
        range: TextRange,
    },
    /// `<`, `<=`, `>`, `>=`, `==`, `===`, `!=`, `!==` -- the operators that answer a bool about two
    /// operands.
    ///
    /// Apart from [`Self::Operator`], which keeps the shapes whose result the operator alone decides
    /// and nothing reads further, because *which* comparison was written is a fact a guard needs:
    /// `$count > 0` rules null out of its true branch and `$count <= 0` out of its false one, and
    /// folded into one bool-producing node the two are indistinguishable. That fold is what left
    /// `if ($count > 0)` narrowing nothing.
    ///
    /// `left` and `right` rather than a list, for the reason [`Self::Arithmetic`] has them: a binary
    /// operator's two operands *are* its whole subtree, so naming them costs no reachability.
    Comparison {
        operator: CompareOp,
        left: ExprId,
        right: ExprId,
        range: TextRange,
    },
    /// `+`, `-`, `*`, `**`, `/` -- the operators whose result the operands decide.
    ///
    /// The operator travels with them because they do not all decide it the same way: only `+`
    /// accepts two arrays, and only `/` can produce a float from two ints. See [`ArithmeticOp`].
    Arithmetic {
        operator: ArithmeticOp,
        left: ExprId,
        right: ExprId,
        range: TextRange,
    },
    /// `exit` or `die` -- the two spellings of a statement nothing runs after.
    ///
    /// One shape for both, because what a reader needs is that control leaves here and not which
    /// keyword said so. Its argument is still lowered: `exit($this->code())` holds a call.
    Terminate {
        inner: IdSlice<ExprId>,
        range: TextRange,
    },
    /// An expression shape that is not modelled — an array access, an operator, an interpolated
    /// string.
    ///
    /// Its nested expressions are still lowered and listed here, for the same reason as
    /// [`StmtData::Unmodelled`]: `$this->attachmentTypes()[$id]` is an array access, and swallowing
    /// its subtree would mean no goto on the call inside it.
    Unmodelled {
        inner: IdSlice<ExprId>,
        range: TextRange,
    },
}

/// Which shape an array element that is more than a bare value was written in.
///
/// An enum rather than a key beside a spread flag, because PHP cannot write both: `'a' => ...$rest`
/// is a syntax error, and a pair of fields could be set to a combination nobody can spell.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ArrayElementKind {
    /// `'app' => $value`, `0 => $value` -- the two ways PHP really keys an array.
    ///
    /// The key's text and where it was written, and no expression node for it. What says which slot
    /// a value went into is the *text* -- the same slot [`ExprData::Index::key`] keeps, for the same
    /// reason -- and a literal has nothing inside it to reach, so a node would be eighty bytes
    /// holding a kind nobody reads and a range this already has.
    ///
    /// Which is what makes the wrapper free: this is the element PHP is mostly made of, a framework's
    /// render and config arrays being nothing else, and the node it takes is the node the key gives
    /// back. Measured on a 41 559-file Drupal checkout, keeping both would have been 6.8 M extra
    /// expressions, a third of the body IR of the whole corpus.
    LiteralKey { name: Name, range: TextRange },
    /// `$slug => $value`, `self::KEY => $value` -- a key that is anything else, including a string
    /// literal too broken to decode.
    ///
    /// Keeps its node, because it holds expressions and they have to stay reachable:
    /// `[$this->slug() => $row]` names a method on the left of the arrow, and it names no slot this
    /// layer can identify.
    ComputedKey { key: ExprId },
    /// `...$rest`, which contributes the elements of another array rather than itself.
    ///
    /// Apart from a keyed element because what it holds is one level further in: reading the value
    /// as an element would say `$items = [...$pens]` holds an array of arrays.
    Spread,
}

/// An argument written as something the type of the call may depend on.
///
/// A string literal or `X::class`, with where it was written. The position matters because which
/// argument decides is the callee's business: a catalog contract means the first, and a
/// `class-string<T>` parameter can be anywhere in the list.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct LiteralArgument {
    pub position: u16,
    /// The parameter this was written under, for `f(id: Foo::class)`.
    ///
    /// Which parameter a name fills takes the callee's signature to say, and lowering has none --
    /// so the name travels and the reader matches it. `None` for a positional argument, whose
    /// [`Self::position`] is the answer already.
    pub name: Option<Name>,
    /// The string as written, the class a `::class` named, or the keyword itself.
    pub value: Name,
    pub kind: ArgumentLiteral,
}

impl LiteralArgument {
    /// Whether [`Self::value`] is the thing the call wrote rather than a shape or a keyword.
    ///
    /// The question every reader that matches a *value* is asking: a catalog key, a class name, a
    /// literal a conditional return type names. A keyword is out because its identity and not its
    /// text is the point -- `f('null')` is not `f(null)` -- and an interpolated skeleton is out
    /// because it is a description of a string and not one.
    pub fn wrote_its_value(&self) -> bool {
        !matches!(
            self.kind,
            ArgumentLiteral::Keyword | ArgumentLiteral::Interpolated
        )
    }

    /// Whether this argument is the same value at every occurrence of the call.
    ///
    /// A different question from [`Self::wrote_its_value`] and it excludes a different kind: a
    /// keyword is as fixed as a quoted string, and an interpolated skeleton is not fixed at all. Its
    /// holes are read again at each call, so two occurrences of `f("a $x")` are only the same value
    /// while nothing writes `$x` between them -- which is exactly what a reader treating them as one
    /// slot would be assuming.
    pub fn is_fixed_where_it_is_written(&self) -> bool {
        !matches!(self.kind, ArgumentLiteral::Interpolated)
    }
}

/// Which sort of literal an argument was written as.
///
/// Two, because only one question needs them apart: `f('null')` passes a string and `f(null)` passes
/// nothing, and a conditional return type keyed on `is null` must not read the first as the second.
/// Everything else asked of an argument -- a catalog key, the class a factory was given -- wants a
/// string either way.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum ArgumentLiteral {
    /// A string as written, or the class a `::class` named. Both are strings at runtime, and the two
    /// uses cannot collide: a catalog has no key spelled like a class.
    Text,
    /// `null`, `true`, or `false`, written as the keyword. [`LiteralArgument::value`] is the keyword.
    Keyword,
    /// An integer as written. [`LiteralArgument::value`] is the digits, because this layer has no
    /// number to hold and the reader is comparing against one written in a docblock.
    Number,
    /// A global constant, as the name the author wrote: `PATHINFO_EXTENSION`.
    ///
    /// The name and not its value, because the value is in whichever file declares it and lowering
    /// reads one file. What a conditional return type tests is the value, so the reader is the one
    /// that resolves it -- the same division a `Foo::class` argument already follows.
    Constant,
    /// `"-$i months"` -- a double-quoted string with expressions in it, kept as a skeleton.
    ///
    /// [`LiteralArgument::value`] is the literal text with one [`INTERPOLATION_HOLE`] written where
    /// each hole was, in source order. The holes themselves are not here: they stay where every
    /// other nested expression goes, inside the argument's own [`ExprData::Unmodelled`], because
    /// what a hole *holds* takes types to say and this layer has none.
    ///
    /// The one reader is a recogniser over the shape of the string -- `strtotime("-$i months")`
    /// cannot fail for any integer `$i`, and nothing but the text around the hole says so. Which is
    /// why a skeleton is not a value: it names no catalog key, no class and no literal a docblock
    /// compares against, and [`LiteralArgument::wrote_its_value`] is how a reader that wants one
    /// says so.
    Interpolated,
}

/// The character an [`ArgumentLiteral::Interpolated`] skeleton stands in a hole's place.
///
/// U+0001, chosen for being a character nobody writes: a skeleton travels in the same slot as a
/// catalog key and a class name, so the safe thing is for it to be unable to equal one even before
/// the readers ask for a kind by name.
pub const INTERPOLATION_HOLE: char = '\u{1}';

impl ExprData {
    pub fn range(&self) -> TextRange {
        match self {
            Self::Variable { range, .. }
            | Self::ArrayLiteral { range, .. }
            | Self::ArrayElement { range, .. }
            | Self::Index { range, .. }
            | Self::Reference { range, .. }
            | Self::Cast { range, .. }
            | Self::Argument { range, .. }
            | Self::IndirectVariable { range }
            | Self::Assign { range, .. }
            | Self::Cloned { range, .. }
            | Self::New { range, .. }
            | Self::MethodCall { range, .. }
            | Self::PropertyAccess { range, .. }
            | Self::StaticCall { range, .. }
            | Self::StaticPropertyAccess { range, .. }
            | Self::ClassConstAccess { range, .. }
            | Self::CallableMember { range, .. }
            | Self::CallableFunction { range, .. }
            | Self::FunctionCall { range, .. }
            | Self::IndirectCall { range, .. }
            | Self::ConstantAccess { range, .. }
            | Self::Instanceof { range, .. }
            | Self::NullTest { range, .. }
            | Self::BoolTest { range, .. }
            | Self::Coalesce { range, .. }
            | Self::Include { range, .. }
            | Self::Not { range, .. }
            | Self::Throw { range, .. }
            | Self::Logical { range, .. }
            | Self::Conditional { range, .. }
            | Self::CompoundAssign { range, .. }
            | Self::Closure { range, .. }
            | Self::Match { range, .. }
            | Self::Literal { range, .. }
            | Self::Operator { range, .. }
            | Self::Comparison { range, .. }
            | Self::Arithmetic { range, .. }
            | Self::Signed { range, .. }
            | Self::Terminate { range, .. }
            | Self::Unmodelled { range, .. } => *range,
        }
    }
}

/// One arm of a `match` expression: `conditions => value`, or `default => value`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct MatchArmData {
    /// The values compared with `===`. Empty for the `default` arm -- `match` has no fallthrough
    /// to fold the way a `switch` does, so this is the only signal an arm needs.
    pub conditions: IdSlice<ExprId>,
    pub body: ExprId,
    pub range: TextRange,
}

/// How the class in a `new`, `::`, or `instanceof` was written.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum ClassTarget {
    Named(NameRefId),
    /// `self`, `static`, `parent` — resolved against whatever class the expression is written in.
    ///
    /// Carries the keyword's own range, because it is a place a cursor can be: goto and hover on
    /// the `self` in `new self()` or `self::make()` have nothing else to find it by. Only here and
    /// not on the type-reference version of the same idea: that one is written into the disk cache,
    /// so giving it a range is a format change rather than a field.
    Relative {
        relative: RelativeTy,
        range: TextRange,
    },
    /// `$class::make()`, `new $class`. The class is a value, so its name is not written anywhere.
    Expr(ExprId),
}

/// What a [`ExprData::CallableMember`] named its method on.
///
/// The two halves cannot be folded into [`ClassTarget::Expr`]: `Repo::find(...)` names a class and
/// `$repo->find(...)` names an instance, and reading the second as the first would resolve `$repo`
/// the way `$class::make()` resolves its receiver -- as a class *name* the variable holds.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum CallableReceiver {
    /// `$repo->find(...)` -- a value, whose class decides which method.
    Value(ExprId),
    /// `Repo::find(...)`, `self::find(...)` -- a class named outright.
    Class(ClassTarget),
}

/// Which outside scope a [`StmtData::Bind`] reaches into.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BindKind {
    /// A slot that keeps its value between calls.
    Static,
    /// The global scope.
    Global,
}

impl BindKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Static => "static",
            Self::Global => "global",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LeaveKind {
    Break,
    Continue,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LogicalOp {
    And,
    Or,
}

/// Which assignment operator was written.
///
/// Two, because two of PHP's assignments leave the target holding something a reader can name from
/// the right-hand side alone. The rest -- `.=`, `+=`, `??`-less compounds of every kind -- have a
/// result the operator decides and are not lowered as assignments at all.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum AssignOp {
    /// `=` -- the target holds what the right-hand side holds.
    Plain,
    /// `??=` -- the target keeps what it already held, unless that was null, in which case it holds
    /// the right-hand side. So the value is a union of the two, with the null the operator was
    /// written to rule out taken off the first.
    Coalesce,
}

impl AssignOp {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Plain => "=",
            Self::Coalesce => "??=",
        }
    }
}

/// Which [`ExprData::Comparison`] operator was written.
///
/// The eight PHP has, kept whole rather than reduced to the two a guard can act on: this says what
/// was written, and knowing which of them mean something is a reader's job. `<>` is not among them
/// because it is `!=` spelled differently -- two spellings of one operator, folded where the
/// difference is still presentational.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum CompareOp {
    Equal,
    Identical,
    NotEqual,
    NotIdentical,
    Less,
    LessOrEqual,
    Greater,
    GreaterOrEqual,
}

impl CompareOp {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Equal => "==",
            Self::Identical => "===",
            Self::NotEqual => "!=",
            Self::NotIdentical => "!==",
            Self::Less => "<",
            Self::LessOrEqual => "<=",
            Self::Greater => ">",
            Self::GreaterOrEqual => ">=",
        }
    }

    /// Whether `null <op> x` holds for *every* right-hand operand, or for none of them.
    ///
    /// `Some(false)` for `>` and `Some(true)` for `<=`, and `None` for everything else. Measured on
    /// PHP 8.5.10 against int, float, string, bool, array and object operands: `null > x` is false
    /// in all sixteen and `null <= x` is true in all sixteen, because null compares as `false`
    /// against a scalar and as `""` against a string, and neither is ever greater than anything.
    ///
    /// `false` answers the same, because a bool compares as a bool against everything and `false`
    /// is the least of them: `false > x` is false and `false <= x` true for all eighteen operands
    /// measured, `null`, `[]` and an object among them. So the answer is about both sentinels.
    ///
    /// `<` and `>=` split on whether the operand is falsy, and the two conversions disagree about
    /// which those are -- `'0'` is falsy where `"" < "0"` is true -- so they answer `None` rather
    /// than a rule that is right about numbers and wrong about strings. A reader wanting them needs
    /// the operand's value, which a literal in this IR does not carry.
    pub fn null_holds_whatever_it_faces(self) -> Option<bool> {
        match self {
            Self::Greater => Some(false),
            Self::LessOrEqual => Some(true),
            Self::Equal
            | Self::Identical
            | Self::NotEqual
            | Self::NotIdentical
            | Self::Less
            | Self::GreaterOrEqual => None,
        }
    }

    /// The same comparison with its operands the other way round, for `0 < $count`.
    pub fn mirrored(self) -> Self {
        match self {
            Self::Less => Self::Greater,
            Self::LessOrEqual => Self::GreaterOrEqual,
            Self::Greater => Self::Less,
            Self::GreaterOrEqual => Self::LessOrEqual,
            equality => equality,
        }
    }
}

/// Which [`ExprData::Arithmetic`] operator was written.
///
/// The five whose result is decided by what they were given rather than by the operator, which is
/// why they share one shape. They still differ in two places, and both differences are the whole
/// reason this enum exists: `+` is the only one that means anything for two arrays, and `/` is the
/// only one that can hand back a float where both operands were ints.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum ArithmeticOp {
    Add,
    Subtract,
    Multiply,
    Power,
    Divide,
}

impl ArithmeticOp {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Add => "+",
            Self::Subtract => "-",
            Self::Multiply => "*",
            Self::Power => "**",
            Self::Divide => "/",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ClosureKind {
    /// `function () use ($x) { ... }`
    Closure,
    /// `static function () { ... }`, which does *not* capture `$this`: PHP raises
    /// `Using $this when not in object context` for one written inside a method.
    StaticClosure,
    /// `fn () => expr`, lowered to a body holding one `return`, so both have one shape.
    ArrowFunction,
    /// `static fn () => expr`, which likewise does not capture `$this`.
    StaticArrowFunction,
}

impl ClosureKind {
    /// Whether the body is one expression, which is what an arrow function's is.
    pub fn is_arrow(self) -> bool {
        matches!(self, Self::ArrowFunction | Self::StaticArrowFunction)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LiteralKind {
    Int,
    Float,
    String,
    True,
    False,
    Null,
}

impl LiteralKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Int => "int",
            Self::Float => "float",
            Self::String => "string",
            Self::True => "true",
            Self::False => "false",
            Self::Null => "null",
        }
    }
}
