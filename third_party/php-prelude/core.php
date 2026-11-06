<?php

#[AllowDynamicProperties]
class stdClass
{
    /**
     * Called when writing to an inaccessible or undeclared property, to let the object handle the
     * value itself. PHP never actually invokes this on a stdClass instance, because stdClass has no
     * inaccessible properties: any property name can be created and written directly, without going
     * through a magic method.
     */
    public function __set(string $name, mixed $value): void {}

    /**
     * Called when reading an inaccessible or undeclared property, to let the object supply the
     * value itself. PHP never actually invokes this on a stdClass instance, because any property
     * that exists on a stdClass object can be read directly, without going through a magic method.
     */
    public function __get(string $name): mixed {}

    /**
     * Called by isset() or empty() on an inaccessible or undeclared property. PHP never actually
     * invokes this on a stdClass instance, for the same reason __get and __set are never invoked:
     * no property on a stdClass object is inaccessible.
     */
    public function __isset(string $name): bool {}

    /**
     * Called by unset() on an inaccessible or undeclared property. PHP never actually invokes this
     * on a stdClass instance, for the same reason __get and __set are never invoked: no property on
     * a stdClass object is inaccessible.
     */
    public function __unset(string $name): void {}
}

/**
 * @template-covariant K
 * @template-covariant V
 *
 * @inheritors IteratorAggregate|Generator|Iterator|PDOStatement|DS\Collection|DOMNodeList|DatePeriod
 */
interface Traversable {}

/**
 * @template-covariant K
 * @template-covariant V
 *
 * @extends Traversable<K, V>
 */
interface IteratorAggregate extends Traversable
{
    /**
     * @return Traversable<K, V>
     *
     * @throws Exception
     */
    public function getIterator(): Traversable;
}

/**
 * @template-covariant K
 * @template-covariant V
 *
 * @extends Traversable<K, V>
 */
interface Iterator extends Traversable
{
    /**
     * @return null|V
     */
    public function current(): mixed;

    /**
     * Advances the iterator to the next element. PHP calls this once per iteration, after the loop
     * body runs and before valid() is checked again.
     */
    public function next(): void;

    /**
     * Returns the key of the current element, or null once the iterator is no longer valid. PHP
     * calls this once per iteration, to bind the loop's key variable in foreach ($it as $key =>
     * $value).
     *
     * @return null|K
     */
    public function key(): mixed;

    /**
     * Returns whether the current position is valid. PHP calls this before every iteration,
     * including the first; the loop ends as soon as it returns false.
     */
    public function valid(): bool;

    /**
     * Rewinds the iterator to its first element. PHP calls this once, before the first call to
     * valid(), at the start of a foreach loop.
     */
    public function rewind(): void;
}

/**
 * @template K
 * @template V
 */
interface ArrayAccess
{
    /**
     * Returns whether the given offset exists. PHP calls this for isset($obj[$key]) and
     * empty($obj[$key]).
     *
     * @param K $offset
     *
     * @no-named-arguments
     */
    public function offsetExists(mixed $offset): bool;

    /**
     * Returns the value stored at the given offset. PHP calls this whenever the object is read
     * using array syntax, such as $obj[$key].
     *
     * @param K $offset
     *
     * @return V
     *
     * @no-named-arguments
     */
    public function offsetGet(mixed $offset): mixed;

    /**
     * Stores a value at the given offset. PHP calls this whenever the object is written using array
     * syntax, such as $obj[$key] = $value; when the syntax omits the key, as in $obj[] = $value,
     * PHP calls this with a null offset and the implementation must choose where to store it, the
     * way a plain array would.
     *
     * @code
     * $obj[] = $value; // offsetSet(null, $value)
     * @endcode
     *
     * @param K $offset
     * @param V $value
     *
     * @no-named-arguments
     */
    public function offsetSet(mixed $offset, mixed $value): void;

    /**
     * Removes the value at the given offset. PHP calls this for unset($obj[$key]).
     *
     * @param K $offset
     *
     * @no-named-arguments
     */
    public function offsetUnset(mixed $offset): void;
}

interface Serializable
{
    /**
     * Returns a string representation of the object for use by serialize(), or null if the object
     * cannot be serialized; may throw an Exception instead. Deprecated since PHP 8.1 in favour of
     * the __serialize() and __unserialize() magic methods.
     *
     * @return string|null
     *
     * @throws Exception
     */
    public function serialize();

    /**
     * Reconstructs the object from the string previously produced by serialize(). PHP calls this
     * instead of running the constructor when unserialize() recreates the object. Deprecated since
     * PHP 8.1 in favour of the __serialize() and __unserialize() magic methods.
     *
     * @param string $data
     *
     * @return void
     */
    public function unserialize(string $data);
}

interface Throwable extends Stringable
{
    /**
     * Returns the exception message passed to the constructor.
     */
    public function getMessage(): string;

    /**
     * Returns the exception code passed to the constructor. Typed as int|string because some
     * subclasses, such as PDOException, use a string SQLSTATE code instead of an integer.
     *
     * @return int|string
     */
    public function getCode();

    /**
     * Returns the path of the file in which the throwable was constructed - the new expression -
     * which can differ from where it was thrown.
     */
    public function getFile(): string;

    /**
     * Returns the line number at which the throwable was constructed, which can differ from where
     * it was thrown.
     */
    public function getLine(): int;

    /**
     * Returns the stack trace as an array of frames, each describing a file, line, function and
     * class, among other details.
     */
    public function getTrace(): array;

    /**
     * Returns the stack trace formatted as a single human-readable string, one frame per line - the
     * same format PHP uses when printing an uncaught throwable.
     */
    public function getTraceAsString(): string;

    /**
     * Returns the throwable that caused this one, for exception chaining, or null when there is
     * none.
     */
    public function getPrevious(): ?Throwable;

    /**
     * Returns a string representation of the throwable that combines its class name, message, file,
     * line and stack trace - the same representation PHP prints for an uncaught throwable.
     *
     * @return string
     */
    public function __toString();
}

class Exception implements Throwable
{
    protected $message;

    protected $code;

    protected string $file;

    protected int $line;

    /**
     * Constructs the exception with the given message, code and, optionally, a previous throwable
     * for exception chaining.
     *
     * @pure
     */
    public function __construct(string $message = '', int $code = 0, ?Throwable $previous = null) {}

    /**
     * Returns the exception message passed to the constructor. Declared final, so no subclass can
     * override it.
     *
     * @mutation-free
     */
    final public function getMessage(): string {}

    /**
     * Returns the exception code passed to the constructor. Typed as int|string because some
     * subclasses, such as PDOException, use a string SQLSTATE code instead of an integer. Declared
     * final, so no subclass can override it.
     *
     * @return int|string
     *
     * @mutation-free
     */
    final public function getCode() {}

    /**
     * Returns the path of the file in which the exception was constructed, which can differ from
     * where it was thrown. Declared final, so no subclass can override it.
     *
     * @mutation-free
     */
    final public function getFile(): string {}

    /**
     * Returns the line number at which the exception was constructed, which can differ from where
     * it was thrown. Declared final, so no subclass can override it.
     *
     * @mutation-free
     */
    final public function getLine(): int {}

    /**
     * Returns the stack trace as an array of frames, each describing a file, line, function and
     * class, among other details. Declared final, so no subclass can override it.
     *
     * @mutation-free
     */
    final public function getTrace(): array {}

    /**
     * Returns the throwable that caused this one, for exception chaining, or null when there is
     * none. Declared final, so no subclass can override it.
     *
     * @mutation-free
     */
    final public function getPrevious(): ?Throwable {}

    /**
     * Returns the stack trace formatted as a single human-readable string, one frame per line.
     * Declared final, so no subclass can override it.
     *
     * @mutation-free
     */
    final public function getTraceAsString(): string {}

    /**
     * Returns a string representation combining the class name, message, file, line and stack
     * trace. Unlike the other accessors on this class, it is not declared final and may be
     * overridden.
     */
    public function __toString(): string {}

    /**
     * Declared private specifically to make Exception objects impossible to clone; calling clone on
     * an exception from outside the class raises an Error.
     */
    private function __clone(): void {}

    /**
     * Called automatically by PHP when the exception is restored by unserialize(), instead of
     * running the constructor; user code does not normally call it directly.
     */
    public function __wakeup(): void {}
}

class Error implements Throwable
{
    protected $message;

    protected $code;

    protected string $file;

    protected int $line;

    /**
     * Constructs the error with the given message, code and, optionally, a previous throwable for
     * exception chaining.
     *
     * @param string $message
     * @param int $code
     * @param null|Throwable $previous
     *
     * @pure
     */
    public function __construct(string $message = '', int $code = 0, ?Throwable $previous = null) {}

    /**
     * Returns the error message passed to the constructor. Declared final, so no subclass can
     * override it.
     *
     * @return string
     *
     * @mutation-free
     */
    final public function getMessage(): string {}

    /**
     * Returns the error code passed to the constructor. Always an int, unlike Exception::getCode(),
     * which some subclasses type as a string. Declared final, so no subclass can override it.
     *
     * @return int
     *
     * @mutation-free
     */
    final public function getCode() {}

    /**
     * Returns the path of the file in which the error was constructed, which can differ from where
     * it was thrown. Declared final, so no subclass can override it.
     *
     * @mutation-free
     */
    final public function getFile(): string {}

    /**
     * Returns the line number at which the error was constructed, which can differ from where it
     * was thrown. Declared final, so no subclass can override it.
     *
     * @mutation-free
     */
    final public function getLine(): int {}

    /**
     * Returns the stack trace as an array of frames, each describing a file, line, function and
     * class, among other details. Declared final, so no subclass can override it.
     *
     * @mutation-free
     */
    final public function getTrace(): array {}

    /**
     * Returns the stack trace formatted as a single human-readable string, one frame per line.
     * Declared final, so no subclass can override it.
     *
     * @mutation-free
     */
    final public function getTraceAsString(): string {}

    /**
     * Returns the throwable that caused this one, for exception chaining, or null when there is
     * none. Declared final, so no subclass can override it.
     *
     * @mutation-free
     */
    final public function getPrevious(): ?Throwable {}

    /**
     * Returns a string representation combining the class name, message, file, line and stack
     * trace. Unlike the other accessors on this class, it is not declared final and may be
     * overridden.
     */
    public function __toString(): string {}

    /**
     * Declared private specifically to make Error objects impossible to clone; calling clone on an
     * error from outside the class raises an Error.
     */
    private function __clone(): void {}

    /**
     * Called automatically by PHP when the error is restored by unserialize(), instead of running
     * the constructor; user code does not normally call it directly.
     */
    public function __wakeup(): void {}
}

#[Mago\AvailableSince(80000)]
class ValueError extends Error {}

class TypeError extends Error {}

class ParseError extends CompileError {}

class ArgumentCountError extends TypeError {}

class ArithmeticError extends Error {}

class CompileError extends Error {}

class DivisionByZeroError extends ArithmeticError {}

#[Mago\AvailableSince(80000)]
class UnhandledMatchError extends Error {}

#[Mago\AvailableSince(80400)]
class RequestParseBodyException extends Exception {}

class ErrorException extends Exception
{
    protected int $severity;

    /**
     * Constructs the error exception with the given message, code, severity, originating filename
     * and line, and an optional previous throwable for exception chaining. The severity is one of
     * the E_* error level constants, such as E_WARNING or E_NOTICE, and defaults to 1, which is
     * E_ERROR.
     *
     * @code
     * throw new ErrorException($message, 0, E_WARNING, $file, $line);
     * @endcode
     *
     * @pure
     */
    public function __construct(
        string $message = '',
        int $code = 0,
        int $severity = 1,
        ?string $filename = null,
        ?int $line = null,
        ?Throwable $previous = null,
    ) {}

    /**
     * Returns the severity level passed to the constructor, one of the E_* error level constants
     * such as E_WARNING or E_NOTICE.
     *
     * @mutation-free
     */
    final public function getSeverity(): int {}
}

final class Closure
{
    /**
     * Declared private, so a Closure cannot be instantiated directly with new; closures are created
     * only from closure syntax, Closure::fromCallable() or Closure::bind().
     */
    private function __construct() {}

    /**
     * Calls the closure with the given arguments, exactly as calling it directly with parentheses
     * does.
     *
     * @no-named-arguments
     */
    public function __invoke(...$_) {}

    /**
     * Returns a new Closure bound to the given object as $this and scoped to the given class or
     * object, leaving the original closure unchanged. Returns null if the binding fails.
     *
     * @param object|null $newThis
     * @param object|class-string|null $newScope
     *
     * @return Closure|null
     *
     * @psalm-ignore-nullable-return
     *
     * @pure
     * @no-named-arguments
     */
    public function bindTo(?object $newThis, object|string|null $newScope = 'static'): ?Closure {}

    /**
     * Static equivalent of bindTo(): returns a new Closure created from the given closure, bound to
     * the given object and scope, without modifying the original closure. Returns null if the
     * binding fails.
     *
     * @param Closure $closure
     * @param object|null $newThis
     * @param object|class-string|null $newScope
     *
     * @return Closure|null
     *
     * @psalm-ignore-nullable-return
     *
     * @pure
     */
    public static function bind(
        Closure $closure,
        ?object $newThis,
        object|string|null $newScope = 'static',
    ): ?Closure {}

    /**
     * Calls the closure with $newThis bound as $this for just that call, without needing bindTo()
     * first, and returns the closure's return value.
     *
     * @code
     * $closure->call($obj, 1, 2); // same as $closure->bindTo($obj)->__invoke(1, 2)
     * @endcode
     *
     * @param object $newThis
     */
    public function call(object $newThis, mixed ...$args): mixed {}

    /**
     * Converts any callable - a function name, a [$object, 'method'] pair, an invokable object, or
     * another Closure - into a Closure instance.
     *
     * @code
     * $fn = Closure::fromCallable('strlen');
     * @endcode
     *
     * @pure
     */
    public static function fromCallable(callable $callback): Closure {}

    /**
     * Returns the Closure object for the closure currently executing. Added in PHP 8.5.
     *
     * @pure
     */
    #[Mago\AvailableSince(80500)]
    public static function getCurrent(): Closure {}
}

interface Countable
{
    /**
     * Returns the number of elements. PHP calls this for count() and sizeof(), which is an alias of
     * count(), when applied to the object.
     */
    public function count(): int;
}

/**
 * @template T of object
 */
final class WeakReference
{
    public function __construct() {}

    /**
     * Creates a weak reference to the given object. Holding the returned WeakReference does not
     * prevent the object from being garbage collected.
     *
     * @template Input of object
     *
     * @param Input $object
     *
     * @return WeakReference<Input>
     *
     * @pure
     */
    public static function create(object $object): WeakReference {}

    /**
     * Returns the referenced object if it is still alive, or null once it has been garbage
     * collected.
     *
     * @code
     * $ref = WeakReference::create($obj);
     * $ref->get(); // $obj, or null once it is collected
     * @endcode
     *
     * @return T|null
     *
     * @pure
     */
    public function get(): ?object {}
}

/**
 * @template K of object
 * @template V
 *
 * @implements ArrayAccess<K, V>
 * @implements IteratorAggregate<K, V>
 */
#[Mago\AvailableSince(80000)]
final class WeakMap implements ArrayAccess, Countable, IteratorAggregate
{
    /**
     * Returns whether the given object exists as a key in the map. Unlike holding WeakReferences
     * manually, entries are removed automatically once the key object is garbage collected.
     *
     * @param K $object
     *
     * @pure
     * @no-named-arguments
     */
    public function offsetExists($object): bool {}

    /**
     * Returns the value stored under the given object key. Throws Error if the object is not a key
     * in the map, unlike plain array access, which would only emit a warning and return null.
     *
     * @param K $object
     *
     * @return V
     *
     * @pure
     * @no-named-arguments
     */
    public function offsetGet($object): mixed {}

    /**
     * Stores value under the given object key, overwriting any existing entry for that object. The
     * entry is removed automatically once no other reference to the object remains, which is the
     * defining behaviour of a weak map.
     *
     * @param K $object
     * @param V $value
     *
     * @no-named-arguments
     */
    public function offsetSet($object, mixed $value): void {}

    /**
     * Removes the entry for the given object key, if one exists. Does nothing if the object is not
     * a key in the map.
     *
     * @param K $object
     *
     * @no-named-arguments
     */
    public function offsetUnset($object): void {}

    /**
     * Returns an iterator over the map's entries, yielding each object key together with its
     * associated value. PHP calls this automatically when the map is used in a foreach loop, since
     * WeakMap implements IteratorAggregate rather than Iterator directly.
     *
     * @return Iterator<K, V>
     *
     * @pure
     */
    public function getIterator(): Iterator {}

    /**
     * Returns the number of object-value pairs currently stored in the map.
     *
     * @return int<0,max>
     *
     * @pure
     */
    public function count(): int {}
}

#[Mago\AvailableSince(80000)]
interface Stringable
{
    /**
     * Converts the object to its string representation. PHP calls this automatically whenever the
     * object is used in a string context, such as concatenation, echo, or an explicit (string)
     * cast, so any class declaring __toString satisfies this interface implicitly.
     */
    public function __toString(): string;
}

#[Attribute(Attribute::TARGET_CLASS)]
final class Attribute
{
    public const TARGET_CLASS = 1;
    public const TARGET_FUNCTION = 2;
    public const TARGET_METHOD = 4;
    public const TARGET_PROPERTY = 8;
    public const TARGET_CLASS_CONSTANT = 16;
    public const TARGET_PARAMETER = 32;
    #[Mago\AvailableSince(80500)]
    public const TARGET_CONSTANT = 64;
    public const TARGET_ALL = 127;
    public const IS_REPEATABLE = 128;

    public int $flags;

    /**
     * Marks a class as usable as an attribute. flags is a bitmask of the Attribute::TARGET_*
     * constants that restricts which kinds of declarations the attribute may be applied to — a
     * class, function, method, property, class constant or parameter — and defaults to allowing all
     * of them; combine with Attribute::IS_REPEATABLE to permit the same attribute more than once on
     * one declaration.
     *
     * @code
     * #[Attribute(Attribute::TARGET_METHOD | Attribute::TARGET_PROPERTY)]
     * class Cacheable {}
     * @endcode
     */
    public function __construct(int $flags = self::TARGET_ALL) {}
}

final class InternalIterator implements Iterator
{
    /**
     * Cannot be instantiated directly, since the constructor is private; instances are created
     * internally rather than via new.
     */
    private function __construct() {}

    /**
     * Returns the value at the iterator's current position. PHP calls this after rewind() or next()
     * when iterating with foreach, and it may also be called directly to inspect the current
     * element without advancing.
     */
    public function current(): mixed {}

    /**
     * Advances the iterator to its next position. PHP calls this automatically after each iteration
     * of a foreach loop.
     */
    public function next(): void {}

    /**
     * Returns the key of the element at the iterator's current position.
     */
    public function key(): mixed {}

    /**
     * Returns whether the current position holds a valid element. foreach calls this before reading
     * current() and key(), and after each call to next(), to decide whether to continue iterating.
     */
    public function valid(): bool {}

    /**
     * Resets the iterator to its first position. PHP calls this automatically once, before the
     * first iteration of a foreach loop.
     */
    public function rewind(): void {}
}

/**
 * @enum-interface
 */
#[Mago\AvailableSince(80100)]
interface UnitEnum
{
    public readonly string $name;

    /**
     * @return list<static>
     *
     * @pure
     */
    public static function cases(): array;
}

/**
 * @enum-interface
 */
#[Mago\AvailableSince(80100)]
interface BackedEnum extends UnitEnum
{
    public readonly int|string $value;

    /**
     * @throws ValueError
     * @throws TypeError
     *
     * @pure
     */
    public static function from(int|string $value): static;

    /**
     * Returns the case of this backed enum whose value equals value, or null if no case matches.
     * Unlike from(), tryFrom() never throws for an unmatched value.
     *
     * @pure
     */
    public static function tryFrom(int|string $value): ?static;
}

/**
 * @enum-interface
 */
interface __INTERNAL_DO_NOT_USE__IntBackedEnum extends BackedEnum
{
    public readonly int $value;

    /**
     * @throws ValueError
     *
     * @pure
     */
    public static function from(int $value): static;

    /**
     * Returns the case of this int-backed enum whose value equals value, or null if no case
     * matches.
     *
     * @pure
     */
    public static function tryFrom(int $value): ?static;
}

/**
 * @enum-interface
 */
interface __INTERNAL_DO_NOT_USE__StringBackedEnum extends BackedEnum
{
    public readonly string $value;

    /**
     * @throws ValueError
     *
     * @pure
     */
    public static function from(string $value): static;

    /**
     * Returns the case of this string-backed enum whose value equals value, or null if no case
     * matches.
     *
     * @pure
     */
    public static function tryFrom(string $value): ?static;
}

/**
 * @template TStart
 * @template TResume
 * @template TReturn
 * @template TSuspend
 */
#[Mago\AvailableSince(80100)]
final class Fiber
{
    public function __construct(callable $callback) {}

    /**
     * @param TStart ...$args
     *
     * @return TSuspend|null
     *
     * @throws FiberError
     * @throws Throwable
     */
    public function start(mixed ...$args): mixed {}

    /**
     * Resumes the fiber's execution from the point where it last called Fiber::suspend(), passing
     * value back as that call's return value. Throws FiberError if the fiber is not currently
     * suspended — for example if it has not been started, is already running, or has already
     * terminated — and rethrows whatever exception the fiber itself raised while running.
     *
     * @param TResume $value
     *
     * @return TSuspend|null
     *
     * @throws FiberError
     * @throws Throwable
     */
    public function resume(mixed $value = null): mixed {}

    /**
     * Resumes the fiber, but raises exception inside it at the point where it last called
     * Fiber::suspend(), instead of returning a value there. Throws FiberError under the same
     * conditions as resume(): if the fiber is not started, already running, or already terminated.
     *
     * @return TSuspend|null
     *
     * @throws FiberError
     * @throws Throwable
     */
    public function throw(Throwable $exception): mixed {}

    /**
     * Returns whether the fiber has been started at least once, via Fiber::start().
     */
    public function isStarted(): bool {}

    /**
     * Returns whether the fiber is currently suspended, having called Fiber::suspend() and not yet
     * been resumed.
     */
    public function isSuspended(): bool {}

    /**
     * Returns whether the fiber is currently executing.
     */
    public function isRunning(): bool {}

    /**
     * Returns whether the fiber has finished executing and can no longer be resumed.
     */
    public function isTerminated(): bool {}

    /**
     * Returns the value that the fiber's callback returned on completion. Throws FiberError if the
     * fiber has not yet terminated.
     *
     * @return TReturn
     *
     * @throws FiberError
     */
    public function getReturn(): mixed {}

    /**
     * Returns the Fiber instance that is currently executing, or null if called from outside any
     * fiber.
     */
    public static function getCurrent(): ?Fiber {}

    /**
     * Suspends the fiber that is currently executing, passing value out to whichever code resumes
     * it, and returns whatever value that code later passes back via Fiber::resume(). Throws
     * FiberError if called from outside a fiber, since there is then nothing to suspend.
     *
     * @param TSuspend $value
     *
     * @return TResume
     *
     * @throws FiberError
     * @throws Throwable
     *
     * @suspends-fiber
     */
    public static function suspend(mixed $value = null): mixed {}
}

#[Mago\AvailableSince(80100)]
final class FiberError extends Error
{
    /**
     * Constructs a FiberError, which takes no arguments. Instances are normally thrown by the
     * engine itself — for example when Fiber::resume() is called on a fiber that is not suspended —
     * rather than constructed directly.
     */
    public function __construct() {}
}

#[Attribute(Attribute::TARGET_METHOD)]
final class ReturnTypeWillChange
{
    /**
     * Marks a method as intentionally returning a type incompatible with the interface or parent
     * method it implements, suppressing the deprecation notice PHP would otherwise raise for the
     * mismatch.
     */
    public function __construct() {}
}

#[Attribute(Attribute::TARGET_CLASS)]
#[Mago\AvailableSince(80200)]
final class AllowDynamicProperties
{
    /**
     * Marks a class, and its subclasses, as permitting properties to be set on instances that were
     * never declared, opting out of the deprecation PHP otherwise raises for dynamic property
     * writes.
     */
    public function __construct() {}
}

#[Attribute(Attribute::TARGET_PARAMETER)]
final class SensitiveParameter
{
    /**
     * Marks a parameter as holding sensitive data, such as a password or key, so that its value is
     * redacted in stack traces and uncaught-exception backtraces rather than shown in plain text.
     *
     * @code
     * function login(#[SensitiveParameter] string $password): void {}
     * @endcode
     */
    public function __construct() {}
}

#[Mago\AvailableSince(80200)]
final class SensitiveParameterValue
{
    private readonly mixed $value;

    /**
     * Wraps a sensitive parameter's real value so it can be hidden from stack traces while
     * remaining retrievable by code that needs it.
     */
    public function __construct(mixed $value) {}

    /**
     * Returns the original, unredacted value that was passed to the sensitive parameter.
     */
    public function getValue(): mixed {}

    /**
     * Called by var_dump() to decide what to display for this object; returns nothing that would
     * expose the wrapped value, keeping it hidden from dumps as well as from backtraces.
     */
    public function __debugInfo(): array {}
}

#[Attribute(Attribute::TARGET_METHOD | Attribute::TARGET_PROPERTY)]
#[Mago\AvailableSince(80300)]
final class Override
{
    /**
     * Marks a method as intentionally overriding a method declared by a parent class or implemented
     * interface. PHP raises a compile-time error if no such parent or interface method exists,
     * catching the case where the overridden method was renamed or removed.
     */
    public function __construct() {}
}

/**
 * @strict-properties
 */
#[Attribute(
    Attribute::TARGET_METHOD
    | Attribute::TARGET_FUNCTION
    | Attribute::TARGET_CLASS_CONSTANT
    | Attribute::TARGET_CONSTANT
    | Attribute::TARGET_CLASS,
)]
#[Mago\AvailableSince(80400)]
final class Deprecated
{
    public readonly ?string $message;
    public readonly ?string $since;

    /**
     * Marks a function, method, class constant or enum case as deprecated, so that using it raises
     * E_DEPRECATED at runtime. message is appended to the generated notice, and since names the
     * version in which the deprecation took effect.
     *
     * @code
     * #[Deprecated(message: 'use save() instead', since: '8.4')]
     * function saveOld(): void {}
     * @endcode
     */
    public function __construct(?string $message = null, ?string $since = null) {}
}

#[Attribute(Attribute::TARGET_METHOD | Attribute::TARGET_FUNCTION)]
#[Mago\AvailableSince(80500)]
final class NoDiscard
{
    public readonly ?string $message;

    /**
     * Marks a function or method's return value as significant, so that calling it as a standalone
     * statement and ignoring the result raises a warning. message is included in the warning to
     * explain why discarding the result is likely a mistake.
     */
    public function __construct(?string $message = null) {}
}

#[Mago\AvailableSince(80500)]
#[Attribute(Attribute::TARGET_ALL)]
final class DelayedTargetValidation {}

/**
 * Returns the version of the Zend Engine that PHP was built against, as a string.
 *
 * @return non-empty-string
 *
 * @pure
 */
function zend_version(): string {}

/**
 * Returns the number of arguments passed to the current function call, including any beyond the
 * declared parameter list. Useful, together with func_get_arg(), for reading extra arguments in a
 * function that has no variadic parameter. Must be called from inside a function body.
 *
 * @pure
 */
function func_num_args(): int {}

/**
 * Returns the value of the argument at the given zero-based position from the current function's
 * call-time argument list, which may include arguments beyond those declared in the signature.
 * Throws an ArgumentCountError if position does not correspond to an argument that was actually
 * passed.
 *
 * @pure
 *
 * @throws ArgumentCountError
 */
function func_get_arg(int $position): mixed {}

/**
 * Returns an array of the arguments passed to the current user-defined function, in the order they
 * were passed.
 *
 * @pure
 */
function func_get_args(): array {}

/**
 * Returns the length of a string. Counts bytes rather than characters, so it is not multibyte-safe
 * for encodings like UTF-8; use mb_strlen() instead.
 *
 * @return int<0, max>
 *
 * @pure
 */
function strlen(string $string): int {}

/**
 * Performs a byte-by-byte binary-safe comparison of two strings, returning a value less than, equal
 * to, or greater than zero. Only the sign is meaningful, not the magnitude.
 *
 * @pure
 */
function strcmp(string $string1, string $string2): int {}

/**
 * Compares only the first $length bytes of two strings, using byte-wise, case-sensitive ordering —
 * unlike strcmp(), which compares the strings in full, and unlike strncasecmp(), which ignores
 * case. Returns a negative, zero, or positive integer depending on whether string1 is less than,
 * equal to, or greater than string2 over that prefix.
 *
 * @pure
 */
function strncmp(string $string1, string $string2, int $length): int {}

/**
 * Compares two strings case-insensitively. The result's sign indicates order; only the sign is
 * meaningful, not its magnitude.
 *
 * @pure
 */
function strcasecmp(string $string1, string $string2): int {}

/**
 * Compares the first length bytes of two strings without regard to case, returning a value less
 * than, equal to, or greater than zero to indicate order. The comparison is byte-wise rather than
 * multibyte aware, and is binary-safe, so it copes with strings containing null bytes.
 *
 * @pure
 */
function strncasecmp(string $string1, string $string2, int $length): int {}

/**
 * Determines whether $haystack begins with $needle, using a case-sensitive comparison.
 *
 * @pure
 */
#[Mago\AvailableSince(80000)]
function str_starts_with(string $haystack, string $needle): bool {}

/**
 * Returns true if $haystack ends with $needle. An empty $needle always returns true.
 *
 * @pure
 */
function str_ends_with(string $haystack, string $needle): bool {}

/**
 * Returns true if $needle occurs anywhere within $haystack. An empty $needle always returns true.
 *
 * @pure
 */
#[Mago\AvailableSince(80000)]
function str_contains(string $haystack, string $needle): bool {}

/**
 * Decrements a string using PHP's alphanumeric string-decrement algorithm, the mirror of
 * str_increment(), carrying across letters and digits rather than doing byte-value or numeric
 * arithmetic. Throws a ValueError when there is no valid predecessor, such as an empty string or a
 * single character already at the floor of its class, like "0", "a" or "A".
 *
 * @pure
 *
 * @throws ValueError
 */
#[Mago\AvailableSince(80300)]
function str_decrement(string $string): string {}

/**
 * Increments a string using Perl-style magic increment semantics: the last alphanumeric character
 * advances, and a character that rolls over (z, Z or 9) resets and carries the increment into the
 * character before it, growing the string by one character if the whole string rolls over. Throws a
 * ValueError if the string is empty or contains characters outside ASCII letters and digits.
 *
 * @code
 * str_increment('a'); // 'b'
 * str_increment('z'); // 'aa'
 * str_increment('a9'); // 'b0'
 * @endcode
 *
 * @pure
 * @return non-empty-string
 *
 * @throws ValueError
 */
#[Mago\AvailableSince(80300)]
function str_increment(string $string): string {}

/**
 * Sets which error types PHP reports and returns the error-reporting level that was in effect
 * before the call. Passing no argument, or null, leaves the setting unchanged and simply returns
 * the current level.
 */
function error_reporting(?int $error_level = null): int {}

/**
 * Defines a named constant $constant_name with the given $value. Returns false and has no effect if
 * the constant is already defined.
 */
function define(string $constant_name, mixed $value, bool $case_insensitive = false): bool {}

/**
 * Checks whether a constant with the given name has been declared, via define() or a top-level
 * const declaration. Only recognises simple global or namespaced constants; it cannot be used to
 * check whether a class constant exists. Returns false, without a warning, if the constant is not
 * defined.
 *
 * @pure
 */
function defined(string $constant_name): bool {}

/**
 * Returns the fully qualified name of an object's class.
 *
 * @pure
 * @return class-string
 */
function get_class(object $object = UNKNOWN): string {}

/**
 * Returns the name of the class that late static binding resolved the current call to. This is the
 * class actually instantiated or called through at runtime, which is not necessarily the class in
 * which the executing method is defined, unlike the lexical __CLASS__.
 *
 * @pure
 * @return class-string
 */
function get_called_class(): string {}

/**
 * Returns the name of the parent class of $object_or_class, given either an object instance or a
 * class name, or false if it has no parent class.
 *
 * @pure
 * @return class-string|false
 */
function get_parent_class(object|string $object_or_class = UNKNOWN): string|false {}

/**
 * Determines whether a class or object defines a given method. Returns true even for private or
 * protected methods, since it checks existence rather than accessibility.
 *
 * @param object|string $object_or_class
 *
 * @pure
 */
function method_exists(object|string $object_or_class, string $method): bool {}

/**
 * Checks whether a property is declared on a class or set on an object instance, regardless of the
 * calling scope's visibility into that property. Unlike isset() on the same property, it returns
 * true even when the property's value is null, and it never triggers __get() or __isset().
 *
 * @param object|class-string $object_or_class
 *
 * @pure
 */
function property_exists(object|string $object_or_class, string $property): bool {}

/**
 * Checks whether a trait with the given name has been declared. Returns false if the name belongs
 * to a class or interface rather than a trait, or if it has not been loaded yet and $autoload is
 * false.
 *
 * @param string $trait
 *
 * @assert-if-true =trait-string $trait
 *
 * @pure
 */
function trait_exists(string $trait, bool $autoload = true): bool {}

/**
 * Returns true if $class names a declared class. Unless $autoload is false, an autoloader is
 * invoked to try to declare it first if it is not already known.
 *
 * @param string $class
 *
 * @assert-if-true =class-string $class
 *
 * @pure
 */
function class_exists(string $class, bool $autoload = true): bool {}

/**
 * Checks whether the given interface name has been declared, triggering autoloading unless
 * $autoload is false.
 *
 * @param string $interface
 *
 * @assert-if-true =interface-string $interface
 *
 * @pure
 */
function interface_exists(string $interface, bool $autoload = true): bool {}

/**
 * Reports whether a function of the given name is defined, whether built in or user-defined.
 * Returns false for language constructs such as echo, isset and print, since they are not real
 * functions.
 *
 * @param string $function
 *
 * @assert-if-true =non-empty-string $function
 *
 * @pure
 */
function function_exists(string $function): bool {}

/**
 * Reports whether an enum with the given name has been declared, triggering autoloading for an
 * unrecognised name unless autoload is set to false.
 *
 * @param string $enum
 *
 * @assert-if-true =enum-string $enum
 *
 * @pure
 */
#[Mago\AvailableSince(80100)]
function enum_exists(string $enum, bool $autoload = true): bool {}

/**
 * Registers alias as another name for class, so that referring to alias afterwards behaves exactly
 * as referring to class itself. Returns false if class does not exist and cannot be autoloaded, or
 * if alias is already taken by another class, interface, trait, or enum; when autoload is false,
 * class must already be loaded rather than triggering an autoloader.
 *
 * @param class-string $class
 * @param non-empty-string $alias
 *
 */
function class_alias(string $class, string $alias, bool $autoload = true): bool {}

/**
 * Returns an array of the full, resolved paths of every file included or required by the script so
 * far, including the main script itself, in the order they were included.
 *
 * @return list<non-empty-string>
 *
 * @pure
 */
function get_included_files(): array {}

/**
 * Returns the canonical, absolute paths of every file included or required so far, in the order
 * each was first pulled in. It is a plain alias of get_included_files(), so files brought in via
 * include or include_once are listed too, not only require.
 *
 * @return list<non-empty-string>
 *
 * @pure
 */
function get_required_files(): array {}

/**
 * Determines whether a class or object descends from, or implements, the given class or interface.
 * Returns false when object_or_class names that same class rather than a true descendant of it,
 * and, if allow_string is false, a string class name is not accepted and an unknown class is not
 * autoloaded.
 *
 * @param class-string $class
 *
 * @return ($allow_string is false ? ($object_or_class is object ? bool : false) : bool)
 *
 * @pure
 */
function is_subclass_of(mixed $object_or_class, string $class, bool $allow_string = true): bool {}

/**
 * Returns true if the given object is an instance of $class or one of its subclasses or
 * implementations. $allow_string must be true for $object_or_class to also be accepted as a class-
 * name string rather than an actual object.
 *
 * @param class-string $class
 *
 * @return ($allow_string is false ? ($object_or_class is object ? bool : false) : bool)
 *
 * @pure
 */
function is_a(mixed $object_or_class, string $class, bool $allow_string = false): bool {}

/**
 * Returns the default property values declared by the given class, keyed by property name. Only
 * properties visible from the calling scope are included, so a call made from outside the class
 * sees only its public defaults.
 *
 * @param class-string $class
 *
 * @pure
 */
function get_class_vars(string $class): array {}

/**
 * Returns an associative array of the object's properties, keyed by property name, restricted to
 * those visible from the calling scope: called from outside the class only public properties
 * appear, while called from inside the class or a subclass any private or protected properties
 * visible there are included too.
 *
 * @param object $object
 *
 * @pure
 */
function get_object_vars(object $object): array {}

/**
 * Returns the names of a class's methods, given either an object instance or a class name. Called
 * from outside the class's own scope only public methods are included; called from within the
 * class, all methods are returned regardless of their visibility.
 *
 * @param object|class-string $object_or_class
 * @return list<non-empty-string>
 *
 * @pure
 */
function get_class_methods(object|string $object_or_class): array {}

/**
 * Raises a user-generated error, handled the same way as a native PHP error or by a custom error
 * handler set with set_error_handler(). The level defaults to E_USER_NOTICE; E_USER_WARNING and
 * E_USER_DEPRECATED report non-fatal conditions, while E_USER_ERROR is fatal and halts script
 * execution after the error is reported.
 */
function trigger_error(string $message, int $error_level = E_USER_NOTICE) {}

/**
 * Alias of trigger_error(): raises a user-generated error, warning, notice or deprecation notice,
 * handled the same way as one of PHP's own runtime diagnostics of that severity. Raising
 * E_USER_ERROR halts script execution, the same as an internal fatal error. $error_level must be
 * one of E_USER_ERROR, E_USER_WARNING, E_USER_NOTICE or E_USER_DEPRECATED; any other value is
 * itself flagged with a warning.
 */
function user_error(string $message, int $error_level = E_USER_NOTICE) {}

/**
 * Registers $callback as the function PHP calls for warnings, notices and other non-fatal errors,
 * restricted by the bitmask $error_levels to only the error types listed there. Returns whichever
 * handler was previously registered, or null if none was, in which case PHP's built-in handler was
 * in effect. Passing null as $callback restores the built-in handler.
 *
 * @return callable|null
 */
function set_error_handler(?callable $callback, int $error_levels = E_ALL) {}

/**
 * Returns the callback currently registered as the custom error handler. Returns null when no
 * custom handler is set and PHP's default internal handler is in effect.
 *
 * @return callable|null
 */
#[Mago\AvailableSince(80500)]
function get_error_handler(): ?callable {}

/**
 * Restores the error handler that was active before the current one, undoing the effect of the most
 * recent set_error_handler() call. Always returns true.
 */
function restore_error_handler(): true {}

/**
 * Registers a callback to run when an exception is thrown and never caught, replacing any handler
 * set previously. Returns the previously registered handler, or null if none was set; passing null
 * as $callback removes the current handler, so uncaught exceptions are then handled by PHP's built-
 * in fatal-error mechanism.
 *
 * @return callable|null
 */
function set_exception_handler(?callable $callback) {}

/**
 * Returns the callback currently registered as the default handler for uncaught exceptions, or null
 * if none is set.
 *
 * @return callable|null
 */
#[Mago\AvailableSince(80500)]
function get_exception_handler(): ?callable {}

/**
 * Restores whichever exception handler was in effect before the most recent set_exception_handler()
 * call, popping it off PHP's internal stack of handlers. Always returns true, even if no handler
 * had been set.
 */
function restore_exception_handler(): true {}

/**
 * Returns the names of every class declared so far in the current script, including internal PHP
 * and extension classes, but not interfaces or traits, which have their own get_declared_interfaces
 * and get_declared_traits functions.
 *
 * @return list<class-string>
 *
 * @pure
 */
function get_declared_classes(): array {}

/**
 * Returns an array of the interface names declared so far, including both user-defined and built-in
 * interfaces, but only those PHP has actually loaded or autoloaded at the point of the call rather
 * than every interface that exists in the project.
 *
 * @return list<interface-string>
 *
 * @pure
 */
function get_declared_interfaces(): array {}

/**
 * Returns the names of every trait declared so far during the running script — the trait-only
 * counterpart of get_declared_classes().
 *
 * @return list<trait-string>
 *
 * @pure
 */
function get_declared_traits(): array {}

/**
 * Returns every function currently defined, split into the built-in ("internal") and user-declared
 * ("user") functions. When exclude_disabled is true, the default, functions disabled via the
 * disable_functions ini directive are left out of the internal list.
 *
 * @return array{internal: list<non-empty-string>, user: list<non-empty-string>}
 *
 * @pure
 */
function get_defined_functions(bool $exclude_disabled = true): array {}

/**
 * Returns every variable currently defined in the local scope as an associative array of name to
 * value, effectively the reverse of extract().
 *
 * @pure
 */
function get_defined_vars(): array {}

/**
 * Returns the type of a resource as a short descriptive string, such as stream or curl, or Unknown
 * for a resource type it does not recognise.
 *
 * @param resource $resource
 *
 * @return non-empty-string
 *
 * @pure
 */
function get_resource_type($resource): string {}

/**
 * Returns the names of every extension compiled into or loaded by the running PHP interpreter.
 * Passing true for $zend_extensions returns the loaded Zend extensions instead of the ordinary PHP
 * extensions.
 *
 * @return non-empty-list<non-empty-string>
 *
 * @pure
 */
function get_loaded_extensions(bool $zend_extensions = false): array {}

/**
 * Checks whether the given PHP extension is currently loaded, without attempting to load it if it
 * is not.
 *
 * @param non-empty-string $extension
 *
 * @pure
 */
function extension_loaded(string $extension): bool {}

/**
 * Returns the names of the functions declared by a PHP extension, or false if that extension is not
 * loaded.
 *
 * @return list<non-empty-string>|false
 *
 * @pure
 */
function get_extension_funcs(string $extension): array|false {}

/**
 * Returns every currently defined constant as a name to value map. With $categorize true, the
 * constants are instead grouped in a two-level array keyed first by the extension that defined
 * them, such as Core or pcre.
 *
 * @code
 * get_defined_constants(true)['Core']['PHP_VERSION']
 * @endcode
 *
 * @pure
 */
function get_defined_constants(bool $categorize = false): array {}

/**
 * Returns the current call stack as a list of frames, ordered from the immediate caller of
 * debug_backtrace() outward to the top of the script. Each frame is an array with keys such as
 * function, line, file, class and type, plus object and args unless $options excludes them; $limit
 * caps how many frames are returned, with 0 meaning no limit.
 *
 * @pure
 */
function debug_backtrace(int $options = DEBUG_BACKTRACE_PROVIDE_OBJECT, int $limit = 0): array {}

/**
 * Prints a backtrace of the current call stack directly to output, in the same format
 * debug_backtrace() would return. Passing DEBUG_BACKTRACE_IGNORE_ARGS as $options omits function
 * arguments from each frame, and a non-zero $limit caps the number of stack frames printed to the
 * most recent ones.
 */
function debug_print_backtrace(int $options = 0, int $limit = 0): void {}

/**
 * Forces an immediate collection of any garbage cycles currently pending in the circular-reference
 * collector's root buffer. Returns the number of cycles collected and freed.
 */
function gc_collect_cycles(): int {}

/**
 * Reports whether PHP's circular-reference garbage collector is currently enabled.
 *
 * @pure
 */
function gc_enabled(): bool {}

/**
 * Activates the circular-reference garbage collector. It already runs by default, so this only has
 * an effect after a prior gc_disable().
 */
function gc_enable(): void {}

/**
 * Disables the circular reference garbage collector, so cyclic references stop being automatically
 * freed until gc_enable() is called again.
 */
function gc_disable(): void {}

/**
 * Returns statistics about the cycle-collecting garbage collector. runs counts how many collection
 * cycles have been performed, collected is the total number of cycles collected so far, threshold
 * is the root-buffer size that triggers the next automatic run, and roots is how many possible
 * cycle roots are currently buffered.
 *
 * @return array{runs: int, collected: int, threshold: int, roots: int}
 *
 * @pure
 */
function gc_status(): array {}

/**
 * Forces the Zend memory manager to release memory held in its internal caches back to the
 * operating system. Returns the number of bytes freed.
 */
function gc_mem_caches(): int {}

/**
 * Returns the resources currently open in the request, optionally limited to a single resource type
 * such as "stream" or "curl". Passing null returns resources of every type, and an empty array is
 * returned if none match.
 *
 * @return list<resource>
 *
 * @pure
 */
function get_resources(?string $type = null): array {}

/**
 * Creates a shallow clone of $object, exactly as the clone operator does, then overwrites the given
 * properties on the copy with the values in $withProperties. This is the only way to change a
 * property, including a readonly one, as part of cloning.
 *
 * @code
 * $moved = clone($point, ['x' => 5]);
 * @endcode
 *
 * @template T of object
 *
 * @param T $object
 * @param array<string, mixed> $withProperties
 *
 * @return T
 */
function clone(object $object, array $withProperties = []): object {}

/**
 * Terminates script execution immediately. A string status is written to the output as though it
 * had been echoed, and the process then exits with status 0; an integer status is not printed and
 * becomes the process exit code instead, and should stay within 0-254 since PHP reserves 255 for
 * its own fatal errors. Registered shutdown functions and object destructors still run before the
 * process ends.
 */
function exit(string|int $status = 0): never {}

/**
 * Terminates script execution immediately; a language alias for exit(). A string argument is
 * printed before the process ends, while an integer argument is used as the process's exit status
 * code and is not printed.
 */
function die(string|int $status = 0): never {}

/**
 * @deprecated
 */
const E_STRICT = 2048;

const E_ERROR = 1;

const E_RECOVERABLE_ERROR = 4096;

const E_WARNING = 2;

const E_PARSE = 4;

const E_NOTICE = 8;

const E_DEPRECATED = 8192;

const E_CORE_ERROR = 16;

const E_CORE_WARNING = 32;

const E_COMPILE_ERROR = 64;

const E_COMPILE_WARNING = 128;

const E_USER_ERROR = 256;

const E_USER_WARNING = 512;

const E_USER_NOTICE = 1024;

const E_USER_DEPRECATED = 16384;

const E_ALL = 30719;

const DEBUG_BACKTRACE_PROVIDE_OBJECT = 1;

const DEBUG_BACKTRACE_IGNORE_ARGS = 2;

const S_MEMORY = 1;

const S_VARS = 4;

const S_FILES = 8;

const S_INCLUDE = 16;

const S_SQL = 32;

const S_EXECUTOR = 64;

const S_MAIL = 128;

const S_SESSION = 256;

const S_MISC = 2;

const S_INTERNAL = 536870912;

const S_ALL = 511;

const ZEND_THREAD_SAFE = false;

const ZEND_DEBUG_BUILD = false;

const PHP_WINDOWS_VERSION_BUILD = 0;

const PHP_WINDOWS_VERSION_MAJOR = 0;

const PHP_WINDOWS_VERSION_MINOR = 0;

const PHP_WINDOWS_VERSION_PLATFORM = 0;

const PHP_WINDOWS_VERSION_PRODUCTTYPE = 0;

const PHP_WINDOWS_VERSION_SP_MAJOR = 0;

const PHP_WINDOWS_VERSION_SP_MINOR = 0;

const PHP_WINDOWS_VERSION_SUITEMASK = 0;

const PHP_WINDOWS_NT_DOMAIN_CONTROLLER = 2;

const PHP_WINDOWS_NT_SERVER = 3;

const PHP_WINDOWS_NT_WORKSTATION = 1;

const PHP_WINDOWS_EVENT_CTRL_C = 0;

const PHP_WINDOWS_EVENT_CTRL_BREAK = 1;

/** @var string */
const PHP_BUILD_DATE = PHP_BUILD_DATE;

/** @var non-falsy-string */
const PHP_VERSION = PHP_VERSION;

/** @var int */
const PHP_MAJOR_VERSION = PHP_MAJOR_VERSION;

/** @var int */
const PHP_MINOR_VERSION = PHP_MINOR_VERSION;

/** @var int */
const PHP_RELEASE_VERSION = PHP_RELEASE_VERSION;

/** @var string */
const PHP_EXTRA_VERSION = PHP_EXTRA_VERSION;

/** @var int */
const PHP_VERSION_ID = PHP_VERSION_ID;

/** @var bool */
const PHP_ZTS = PHP_ZTS;

/** @var bool */
const PHP_DEBUG = PHP_DEBUG;

/** @var non-falsy-string */
const PHP_OS = PHP_OS;

/** @var non-falsy-string */
const PHP_OS_FAMILY = PHP_OS_FAMILY;

/** @var non-falsy-string */
const PHP_SAPI = PHP_SAPI;

/** @var bool */
const PHP_CLI_PROCESS_TITLE = PHP_CLI_PROCESS_TITLE;

/** @var string */
const DEFAULT_INCLUDE_PATH = DEFAULT_INCLUDE_PATH;

/** @var string */
const PEAR_INSTALL_DIR = PEAR_INSTALL_DIR;

/** @var string */
const PEAR_EXTENSION_DIR = PEAR_EXTENSION_DIR;

/** @var string */
const PHP_EXTENSION_DIR = PHP_EXTENSION_DIR;

/** @var string */
const PHP_BINARY = PHP_BINARY;

/** @var string */
const PHP_PREFIX = PHP_PREFIX;

/** @var string */
const PHP_BINDIR = PHP_BINDIR;

/** @var string */
const PHP_LIBDIR = PHP_LIBDIR;

/** @var string */
const PHP_DATADIR = PHP_DATADIR;

/** @var string */
const PHP_SYSCONFDIR = PHP_SYSCONFDIR;

/** @var string */
const PHP_LOCALSTATEDIR = PHP_LOCALSTATEDIR;

/** @var string */
const PHP_CONFIG_FILE_PATH = PHP_CONFIG_FILE_PATH;

/** @var string */
const PHP_CONFIG_FILE_SCAN_DIR = PHP_CONFIG_FILE_SCAN_DIR;

/** @var non-falsy-string */
const PHP_SHLIB_SUFFIX = PHP_SHLIB_SUFFIX;

/** @var non-falsy-string */
const PHP_EOL = PHP_EOL;

const SUHOSIN_PATCH = 1;

const SUHOSIN_PATCH_VERSION = SUHOSIN_PATCH_VERSION;

const PHP_MAXPATHLEN = 4096;

const PHP_INT_MAX = 9223372036854775807;

const PHP_INT_MIN = -9223372036854775808;

const PHP_INT_SIZE = 8;

const PHP_FLOAT_DIG = 15;

const PHP_FLOAT_EPSILON = 2.2204460492503e-16;

const PHP_FLOAT_MAX = 1.7976931348623e+308;

const PHP_FLOAT_MIN = 2.2250738585072e-308;

const ZEND_MULTIBYTE = 0;

const PHP_OUTPUT_HANDLER_START = 1;

const PHP_OUTPUT_HANDLER_CONT = 2;

const PHP_OUTPUT_HANDLER_END = 4;

const UPLOAD_ERR_OK = 0;

const UPLOAD_ERR_INI_SIZE = 1;

const UPLOAD_ERR_FORM_SIZE = 2;

const UPLOAD_ERR_PARTIAL = 3;

const UPLOAD_ERR_NO_FILE = 4;

const UPLOAD_ERR_NO_TMP_DIR = 6;

const UPLOAD_ERR_CANT_WRITE = 7;

const UPLOAD_ERR_EXTENSION = 8;

/** @var resource */
const STDIN = STDIN;

/** @var resource */
const STDOUT = STDOUT;

/** @var resource */
const STDERR = STDERR;

#[Mago\AvailableSince(80100)]
const PHP_FD_SETSIZE = 1024;

const PHP_OUTPUT_HANDLER_WRITE = 0;

const PHP_OUTPUT_HANDLER_FLUSH = 4;

const PHP_OUTPUT_HANDLER_CLEAN = 2;

const PHP_OUTPUT_HANDLER_FINAL = 8;

const PHP_OUTPUT_HANDLER_CLEANABLE = 16;

const PHP_OUTPUT_HANDLER_FLUSHABLE = 32;

const PHP_OUTPUT_HANDLER_REMOVABLE = 64;

const PHP_OUTPUT_HANDLER_STDFLAGS = 112;

const PHP_OUTPUT_HANDLER_STARTED = 4096;

const PHP_OUTPUT_HANDLER_DISABLED = 8192;

const PHP_MANDIR = '/usr/local/php/php/man';

const PHP_SBINDIR = '/usr/local/sbin';

const PHP_OUTPUT_HANDLER_PROCESSED = 16384;
