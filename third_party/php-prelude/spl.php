<?php

class LogicException extends Exception {}

class BadFunctionCallException extends LogicException {}

class BadMethodCallException extends BadFunctionCallException {}

class DomainException extends LogicException {}

class InvalidArgumentException extends LogicException {}

class LengthException extends LogicException {}

class OutOfRangeException extends LogicException {}

class RuntimeException extends Exception {}

class OutOfBoundsException extends RuntimeException {}

class OverflowException extends RuntimeException {}

class RangeException extends RuntimeException {}

class UnderflowException extends RuntimeException {}

class UnexpectedValueException extends RuntimeException {}

/**
 * @template-implements Iterator<never, never>
 */
class EmptyIterator implements Iterator
{
    /**
     * @return never
     */
    public function current() {}

    /**
     * Always throws, since an EmptyIterator never has a current element to return a key for.
     *
     * @return never
     */
    public function key() {}

    /**
     * Does nothing, since an always-empty iterator has no elements to advance past.
     *
     * @return void
     */
    public function next() {}

    /**
     * Does nothing, since an always-empty iterator has no position to reset to.
     *
     * @return void
     */
    public function rewind() {}

    /**
     * Always returns false, since an EmptyIterator never has an element to iterate.
     *
     * @return false
     */
    public function valid() {}
}

/**
 * @template-covariant K
 * @template-covariant V
 * @template-covariant TIterator as Iterator<K, V>
 *
 * @template-implements OuterIterator<K, V>
 *
 * @template-extends FilterIterator<K, V, TIterator>
 */
class CallbackFilterIterator extends FilterIterator implements OuterIterator
{
    /**
     * @param TIterator $iterator
     * @param (callable(V, K, TIterator): bool) $callback
     */
    public function __construct(Iterator $iterator, callable $callback) {}

    public function accept(): bool {}

    /**
     * Returns the value at the iterator's current position, which always satisfies the filter
     * callback, since iteration skips over elements the callback rejects.
     *
     * @return V|null
     * @ignore-nullable-return
     */
    public function current() {}

    /**
     * Returns the key of the element at the iterator's current position.
     *
     * @return K|null
     * @ignore-nullable-return
     */
    public function key() {}
}

/**
 * @template K
 * @template V
 * @template TIterator as RecursiveIterator<K, V>
 *
 * @template-implements RecursiveIterator<K, V>
 * @template-extends CallbackFilterIterator<K, V, TIterator>
 */
class RecursiveCallbackFilterIterator extends CallbackFilterIterator implements RecursiveIterator
{
    /**
     * @param TIterator $iterator
     * @param (callable(V, K, TIterator): bool) $callback
     */
    public function __construct(RecursiveIterator $iterator, callable $callback) {}

    /**
     * Returns a new RecursiveCallbackFilterIterator over the current element's children, applying
     * the same filter callback to them. PHP's RecursiveIteratorIterator calls this when
     * hasChildren() returns true, to descend into the current element.
     *
     * @return RecursiveCallbackFilterIterator<K, V, TIterator>
     */
    public function getChildren() {}

    /**
     * Returns whether the current element has children to recurse into. RecursiveIteratorIterator
     * calls this to decide whether to call getChildren() and descend.
     *
     * @return bool
     */
    public function hasChildren() {}

    /**
     * Returns the value at the iterator's current position, which always satisfies the filter
     * callback.
     *
     * @return V|null
     */
    public function current() {}

    /**
     * Returns the key of the element at the iterator's current position.
     *
     * @return K|null
     */
    public function key() {}
}

/**
 * @template-covariant K
 * @template-covariant V
 *
 * @template-extends Iterator<K, V>
 */
interface RecursiveIterator extends Iterator
{
    /**
     * @return bool
     */
    public function hasChildren();

    /**
     * Returns an iterator over the current element's children. RecursiveIteratorIterator calls this
     * when hasChildren() returns true, to descend into the current element.
     *
     * @return RecursiveIterator<K, V>
     */
    public function getChildren();
}

/**
 * @template K
 * @template-covariant V
 *
 * @implements OuterIterator<K, V>
 */
class RecursiveIteratorIterator implements OuterIterator
{
    public const LEAVES_ONLY = 0;
    public const SELF_FIRST = 1;
    public const CHILD_FIRST = 2;
    public const CATCH_GET_CHILD = 16;

    /**
     * @param Traversable<K, V> $iterator
     */
    public function __construct(Traversable $iterator, int $mode = self::LEAVES_ONLY, int $flags = 0) {}

    /**
     * Rewinds iteration to the very first element of the tree, descending into the first child
     * branch as needed for the current traversal mode.
     */
    public function rewind(): void {}

    /**
     * Reports whether the iterator's current position, at whatever depth it has reached, refers to
     * a valid element.
     */
    public function valid(): bool {}

    /**
     * Returns the key of the current element at whatever depth iteration has reached, or null if
     * there is no current element.
     *
     * @return null|K
     */
    public function key(): mixed {}

    /**
     * Returns the value of the current element at whatever depth iteration has reached, or null if
     * there is no current element.
     *
     * @return null|V
     */
    public function current(): mixed {}

    /**
     * Advances to the next element in depth-first order, descending into a node's children before
     * moving on to its siblings, and ascending back up the tree once a branch is exhausted.
     */
    public function next(): void {}

    /**
     * Returns how many levels deep the current element is nested, counting the top-level iterator
     * as depth 0.
     */
    public function getDepth(): int {}

    /**
     * Returns the sub-iterator active at the given depth, or, called with no level, the sub-
     * iterator at whatever depth iteration has currently reached; returns null if no sub-iterator
     * exists at that depth.
     *
     * @return RecursiveIterator<K, V>|null
     */
    public function getSubIterator(?int $level = null): ?RecursiveIterator {}

    /**
     * Returns the sub-iterator that is currently active, equivalent to calling getSubIterator()
     * with no level.
     *
     * @return RecursiveIterator<K, V>
     */
    public function getInnerIterator(): RecursiveIterator {}

    /**
     * Called once, at the very start of iteration during the first rewind(), before any traversal
     * happens. It is a template method meant to be overridden for setup and does nothing by
     * default.
     */
    public function beginIteration(): void {}

    /**
     * Called once iteration finishes, when valid() first returns false. It is a template method
     * meant to be overridden for teardown and does nothing by default.
     */
    public function endIteration(): void {}

    /**
     * Reports whether the current element of the active sub-iterator itself has children, by
     * delegating to that sub-iterator's own hasChildren(). It is a template method that can be
     * overridden to change what counts as having children.
     */
    public function callHasChildren(): bool {}

    /**
     * Returns a new iterator over the current element's children by delegating to the active sub-
     * iterator's getChildren(), or null if the current element has none. It is a template method
     * that can be overridden to change how children are fetched or wrapped.
     *
     * @return RecursiveIterator<K, V>|null
     */
    public function callGetChildren(): ?RecursiveIterator {}

    /**
     * Called just before iteration recurses into a node's children. It is a template method meant
     * to be overridden to react to descending a level and does nothing by default.
     */
    public function beginChildren(): void {}

    /**
     * Called just after iteration finishes a node's children and ascends back up a level. It is a
     * template method meant to be overridden and does nothing by default.
     */
    public function endChildren(): void {}

    /**
     * Called each time iteration moves to a new current element, at any depth. It is a template
     * method meant to be overridden as a hook and does nothing by default.
     */
    public function nextElement(): void {}

    /**
     * Limits recursion to the given number of levels below the top level. The default, -1, removes
     * any limit; 0 restricts iteration to the top level only, making the iterator behave as if it
     * were not recursive at all.
     */
    public function setMaxDepth(int $maxDepth = -1): void {}

    /**
     * Returns the recursion limit previously set with setMaxDepth(), or false if no limit has been
     * set.
     */
    public function getMaxDepth(): int|false {}
}

/**
 * @template-covariant K
 * @template-covariant V
 *
 * @template-extends Iterator<K, V>
 */
interface OuterIterator extends Iterator
{
    /**
     * Returns the iterator that the implementing object wraps. PHP's iteration engine never calls
     * this itself; it exists purely so calling code can reach the wrapped iterator directly, and
     * every SPL class that wraps another iterator, such as IteratorIterator, implements it.
     *
     * @return Iterator<K, V>
     */
    public function getInnerIterator();
}

/**
 * @template K
 * @template-covariant V
 * @template I of Traversable<K, V>
 *
 * @implements OuterIterator<K, V>
 *
 * @mixin I
 */
class IteratorIterator implements OuterIterator
{
    /**
     * Wraps any Traversable, including a Generator or an object implementing IteratorAggregate, so
     * that it can be used as a plain Iterator.
     *
     * @param I $iterator
     * @param class-string|null $class
     */
    public function __construct(Traversable $iterator, ?string $class = null) {}

    /**
     * Returns the iterator being wrapped, as given to, or derived from, the constructor, or null if
     * none is available.
     *
     * @return Iterator<K, V>|null
     */
    public function getInnerIterator(): ?Iterator {}

    /**
     * Rewinds the wrapped iterator to its first element by calling its rewind().
     *
     * @return void
     */
    public function rewind(): void {}

    /**
     * Reports whether the wrapped iterator's current position is valid, by calling its valid().
     */
    public function valid(): bool {}

    /**
     * Returns the wrapped iterator's current key, or null if there is none.
     *
     * @return null|K
     */
    public function key(): mixed {}

    /**
     * Returns the wrapped iterator's current value, or null if there is none.
     *
     * @return null|V
     */
    public function current(): mixed {}

    /**
     * Advances the wrapped iterator to its next element by calling its next().
     */
    public function next(): void {}

    /**
     * Forwards a method call to the wrapped iterator whenever IteratorIterator itself does not
     * define that method, so any extra methods the inner iterator implements can be called directly
     * on the wrapper.
     */
    public function __call(string $name, array $argv): mixed {}
}

/**
 * @template-covariant K
 * @template-covariant V
 * @template-covariant TIterator as Traversable<K, V>
 *
 * @template-extends IteratorIterator<K, V, TIterator>
 */
abstract class FilterIterator extends IteratorIterator
{
    /**
     * Decides whether the wrapped iterator's current element should be included in iteration. A
     * subclass must implement it, and PHP's iteration machinery calls it after every rewind() and
     * next() to silently skip elements for which it returns false.
     *
     * @return bool
     */
    abstract public function accept();

    /**
     * Returns the wrapped iterator's current value. It is only ever reached at an element for which
     * accept() returned true.
     *
     * @return null|V
     * @ignore-nullable-return
     */
    public function current() {}

    /**
     * Returns the wrapped iterator's current key. It is only ever reached at an element for which
     * accept() returned true.
     *
     * @return null|K
     * @ignore-nullable-return
     */
    public function key() {}
}

/**
 * @template K
 * @template V
 * @template TIterator as RecursiveIterator<K, V>
 *
 * @template-extends FilterIterator<K, V, TIterator>
 * @template-implements RecursiveIterator<K, V>
 */
abstract class RecursiveFilterIterator extends FilterIterator implements RecursiveIterator
{
    /**
     * Wraps a RecursiveIterator so that only elements accepted by accept() are iterated, while
     * still supporting recursion into children.
     *
     * @param TIterator $iterator
     */
    public function __construct(RecursiveIterator $iterator) {}

    /**
     * Returns the current element's children, wrapped in a new instance of the same filter class,
     * so that filtering by accept() continues into the recursion.
     *
     * @return TIterator
     */
    public function getChildren() {}

    /**
     * Reports whether the wrapped iterator's current element has children, by delegating to its own
     * hasChildren().
     *
     * @return bool
     */
    public function hasChildren() {}

    /**
     * Returns the wrapped iterator's current value. It is only ever reached at an element for which
     * accept() returned true.
     *
     * @return V|null
     * @ignore-nullable-return
     */
    public function current() {}

    /**
     * Returns the wrapped iterator's current key. It is only ever reached at an element for which
     * accept() returned true.
     *
     * @return K|null
     * @ignore-nullable-return
     */
    public function key() {}
}

/**
 * @template K
 * @template V
 * @template TIterator as RecursiveIterator<K, V>
 *
 * @template-extends RecursiveFilterIterator<K, V, TIterator>
 */
class ParentIterator extends RecursiveFilterIterator implements RecursiveIterator, OuterIterator
{
    /**
     * Accepts only elements that have children, filtering out leaves so that iteration visits
     * branch nodes alone.
     *
     * @return bool
     */
    public function accept() {}

    /**
     * Wraps a RecursiveIterator so that iteration visits only the elements that have children,
     * skipping every leaf.
     *
     * @param TIterator $iterator
     */
    public function __construct(RecursiveIterator $iterator) {}

    /**
     * Returns the current element's children wrapped in a new ParentIterator, so that only branch
     * nodes are visited at the next depth as well.
     *
     * @return ParentIterator<K,V>
     */
    public function getChildren() {}

    /**
     * Reports whether the wrapped iterator's current element has children.
     *
     * @return bool
     */
    public function hasChildren() {}

    /**
     * Advances to the next branch node, skipping over any leaf elements the wrapped iterator has in
     * between.
     *
     * @return void
     */
    public function next() {}

    /**
     * Rewinds to the first branch node, skipping over any leading leaf elements.
     *
     * @return void
     */
    public function rewind() {}

    /**
     * Returns the current branch element's value.
     *
     * @return V|null
     * @ignore-nullable-return
     */
    public function current() {}

    /**
     * Returns the current branch element's key.
     *
     * @return K|null
     * @ignore-nullable-return
     */
    public function key() {}
}

/**
 * @template-covariant K
 * @template-covariant V
 *
 * @template-extends Iterator<K, V>
 */
interface SeekableIterator extends Iterator
{
    public function seek(int $offset): void;
}

/**
 * @template-covariant K
 * @template-covariant V
 * @template-covariant TIterator as Iterator<K, V>
 *
 * @template-implements OuterIterator<K, V>
 * @template-extends IteratorIterator<K, V, TIterator>
 */
class LimitIterator extends IteratorIterator implements OuterIterator
{
    /**
     * Wraps an iterator so that only a slice of it is visited: iteration starts at position $offset
     * and stops after $limit elements, or continues to the end when $limit is -1, the default.
     *
     * @code
     * // skips the first 10 elements, then yields up to 5 more
     * new LimitIterator($iterator, 10, 5);
     * @endcode
     *
     * @param TIterator $iterator
     */
    public function __construct(Iterator $iterator, int $offset = 0, int $limit = -1) {}

    /**
     * Returns the current element's value. It is valid only while iteration is within the offset-
     * to-offset-plus-limit window.
     *
     * @return V|null
     * @ignore-nullable-return
     */
    public function current() {}

    /**
     * Returns the current element's key. It is valid only while iteration is within the offset-to-
     * offset-plus-limit window.
     *
     * @return K|null
     * @ignore-nullable-return
     */
    public function key() {}
}

/**
 * @template-covariant K
 * @template-covariant V
 * @template-covariant TIterator as Iterator<K, V>
 *
 * @template-implements OuterIterator<K, V>
 * @template-implements ArrayAccess<K, V>
 *
 * @template-extends IteratorIterator<K, V, TIterator>
 */
class CachingIterator extends IteratorIterator implements OuterIterator, ArrayAccess, Countable
{
    const CALL_TOSTRING = 1;
    const CATCH_GET_CHILD = 16;
    const TOSTRING_USE_KEY = 2;
    const TOSTRING_USE_CURRENT = 4;
    const TOSTRING_USE_INNER = 8;
    const FULL_CACHE = 256;

    /**
     * Wraps an iterator so that elements can be looked back at through getCache(), and $flags,
     * which can be combined with bitwise OR, controls what is cached and how an element is rendered
     * as a string: CALL_TOSTRING converts each element to a string, TOSTRING_USE_KEY uses the key
     * rather than the value for that conversion, TOSTRING_USE_CURRENT uses the current element
     * explicitly, TOSTRING_USE_INNER defers to the wrapped iterator's own string conversion,
     * FULL_CACHE keeps every element seen rather than only the most recent one, and CATCH_GET_CHILD
     * ignores exceptions thrown while fetching children instead of letting them propagate.
     *
     * @code
     * new CachingIterator($iterator, CachingIterator::FULL_CACHE | CachingIterator::TOSTRING_USE_CURRENT);
     * @endcode
     *
     * @param TIterator $iterator
     */
    public function __construct(Iterator $iterator, int $flags = self::CALL_TOSTRING) {}

    /** @return non-negative-int */
    public function count(): int {}

    /** @param K $key */
    public function offsetExists($key): bool {}

    /**
     * @param K $key
     *
     * @return V
     */
    public function offsetGet($key): mixed {}

    /**
     * @param K $key
     * @param V $value
     */
    public function offsetSet($key, mixed $value): void {}

    /** @param K $key */
    public function offsetUnset($key): void {}

    /**
     * Reports whether the wrapped iterator has a further element after the current one.
     *
     * @return bool
     */
    public function hasNext() {}

    /**
     * Returns the wrapped iterator's current value.
     *
     * @return V|null
     * @ignore-nullable-return
     */
    public function current() {}

    /**
     * Returns the wrapped iterator's current key.
     *
     * @return K|null
     * @ignore-nullable-return
     */
    public function key() {}

    /**
     * Returns everything cached so far, keyed by the wrapped iterator's own keys. It only
     * accumulates more than the single most recent element when the FULL_CACHE flag was passed to
     * the constructor.
     *
     * @return array<array-key, V>
     */
    public function getCache() {}
}

/**
 * @template K
 * @template V
 * @template TIterator as Iterator<K, V>
 *
 * @template-implements RecursiveIterator<K, V>
 * @template-extends CachingIterator<K, V, TIterator>
 */
class RecursiveCachingIterator extends CachingIterator implements RecursiveIterator
{
    /**
     * Returns the current element's children wrapped in a new RecursiveCachingIterator constructed
     * with the same caching flags, so caching continues into the recursion.
     *
     * @return RecursiveCachingIterator<K,V, TIterator>
     */
    public function getChildren() {}

    /**
     * Reports whether the wrapped iterator's current element has children.
     *
     * @return bool
     */
    public function hasChildren() {}
}

/**
 * @template-covariant K
 * @template-covariant V
 * @template-covariant TIterator as Iterator<K, V>
 *
 * @template-extends IteratorIterator<K, V, TIterator>
 */
class NoRewindIterator extends IteratorIterator
{
    /**
     * Wraps an iterator so that only the very first rewind() has any effect; every later call to
     * rewind() is silently ignored. This keeps an iterator that cannot safely be rewound, such as a
     * Generator, usable in code that calls rewind() more than once, for example a second foreach
     * over the same variable.
     *
     * @param TIterator $iterator
     */
    public function __construct(Iterator $iterator) {}

    /**
     * Returns the wrapped iterator's current value.
     *
     * @return V|null
     * @ignore-nullable-return
     */
    public function current() {}

    /**
     * Returns the wrapped iterator's current key.
     *
     * @return K|null
     * @ignore-nullable-return
     */
    public function key() {}
}

/**
 * @template K
 * @template V
 * @template TIterator as Iterator<K, V>
 *
 * @template-extends IteratorIterator<K, V, TIterator>
 */
class AppendIterator extends IteratorIterator
{
    /**
     * Constructs an AppendIterator with no iterators queued; iterators are added afterwards with
     * append(), and iteration visits them in the order they were appended.
     */
    public function __construct() {}

    /**
     * Queues another iterator to be traversed after the ones already appended. It can be called
     * during iteration to add more without restarting it.
     *
     * @code
     * $it = new AppendIterator();
     * $it->append(new ArrayIterator([1, 2]));
     * $it->append(new ArrayIterator([3, 4]));
     * @endcode
     *
     * @param TIterator $iterator
     * @return void
     */
    public function append(Iterator $iterator) {}

    /**
     * Returns the ArrayIterator that AppendIterator uses internally to hold the queue of appended
     * iterators.
     *
     * @return ArrayIterator<K, V>
     */
    public function getArrayIterator() {}

    /**
     * Returns the position, counting from zero, of whichever appended iterator is presently being
     * iterated.
     *
     * @return int
     */
    public function getIteratorIndex() {}

    /**
     * Returns the current element by delegating to whichever appended iterator is presently active,
     * and returns null once every appended iterator has been exhausted.
     *
     * @return V|null
     * @ignore-nullable-return
     */
    public function current() {}

    /**
     * Returns the current key by delegating to whichever appended iterator is presently active, and
     * returns null once every appended iterator has been exhausted.
     *
     * @return K|null
     * @ignore-nullable-return
     */
    public function key() {}
}

/**
 * @template-covariant K
 * @template-covariant V
 * @template-covariant TIterator as Iterator<K, V>
 *
 * @template-extends IteratorIterator<K, V, TIterator>
 */
class InfiniteIterator extends IteratorIterator
{
    /**
     * Wraps $iterator so that reaching its end automatically triggers a rewind, making iteration
     * continue indefinitely rather than stopping; commonly paired with LimitIterator to cap how
     * many elements are actually consumed.
     *
     * @code
     * new LimitIterator(new InfiniteIterator($it), 0, 5);
     * @endcode
     *
     * @param TIterator $iterator
     */
    public function __construct(Iterator $iterator) {}

    /**
     * Returns the current element of the wrapped iterator, unchanged.
     *
     * @return V|null
     * @ignore-nullable-return
     */
    public function current() {}

    /**
     * Returns the current key of the wrapped iterator, unchanged.
     *
     * @return K|null
     * @ignore-nullable-return
     */
    public function key() {}
}

/**
 * @template K
 * @template V
 * @template TIterator as Iterator<K, V>
 *
 * @template-extends FilterIterator<K, V, TIterator>
 */
class RegexIterator extends FilterIterator
{
    const MATCH = 0;
    const GET_MATCH = 1;
    const ALL_MATCHES = 2;
    const SPLIT = 3;
    const REPLACE = 4;
    const USE_KEY = 1;

    /**
     * Filters $iterator down to the elements whose value, or key when $flags includes
     * RegexIterator::USE_KEY, matches $regex. $mode controls what current() then returns:
     * RegexIterator::MATCH returns the value unchanged, GET_MATCH returns the array of captured
     * groups, ALL_MATCHES returns every match found, SPLIT returns the pieces produced by splitting
     * on the pattern, and REPLACE substitutes the matches. $preg_flags is passed straight through
     * to the underlying PCRE function.
     *
     * @code
     * new RegexIterator($it, '/^foo/', RegexIterator::GET_MATCH);
     * @endcode
     *
     * @param TIterator $iterator
     * @param string $regex
     * @param RegexIterator::MATCH|RegexIterator::GET_MATCH|RegexIterator::ALL_MATCHES|RegexIterator::SPLIT|RegexIterator::REPLACE $mode
     * @param 0|RegexIterator::USE_KEY $flags
     */
    public function __construct(
        Iterator $iterator,
        string $regex,
        int $mode = self::MATCH,
        int $flags = 0,
        int $preg_flags = 0,
    ) {}

    public function accept(): bool {}

    /**
     * Returns the current element as shaped by the mode passed to the constructor -- the original
     * value unchanged in RegexIterator::MATCH mode, or an array of the regular-expression matches
     * in RegexIterator::GET_MATCH and RegexIterator::ALL_MATCHES mode -- and returns null once
     * iteration is exhausted.
     *
     * @return null|V
     * @ignore-nullable-return
     */
    public function current() {}

    /**
     * Returns the current key, and returns null once iteration is exhausted; when the constructor's
     * flags include RegexIterator::USE_KEY, the key rather than the value is what the pattern was
     * matched against.
     *
     * @return null|K
     * @ignore-nullable-return
     */
    public function key() {}
}

/**
 * @template K
 * @template V
 * @template TIterator as RecursiveIterator<K, V>
 *
 * @template-implements RecursiveIterator<K, V>
 * @template-extends RegexIterator<K, V, TIterator>
 */
class RecursiveRegexIterator extends RegexIterator implements RecursiveIterator
{
    const MATCH = 0;
    const GET_MATCH = 1;
    const ALL_MATCHES = 2;
    const SPLIT = 3;
    const REPLACE = 4;
    const USE_KEY = 1;

    /**
     * @param TIterator $iterator
     * @param string $regex
     * @param RecursiveRegexIterator::MATH|RecursiveRegexIterator::GET_MATCH|RecursiveRegexIterator::ALL_MATCHES|RecursiveRegexIterator::SPLIT|RecursiveRegexIterator::REPLACE $mode
     * @param RecursiveRegexIterator::USE_KEY|0 $flags
     * @param int $preg_flags
     */
    public function __construct(
        RecursiveIterator $iterator,
        string $regex,
        int $mode = self::MATCH,
        int $flags = 0,
        int $preg_flags = 0,
    ) {}

    /**
     * Returns a RecursiveRegexIterator over the current element's children, applying the same
     * pattern, mode and flags recursively.
     *
     * @return RecursiveRegexIterator<K, V>
     */
    public function getChildren() {}

    public function hasChildren(): bool {}
}

/**
 * @template K
 * @template V
 *
 * @template-extends RecursiveIteratorIterator<K, V>
 * @template-implements OuterIterator<K, V>
 */
class RecursiveTreeIterator extends RecursiveIteratorIterator implements OuterIterator
{
    const LEAVES_ONLY = 0;
    const SELF_FIRST = 1;
    const CHILD_FIRST = 2;
    const CATCH_GET_CHILD = 16;

    const BYPASS_CURRENT = 4;
    const BYPASS_KEY = 8;
    const PREFIX_LEFT = 0;
    const PREFIX_MID_HAS_NEXT = 1;
    const PREFIX_MID_LAST = 2;
    const PREFIX_END_HAS_NEXT = 3;
    const PREFIX_END_LAST = 4;
    const PREFIX_RIGHT = 5;

    /**
     * @return void
     */
    public function beginChildren() {}

    /**
     * Called automatically by the iteration machinery once, before traversal of the tree begins;
     * not intended to be called directly.
     *
     * @return RecursiveIterator
     */
    public function beginIteration() {}

    /**
     * Returns the children of whichever element the innermost wrapped iterator currently sits on,
     * by delegating to that iterator's getChildren().
     *
     * @return RecursiveIterator
     */
    public function callGetChildren() {}

    /**
     * Returns whether the element the innermost wrapped iterator currently sits on has children, by
     * delegating to that iterator's hasChildren().
     *
     * @return bool
     */
    public function callHasChildren() {}

    /**
     * Constructs a tree-formatting iterator over $it. $flags controls whether the tree-branch
     * drawing is applied to current() and key(): RecursiveTreeIterator::BYPASS_CURRENT skips it for
     * current(), and RecursiveTreeIterator::BYPASS_KEY, set by default, skips it for key(), so only
     * current() is decorated unless told otherwise. $cit_flags defaults to
     * RecursiveTreeIterator::CATCH_GET_CHILD, which suppresses exceptions thrown by an element's
     * getChildren() and treats that element as childless instead of letting the error propagate.
     * $mode chooses the traversal order: LEAVES_ONLY visits only leaf nodes, SELF_FIRST, the
     * default, visits a node before its children, and CHILD_FIRST visits the children before the
     * node itself.
     *
     * @param RecursiveIterator<K, V>|IteratorAggregate<K, V> $it
     * @param int $flags
     * @param RecursiveTreeIterator::CATCH_GET_CHILD $cit_flags
     * @param RecursiveTreeIterator::LEAVES_ONLY|RecursiveTreeIterator::SELF_FIRST|RecursiveTreeIterator::CHILD_FIRST $mode
     */
    public function __construct(
        $it,
        int $flags = self::BYPASS_KEY,
        int $cit_flags = self::CATCH_GET_CHILD,
        int $mode = self::SELF_FIRST,
    ) {}

    /**
     * Returns the current node's display text with its tree-branch prefix, unless
     * RecursiveTreeIterator::BYPASS_CURRENT was set on the constructor, and returns null once
     * iteration is exhausted.
     *
     * @return null|V
     * @ignore-nullable-return
     */
    public function current() {}

    /**
     * Called automatically by the iteration machinery after finishing traversal of a node's
     * children; not intended to be called directly.
     */
    public function endChildren(): void {}

    /**
     * Called automatically by the iteration machinery once traversal of the whole tree has
     * finished; not intended to be called directly.
     */
    public function endIteration(): void {}

    /**
     * Returns the current node's display text on its own, without the tree-branch prefix that
     * current() prepends.
     */
    public function getEntry(): string {}

    /**
     * Returns the string appended after each node's entry text, empty unless set with setPostfix().
     */
    public function getPostfix(): string {}

    /**
     * Returns the tree-branch drawing prefixed to the current node's entry, showing its depth and
     * its position among its siblings.
     */
    public function getPrefix(): string {}

    /**
     * Returns the current node's key, plain rather than tree-decorated because
     * RecursiveTreeIterator::BYPASS_KEY is set by default, and returns null once iteration is
     * exhausted.
     *
     * @return null|K
     * @ignore-nullable-return
     */
    public function key() {}

    /**
     * Called automatically by foreach, or by a manual call, to advance to the next node in the
     * traversal order chosen by the constructor's mode.
     */
    public function next(): void {}

    public function nextElement(): void {}

    /**
     * Called automatically at the start of a foreach loop, or by a manual call, to reposition the
     * iterator at the first node of the tree, in the traversal order chosen by the constructor's
     * mode.
     */
    public function rewind(): void {}

    /**
     * Sets, in place, the string appended after each node's entry text; see getPostfix().
     */
    public function setPostfix(string $postfix): void {}

    /**
     * Replaces the string used for one piece of the tree-branch drawing, chosen by one of the
     * RecursiveTreeIterator::PREFIX_* constants -- for instance the branch drawn before a node with
     * following siblings versus the branch drawn before the last one.
     */
    public function setPrefixPart(int $part, string $value): void {}

    /**
     * Called by PHP before every current() or key() access, including the first, to check whether
     * traversal still has a node to visit; foreach stops once it returns false.
     */
    public function valid(): bool {}
}

/**
 * @template K of array-key
 * @template V
 *
 * @template-implements IteratorAggregate<K, V>
 * @template-implements ArrayAccess<K, V>
 */
class ArrayObject implements IteratorAggregate, ArrayAccess, Serializable, Countable
{
    const STD_PROP_LIST = 1;
    const ARRAY_AS_PROPS = 2;

    /**
     * Constructs an ArrayObject wrapping $array -- an array, or an object's public properties -- as
     * its initial contents. $flags selects ArrayObject::ARRAY_AS_PROPS or
     * ArrayObject::STD_PROP_LIST behaviour, and $iterator_class names the class getIterator()
     * instantiates, ArrayIterator by default.
     *
     * @param array<K, V>|object $input
     * @param int $flags
     * @param class-string<ArrayIterator<K,V>>|class-string<ArrayObject<K,V>> $iterator_class
     */
    public function __construct($input = null, $flags = 0, $iterator_class = 'ArrayIterator') {}

    /**
     * Called by PHP for isset($obj[$offset]) and empty($obj[$offset]); returns whether an entry
     * exists at that offset.
     *
     * @param K $offset
     *
     * @return bool
     *
     * @no-named-arguments
     */
    public function offsetExists($offset) {}

    /**
     * Called by PHP for $obj[$offset]; returns the entry stored at that offset.
     *
     * @param K $offset
     *
     * @return V
     *
     * @no-named-arguments
     */
    public function offsetGet($offset) {}

    /**
     * Called by PHP for $obj[$offset] = $value; stores $value at $offset, or appends it when
     * $offset is null, as with $obj[] = $value.
     *
     * @code
     * $obj[] = $value; // same as offsetSet(null, $value)
     * @endcode
     *
     * @param K $offset
     * @param V $value
     *
     * @return void
     *
     * @no-named-arguments
     */
    public function offsetSet($offset, $value) {}

    /**
     * Called by PHP for unset($obj[$offset]); removes the entry stored at that offset.
     *
     * @param K $offset
     *
     * @return void
     *
     * @no-named-arguments
     */
    public function offsetUnset($offset) {}

    /**
     * Appends $value to the end of the ArrayObject, equivalent to $obj[] = $value.
     *
     * @param V $value
     *
     * @return void
     */
    public function append($value) {}

    /**
     * Returns a plain array holding a copy of the ArrayObject's current entries; modifying the
     * returned array does not affect the object.
     *
     * @return array<K, V>
     */
    public function getArrayCopy() {}

    /**
     * Returns the number of entries currently stored; called by PHP's count() function when it is
     * applied to the object.
     *
     * @return int
     */
    public function count() {}

    /**
     * Returns the bitmask of behaviour flags currently in effect, as set by the constructor or
     * setFlags(). ArrayObject::ARRAY_AS_PROPS makes the stored entries readable and writable as if
     * they were declared properties, in addition to array-style access, while
     * ArrayObject::STD_PROP_LIST keeps access array-style only.
     *
     * @return int
     */
    public function getFlags() {}

    /**
     * Changes the behaviour flags in place, taking effect immediately for subsequent accesses.
     * ArrayObject::ARRAY_AS_PROPS makes the stored entries readable and writable as if they were
     * declared properties, in addition to array-style access, while ArrayObject::STD_PROP_LIST
     * keeps access array-style only.
     *
     * @param int $flags
     *
     * @return void
     */
    public function setFlags($flags) {}

    /**
     * Sorts the entries by value in place, preserving key associations.
     *
     * @return void
     */
    public function asort() {}

    /**
     * Sorts the entries by key in place.
     *
     * @return void
     */
    public function ksort() {}

    /**
     * Sorts the entries by value in place using $cmp_function, preserving key associations; the
     * callback returns a negative, zero, or positive integer depending on whether its first
     * argument sorts before, equal to, or after its second.
     *
     * @param (callable(V, V): int) $cmp_function
     *
     * @return void
     */
    public function uasort($cmp_function) {}

    /**
     * Sorts the entries by key in place using $cmp_function; the callback returns a negative, zero,
     * or positive integer depending on whether its first argument sorts before, equal to, or after
     * its second.
     *
     * @param (callable(K, K):int) $cmp_function
     *
     * @return void
     */
    public function uksort($cmp_function) {}

    /**
     * Sorts the entries by value in place using a natural-order comparison, so a value such as
     * img10 sorts after img2 rather than before it as a plain string comparison would.
     *
     * @return void
     */
    public function natsort() {}

    /**
     * Sorts the entries by value in place using a case-insensitive natural-order comparison.
     *
     * @return void
     */
    public function natcasesort() {}

    /**
     * Called by PHP when restoring a previously serialised ArrayObject; rebuilds its state from
     * $serialized.
     *
     * @param string $serialized
     *
     * @return void
     */
    public function unserialize($serialized) {}

    /**
     * Called by PHP when serialising the object, for example via serialize(); returns a string
     * encoding its current state for unserialize() to restore.
     *
     * @return string
     */
    public function serialize() {}

    /**
     * Called by PHP when the object is used in a foreach loop; returns an ArrayIterator over its
     * current entries.
     *
     * @return ArrayIterator<K, V>
     */
    public function getIterator() {}

    /**
     * Replaces the ArrayObject's entries with $input, and returns the entries it held immediately
     * before the exchange rather than the new ones.
     *
     * @code
     * $old = $obj->exchangeArray($new); // $old is what $obj contained before
     * @endcode
     *
     * @param mixed $input
     *
     * @return array
     */
    public function exchangeArray($input) {}

    /**
     * Sets, in place, the class that getIterator() instantiates; it must be ArrayIterator or a
     * subclass of it.
     *
     * @param class-string<ArrayIterator<K,V>>|class-string<ArrayObject<K,V>> $iterator_class
     *
     * @return void
     */
    public function setIteratorClass($iterator_class) {}

    /**
     * Returns the class name that getIterator() currently instantiates.
     *
     * @return class-string<ArrayIterator<K, V>>|class-string<ArrayObject<K, V>>
     */
    public function getIteratorClass() {}
}

/**
 * @template K as array-key
 * @template V
 *
 * @template-implements SeekableIterator<K, V>
 * @template-implements ArrayAccess<K, V>
 */
class ArrayIterator implements SeekableIterator, ArrayAccess, Serializable, Countable
{
    const STD_PROP_LIST = 1;
    const ARRAY_AS_PROPS = 2;

    /**
     * @param array<K, V> $array
     * @param int $flags
     */
    public function __construct($array = [], $flags = 0) {}

    /**
     * Reports whether the given offset exists in the underlying array. Called by PHP when isset()
     * or empty() is used on the iterator with array syntax, such as isset($iterator[$offset]).
     *
     * @param K $offset
     *
     * @return bool
     */
    public function offsetExists($offset) {}

    /**
     * Returns the value stored at the given offset. Called by PHP for array-style access, such as
     * $iterator[$offset]. Returns null if the offset does not exist.
     *
     * @param K $offset
     *
     * @return V|null
     *
     * @ignore-nullable-return
     */
    public function offsetGet($offset) {}

    /**
     * Sets the value at the given offset, creating it if it does not already exist. Called by PHP
     * for array-style assignment, such as $iterator[$offset] = $value; passing null as the offset
     * appends the value instead, exactly as $iterator[] = $value does. Mutates the receiver and
     * returns nothing.
     *
     * @code
     * $iterator[] = $value; // same as $iterator->offsetSet(null, $value)
     * @endcode
     *
     * @param K $offset
     *
     * @param V $value
     *
     * @return void
     */
    public function offsetSet($offset, $value) {}

    /**
     * Removes the element at the given offset from the underlying array. Called by PHP for
     * unset($iterator[$offset]).
     *
     * @param K $offset
     *
     * @return void
     */
    public function offsetUnset($offset) {}

    /**
     * Appends a value to the end of the underlying array, equivalent to $iterator[] = $value.
     * Mutates the receiver and returns nothing.
     *
     * @param V $value
     *
     * @return void
     */
    public function append($value) {}

    /**
     * Returns a copy of the underlying array, keyed the same way iteration over the object would
     * produce. The receiver itself is left unchanged.
     *
     * @return array<K, V>
     */
    public function getArrayCopy() {}

    /**
     * Returns the number of elements in the underlying array. Called automatically by PHP's count()
     * function, since the class implements Countable.
     *
     * @return int
     */
    public function count() {}

    /**
     * Returns the flags bitmask controlling how the iterator exposes an underlying object's
     * properties: STD_PROP_LIST (1) makes them behave as ordinary properties when listed, for
     * example by var_dump or foreach, and ARRAY_AS_PROPS (2) additionally allows entries to be read
     * and written as if they were properties.
     *
     * @return int
     */
    public function getFlags() {}

    /**
     * Sets the flags bitmask controlling how the iterator exposes an underlying object's
     * properties: STD_PROP_LIST (1) makes them behave as ordinary properties when listed, for
     * example by var_dump or foreach, and ARRAY_AS_PROPS (2) additionally allows entries to be read
     * and written as if they were properties. Mutates the receiver and returns nothing.
     *
     * @param int $flags
     *
     * @return void
     */
    public function setFlags($flags) {}

    /**
     * Sorts the underlying array in place by value, preserving key associations, as with the
     * asort() function.
     *
     * @return void
     */
    public function asort() {}

    /**
     * Sorts the underlying array in place by key, as with the ksort() function.
     *
     * @return void
     */
    public function ksort() {}

    /**
     * Sorts the underlying array in place by value using a user-supplied comparison callback,
     * preserving key associations, as with the uasort() function. The callback receives two values
     * and must return a negative, zero, or positive integer according to their order.
     *
     * @param (callable(V,V): int) $cmp_function
     *
     * @return void
     */
    public function uasort($cmp_function) {}

    /**
     * Sorts the underlying array in place by key using a user-supplied comparison callback, as with
     * the uksort() function. The callback receives two keys and must return a negative, zero, or
     * positive integer according to their order.
     *
     * @param (callable(K,K): int) $cmp_function
     *
     * @return void
     */
    public function uksort($cmp_function) {}

    /**
     * Sorts the underlying array in place by value using a case-sensitive natural-order comparison,
     * preserving key associations, as with the natsort() function.
     *
     * @return void
     */
    public function natsort() {}

    /**
     * Sorts the underlying array in place by value using a case-insensitive natural-order
     * comparison, preserving key associations, as with the natcasesort() function.
     *
     * @return void
     */
    public function natcasesort() {}

    /**
     * Reconstructs the iterator's underlying array from a string previously produced by
     * serialize(). Called automatically by PHP when an instance is recreated through the
     * unserialize() function.
     *
     * @param string $serialized
     *
     * @return void
     */
    public function unserialize($serialized) {}

    /**
     * Returns a string representation of the underlying array for use by PHP's serialize()
     * function. Called automatically by PHP when the instance is passed to serialize().
     *
     * @return string
     */
    public function serialize() {}

    /**
     * Moves the iterator back to the first element. Called automatically by PHP at the start of a
     * foreach loop.
     *
     * @return void
     */
    public function rewind() {}

    /**
     * Returns the value at the current iterator position. Called automatically by PHP on each pass
     * of a foreach loop. Returns null once the position has moved past the last element.
     *
     * @return V|null
     *
     * @ignore-nullable-return
     */
    public function current() {}

    /**
     * Returns the key at the current iterator position. Called automatically by PHP on each pass of
     * a foreach loop. Returns null once the position has moved past the last element.
     *
     * @return K|null
     *
     * @ignore-nullable-return
     */
    public function key() {}

    /**
     * Advances the iterator to the next element. Called automatically by PHP after each pass of a
     * foreach loop.
     *
     * @return void
     */
    public function next() {}

    /**
     * Reports whether the current iterator position refers to an existing element. Called
     * automatically by PHP before each pass of a foreach loop to decide whether to continue
     * iterating.
     *
     * @return bool
     */
    public function valid() {}

    /**
     * Moves the iterator to the given zero-based position. Throws an OutOfBoundsException if the
     * position does not exist.
     *
     * @param int $offset
     *
     * @return void
     *
     * @throws OutOfBoundsException
     */
    public function seek($offset) {}
}

/**
 * @template K
 * @template V
 *
 * @template-implements RecursiveIterator<K, V>
 * @template-extends ArrayIterator<K, V>
 */
class RecursiveArrayIterator extends ArrayIterator implements RecursiveIterator
{
    const STD_PROP_LIST = 1;
    const ARRAY_AS_PROPS = 2;
    const CHILD_ARRAYS_ONLY = 4;

    /**
     * @return ?RecursiveArrayIterator<K, V>
     */
    public function getChildren() {}

    /**
     * Reports whether the element at the current position can itself be iterated as a nested array.
     * Called by RecursiveIteratorIterator to decide whether to descend into it via getChildren().
     *
     * @return bool
     */
    public function hasChildren() {}

    /**
     * Returns the value at the current iterator position. Called automatically by PHP on each pass
     * of a foreach loop. Returns null once the position has moved past the last element.
     *
     * @return V|null
     *
     * @ignore-nullable-return
     */
    public function current() {}

    /**
     * Returns the key at the current iterator position. Called automatically by PHP on each pass of
     * a foreach loop. Returns null once the position has moved past the last element.
     *
     * @return K|null
     *
     * @ignore-nullable-return
     */
    public function key() {}
}

class SplFileInfo implements Stringable
{
    /**
     * Creates a file info object for the given path. The file does not need to exist yet; nothing
     * is looked up on disk until another method is called.
     */
    public function __construct(string $filename) {}

    /**
     * Returns the directory portion of the path, without the filename and without a trailing slash.
     */
    public function getPath(): string {}

    /**
     * Returns the filename component of the path, including its extension but excluding the
     * directory.
     */
    public function getFilename(): string {}

    /**
     * Returns the file extension without the leading dot, or an empty string if the filename has
     * none.
     */
    public function getExtension(): string {}

    /**
     * Returns the filename component of the path, with $suffix stripped from the end when it
     * matches.
     */
    public function getBasename(string $suffix = ''): string {}

    /**
     * Returns the full path together with the filename, as it was passed to the constructor.
     */
    public function getPathname(): string {}

    /**
     * Returns the file's permission bits in the same raw form as the fileperms() function,
     * including the file-type bits, so the familiar octal permissions have to be masked out of the
     * low bits.
     *
     * @code
     * decoct($info->getPerms() & 0777)
     * @endcode
     */
    public function getPerms(): int|false {}

    /**
     * Returns the filesystem inode number of the file. Throws RuntimeException if the underlying
     * stat call fails.
     *
     * @throws RuntimeException
     */
    public function getInode(): int|false {}

    /**
     * Returns the size of the file in bytes. Throws RuntimeException if the underlying stat call
     * fails.
     *
     * @throws RuntimeException
     */
    public function getSize(): int|false {}

    /**
     * Returns the numeric user ID that owns the file, not a username. Throws RuntimeException if
     * the underlying stat call fails.
     *
     * @throws RuntimeException
     */
    public function getOwner(): int|false {}

    /**
     * Returns the numeric group ID that owns the file, not a group name. Throws RuntimeException if
     * the underlying stat call fails.
     *
     * @throws RuntimeException
     */
    public function getGroup(): int|false {}

    /**
     * Returns the time the file was last accessed, as a Unix timestamp. Throws RuntimeException if
     * the underlying stat call fails.
     *
     * @throws RuntimeException
     */
    public function getATime(): int|false {}

    /**
     * Returns the time the file's contents were last modified, as a Unix timestamp.
     */
    public function getMTime(): int|false {}

    /**
     * Returns the time the file's inode metadata (permissions, ownership, or link count) last
     * changed, as a Unix timestamp; on Unix this is not the file's creation time. Throws
     * RuntimeException if the underlying stat call fails.
     *
     * @throws RuntimeException
     */
    public function getCTime(): int|false {}

    /**
     * Returns a short string describing the type of entry, such as file, dir, or link. Throws
     * RuntimeException if the underlying stat call fails.
     *
     * @throws RuntimeException
     */
    public function getType(): string|false {}

    /**
     * Returns whether the file is writable by the current process.
     */
    public function isWritable(): bool {}

    /**
     * Returns whether the file is readable by the current process.
     */
    public function isReadable(): bool {}

    /**
     * Returns whether the file is executable by the current process.
     */
    public function isExecutable(): bool {}

    /**
     * Returns whether the entry is a regular file, as opposed to a directory, link, or other
     * special type.
     */
    public function isFile(): bool {}

    /**
     * Returns whether the entry is a directory.
     */
    public function isDir(): bool {}

    /**
     * Returns whether the entry is a symbolic link.
     */
    public function isLink(): bool {}

    /**
     * Returns the path that a symbolic link points to. Throws RuntimeException if the underlying
     * system call fails, for example when the entry is not actually a link.
     *
     * @throws RuntimeException
     */
    public function getLinkTarget(): string|false {}

    /**
     * Returns the absolute path to the file with symbolic links and any . or .. segments resolved,
     * or false if the path does not exist.
     */
    public function getRealPath(): string|false {}

    /**
     * Returns a new file info object for the same path, instantiating $class if given, or otherwise
     * the class previously set with setInfoClass() (SplFileInfo by default).
     *
     * @template T of SplFileInfo
     *
     * @param null|class-string<T> $class
     *
     * @return ($class is null ? SplFileInfo : T)
     */
    public function getFileInfo(?string $class = null): SplFileInfo {}

    /**
     * Returns a new file info object for the file's containing directory, or null if it has none,
     * instantiating $class if given, or otherwise the class set with setInfoClass().
     *
     * @template T of SplFileInfo
     *
     * @param null|class-string<T> $class
     *
     * @return ($class is null ? null|SplFileInfo : null|T)
     */
    public function getPathInfo(?string $class = null): ?SplFileInfo {}

    /**
     * Opens the file and returns it as a file object, using the same mode string as fopen(). The
     * returned instance is SplFileObject unless changed with setFileClass(). Throws
     * RuntimeException if the file cannot be opened.
     *
     * @param null|resource $context
     *
     * @throws RuntimeException
     */
    public function openFile(string $mode = 'r', bool $useIncludePath = false, $context = null): SplFileObject {}

    /**
     * Sets the class that openFile() instantiates for this object in place of SplFileObject.
     * Mutates the receiver and returns nothing.
     *
     * @template T of SplFileInfo
     *
     * @param class-string<T> $class
     */
    public function setFileClass(string $class = SplFileObject::class): void {}

    /**
     * Sets the class that getFileInfo() and getPathInfo() instantiate for this object in place of
     * SplFileInfo. Mutates the receiver and returns nothing.
     *
     * @template T of SplFileInfo
     *
     * @param class-string<T> $class
     */
    public function setInfoClass(string $class = SplFileInfo::class): void {}

    /**
     * Returns the full path and filename as a string, the same value as getPathname(). Called
     * automatically whenever the object is used in a string context.
     *
     * @return string
     */
    public function __toString(): string {}

    final public function _bad_state_ex(): void {}

    public function __wakeup() {}

    /**
     * Returns the data shown when the object is passed to var_dump(). Called automatically by PHP
     * and has no effect on normal property access.
     */
    public function __debugInfo(): array {}
}

/**
 * @template-implements SeekableIterator<int, DirectoryIterator>
 */
class DirectoryIterator extends SplFileInfo implements SeekableIterator
{
    public function __construct(string $path) {}

    /**
     * Returns the current directory entry. Called automatically by PHP's foreach when the object is
     * iterated.
     *
     * @return null|DirectoryIterator
     *
     * @ignore-nullable-return
     */
    public function current() {}

    /**
     * Returns the zero-based position of the current entry within the directory listing. Called
     * automatically by PHP's foreach when the object is iterated.
     *
     * @return null|int
     *
     * @ignore-nullable-return
     */
    public function key() {}

    /**
     * Advances to the next directory entry, mutating the iterator's internal position. Called
     * automatically by PHP after each step of a foreach loop; returns nothing.
     *
     * @return void
     */
    public function next() {}

    /**
     * Resets the directory listing back to the first entry, mutating the iterator's internal
     * position. Called automatically by PHP before a foreach loop begins; returns nothing.
     *
     * @return void
     */
    public function rewind() {}

    /**
     * Moves the iterator directly to the entry at the given zero-based position, mutating its
     * internal cursor. Check valid() afterwards, since the offset may be beyond the last entry.
     *
     * @param int $offset
     */
    public function seek($offset) {}

    /**
     * Returns whether the current position refers to an actual directory entry. Called
     * automatically by PHP to decide whether a foreach loop should continue.
     *
     * @return bool
     */
    public function valid() {}

    /**
     * Returns whether the current entry is the . or .. pseudo-directory entry.
     */
    public function isDot(): bool {}
}

/**
 * @implements Iterator<string, string|SplFileInfo>
 */
class FilesystemIterator extends DirectoryIterator implements Iterator
{
    const CURRENT_AS_PATHNAME = 32;
    const CURRENT_AS_FILEINFO = 0;
    const CURRENT_AS_SELF = 16;
    const CURRENT_MODE_MASK = 240;
    const KEY_AS_PATHNAME = 0;
    const KEY_AS_FILENAME = 256;
    const FOLLOW_SYMLINKS = 512;
    const KEY_MODE_MASK = 3840;
    const NEW_CURRENT_AND_KEY = 256;
    const SKIP_DOTS = 4096;
    const UNIX_PATHS = 8192;

    public function __construct(
        string $path,
        int $flags = self::KEY_AS_PATHNAME | self::CURRENT_AS_FILEINFO | self::SKIP_DOTS,
    ) {}

    /**
     * Returns the current entry in the form selected by the CURRENT_* flags: an SplFileInfo object
     * when CURRENT_AS_FILEINFO is set (the default), the pathname string when CURRENT_AS_PATHNAME
     * is set, or the iterator object itself when CURRENT_AS_SELF is set.
     *
     * @return string|SplFileInfo|null
     *
     * @ignore-nullable-return
     */
    public function current(): string|SplFileInfo|null {}

    /**
     * Returns the key for the current entry: the full pathname by default, or just the filename
     * when the KEY_AS_FILENAME flag is set.
     *
     * @return string|null
     *
     * @ignore-nullable-return
     */
    public function key() {}

    /**
     * Returns the currently set flags bitmask that controls how current() and key() represent each
     * entry.
     *
     * @return int
     */
    public function getFlags() {}

    /**
     * Replaces the flags bitmask that controls how current() and key() represent each entry.
     * Mutates the receiver and returns nothing.
     *
     * @param int $flags
     *
     * @return void
     */
    public function setFlags($flags) {}
}

/**
 * @template-implements RecursiveIterator<string, RecursiveDirectoryIterator|string|SplFileInfo>
 * @template-implements SeekableIterator<string, RecursiveDirectoryIterator|string|SplFileInfo>
 */
class RecursiveDirectoryIterator extends FilesystemIterator implements RecursiveIterator, SeekableIterator
{
    const CURRENT_AS_PATHNAME = 32;
    const CURRENT_AS_FILEINFO = 0;
    const CURRENT_AS_SELF = 16;
    const CURRENT_MODE_MASK = 240;
    const KEY_AS_PATHNAME = 0;
    const KEY_AS_FILENAME = 256;
    const FOLLOW_SYMLINKS = 512;
    const KEY_MODE_MASK = 3840;
    const NEW_CURRENT_AND_KEY = 256;
    const SKIP_DOTS = 4096;
    const UNIX_PATHS = 8192;

    public function __construct(string $path, int $flags = self::KEY_AS_PATHNAME | self::CURRENT_AS_FILEINFO) {}

    public function getChildren(): RecursiveDirectoryIterator {}

    public function hasChildren(bool $allowLinks = false): bool {}

    public function getSubPath(): string {}

    public function getSubPathname(): string {}

    /**
     * @return RecursiveDirectoryIterator|string|SplFileInfo|null
     *
     * @ignore-nullable-return
     */
    public function current() {}

    /**
     * Returns the key for the current entry: the full pathname by default, or just the filename
     * when the KEY_AS_FILENAME flag is set.
     *
     * @return string|null
     *
     * @ignore-nullable-return
     */
    public function key() {}
}

/**
 * @template-extends FilesystemIterator<string, GlobIterator|SplFileInfo|string>
 */
class GlobIterator extends FilesystemIterator implements Countable
{
    /**
     * @return int<0, max>
     */
    public function count() {}
}

class SplFileObject extends SplFileInfo implements RecursiveIterator, SeekableIterator
{
    public const DROP_NEW_LINE = 1;
    public const READ_AHEAD = 2;
    public const SKIP_EMPTY = 4;
    public const READ_CSV = 8;

    /**
     * Opens the file at filename using the given fopen-style mode, optionally searching the include
     * path. Throws RuntimeException if the file cannot be opened, for example if it does not exist,
     * and LogicException if filename names a directory.
     *
     * @param resource|null $context
     *
     * @throws RuntimeException
     * @throws LogicException
     */
    public function __construct(string $filename, string $mode = 'r', bool $useIncludePath = false, $context = null) {}

    /**
     * Rewinds the file pointer to the first line and reads it, ready for iteration. PHP calls this
     * at the start of a foreach loop. Throws RuntimeException if the file cannot be rewound.
     *
     * @throws RuntimeException
     */
    public function rewind(): void {}

    /**
     * Returns true once the file pointer has reached the end of the file.
     */
    public function eof(): bool {}

    /**
     * Reports whether the current position still has a line available. PHP calls this after
     * rewind() and next() to decide whether a foreach loop should continue, and it returns false
     * once end of file is reached.
     */
    public function valid(): bool {}

    /**
     * Reads and returns the next line from the file, including its trailing newline, advancing the
     * file pointer past it. Throws RuntimeException on failure.
     *
     * @throws RuntimeException
     */
    public function fgets(): string {}

    /**
     * Reads up to length bytes from the current file position, counting bytes rather than
     * characters. Returns false on failure.
     */
    public function fread(int $length): string|false {}

    /**
     * Reads the current line and parses it as CSV, splitting fields on separator and unquoting
     * enclosure, returning the fields as an array. Returns false on failure or once end of file is
     * reached.
     *
     * @return array|false|null
     */
    public function fgetcsv(string $separator = ',', string $enclosure = "\"", string $escape = "\\") {}

    /**
     * Formats fields as one CSV line, using separator, enclosure and escape, and writes it to the
     * file. Returns the number of bytes written, or false on failure.
     */
    public function fputcsv(
        array $fields,
        string $separator = ',',
        string $enclosure = '"',
        string $escape = "\\",
        string $eol = PHP_EOL,
    ): int|false {}

    /**
     * Sets the delimiter, enclosure and escape characters used when a line is parsed as CSV,
     * whether by fgetcsv() or by the READ_CSV flag.
     */
    public function setCsvControl(string $separator = ',', string $enclosure = "\"", string $escape = "\\"): void {}

    /**
     * Returns the current CSV control characters as an indexed array of separator, enclosure and
     * escape, in that order.
     */
    public function getCsvControl(): array {}

    /**
     * Acquires or releases a portable advisory lock on the file, mirroring the standalone flock()
     * function: operation is LOCK_SH for a shared (read) lock, LOCK_EX for an exclusive (write)
     * lock, or LOCK_UN to release one, optionally combined with LOCK_NB to fail immediately rather
     * than block. When LOCK_NB is used and the lock could not be acquired, wouldBlock is set to
     * true. Returns false on failure.
     */
    public function flock(int $operation, int &$wouldBlock = null): bool {}

    /**
     * Flushes any buffered output to the underlying file. Returns true on success, or false on
     * failure.
     */
    public function fflush(): bool {}

    /**
     * Returns the current file pointer offset in bytes from the start of the file, or false on
     * failure.
     */
    public function ftell(): int|false {}

    /**
     * Moves the file pointer to a byte offset, interpreted relative to whence: SEEK_SET (the
     * default) from the start of the file, SEEK_CUR from the current position, or SEEK_END from the
     * end. Returns 0 on success and -1 on failure, unlike most of this class's other methods, which
     * return false.
     */
    public function fseek(int $offset, int $whence = SEEK_SET): int {}

    /**
     * Reads and returns a single byte from the file, advancing the pointer by one. Returns false at
     * end of file or on failure.
     */
    public function fgetc(): string|false {}

    /**
     * Reads from the current file position through to end of file and writes those bytes directly
     * to the output buffer. Returns the number of bytes output.
     */
    public function fpassthru(): int {}

    /**
     * Parses the current line against a sscanf-style format string. Called with only a format, it
     * returns the parsed values as an array. Called with additional variables passed by reference,
     * it assigns the parsed values to them instead and returns the number of values assigned as an
     * int.
     */
    public function fscanf(string $format, mixed &...$vars): array|int|null {}

    /**
     * Writes data to the file at the current position. If length is given and non-zero, writes at
     * most that many bytes rather than the whole string. Returns the number of bytes written, or
     * false on failure.
     */
    public function fwrite(string $data, int $length = 0): int|false {}

    /**
     * Returns statistics about the underlying file in the same shape as the standalone stat()
     * function: an array with both numeric and named keys, including size, mode, uid, gid, atime,
     * mtime and ctime.
     */
    public function fstat(): array {}

    /**
     * Truncates the file to size bytes, discarding any data beyond that point, or extending the
     * file with null bytes if it was shorter. Returns true on success, or false on failure.
     */
    public function ftruncate(int $size): bool {}

    /**
     * Returns the current line: a string, or an array of CSV fields when the READ_CSV flag is set.
     * PHP calls this inside a foreach loop to obtain the value. Returns false on failure.
     */
    public function current(): string|array|false {}

    /**
     * Returns the current line number, starting at 0. PHP calls this inside a foreach loop to
     * obtain the key.
     */
    public function key(): int {}

    /**
     * Reads the next line and advances the internal line counter, without returning the line
     * itself. PHP calls this at the end of each foreach iteration, after which current() and key()
     * reflect the new position.
     */
    public function next(): void {}

    /**
     * Sets the bitmask of flags controlling how lines are read: DROP_NEW_LINE strips the trailing
     * line ending from each line, READ_AHEAD reads a line during rewind()/next() rather than lazily
     * so eof() and valid() correctly reflect whether more data remains, SKIP_EMPTY skips empty
     * lines and requires READ_AHEAD to work correctly, and READ_CSV parses each line as CSV.
     */
    public function setFlags(int $flags): void {}

    /**
     * Returns the current bitmask of flags set by setFlags().
     */
    public function getFlags(): int {}

    /**
     * Sets the maximum number of bytes read per line by the line-reading methods; 0 means no limit.
     * Throws DomainException if maxLength is negative.
     *
     * @throws DomainException
     */
    public function setMaxLineLen(int $maxLength): void {}

    /**
     * Returns the maximum line length set by setMaxLineLen(), or 0 if unlimited.
     *
     * @return int<0, max>
     */
    public function getMaxLineLen(): int {}

    /**
     * Always returns false: a file's lines have no children, so this exists only to satisfy the
     * RecursiveIterator contract.
     *
     * @return bool
     */
    public function hasChildren() {}

    /**
     * Always returns null, since hasChildren() always reports false and PHP never calls this to
     * obtain children of a plain file.
     *
     * @return null|RecursiveIterator
     */
    public function getChildren() {}

    /**
     * Moves to the given 0-based line number by reading through the file; unlike fseek(), this is
     * not a constant-time operation. Throws LogicException if line is negative.
     *
     * @throws LogicException
     */
    public function seek(int $line): void {}

    /**
     * An alias for fgets(): reads and returns the next line from the file.
     */
    public function getCurrentLine(): string {}

    /**
     * Returns the current line of the file as a string, used when the object is read in a string
     * context such as echo or a string cast.
     */
    public function __toString(): string {}
}

class SplTempFileObject extends SplFileObject
{
    /**
     * Creates a file-like object backed by a temporary php://temp stream rather than a named file.
     * maxMemory is the number of bytes held in memory before the data spills to a temporary file on
     * disk: 0 forces the disk-backed temporary file immediately, and a negative value keeps
     * everything in memory with no disk spill. Throws RuntimeException if the temporary stream
     * cannot be opened.
     *
     * @throws RuntimeException
     */
    public function __construct(int $maxMemory = 2097152) {}
}

/**
 * @template V
 * @template-implements Iterator<int, V>
 * @template-implements ArrayAccess<int, V>
 */
class SplDoublyLinkedList implements Iterator, Countable, ArrayAccess, Serializable
{
    public const IT_MODE_LIFO = 2;
    public const IT_MODE_FIFO = 0;
    public const IT_MODE_DELETE = 1;
    public const IT_MODE_KEEP = 0;

    /**
     * @param V $value
     */
    public function add(int $index, mixed $value): void {}

    /**
     * Removes and returns the value at the end of the list, shrinking it by one. Throws
     * RuntimeException if the list is empty.
     *
     * @return V
     *
     * @throws RuntimeException
     */
    public function pop(): mixed {}

    /**
     * Removes and returns the value at the front of the list, shrinking it by one. Throws
     * RuntimeException if the list is empty.
     *
     * @return V
     *
     * @throws RuntimeException
     */
    public function shift(): mixed {}

    /**
     * Adds a value to the end of the list.
     *
     * @param V $value
     */
    public function push(mixed $value): void {}

    /**
     * Adds a value to the front of the list.
     *
     * @param V $value
     */
    public function unshift(mixed $value): void {}

    /**
     * Returns the value at the end of the list without removing it. Throws RuntimeException if the
     * list is empty.
     *
     * @return V
     *
     * @throws RuntimeException
     */
    public function top(): mixed {}

    /**
     * Returns the value at the front of the list without removing it. Throws RuntimeException if
     * the list is empty.
     *
     * @return V
     *
     * @throws RuntimeException
     */
    public function bottom(): mixed {}

    /**
     * Returns the number of elements currently in the list.
     */
    public function count(): int {}

    /**
     * Returns whether the list holds no elements.
     */
    public function isEmpty(): bool {}

    /**
     * Sets flags controlling how the list is iterated: one half chooses the traversal direction,
     * front-to-back (IT_MODE_FIFO) or back-to-front (IT_MODE_LIFO), and the other half chooses
     * whether visited elements are removed from the list (IT_MODE_DELETE) or left in place
     * (IT_MODE_KEEP). Combine one flag from each half with bitwise OR.
     *
     * @code
     * $list->setIteratorMode(SplDoublyLinkedList::IT_MODE_LIFO | SplDoublyLinkedList::IT_MODE_DELETE);
     * @endcode
     */
    public function setIteratorMode(int $mode): int {}

    /**
     * Returns the flags currently set by setIteratorMode(), combining the traversal-direction half
     * and the delete-or-keep half.
     */
    public function getIteratorMode(): int {}

    /**
     * ArrayAccess contract method that PHP calls for isset($list[$index]) and empty($list[$index]);
     * returns whether a node exists at that position.
     */
    public function offsetExists($index): bool {}

    /**
     * ArrayAccess contract method that PHP calls for $list[$index]; returns the value stored at
     * that position.
     *
     * @return V
     */
    public function offsetGet($index): mixed {}

    /**
     * ArrayAccess contract method that PHP calls for $list[$index] = $value. Passing null as the
     * index, as $list[] = $value does, pushes the value onto the end of the list instead of
     * replacing anything.
     *
     * @code
     * $list[] = $value; // pushes, same as $list->push($value);
     * @endcode
     *
     * @param V $value
     */
    public function offsetSet($index, mixed $value): void {}

    /**
     * ArrayAccess contract method that PHP calls for unset($list[$index]); removes the node at that
     * position and shifts every later element's index down by one.
     */
    public function offsetUnset($index): void {}

    /**
     * Iterator contract method that PHP calls at the start of a foreach, positioning the iterator
     * at the front of the list for FIFO mode or the back for LIFO mode.
     */
    public function rewind(): void {}

    /**
     * Iterator contract method that PHP calls to read the value at the iterator's current position.
     *
     * @return V
     */
    public function current(): mixed {}

    /**
     * Iterator contract method that PHP calls to read the numeric position of the iterator's
     * current element.
     */
    public function key(): int {}

    /**
     * Iterator contract method that advances the iterator by one position, toward the back in FIFO
     * mode or toward the front in LIFO mode. If the iterator mode includes IT_MODE_DELETE, the
     * element just left behind is also removed from the list, so a foreach in that mode consumes
     * it.
     */
    public function next(): void {}

    /**
     * Moves the iterator back one position, the opposite direction to next() under the current
     * iterator mode. It is not part of the Iterator interface; it exists for manual traversal
     * against the foreach direction.
     */
    public function prev(): void {}

    /**
     * Iterator contract method that PHP calls after rewind() or next() to decide whether the
     * current position still holds an element; a foreach loop ends once it returns false.
     */
    public function valid(): bool {}

    /**
     * Serializable contract method that rebuilds the list's elements from the string produced by
     * serialize(). Superseded in modern code by __unserialize().
     */
    public function unserialize(string $data): void {}

    /**
     * Serializable contract method that returns a string encoding the list's elements, for later
     * use by unserialize(). Superseded in modern code by __serialize().
     */
    public function serialize(): string {}

    /**
     * Magic method PHP calls to build the array that var_dump() displays for the object, rather
     * than dumping its internal representation directly.
     */
    public function __debugInfo(): array {}

    /**
     * Magic method PHP's serialize() calls to obtain the array of data representing the object's
     * state; the modern replacement for the Serializable interface's serialize().
     */
    public function __serialize(): array {}

    /**
     * Magic method PHP's unserialize() calls with the array __serialize() produced, to rebuild the
     * object's state.
     */
    public function __unserialize(array $data): void {}
}

/**
 * @template V
 */
class SplQueue extends SplDoublyLinkedList
{
    /**
     * @param V $value
     */
    public function enqueue(mixed $value): void {}

    /**
     * Removes and returns the value at the front of the queue, equivalent to shift(). Throws
     * RuntimeException if the queue is empty.
     *
     * @return V
     *
     * @throws RuntimeException
     */
    public function dequeue(): mixed {}

    /**
     * Sets flags controlling how the queue is iterated: one half chooses the traversal direction,
     * front-to-back or back-to-front, and the other half chooses whether visited elements are
     * removed from the queue or left in place. Combine one flag from each half with bitwise OR.
     *
     * @param int $mode
     *
     * @return void
     */
    public function setIteratorMode($mode) {}
}

/**
 * @template V
 * @template-extends SplDoublyLinkedList<V>
 */
class SplStack extends SplDoublyLinkedList
{
    /**
     * @param int $mode
     *
     * @return void
     */
    public function setIteratorMode($mode) {}
}

/**
 * @template V
 * @template-implements Iterator<int, V>
 */
abstract class SplHeap implements Iterator, Countable
{
    /**
     * @return V
     */
    public function extract(): mixed {}

    /**
     * Adds a value to the heap, reordering it according to compare() so the highest-priority
     * element stays at the top. Always returns true.
     *
     * @param V $value
     */
    public function insert(mixed $value): bool {}

    /**
     * Returns the value at the top of the heap, the highest-priority element according to
     * compare(), without removing it. Throws RuntimeException if the heap is empty.
     *
     * @return V
     *
     * @throws RuntimeException
     */
    public function top(): mixed {}

    /**
     * Returns the number of elements currently in the heap.
     */
    public function count(): int {}

    /**
     * Returns whether the heap holds no elements.
     */
    public function isEmpty(): bool {}

    /**
     * Iterator contract method that PHP calls at the start of a foreach, before the first current()
     * or valid() check.
     */
    public function rewind(): void {}

    /**
     * Iterator contract method that returns the element at the top of the heap, the same value
     * top() would return, without removing it. Iterating a heap with foreach consumes it: each step
     * to the next element extracts the top, so the heap is empty once the loop ends.
     *
     * @return V
     */
    public function current(): mixed {}

    /**
     * Iterator contract method that returns a numeric position for the current element.
     */
    public function key(): int {}

    /**
     * Iterator contract method that extracts the element at the top of the heap, exactly as
     * extract() would. This is what makes a foreach over a heap destructive: the heap is empty by
     * the time the loop finishes.
     */
    public function next(): void {}

    /**
     * Iterator contract method that PHP calls after rewind() or next() to decide whether to
     * continue a foreach; true while the heap still holds elements, false once it has been fully
     * extracted.
     */
    public function valid(): bool {}

    /**
     * Clears the heap's corrupted flag. insert() and extract() set that flag if the compare()
     * callback throws an exception partway through reordering the heap, and further heap operations
     * are refused while it is set.
     */
    public function recoverFromCorruption(): bool {}

    /**
     * Decides the relative priority of two values for heap ordering: return a positive number if
     * $value1 belongs nearer the top than $value2, negative if the reverse, or zero if they are
     * equivalent, the same convention as a usort() callback. insert() and extract() call this
     * automatically to keep the heap ordered; it is not meant to be called directly.
     *
     * @param V $value1
     * @param V $value2
     *
     * @return int
     */
    abstract protected function compare($value1, $value2);

    /**
     * Returns whether the heap is currently in the corrupted state that a failing compare() call
     * sets.
     */
    public function isCorrupted(): bool {}

    /**
     * Magic method PHP calls to build the array that var_dump() displays for the object.
     */
    public function __debugInfo(): array {}
}

/**
 * @template V
 * @template-extends SplHeap<V>
 */
class SplMinHeap extends SplHeap
{
    /**
     * Orders the heap so the smallest value settles at the top: returns a positive number when
     * $value1 is smaller than $value2, negative when it is larger, zero when they are equivalent,
     * the reverse of a natural usort() comparison. Called automatically by insert() and extract();
     * not meant to be called directly.
     *
     * @param V $value1
     * @param V $value2
     *
     * @return int
     */
    protected function compare(mixed $value1, mixed $value2): int {}

    /**
     * Removes and returns the smallest value in the heap, reducing its size by one. Throws
     * RuntimeException if the heap is empty.
     *
     * @return V
     *
     * @throws RuntimeException
     */
    public function extract() {}

    /**
     * Adds a value to the heap, reordering it so the smallest value settles at the top. Always
     * returns true.
     *
     * @param V $value
     *
     * @return true
     */
    public function insert($value) {}

    /**
     * Returns the smallest value in the heap without removing it. Throws RuntimeException if the
     * heap is empty.
     *
     * @return V
     *
     * @throws RuntimeException
     */
    public function top() {}

    /**
     * Returns the number of elements currently in the heap.
     *
     * @return int
     */
    public function count() {}

    /**
     * Returns whether the heap holds no elements.
     *
     * @return bool
     */
    public function isEmpty() {}

    /**
     * Iterator contract method that PHP calls at the start of a foreach, before the first current()
     * or valid() check.
     *
     * @return void
     */
    public function rewind() {}

    /**
     * Iterator contract method that returns the smallest remaining value, the same one top() would
     * return, without removing it. Iterating with foreach consumes the heap, since each step to the
     * next element extracts the top.
     *
     * @return V
     */
    public function current() {}

    /**
     * Iterator contract method that returns a numeric position for the current element.
     *
     * @return int
     */
    public function key() {}

    /**
     * Iterator contract method that extracts the smallest remaining value, exactly as extract()
     * would, which is what makes a foreach over the heap destructive.
     *
     * @return void
     */
    public function next() {}

    /**
     * Iterator contract method that PHP calls after rewind() or next() to decide whether to
     * continue a foreach; true while the heap still holds elements.
     *
     * @return bool
     */
    public function valid() {}

    /**
     * Clears the heap's corrupted flag, set by insert() or extract() when the compare() callback
     * throws partway through reordering the heap.
     *
     * @return void
     */
    public function recoverFromCorruption() {}
}

/**
 * @template V
 * @template-extends SplHeap<V>
 */
class SplMaxHeap extends SplHeap
{
    /**
     * Determines the heap ordering used internally by insert() and extract(): PHP's heap algorithm
     * calls this to decide whether one element should rank above another. Returns a positive number
     * when the first element should surface before the second, negative when it should surface
     * after, and zero when they are equivalent -- the same convention as the spaceship operator,
     * which is what makes this a max-heap: the greatest element is always extracted first.
     *
     * @param V $value1
     * @param V $value2
     *
     * @return int
     */
    protected function compare(mixed $value1, mixed $value2): int {}
}

/**
 * @template TPriority
 * @template V
 *
 * @template-implements Iterator<int, V>
 */
class SplPriorityQueue implements Iterator, Countable
{
    public const EXTR_BOTH = 3;
    public const EXTR_PRIORITY = 2;
    public const EXTR_DATA = 1;

    /**
     * @param TPriority $priority1
     * @param TPriority $priority2
     */
    public function compare(mixed $priority1, mixed $priority2): int {}

    /**
     * Adds $value to the queue with $priority determining extraction order, and always returns
     * true. Note the argument order: the value comes first and the priority second. Elements with a
     * higher priority are extracted before elements with a lower one.
     *
     * @param V $value
     * @param TPriority $priority
     */
    public function insert(mixed $value, mixed $priority): true {}

    /**
     * Controls what current(), top() and extract() hand back afterwards:
     * SplPriorityQueue::EXTR_DATA returns the value alone (the default),
     * SplPriorityQueue::EXTR_PRIORITY returns the priority alone, and SplPriorityQueue::EXTR_BOTH
     * returns both together as an array with 'data' and 'priority' keys.
     *
     * @code
     * $queue->setExtractFlags(SplPriorityQueue::EXTR_BOTH);
     * $top = $queue->top(); // ['data' => ..., 'priority' => ...]
     * @endcode
     */
    public function setExtractFlags(int $flags): int {}

    /**
     * Returns the element at the front of the queue -- the one with the highest priority -- without
     * removing it. The shape of the value follows whatever setExtractFlags() last set: the value
     * alone by default, the priority alone, or an array with 'data' and 'priority' keys for
     * EXTR_BOTH. Throws RuntimeException if the queue is empty.
     *
     * @return V
     *
     * @throws RuntimeException
     */
    public function top(): mixed {}

    /**
     * Removes and returns the element at the front of the queue -- the one with the highest
     * priority. As with top(), the shape of the returned value follows the extract flags set via
     * setExtractFlags(). Throws RuntimeException if the queue is empty.
     *
     * @return V
     *
     * @throws RuntimeException
     */
    public function extract(): mixed {}

    /**
     * Implements Countable::count(), called automatically when the queue is passed to count().
     * Returns the number of elements currently in the queue.
     */
    public function count(): int {}

    /**
     * Returns true if the queue holds no elements, false otherwise.
     */
    public function isEmpty(): bool {}

    /**
     * Implements Iterator::rewind(), called by PHP at the start of a foreach loop. Because
     * iterating a priority queue extracts elements one by one, this has no effect: it cannot
     * restore anything already extracted, so the queue can only be iterated through once.
     */
    public function rewind(): void {}

    /**
     * Implements Iterator::current(), called by PHP at the start of each loop iteration. Returns
     * the same element top() would return, without removing it; its shape likewise follows the
     * extract flags set via setExtractFlags().
     *
     * @return V
     */
    public function current(): mixed {}

    /**
     * Implements Iterator::key(), called by PHP during a foreach loop. Returns an integer
     * identifying the current element's position; because extracting from a priority queue is
     * destructive, this position is not a stable index and is unrelated to insertion order.
     *
     * @return int
     */
    public function key(): int {}

    /**
     * Implements Iterator::next(), called by PHP after each loop body executes. Extracts and
     * discards the front element, which is what actually advances the queue during a foreach loop.
     *
     * @return void
     */
    public function next(): void {}

    /**
     * Implements Iterator::valid(), called by PHP to decide whether to continue a foreach loop.
     * Returns true while the queue still holds at least one element.
     *
     * @return bool
     */
    public function valid(): bool {}

    /**
     * Clears the corrupted state left when an earlier compare() call threw an exception during a
     * heap operation, so the queue can be used again, and always returns true. It does not repair
     * the queue's internal ordering, which may remain inconsistent from the earlier failure.
     */
    public function recoverFromCorruption(): true {}

    /**
     * Returns true if an earlier compare() call threw an exception during a heap operation and left
     * the queue's internal ordering inconsistent, which blocks further heap operations until
     * recoverFromCorruption() is called.
     */
    public function isCorrupted(): bool {}

    /**
     * Returns the extraction mode currently set by setExtractFlags(): one of
     * SplPriorityQueue::EXTR_DATA, EXTR_PRIORITY or EXTR_BOTH.
     */
    public function getExtractFlags(): int {}

    /**
     * Supplies the data var_dump() displays for this object. PHP calls this automatically; it is
     * not meant to be called directly.
     */
    public function __debugInfo(): array {}
}

/**
 * @template V
 *
 * @template-implements Iterator<int, V>
 * @template-implements ArrayAccess<int, V>
 * @template-implements IteratorAggregate<int, V>
 */
class SplFixedArray implements Iterator, ArrayAccess, Countable, IteratorAggregate, JsonSerializable
{
    public function __construct(int $size = 0) {}

    /**
     * @return int<0, max>
     */
    public function count(): int {}

    /**
     * Returns the fixed array's elements as an ordinary indexed array, in the same order, without
     * modifying the fixed array itself.
     *
     * @return list<V>
     */
    public function toArray(): array {}

    /**
     * Builds a new SplFixedArray from a plain PHP array. When $preserveKeys is true (the default),
     * each element keeps the index it had in $array, so a gap or a non-sequential integer key
     * leaves the corresponding slot null; when false, elements are renumbered sequentially starting
     * at 0.
     *
     * @code
     * SplFixedArray::fromArray([2 => 'a', 5 => 'b'], false); // size 2, elements 'a' then 'b'
     * @endcode
     */
    public static function fromArray(array $array, bool $preserveKeys = true): SplFixedArray {}

    /**
     * Returns the fixed array's size -- the fixed number of slots it holds, the same value count()
     * returns.
     *
     * @return int<0, max>
     */
    public function getSize(): int {}

    /**
     * Changes the fixed array's size in place. Growing pads the new slots with null; shrinking
     * permanently discards the elements beyond the new size.
     *
     * @return bool
     */
    public function setSize(int $size) {}

    /**
     * Implements ArrayAccess::offsetExists(), called by PHP for isset($array[$index]) and
     * empty($array[$index]). Returns true only when $index falls within the array's bounds, from 0
     * up to (but not including) the array's size.
     *
     * @param int $index
     */
    public function offsetExists($index): bool {}

    /**
     * Implements ArrayAccess::offsetGet(), called by PHP to evaluate $array[$index]. Throws
     * RuntimeException if $index is outside the array's bounds.
     *
     * @param int $index
     *
     * @return V
     *
     * @throws RuntimeException
     */
    public function offsetGet($index): mixed {}

    /**
     * Implements ArrayAccess::offsetSet(), called by PHP for $array[$index] = $value. Throws
     * RuntimeException if $index is outside the array's bounds; unlike a native array, writing to
     * an out-of-range index never grows the array.
     *
     * @param int $index
     * @param V $value
     *
     * @throws RuntimeException
     */
    public function offsetSet($index, mixed $value): void {}

    /**
     * Implements ArrayAccess::offsetUnset(), called by PHP for unset($array[$index]). Resets the
     * slot to null rather than removing it or shifting later elements down, so the array's size is
     * unchanged.
     *
     * @param int $index
     */
    public function offsetUnset($index): void {}

    /**
     * Implements Iterator::rewind(), called by PHP at the start of a foreach loop. Moves the
     * internal read pointer back to index 0.
     *
     * @return void
     */
    public function rewind() {}

    /**
     * Implements Iterator::current(), called by PHP at the start of each loop iteration. Returns
     * the value at the internal pointer's current index.
     *
     * @return v
     */
    public function current() {}

    /**
     * Implements Iterator::key(), called by PHP during a foreach loop. Returns the internal
     * pointer's current index.
     *
     * @return int
     */
    public function key() {}

    /**
     * Implements Iterator::next(), called by PHP after each loop body executes. Advances the
     * internal pointer to the next index.
     *
     * @return void
     */
    public function next() {}

    /**
     * Implements Iterator::valid(), called by PHP to decide whether to continue a foreach loop.
     * Returns true while the internal pointer is still within the array's bounds.
     *
     * @return bool
     */
    public function valid(): bool {}

    /**
     * Restores the object's internal state when it is unserialized. PHP calls this automatically;
     * it is not meant to be called directly.
     */
    #[Deprecated('The function is deprecated', since: '8.4')]
    public function __wakeup(): void {}

    /**
     * Supplies the data used to serialize the object with serialize(). PHP calls this
     * automatically; it is not meant to be called directly.
     */
    public function __serialize(): array {}

    /**
     * Rebuilds the object's internal state from the data produced by __serialize(). PHP calls this
     * automatically during unserialize(); it is not meant to be called directly.
     */
    public function __unserialize(array $data): void {}

    /**
     * Implements IteratorAggregate::getIterator(), which PHP calls when the object is iterated
     * through that interface rather than through Iterator directly. Returns an iterator over the
     * array's elements.
     *
     * @return Iterator<int, V>
     */
    public function getIterator(): Iterator {}

    /**
     * Implements JsonSerializable::jsonSerialize(), called automatically by json_encode(). Returns
     * the array's elements as a plain indexed array, so the fixed array is rendered as a JSON array
     * rather than a JSON object.
     */
    public function jsonSerialize(): array {}
}

interface SplObserver
{
    /**
     * Part of the Observer pattern: implement this to react when an observed subject changes. PHP
     * never calls this itself -- SplSubject::notify() calls update() on every attached observer,
     * passing itself as $subject.
     */
    public function update(SplSubject $subject): void;
}

interface SplSubject
{
    /**
     * Part of the Observer pattern: registers $observer so it receives a call to update() on every
     * subsequent notify(). How and where attached observers are stored is left to the
     * implementation.
     */
    public function attach(SplObserver $observer): void;

    /**
     * Part of the Observer pattern: unregisters $observer so it no longer receives update() calls
     * from notify().
     */
    public function detach(SplObserver $observer): void;

    /**
     * Part of the Observer pattern: calls update() on every currently attached observer, passing
     * the subject itself as the argument.
     */
    public function notify(): void;
}

/**
 * @template O of object
 * @template V
 *
 * @template-implements SeekableIterator<int, O>
 * @template-implements ArrayAccess<O, V>
 */
class SplObjectStorage implements Countable, SeekableIterator, Serializable, ArrayAccess
{
    /**
     * Adds $object to the storage, associating it with $info as its data. If $object is already
     * stored, this replaces the previously associated data rather than adding a duplicate entry.
     *
     * @param O $object
     * @param V $info
     */
    public function attach(object $object, mixed $info = null): void {}

    /**
     * Removes $object, and its associated data, from the storage. Does nothing if $object was never
     * attached.
     *
     * @param O $object
     *
     * @return void
     */
    public function detach(object $object): void {}

    /**
     * Returns whether $object is currently stored in this SplObjectStorage.
     *
     * @param O $object
     */
    public function contains(object $object): bool {}

    /**
     * Adds every object, together with its associated data, from $storage into this storage. An
     * object already present in this storage has its data overwritten by the value from $storage.
     * $storage itself is left unchanged.
     *
     * @param SplObjectStorage<O, V> $storage
     */
    public function addAll(SplObjectStorage $storage): int {}

    /**
     * Removes every object from this storage that is also present in $storage. $storage itself is
     * left unchanged.
     *
     * @param SplObjectStorage<O, V> $storage
     */
    public function removeAll(SplObjectStorage $storage): int {}

    /**
     * Keeps only the objects that are also present in $storage, removing everything else from this
     * storage. $storage itself is left unchanged.
     *
     * @param SplObjectStorage<O, V> $storage
     */
    public function removeAllExcept(SplObjectStorage $storage): int {}

    /**
     * Returns the data associated with the object at the current iterator position. Only meaningful
     * while valid() is true.
     *
     * @return V
     */
    public function getInfo(): mixed {}

    /**
     * Replaces the data associated with the object at the current iterator position. Only
     * meaningful while valid() is true.
     *
     * @param V $info
     */
    public function setInfo(mixed $info): void {}

    /**
     * Returns the number of objects held in the storage. The $mode parameter exists only for
     * compatibility with the Countable interface and has no effect, since the storage has no nested
     * structure to count recursively.
     *
     * @param int $mode
     *
     * @return int
     */
    public function count(int $mode = COUNT_NORMAL): int {}

    /**
     * Moves the iterator to the first object in the storage. Called by PHP at the start of a
     * foreach loop.
     *
     * @return void
     */
    public function rewind(): void {}

    /**
     * Returns whether the current iterator position points at an object. Called by PHP to decide
     * whether a foreach loop should continue.
     *
     * @return bool
     */
    public function valid(): bool {}

    /**
     * Returns the numeric index of the current iterator position; this is a position in iteration
     * order, not a property of the object itself.
     *
     * @return int
     */
    public function key(): int {}

    /**
     * Returns the object at the current iterator position. Called by PHP to obtain the value in a
     * foreach loop.
     *
     * @return O
     */
    public function current(): object {}

    /**
     * Moves the iterator to the next object in the storage. Called by PHP after each iteration of a
     * foreach loop.
     *
     * @return void
     */
    public function next(): void {}

    /**
     * Restores the storage's objects and their associated data from a string previously produced by
     * serialize().
     */
    public function unserialize(string $data): void {}

    /**
     * Returns a string encoding the stored objects and their associated data, for later restoration
     * with unserialize().
     */
    public function serialize(): string {}

    /**
     * Called by PHP for isset($storage[$object]) and empty($storage[$object]). Returns whether
     * $object is currently attached, equivalent to contains($object).
     *
     * @param O $object
     */
    public function offsetExists($object): bool {}

    /**
     * Called by PHP for $storage[$object] = $info. Attaches $object with the given $info,
     * equivalent to attach($object, $info), overwriting any data already stored for $object.
     *
     * @param O $object
     * @param V $info
     *
     * @return void
     */
    public function offsetSet(mixed $object, mixed $info = null): void {}

    /**
     * Called by PHP for unset($storage[$object]). Detaches $object, equivalent to detach($object).
     * Does nothing if $object was never attached.
     *
     * @param O $object
     */
    public function offsetUnset($object): void {}

    /**
     * Called by PHP for $storage[$object]. Returns the data associated with $object. Throws
     * UnexpectedValueException if $object is not stored.
     *
     * @param O $object
     *
     * @return V
     *
     * @throws UnexpectedValueException
     */
    public function offsetGet($object): mixed {}

    /**
     * Returns the hash SplObjectStorage uses internally to identify and index $object, based on the
     * object's identity rather than its properties. Intended to be overridden in a subclass to
     * change how objects are compared within the storage.
     *
     * @param O $object
     */
    public function getHash(object $object): string {}

    /**
     * Returns an array representation of the storage's objects and their associated data. Called by
     * PHP's serialize() function in preference to the serialize() method, when both are defined.
     */
    public function __serialize(): array {}

    /**
     * Restores the storage's objects and data from the array produced by __serialize(). Called by
     * PHP's unserialize() function in preference to the unserialize() method, when both are
     * defined.
     */
    public function __unserialize(array $data): void {}

    /**
     * Returns the array of values shown for this object by var_dump(), letting the dump present the
     * stored objects and data instead of the storage's internal properties.
     */
    public function __debugInfo(): array {}

    /**
     * Moves the iterator to the entry at the given zero-based $offset. Throws OutOfRangeException
     * if $offset is not a valid position.
     *
     * @throws OutOfRangeException
     */
    public function seek(int $offset): void {}
}

class MultipleIterator implements Iterator
{
    public const MIT_NEED_ANY = 0;
    public const MIT_NEED_ALL = 1;
    public const MIT_KEYS_NUMERIC = 0;
    public const MIT_KEYS_ASSOC = 2;

    /**
     * Creates a MultipleIterator that iterates several attached Iterator instances together,
     * advancing and reading them as one group rather than one after another. $flags controls two
     * independent choices: MIT_NEED_ALL requires every attached iterator to be valid for the
     * combined iterator to be valid, whereas MIT_NEED_ANY needs only one of them; MIT_KEYS_NUMERIC
     * makes key() return numeric indices, whereas MIT_KEYS_ASSOC makes it return the string
     * supplied to attachIterator().
     */
    public function __construct(int $flags = MultipleIterator::MIT_NEED_ALL | MultipleIterator::MIT_KEYS_NUMERIC) {}

    /**
     * Returns the current bitmask of MIT_NEED_* and MIT_KEYS_* flags governing this iterator's
     * behaviour.
     */
    public function getFlags(): int {}

    /**
     * Replaces the bitmask of MIT_NEED_* and MIT_KEYS_* flags governing this iterator's behaviour,
     * the same flags accepted by the constructor.
     */
    public function setFlags(int $flags): void {}

    /**
     * Attaches $iterator so that it is iterated alongside any others already attached. If $info is
     * a string, it becomes that iterator's key in key() and current() when the MIT_KEYS_ASSOC flag
     * is set; if omitted, a numeric index is used instead.
     *
     * @code
     * $multi = new MultipleIterator(MultipleIterator::MIT_KEYS_ASSOC);
     * $multi->attachIterator($names, 'name');
     * $multi->attachIterator($ages, 'age');
     * @endcode
     */
    public function attachIterator(Iterator $iterator, int|string|null $info = null): void {}

    /**
     * Removes $iterator from the set of iterators being iterated together, if it was attached.
     */
    public function detachIterator(Iterator $iterator): void {}

    /**
     * Returns whether $iterator is currently attached to this MultipleIterator.
     */
    public function containsIterator(Iterator $iterator): bool {}

    /**
     * Returns how many Iterator instances are currently attached.
     */
    public function countIterators(): int {}

    /**
     * Rewinds every attached iterator to its first entry. Called by PHP at the start of a foreach
     * loop.
     */
    public function rewind(): void {}

    /**
     * Returns whether the current position is valid. With the MIT_NEED_ALL flag every attached
     * iterator must be valid; with MIT_NEED_ANY at least one must be. Called by PHP to decide
     * whether a foreach loop should continue.
     */
    public function valid(): bool {}

    /**
     * Returns an array combining the current key of every attached iterator, indexed numerically or
     * by the string given to attachIterator(), depending on whether the MIT_KEYS_NUMERIC or
     * MIT_KEYS_ASSOC flag is set. Called by PHP to obtain the key in a foreach loop.
     */
    public function key(): array {}

    /**
     * Returns an array combining the current value of every attached iterator, in the same key
     * arrangement as key(). Throws InvalidArgumentException if no iterators are attached, or
     * RuntimeException if the MIT_NEED_ALL flag is set and one of the attached iterators is not
     * currently valid.
     *
     * @throws RuntimeException
     * @throws InvalidArgumentException
     */
    public function current(): array {}

    /**
     * Advances every attached iterator to its next entry. Called by PHP after each iteration of a
     * foreach loop.
     */
    public function next(): void {}

    /**
     * Returns the array of values shown for this object by var_dump().
     */
    public function __debugInfo(): array {}
}

/**
 * Returns the names of PHP's built-in SPL classes and interfaces, useful mainly for enumeration or
 * membership checks against the SPL vocabulary rather than everyday use.
 *
 * @return list<class-string>
 */
function spl_classes(): array {}

/**
 * The default autoloader used by spl_autoload_register() when no custom function is supplied: it
 * searches the include path for a file named after class with one of the given file_extensions,
 * falling back to the extensions configured by the spl_autoload_extensions ini setting when
 * file_extensions is omitted.
 *
 * @param class-string $class
 */
function spl_autoload(string $class, ?string $file_extensions = null): void {}

/**
 * Gets or sets the comma-separated list of file extensions that the default spl_autoload
 * implementation searches for when it looks for a class file. Called with no argument it returns
 * the current list; called with one it replaces the list and returns the list now in effect.
 */
function spl_autoload_extensions(?string $file_extensions = null): string {}

/**
 * Appends $callback to the queue of autoloaders that PHP tries, in registration order, whenever
 * code references a class, interface, trait or enum that is not yet defined — or prepends it when
 * $prepend is true. Returns false if the callback could not be registered.
 *
 * @param null|(callable(class-string): void) $callback
 *
 * @throws TypeError
 */
function spl_autoload_register(?callable $callback = null, bool $throw = true, bool $prepend = false): bool {}

/**
 * Removes a callback previously registered with spl_autoload_register() from the autoloader queue.
 * Returns false if the given callback was never registered.
 *
 * @param null|(callable(class-string): void) $callback
 */
function spl_autoload_unregister(callable $callback): bool {}

/**
 * Returns the autoload callbacks currently registered with spl_autoload_register(), in the order
 * they were registered.
 *
 * @return list<(callable(class-string): void)>
 */
function spl_autoload_functions(): array {}

/**
 * Manually runs every autoload function registered with spl_autoload_register() in turn, trying to
 * load the given class, interface, enum or trait. PHP calls this automatically whenever an
 * unresolved class name is used; calling it directly retries autoloading even after an earlier
 * attempt has already failed.
 *
 * @param class-string $class
 */
function spl_autoload_call(string $class): void {}

/**
 * Returns a unique string identifier for an object instance, derived from its internal handle. The
 * identifier is only guaranteed unique while the object is alive; once it is garbage-collected, its
 * handle may be reused by a later object.
 *
 * @pure
 * @return non-falsy-string
 */
function spl_object_hash(object $object): string {}

/**
 * Returns an integer id that identifies a given object for as long as it exists. Once an object is
 * destroyed, its id can be reused by a later object, so the id is only meaningful while the object
 * it names is still alive.
 *
 * @pure
 */
function spl_object_id(object $object): int {}

/**
 * Converts a Traversable, or array, into an array. Keys are preserved by default, which silently
 * drops values when the iterator yields the same key more than once; pass false to get a plain
 * sequential list instead.
 *
 * @template K
 * @template V
 *
 * @param Traversable<K, V>|array<K, V> $iterator <p>
 *
 * @return ($preserve_keys is true ? array<K, V> : list<V>)
 */
function iterator_to_array(Traversable|array $iterator, bool $preserve_keys = true): array {}

/**
 * Counts the elements produced by iterating over $iterator, which for a plain array is simply its
 * element count. Because iterating is the only way to count a Traversable, this fully consumes a
 * Generator or any other non-rewindable iterator.
 *
 * @return int<0, max>
 */
function iterator_count(Traversable|array $iterator): int {}

/**
 * Repeatedly calls $callback, passing the elements of $args as its arguments, once for each
 * remaining position of $iterator, stopping early if $callback returns false. Returns how many
 * times it was called. $callback is not automatically given the iterator's current element; include
 * the iterator itself in $args to inspect it.
 */
function iterator_apply(Traversable $iterator, callable $callback, ?array $args = null): int {}

/**
 * Returns the ancestors of a class or object, ordered from its immediate parent up to the root
 * class, or false if the class does not exist. $autoload controls whether the class autoloader is
 * invoked when the class named, or the object's class, is not already loaded; with it disabled, an
 * unloaded class yields false instead of triggering autoloading.
 *
 * @param object|class-string $object_or_class
 *
 * @return array<class-string, class-string>|false
 *
 * @pure
 */
function class_parents($object_or_class, bool $autoload = true): array|false {}

/**
 * Returns the traits used directly in the body of the given class or object, keyed and valued by
 * their fully-qualified names. Traits used by a parent class are not included. Returns false, with
 * a warning, if the class does not exist and cannot be autoloaded.
 *
 * @param object|class-string $object_or_class
 *
 * @return array<trait-string, trait-string>|false
 *
 * @pure
 */
function class_uses($object_or_class, bool $autoload = true): array|false {}

/**
 * Returns every interface implemented by the given class or object, including those inherited
 * through parent classes and extended interfaces. $object_or_class may be an object instance or a
 * class name; $autoload controls whether an undeclared class is autoloaded before it is inspected.
 * Returns false if the class does not exist.
 *
 * @param object|class-string $object_or_class
 *
 * @return ($object_or_class is object ? array<interface-string, interface-string> : array<interface-string, interface-string>|false)
 *
 * @pure
 */
function class_implements($object_or_class, bool $autoload = true): array|false {}
