<?php

class ReflectionException extends Exception {}

interface Reflector extends Stringable
{
}

class Reflection
{
    /**
     * Converts a bitmask of reflection modifier constants, such as ReflectionMethod::IS_PUBLIC,
     * IS_PROTECTED, IS_PRIVATE, IS_STATIC, IS_ABSTRACT and IS_FINAL, into their names in a fixed,
     * canonical order, for building a readable signature or dump.
     *
     * @return list<string>
     */
    public static function getModifierNames(int $modifiers): array {}
}

/**
 * @template T of Fiber
 */
#[Mago\AvailableSince(80100)]
final class ReflectionFiber
{
    /**
     * Creates a reflection object for introspecting the given fiber's suspended execution state.
     *
     * @param T $fiber
     */
    public function __construct(Fiber $fiber) {}

    /**
     * Returns the Fiber instance this reflection object was created for.
     *
     * @return T
     *
     * @pure
     */
    public function getFiber(): Fiber {}

    /**
     * Returns the path of the file the fiber is currently suspended in, or null if the fiber has
     * not started or has already terminated.
     *
     * @pure
     */
    public function getExecutingFile(): ?string {}

    /**
     * Returns the line number the fiber is currently suspended at, or null if the fiber has not
     * started or has already terminated.
     *
     * @pure
     */
    public function getExecutingLine(): ?int {}

    /**
     * Returns the callable the fiber was constructed with.
     *
     * @pure
     */
    public function getCallable(): callable {}

    /**
     * Returns a backtrace of the fiber's suspended call stack, in the same format as
     * debug_backtrace(). $options accepts the same DEBUG_BACKTRACE_* flags as debug_backtrace(),
     * such as DEBUG_BACKTRACE_IGNORE_ARGS to omit function arguments.
     *
     * @return list<array<string, mixed>>
     *
     * @pure
     */
    public function getTrace(int $options = DEBUG_BACKTRACE_PROVIDE_OBJECT): array {}
}

final class ReflectionGenerator
{
    /**
     * Constructs a reflection for introspecting the current execution state of the given generator.
     */
    public function __construct(Generator $generator) {}

    /**
     * Returns the line number of the statement the generator is currently suspended at or
     * executing.
     *
     * @pure
     */
    public function getExecutingLine(): int {}

    /**
     * Returns the path of the file containing the code the generator is currently executing.
     *
     * @pure
     */
    public function getExecutingFile(): string {}

    /**
     * Returns a backtrace of the generator's currently suspended call stack, in the same array
     * format as debug_backtrace(). The options bitmask accepts the same DEBUG_BACKTRACE_* flags as
     * debug_backtrace(), such as DEBUG_BACKTRACE_IGNORE_ARGS to omit function arguments.
     *
     * @return list<array<string, mixed>>
     *
     * @pure
     */
    public function getTrace(int $options = DEBUG_BACKTRACE_PROVIDE_OBJECT): array {}

    /**
     * Returns a reflection of the function or method that the generator is an instance of.
     *
     * @pure
     */
    public function getFunction(): ReflectionFunctionAbstract {}

    /**
     * Returns the object bound to $this inside the generator, or null if the generator's function
     * is not a method with a bound object.
     *
     * @pure
     */
    public function getThis(): ?object {}

    /**
     * Returns the innermost generator actually executing code, following any yield from delegation
     * chain down from this generator.
     *
     * @pure
     */
    public function getExecutingGenerator(): Generator {}

    /**
     * Returns whether the generator has finished execution and can no longer be advanced.
     *
     * @pure
     */
    public function isClosed(): bool {}
}

class ReflectionExtension implements Reflector
{
    /**
     * @readonly
     */
    public string $name;

    /**
     * Constructs a reflection for the named loaded PHP extension. Throws ReflectionException if no
     * extension with that name is loaded.
     *
     * @throws ReflectionException
     */
    public function __construct(string $name) {}

    /**
     * Returns a formatted, multi-line textual summary of the extension, including its name,
     * version, and the functions, constants and INI entries it defines.
     */
    public function __toString(): string {}

    /**
     * Returns the extension's name, exactly as the extension itself declares it.
     *
     * @pure
     */
    public function getName(): string {}

    /**
     * Returns the extension's version string, or null if the extension does not report a version.
     *
     * @pure
     */
    public function getVersion(): ?string {}

    /**
     * Returns the functions declared by the extension, as an array mapping each function name to
     * its ReflectionFunction.
     *
     * @return array<string, ReflectionFunction>
     *
     * @pure
     */
    public function getFunctions(): array {}

    /**
     * Returns the constants declared by the extension, as an array mapping each constant name to
     * its value.
     *
     * @return array<string, mixed>
     *
     * @pure
     */
    public function getConstants(): array {}

    /**
     * Returns the php.ini directives belonging to the extension, as an array mapping each directive
     * name to its current setting.
     *
     * @return array<string, mixed>
     *
     * @pure
     */
    public function getINIEntries(): array {}

    /**
     * Returns the classes declared by the extension, as an array mapping each fully qualified class
     * name to its ReflectionClass.
     *
     * @return array<class-string, ReflectionClass>
     *
     * @pure
     */
    public function getClasses(): array {}

    /**
     * Returns the names of the classes declared by the extension, as a list of fully qualified
     * class names.
     *
     * @return list<class-string>
     *
     * @pure
     */
    public function getClassNames(): array {}

    /**
     * Returns the extension's dependencies, as an array mapping each dependency's name to a
     * description of the required relationship and version constraint.
     *
     * @return array<string, string>
     *
     * @pure
     */
    public function getDependencies(): array {}

    /**
     * Prints the same extension information block shown by phpinfo() directly to output, for this
     * extension only.
     */
    public function info(): void {}

    /**
     * Returns whether the extension was loaded once at PHP startup and stays loaded for the life of
     * the process, rather than being loaded and unloaded per request.
     *
     * @pure
     */
    public function isPersistent(): bool {}

    /**
     * Returns whether the extension was loaded temporarily during the request rather than at PHP
     * startup.
     *
     * @pure
     */
    public function isTemporary(): bool {}

    /**
     * Declared private to prevent cloning; cloning a ReflectionExtension directly is not supported.
     */
    private function __clone(): void {}
}

abstract class ReflectionType implements Stringable
{
    /**
     * Returns whether null is an acceptable value for the type: true when the type itself is
     * nullable, or, for a union type, when null is one of its member types.
     *
     * @pure
     */
    public function allowsNull(): bool {}

    /**
     * Returns the type exactly as it would be written in a type declaration, including a leading
     * question mark for a nullable named type and the | or & separators for a union or intersection
     * type.
     */
    public function __toString(): string {}

    /**
     * Declared private to prevent cloning; cloning a ReflectionType directly is not supported.
     */
    private function __clone(): void {}
}

class ReflectionNamedType extends ReflectionType
{
    /**
     * Returns the type's name, such as a scalar type keyword, self, static, or a class name. The
     * name never carries a leading question mark; use allowsNull() to check nullability separately.
     *
     * @code
     * // for a parameter declared ?string
     * $type->getName();     // 'string'
     * $type->allowsNull();  // true
     * @endcode
     *
     * @pure
     */
    public function getName(): string {}

    /**
     * Returns whether the type is one of PHP's builtin types (such as int, string, bool, array,
     * callable, iterable, object, mixed, void or never) rather than the name of a class, interface
     * or enum.
     *
     * @pure
     */
    public function isBuiltin(): bool {}
}

#[Mago\AvailableSince(80100)]
class ReflectionIntersectionType extends ReflectionType
{
    /**
     * Returns the individual types that make up the intersection, as a list in the order they were
     * declared.
     *
     * @return list<ReflectionType>
     *
     * @pure
     */
    public function getTypes(): array {}
}

class ReflectionUnionType extends ReflectionType
{
    /**
     * Returns the individual types that make up the union, as a list in the order they were
     * declared.
     *
     * @return list<ReflectionNamedType|ReflectionIntersectionType>
     *
     * @pure
     */
    public function getTypes(): array {}
}

class ReflectionZendExtension implements Reflector
{
    /**
     * @readonly
     */
    public string $name;

    /**
     * Constructs a reflection for the named loaded Zend extension. Throws ReflectionException if no
     * Zend extension with that name is loaded.
     *
     * @throws ReflectionException
     */
    public function __construct(string $name) {}

    /**
     * Returns a formatted textual summary of the Zend extension, including its name, version,
     * author, URL and copyright.
     */
    public function __toString(): string {}

    /**
     * Returns the Zend extension's name.
     *
     * @pure
     */
    public function getName(): string {}

    /**
     * Returns the Zend extension's version string.
     *
     * @pure
     */
    public function getVersion(): string {}

    /**
     * Returns the name of the Zend extension's author.
     *
     * @pure
     */
    public function getAuthor(): string {}

    /**
     * Returns the Zend extension's URL.
     *
     * @pure
     */
    public function getURL(): string {}

    /**
     * Returns the Zend extension's copyright notice.
     *
     * @pure
     */
    public function getCopyright(): string {}

    /**
     * Declared private to prevent cloning; cloning a ReflectionZendExtension directly is not
     * supported.
     */
    private function __clone(): void {}
}

/**
 * @template T of object
 */
class ReflectionAttribute implements Reflector
{
    /**
     * @var class-string<T>
     *
     * @readonly
     */
    public string $name;

    public const IS_INSTANCEOF = 2;

    private function __construct() {}

    /**
     * Returns a formatted textual summary of the attribute, including its class name and arguments.
     */
    public function __toString(): string {}

    /**
     * @return class-string<T>
     *
     * @pure
     */
    public function getName(): string {}

    /**
     * Returns a bitmask identifying what kind of declaration the attribute is attached to, made up
     * of the Attribute::TARGET_* constants such as TARGET_CLASS, TARGET_FUNCTION, TARGET_METHOD,
     * TARGET_PROPERTY, TARGET_CLASS_CONSTANT and TARGET_PARAMETER.
     *
     * @pure
     */
    public function getTarget(): int {}

    /**
     * Returns whether the same attribute class was applied more than once to the declaration this
     * attribute was reflected from.
     *
     * @pure
     */
    public function isRepeated(): bool {}

    /**
     * Returns the arguments the attribute was declared with, as an array keyed by parameter name
     * for named arguments and by position for positional arguments.
     *
     * @pure
     */
    public function getArguments(): array {}

    /**
     * Instantiates the attribute by calling its class's constructor with the arguments captured at
     * the declaration site, and returns the resulting object.
     *
     * @return T
     */
    public function newInstance(): object {}

    /**
     * Declared private to prevent cloning; cloning a ReflectionAttribute directly is not supported.
     */
    private function __clone(): void {}
}

/**
 * @template T of object = object
 */
class ReflectionClassConstant implements Reflector
{
    /**
     * @readonly
     */
    public string $name;

    /**
     * @var class-string<T>
     *
     * @readonly
     */
    public string $class;

    /**
     * @readonly
     */
    public bool $isFinal;

    public const IS_PUBLIC = 1;

    public const IS_PROTECTED = 2;

    public const IS_PRIVATE = 4;

    public const IS_FINAL = 5;

    /**
     * @param class-string<T>|T $class
     *
     * @throws ReflectionException
     */
    public function __construct(string|object $class, string $constant) {}

    /**
     * Returns the class in which this constant is actually declared, which may be an ancestor class
     * or an implemented interface rather than the class the reflection was constructed from.
     *
     * @pure
     */
    public function getDeclaringClass(): ReflectionClass {}

    /**
     * Returns the constant's doc comment exactly as written with its delimiters intact, or false if
     * the constant has no doc comment.
     *
     * @pure
     */
    public function getDocComment(): string|false {}

    /**
     * Returns a bitmask of the constant's visibility and finality flags, combining the IS_PUBLIC,
     * IS_PROTECTED, IS_PRIVATE and IS_FINAL constants declared on this class.
     *
     * @pure
     */
    public function getModifiers(): int {}

    /**
     * Returns the constant's own name, without any declaring class or interface prefix.
     *
     * @pure
     */
    public function getName(): string {}

    /**
     * Returns the value held by the constant.
     *
     * @pure
     */
    public function getValue(): mixed {}

    /**
     * Returns true if the constant is declared private, meaning it is visible only from within the
     * declaring class.
     *
     * @pure
     */
    public function isPrivate(): bool {}

    /**
     * Returns true if the constant is declared protected, meaning it is visible from the declaring
     * class and its subclasses.
     *
     * @pure
     */
    public function isProtected(): bool {}

    /**
     * Returns true if the constant is declared public, meaning it is visible from anywhere.
     *
     * @pure
     */
    public function isPublic(): bool {}

    /**
     * Returns a human-readable, formatted description of the constant, including its visibility and
     * value, in the style used elsewhere by the Reflection API's debugging output.
     */
    public function __toString(): string {}

    /**
     * Returns the attributes declared on the constant, each wrapped in a ReflectionAttribute.
     * Passing $name restricts the result to attributes of that exact class; adding
     * ReflectionAttribute::IS_INSTANCEOF to $flags instead matches attributes whose class is $name
     * or extends it. Returns an empty array if nothing matches.
     *
     * @code
     * $constant->getAttributes(SomeAttribute::class, ReflectionAttribute::IS_INSTANCEOF);
     * @endcode
     *
     * @template A
     *
     * @param class-string<A>|null $name
     * @param int $flags
     *
     * @return list<ReflectionAttribute<A>>
     *
     * @pure
     */
    public function getAttributes(?string $name = null, int $flags = 0): array {}

    /**
     * Prevents the reflection object from being cloned; calling clone on it throws an Error.
     */
    private function __clone(): void {}

    /**
     * Returns true if the reflected member is actually an enum case rather than an ordinary class
     * constant.
     *
     * @pure
     */
    #[Mago\AvailableSince(80100)]
    public function isEnumCase(): bool {}

    /**
     * Returns true if the constant is declared final and cannot be overridden by a subclass or
     * implementing class.
     *
     * @pure
     */
    #[Mago\AvailableSince(80100)]
    public function isFinal(): bool {}

    /**
     * Returns true if the constant declares an explicit type.
     *
     * @pure
     */
    #[Mago\AvailableSince(80300)]
    public function hasType(): bool {}

    /**
     * Returns the constant's declared type, or null if it has none.
     *
     * @pure
     */
    #[Mago\AvailableSince(80300)]
    public function getType(): ?ReflectionType {}

    /**
     * Returns true if the constant is marked deprecated.
     *
     * @pure
     */
    #[Mago\AvailableSince(80400)]
    public function isDeprecated(): bool {}
}

#[Mago\AvailableSince(80400)]
enum PropertyHookType: string
{
    case Get = 'get';
    case Set = 'set';
}

/**
 * @template T of object = object
 */
class ReflectionProperty implements Reflector
{
    public const IS_ABSTRACT = 64;

    public const IS_VIRTUAL = 512;

    public const IS_STATIC = 16;

    public const IS_PUBLIC = 1;

    public const IS_PROTECTED = 2;

    public const IS_PRIVATE = 4;

    public const IS_READONLY = 128;

    public const IS_PROTECTED_SET = 2048;

    public const IS_PRIVATE_SET = 4096;

    public const IS_FINAL = 32;

    /**
     * @readonly
     */
    public string $name;

    /**
     * @var class-string<T>
     *
     * @readonly
     */
    public string $class;

    /**
     * @param class-string<T>|T $class
     *
     * @throws ReflectionException
     */
    public function __construct(object|string $class, string $property) {}

    /**
     * Returns a human-readable description of the property, showing its modifiers, type and name,
     * intended for debugging output rather than for use as the property's own name or value.
     */
    public function __toString(): string {}

    /**
     * Returns the property's name on its own, without the declaring class or any namespace.
     *
     * @pure
     */
    public function getName(): string {}

    /**
     * Returns the property's current value: on the given object for an instance property, or on the
     * class itself for a static property when no object is given. An instance property requires a
     * matching object to be supplied.
     *
     * @pure
     */
    public function getValue(?object $object = null): mixed {}

    /**
     * Sets the property to a new value, writing directly to the given object rather than going
     * through a constructor or setter. For an instance property the object comes first and the
     * value second; for a static property, the value alone is passed and no object is needed. Since
     * PHP 8.1 this works on non-public properties without a prior call to setAccessible.
     *
     * @code
     * $prop->setValue($instance, 42);
     * $staticProp->setValue(42); // static property: value only, no object
     * @endcode
     */
    public function setValue(mixed $objectOrValue, mixed $value = UNKNOWN): void {}

    /**
     * Returns whether the property is declared public.
     *
     * @pure
     */
    public function isPublic(): bool {}

    /**
     * Returns whether the property is declared private.
     *
     * @pure
     */
    public function isPrivate(): bool {}

    /**
     * Returns whether the property is declared protected.
     *
     * @pure
     */
    public function isProtected(): bool {}

    /**
     * Returns whether the property is static, meaning it belongs to the class itself rather than to
     * individual instances.
     *
     * @pure
     */
    public function isStatic(): bool {}

    /**
     * Returns whether the property is one of the class's own declared properties, as opposed to one
     * added dynamically to a particular object at runtime.
     *
     * @pure
     */
    public function isDefault(): bool {}

    /**
     * Returns a bitmask of the property's modifiers, combining constants such as
     * ReflectionProperty::IS_PUBLIC, IS_PROTECTED, IS_PRIVATE, IS_STATIC and IS_READONLY.
     *
     * @pure
     */
    public function getModifiers(): int {}

    /**
     * Returns the class that actually declares the property, which may be an ancestor of the class
     * this reflection was obtained from when the property is inherited rather than overridden.
     *
     * @return ReflectionClass<T>
     *
     * @pure
     */
    public function getDeclaringClass(): ReflectionClass {}

    /**
     * Returns the property's doc comment exactly as written in the source, including its
     * delimiters, or false if the property has none.
     *
     * @pure
     */
    public function getDocComment(): string|false {}

    /**
     * Historically required before reading or writing a non-public property through reflection. Has
     * been a no-op since PHP 8.1, because reflection can access private and protected properties
     * without it.
     *
     * @pure
     */
    #[Deprecated(since: '8.5', message: 'as it has no effect')]
    public function setAccessible(bool $accessible): void {}

    /**
     * Returns the property's declared type: a plain named type, or a union or intersection type
     * depending on how it was declared, or null if the property has no type declaration.
     *
     * @pure
     */
    public function getType(): ReflectionNamedType|ReflectionUnionType|ReflectionIntersectionType|null {}

    /**
     * Returns whether the property has a declared type at all.
     *
     * @pure
     */
    public function hasType(): bool {}

    /**
     * Returns whether the property currently holds a value on the given object, or on the class
     * itself for a static property with no object given. This is a runtime check rather than a
     * declaration check: a typed property with no default value exists in an uninitialised state
     * until first assigned, and reading it in that state throws an Error rather than returning
     * null.
     *
     * @param ?T $object
     *
     * @pure
     */
    public function isInitialized(?object $object = null): bool {}

    /**
     * Returns whether the property was declared through constructor property promotion rather than
     * as an ordinary property declaration.
     *
     * @pure
     */
    public function isPromoted(): bool {}

    /**
     * Declared private to prevent the reflection object from being cloned; attempting to clone it
     * from outside the class throws an Error.
     */
    private function __clone(): void {}

    /**
     * Returns whether the property has a default value declared in the class, as distinct from one
     * only ever assigned at runtime.
     */
    public function hasDefaultValue(): bool {}

    /**
     * Returns the default value declared for the property, or null if it has none. Because an
     * actual declared default can itself be null, checking hasDefaultValue first is the only way to
     * tell the two cases apart.
     *
     * @pure
     */
    public function getDefaultValue(): mixed {}

    /**
     * Returns the attributes attached to the property, each wrapped in a ReflectionAttribute. With
     * name given, only attributes of that exact class are returned, unless flags includes
     * ReflectionAttribute::IS_INSTANCEOF, in which case attributes whose class is an instance of
     * name are matched too. Returns an empty list when nothing matches.
     *
     * @code
     * $prop->getAttributes(SomeInterface::class, ReflectionAttribute::IS_INSTANCEOF);
     * @endcode
     *
     * @template A
     *
     * @param class-string<A>|null $name
     * @param int $flags
     *
     * @return list<ReflectionAttribute<A>>
     *
     * @pure
     */
    public function getAttributes(?string $name = null, int $flags = 0): array {}

    /**
     * Returns whether the property is declared readonly, meaning it can be initialised only once,
     * from within the declaring class's own scope.
     */
    #[Mago\AvailableSince(80100)]
    public function isReadOnly(): bool {}

    /**
     * Returns the property's current value on the given object without triggering the object's lazy
     * initialiser.
     */
    #[Mago\AvailableSince(80400)]
    public function getRawValue(object $object): mixed {}

    /**
     * Sets the property's value on the given object without triggering the object's lazy
     * initialiser. Writes directly to the object passed in.
     */
    #[Mago\AvailableSince(80400)]
    public function setRawValue(object $object, mixed $value): void {}

    /**
     * Returns whether the property is declared abstract: its hooks are declared without an
     * implementation, and a subclass must supply one.
     */
    #[Mago\AvailableSince(80400)]
    public function isAbstract(): bool {}

    /**
     * Returns whether the property is virtual, meaning it has no storage of its own and every read
     * or write is handled entirely by its get and set hooks.
     */
    #[Mago\AvailableSince(80400)]
    public function isVirtual(): bool {}

    #[Mago\AvailableSince(80400)]
    public function getSettableType(): ?ReflectionType {}

    /**
     * Returns whether the property defines any hooks at all, get or set.
     */
    #[Mago\AvailableSince(80400)]
    public function hasHooks(): bool {}

    /**
     * Returns the hooks defined on the property, as an array keyed by PropertyHookType case, Get or
     * Set, with a ReflectionMethod value for each hook the property implements.
     */
    #[Mago\AvailableSince(80400)]
    public function getHooks(): array {}

    /**
     * Returns whether the property defines a hook of the given kind, get or set.
     */
    #[Mago\AvailableSince(80400)]
    public function hasHook(PropertyHookType $type): bool {}

    /**
     * Returns the ReflectionMethod for the property's hook of the given kind, or null if the
     * property does not define that hook.
     */
    #[Mago\AvailableSince(80400)]
    public function getHook(PropertyHookType $type): ?ReflectionMethod {}

    /**
     * Returns whether writes to the property are restricted to private scope, the asymmetric
     * visibility written as private(set), even though the property may be readable more widely.
     */
    #[Mago\AvailableSince(80400)]
    public function isPrivateSet(): bool {}

    /**
     * Returns whether writes to the property are restricted to protected scope, the asymmetric
     * visibility written as protected(set), meaning only the declaring class and its subclasses may
     * assign it, even though the property may be readable more widely.
     */
    #[Mago\AvailableSince(80400)]
    public function isProtectedSet(): bool {}

    /**
     * Sets the property's value directly on the given object, bypassing the object's lazy
     * initialiser entirely. Intended for use from inside the initialiser itself, where calling the
     * ordinary setter would trigger initialisation recursively.
     *
     * @param T $object
     */
    #[Mago\AvailableSince(80400)]
    public function setRawValueWithoutLazyInitialization(object $object, mixed $value): void {}

    /**
     * Marks the property as already initialised on the given lazy object, without assigning it a
     * value, so that reading or writing it afterwards does not invoke the object's initialiser.
     *
     * @param T $object
     */
    #[Mago\AvailableSince(80400)]
    public function skipLazyInitialization(object $object): void {}

    public function isDynamic(): bool {}

    /**
     * Returns whether the property is declared final, so that a subclass cannot redeclare it.
     */
    public function isFinal(): bool {}

    /**
     * Returns whether the property is still uninitialised on the given lazy object, such that
     * accessing it would trigger the object's initialiser.
     *
     * @param T $object
     */
    public function isLazy(object $object): bool {}

    /**
     * @param T $object
     */
    #[Mago\AvailableSince(80600)]
    public function isReadable(?object $object = null): bool {}

    /**
     * @param T $object
     */
    #[Mago\AvailableSince(80600)]
    public function isWriteable(?object $object = null): bool {}
}

final class ReflectionReference
{
    /**
     * Cannot be called directly; a ReflectionReference is only obtained through
     * ReflectionReference::fromArrayElement.
     */
    private function __construct() {}

    /**
     * Returns a ReflectionReference describing the array element at the given key, or null if that
     * key does not exist or the element there is not a reference. The array is taken by value
     * rather than by reference deliberately: passing it by reference would itself create a new
     * reference at that element, defeating the very check this method exists to perform.
     *
     * @code
     * $ref = ReflectionReference::fromArrayElement($array, 'key');
     * if ($ref !== null) { echo $ref->getId(); }
     * @endcode
     */
    public static function fromArrayElement(array $array, string|int $key): ?ReflectionReference {}

    /**
     * Returns an opaque string identifying the underlying reference. Two ReflectionReference
     * instances describe the same reference if and only if their ids are equal, which is what makes
     * it possible to tell whether two array elements share one reference rather than merely holding
     * equal values.
     *
     * @pure
     */
    public function getId(): string {}

    /**
     * Declared private to prevent the reflection object from being cloned; attempting to clone it
     * from outside the class throws an Error.
     */
    private function __clone(): void {}
}

#[Mago\AvailableSince(80500)]
final class ReflectionConstant implements Reflector
{
    /**
     * @readonly
     */
    public string $name;

    /**
     * Constructs a reflection of the constant with the given name, which may include a namespace
     * prefix. Throws a ReflectionException if no constant with that name is defined.
     *
     * @throws ReflectionException
     */
    public function __construct(string $name) {}

    /**
     * Returns the constant's full name, including its namespace prefix if it has one.
     */
    public function getName(): string {}

    /**
     * Returns the namespace portion of the constant's name, or an empty string if the constant is
     * not namespaced.
     */
    public function getNamespaceName(): string {}

    /**
     * Returns the constant's name with any namespace prefix removed.
     */
    public function getShortName(): string {}

    /**
     * Returns the constant's current value.
     */
    public function getValue(): mixed {}

    /**
     * Returns whether the constant is marked deprecated, so that referencing it emits a deprecation
     * notice.
     */
    public function isDeprecated(): bool {}

    /**
     * Returns a human-readable description of the constant, formatted for debugging output rather
     * than for use as its name or value.
     */
    public function __toString(): string {}
}

class ReflectionParameter implements Reflector
{
    /**
     * @readonly
     */
    public string $name;

    /**
     * Constructs a reflection of a single parameter of a function, method or closure. $function
     * names the function, an array of [class, method], or a closure or callable to inspect, and
     * $param gives the parameter's name or its zero-based position. Throws ReflectionException if
     * the function or the parameter cannot be found.
     *
     * @param string|array{0: class-string, 1: string}|object $function
     *
     * @throws ReflectionException
     */
    public function __construct(string|array|object $function, string|int $param) {}

    /**
     * Returns a human-readable description of the parameter, including its position, type, name and
     * whether it is required or optional, intended for debugging output rather than programmatic
     * use.
     */
    public function __toString(): string {}

    /**
     * Returns the parameter's name, without the leading dollar sign or any type hint.
     *
     * @pure
     */
    public function getName(): string {}

    /**
     * Reports whether the parameter is declared to be passed by reference, with a leading &.
     *
     * @pure
     */
    public function isPassedByReference(): bool {}

    /**
     * @pure
     */
    public function canBePassedByValue(): bool {}

    /**
     * Returns the ReflectionFunction or ReflectionMethod that declares this parameter.
     *
     * @pure
     */
    public function getDeclaringFunction(): ReflectionFunctionAbstract {}

    /**
     * Returns the class that declares the method this parameter belongs to, or null if the
     * parameter belongs to a plain function or an unbound closure rather than a method.
     *
     * @pure
     */
    public function getDeclaringClass(): ?ReflectionClass {}

    /**
     * @pure
     */
    #[Deprecated(
        since: '8.0',
        reason: 'Use ReflectionParameter::getType() and the ReflectionType APIs should be used instead.',
    )]
    /**
     * Returns a ReflectionClass for the parameter's declared type, provided that type names a
     * single class or interface. Returns null if the parameter has no type, or if its type is not a
     * single class, such as a scalar, union or intersection type. Throws ReflectionException if the
     * named class cannot be located. Superseded by getType(), which reports the declared type
     * exactly as written.
     *
     * @throws ReflectionException
     */
    public function getClass(): ?ReflectionClass {}

    /**
     * Reports whether the parameter has any declared type at all; the type itself is obtained from
     * getType().
     *
     * @pure
     */
    public function hasType(): bool {}

    /**
     * Returns the parameter's declared type, or null if the parameter is untyped.
     *
     * @pure
     */
    public function getType(): ReflectionNamedType|ReflectionUnionType|ReflectionIntersectionType|null {}

    /**
     * @pure
     */
    #[Deprecated(
        since: '8.0',
        reason: 'Use ReflectionParameter::getType() and the ReflectionType APIs should be used instead.',
    )]
    /**
     * Reports whether the parameter's declared type is exactly array.
     */
    public function isArray(): bool {}

    /**
     * @pure
     */
    #[Deprecated(
        since: '8.0',
        reason: 'Use ReflectionParameter::getType() and the ReflectionType APIs should be used instead.',
    )]
    /**
     * Reports whether the parameter's declared type is exactly callable.
     */
    public function isCallable(): bool {}

    /**
     * Reports whether the parameter accepts a null argument. This is true for a parameter with no
     * type declaration, for one whose type is explicitly nullable or includes null, and for one
     * whose default value is null even where the type itself is not written as nullable.
     *
     * @pure
     */
    public function allowsNull(): bool {}

    /**
     * Returns the parameter's zero-based position within its function's parameter list.
     *
     * @code
     * function f($a, $b, $c) {}
     * // getPosition() for $c returns 2
     * @endcode
     *
     * @pure
     */
    public function getPosition(): int {}

    /**
     * Reports whether the parameter can be omitted when calling the function. True for a variadic
     * parameter even though it has no default value of its own, and false for a parameter that has
     * a default value but is followed by a later required parameter, which can happen when an
     * overriding method inserts a required parameter after an inherited optional one. Differs from
     * isDefaultValueAvailable(), which only reports whether this parameter itself declares a
     * default.
     *
     * @pure
     */
    public function isOptional(): bool {}

    /**
     * Reports whether the parameter itself declares a default value in its signature. Returns false
     * for a variadic parameter, even though such a parameter can still be omitted.
     *
     * @pure
     */
    public function isDefaultValueAvailable(): bool {}

    /**
     * Returns the parameter's default value, with any constant, class constant or enum case
     * reference resolved to its actual value. Throws ReflectionException if the parameter has no
     * default value.
     *
     * @throws ReflectionException
     *
     * @pure
     */
    public function getDefaultValue(): mixed {}

    /**
     * Reports whether the parameter's default value is written as a reference to a constant, class
     * constant or enum case, rather than as a literal value.
     *
     * @pure
     */
    public function isDefaultValueConstant(): bool {}

    /**
     * Returns the name of the constant, class constant or enum case referenced as the parameter's
     * default value, or null if the default value is a literal rather than such a reference. Throws
     * ReflectionException if the parameter has no default value at all.
     *
     * @code
     * function f($x = PHP_EOL) {}
     * // getDefaultValueConstantName() returns "PHP_EOL"; getDefaultValue() returns the newline itself
     * @endcode
     *
     * @throws ReflectionException
     *
     * @pure
     */
    public function getDefaultValueConstantName(): ?string {}

    /**
     * Reports whether the parameter is the function's variadic parameter, which collects any number
     * of trailing arguments.
     *
     * @code
     * function f($a, ...$rest) {}
     * // isVariadic() is true only for $rest
     * @endcode
     *
     * @pure
     */
    public function isVariadic(): bool {}

    /**
     * Reports whether the parameter is a promoted constructor property, declared with a visibility
     * modifier so that it is also registered as a class property.
     *
     * @pure
     */
    public function isPromoted(): bool {}

    /**
     * Returns the attributes declared on the parameter, as a list of ReflectionAttribute instances.
     * When $name is given, only attributes of that class are returned; if $flags includes
     * ReflectionAttribute::IS_INSTANCEOF, $name is matched against attributes whose class is an
     * instance of it, rather than requiring an exact match.
     *
     * @template A
     *
     * @param class-string<A>|null $name
     * @param int $flags
     *
     * @return list<ReflectionAttribute<A>>
     *
     * @pure
     */
    public function getAttributes(?string $name = null, int $flags = 0): array {}

    /**
     * Prevents the parameter from being cloned; being declared private, any attempt to clone an
     * instance fails.
     */
    private function __clone(): void {}
}

abstract class ReflectionFunctionAbstract implements Reflector
{
    /**
     * @readonly
     */
    public string $name;

    private function __clone(): void {}

    public function inNamespace(): bool {}

    /**
     * Reports whether this reflects a closure, as opposed to a named function or method.
     *
     * @pure
     */
    public function isClosure(): bool {}

    /**
     * Reports whether the function is marked deprecated - true for many deprecated internal
     * functions and methods, and for a user-defined function or method carrying a #[Deprecated]
     * attribute.
     *
     * @pure
     */
    public function isDeprecated(): bool {}

    /**
     * Reports whether the function is implemented by the engine or an extension, rather than
     * defined in PHP source code.
     *
     * @pure
     */
    public function isInternal(): bool {}

    /**
     * Reports whether the function is defined in PHP source code, the logical complement of
     * isInternal().
     *
     * @pure
     */
    public function isUserDefined(): bool {}

    /**
     * Reports whether the function's body contains a yield expression, making it a generator.
     *
     * @pure
     */
    public function isGenerator(): bool {}

    /**
     * Reports whether the function's last parameter is variadic, letting it accept a variable
     * number of trailing arguments.
     *
     * @pure
     */
    public function isVariadic(): bool {}

    /**
     * Returns the object bound to $this inside the closure, or null if the closure is unbound, for
     * example a static closure, or this instance does not reflect a closure.
     *
     * @pure
     */
    public function getClosureThis(): ?object {}

    /**
     * Returns the class used to resolve self, parent, static and private or protected member access
     * inside the closure, or null if the closure has no such scope.
     *
     * @pure
     */
    public function getClosureScopeClass(): ?ReflectionClass {}

    /**
     * Returns the class used for late static binding inside the closure - typically the class of
     * the object the closure is bound to - or null if the closure is not bound to an object.
     *
     * @pure
     */
    #[Mago\AvailableSince(80100)]
    public function getClosureCalledClass(): ?ReflectionClass {}

    /**
     * Returns the raw doc comment immediately preceding the function's declaration, or false if it
     * has none.
     *
     * @pure
     */
    public function getDocComment(): string|false {}

    /**
     * Returns the line number on which the function's declaration ends, or false for an internal
     * function.
     *
     * @pure
     */
    public function getEndLine(): int|false {}

    /**
     * Returns a ReflectionExtension for the extension that defines the function, or null if the
     * function is user-defined.
     *
     * @pure
     */
    public function getExtension(): ?ReflectionExtension {}

    /**
     * Returns the name of the extension that defines the function, or false if the function is
     * user-defined.
     *
     * @pure
     */
    public function getExtensionName(): string|false {}

    /**
     * Returns the absolute path of the file in which the function is defined, or false for an
     * internal function.
     *
     * @pure
     */
    public function getFileName(): string|false {}

    /**
     * Returns the function's name: the namespaced name for a plain function, or the bare method
     * name, without the class, for a method.
     *
     * @pure
     */
    public function getName(): string {}

    /**
     * Returns the name of the namespace the function is declared in, without a leading or trailing
     * backslash, or an empty string if it is not namespaced.
     *
     * @pure
     */
    public function getNamespaceName(): string {}

    /**
     * Returns the total number of parameters the function declares, including optional ones.
     *
     * @pure
     */
    public function getNumberOfParameters(): int {}

    /**
     * Returns the number of parameters that must be supplied at a call site: those with no default
     * value and that are not variadic.
     *
     * @pure
     */
    public function getNumberOfRequiredParameters(): int {}

    /**
     * Returns the function's parameters, as a list of ReflectionParameter instances in declaration
     * order.
     *
     * @return ReflectionParameter[]
     *
     * @pure
     */
    public function getParameters(): array {}

    /**
     * Returns the function's declared return type, or null if it has none.
     *
     * @pure
     */
    public function getReturnType(): ReflectionNamedType|ReflectionUnionType|ReflectionIntersectionType|null {}

    /**
     * Returns the function's name without any namespace prefix. Differs from getName() only for a
     * namespaced plain function; for a method the two are the same.
     *
     * @pure
     */
    public function getShortName(): string {}

    /**
     * Returns the line number on which the function's declaration begins, or false for an internal
     * function.
     *
     * @pure
     */
    public function getStartLine(): int|false {}

    /**
     * Returns the function's static local variables, as an array mapping each variable's name to
     * its current value.
     *
     * @pure
     */
    public function getStaticVariables(): array {}

    /**
     * Reports whether the function is declared to return by reference.
     *
     * @pure
     */
    public function returnsReference(): bool {}

    /**
     * Reports whether the function has a declared return type.
     *
     * @pure
     */
    public function hasReturnType(): bool {}

    /**
     * Returns the attributes declared on the function, as a list of ReflectionAttribute instances.
     * When $name is given, only attributes of that class are returned; if $flags includes
     * ReflectionAttribute::IS_INSTANCEOF, $name is matched against attributes whose class is an
     * instance of it, rather than requiring an exact match.
     *
     * @template A
     *
     * @param class-string<A>|null $name
     *
     * @return list<ReflectionAttribute<A>>
     *
     * @pure
     */
    public function getAttributes(?string $name = null, int $flags = 0): array {}

    /**
     * Returns the variables captured into the closure's use clause, as an array mapping each
     * variable's name to its captured value.
     *
     * @pure
     */
    #[Mago\AvailableSince(80100)]
    public function getClosureUsedVariables(): array {}

    /**
     * Reports whether the function's declared return type is a tentative one added by the engine to
     * a built-in method - a transitional type that an overriding method is not yet strictly
     * required to match.
     *
     * @pure
     */
    #[Mago\AvailableSince(80100)]
    public function hasTentativeReturnType(): bool {}

    /**
     * Returns the function's tentative return type, or null if it has none. Use getReturnType() for
     * a return type that is not tentative.
     *
     * @pure
     */
    #[Mago\AvailableSince(80100)]
    public function getTentativeReturnType(): ?ReflectionType {}

    /**
     * Reports whether the function is declared static: for a method, whether it carries the static
     * modifier; for a closure, whether it was created with the static keyword and so has no $this
     * and cannot be bound to an object.
     *
     * @pure
     */
    public function isStatic(): bool {}

    /**
     * Returns a human-readable description of the function's signature, intended for debugging
     * output rather than programmatic use.
     */
    public function __toString(): string {}
}

class ReflectionFunction extends ReflectionFunctionAbstract
{
    /**
     * @readonly
     */
    public string $name;

    public const IS_DEPRECATED = 2048;

    /**
     * Constructs a ReflectionFunction for the given function, accepting either its name as a string
     * or a Closure instance directly. Throws ReflectionException if a named function does not
     * exist.
     *
     * @throws ReflectionException
     */
    public function __construct(Closure|string $function) {}

    /**
     * Builds a human-readable, multi-line description of the function, in the same style
     * Reflection's own debug output uses, listing its name, parameters and other declared
     * information. The format is meant for display and diagnostics rather than for parsing.
     */
    public function __toString(): string {}

    /**
     * Reports whether the function was disabled through the disable_functions directive. The method
     * is deprecated and on current PHP versions always returns false.
     * @pure
     */
    #[\Deprecated(since: '8.0', reason: 'ReflectionFunction can no longer be constructed for disabled functions.')]
    public function isDisabled(): bool {}

    /**
     * Calls the function, passing the given arguments in order, and returns its result. Because the
     * arguments are collected into a variadic parameter they are always passed by value, so a
     * function with by-reference parameters cannot be called correctly through this method;
     * invokeArgs, given an array literal built from references, must be used for that instead. Any
     * exception the function itself throws propagates unchanged, and the call throws
     * ReflectionException if it cannot be made at all.
     *
     * @throws ReflectionException
     */
    public function invoke(mixed ...$args): mixed {}

    /**
     * Calls the function, passing the elements of $args as its arguments in order, and returns its
     * result. Because $args is an ordinary array rather than a variadic parameter, building it with
     * reference elements lets a by-reference parameter of the function write back into the caller's
     * variables. Any exception the function itself throws propagates unchanged, and the call throws
     * ReflectionException if it cannot be made at all.
     *
     * @code
     * function f(&$x) { $x++; }
     * (new ReflectionFunction('f'))->invokeArgs([&$n]);
     * @endcode
     *
     * @throws ReflectionException
     */
    public function invokeArgs(array $args): mixed {}

    /**
     * Returns a Closure that, when called, invokes the underlying function with the same arguments.
     *
     * @return Closure|null
     *
     * @pure
     */
    public function getClosure(): Closure {}

    /**
     * Reports whether the function is an anonymous function, that is, a closure or arrow function
     * rather than one declared with a name.
     *
     * @pure
     */
    public function isAnonymous(): bool {}
}

/**
 * @template T of object = object
 */
class ReflectionMethod extends ReflectionFunctionAbstract
{
    /**
     * @readonly
     */
    public string $name;

    /**
     * @var class-string<T>
     *
     * @readonly
     */
    public string $class;

    public const IS_STATIC = 16;

    public const IS_PUBLIC = 1;

    public const IS_PROTECTED = 2;

    public const IS_PRIVATE = 4;

    public const IS_ABSTRACT = 64;

    public const IS_FINAL = 32;

    /**
     * Constructs a ReflectionMethod for a given method, either from an object instance or a class
     * name together with the method name, or from a single string combining both as
     * 'ClassName::methodName' when the second argument is omitted. Throws ReflectionException if
     * the class or the method does not exist.
     *
     * @param class-string<T>|T $objectOrMethod
     *
     * @throws ReflectionException
     */
    public function __construct(object|string $objectOrMethod, ?string $method = null) {}

    /**
     * Builds a human-readable, multi-line description of the method, in the same style Reflection's
     * own debug output uses, listing its name, visibility, parameters and other declared
     * information. The format is meant for display and diagnostics rather than for parsing.
     */
    public function __toString(): string {}

    /**
     * Reports whether the method is declared public.
     *
     * @pure
     */
    public function isPublic(): bool {}

    /**
     * Reports whether the method is declared private.
     *
     * @pure
     */
    public function isPrivate(): bool {}

    /**
     * Reports whether the method is declared protected.
     *
     * @pure
     */
    public function isProtected(): bool {}

    /**
     * Reports whether the method is declared abstract.
     *
     * @pure
     */
    public function isAbstract(): bool {}

    /**
     * Reports whether the method is declared final.
     *
     * @pure
     */
    public function isFinal(): bool {}

    /**
     * Reports whether the method is declared static.
     *
     * @pure
     */
    public function isStatic(): bool {}

    /**
     * Reports whether this method is the class's constructor, that is, whether it is named
     * __construct.
     *
     * @pure
     */
    public function isConstructor(): bool {}

    /**
     * Reports whether this method is the class's destructor, that is, whether it is named
     * __destruct.
     *
     * @pure
     */
    public function isDestructor(): bool {}

    /**
     * Returns a Closure that, when called, invokes the method. For a static method the $object
     * argument is ignored and may be omitted; for an instance method it must be an instance of the
     * declaring class, and the returned closure is bound to it. Throws ValueError if $object is
     * required but not given, and ReflectionException if the closure cannot otherwise be created.
     *
     * @param T|null $object
     *
     * @throws ValueError
     * @throws ReflectionException
     */
    public function getClosure(?object $object = null): Closure {}

    /**
     * Returns a bitmask combining the method's IS_* modifier constants, such as IS_STATIC,
     * IS_PUBLIC, IS_PROTECTED, IS_PRIVATE, IS_ABSTRACT and IS_FINAL, reflecting exactly how the
     * method was declared.
     *
     * @pure
     */
    public function getModifiers(): int {}

    /**
     * Calls the method on the given object and returns its result. For a static method $object is
     * ignored and may be null; for an instance method it must be an instance of the declaring
     * class. Since PHP 8.1 a private or protected method can be invoked directly without first
     * calling setAccessible; on earlier versions setAccessible(true) must be called first, or the
     * call throws ReflectionException. Because the remaining arguments are collected into a
     * variadic parameter they are always passed by value, so a method with by-reference parameters
     * cannot be called correctly through this method; invokeArgs must be used for that instead. Any
     * exception the method itself throws propagates unchanged, and the call throws
     * ReflectionException if it cannot be made at all.
     *
     * @param T|null $object
     *
     * @throws ReflectionException
     */
    public function invoke($object, ...$args) {}

    /**
     * Calls the method on the given object, passing the elements of $args as its arguments in
     * order, and returns its result. For a static method $object is ignored and may be null; for an
     * instance method it must be an instance of the declaring class. Since PHP 8.1 a private or
     * protected method can be invoked directly without first calling setAccessible; on earlier
     * versions setAccessible(true) must be called first, or the call throws ReflectionException.
     * Because $args is an ordinary array, building it with reference elements lets a by-reference
     * parameter of the method write back into the caller's variables. Any exception the method
     * itself throws propagates unchanged, and the call throws ReflectionException if it cannot be
     * made at all.
     *
     * @param T|null $object
     *
     * @throws ReflectionException
     */
    public function invokeArgs(?object $object, array $args): mixed {}

    /**
     * Returns the class in which this method is actually declared, which may differ from the class
     * this ReflectionMethod was obtained from when the method is inherited rather than overridden
     * there.
     *
     * @pure
     */
    public function getDeclaringClass(): ReflectionClass {}

    /**
     * Returns the ReflectionMethod for the method in a parent class or implemented interface that
     * this method overrides or implements. Throws ReflectionException if the method has no such
     * prototype.
     *
     * @throws ReflectionException
     *
     * @pure
     */
    public function getPrototype(): ReflectionMethod {}

    /**
     * Historically marked a private or protected method as callable through invoke and invokeArgs
     * without further checks. Since PHP 8.1 every method is callable through reflection regardless
     * of its declared visibility, so calling this has no effect.
     *
     * @pure
     */
    #[Deprecated(since: '8.5', message: 'as it has no effect')]
    public function setAccessible(bool $accessible): void {}

    /**
     * Reports whether the method overrides or implements a method declared in a parent class or
     * interface, that is, whether getPrototype would succeed instead of throwing.
     *
     * @pure
     */
    #[Mago\AvailableSince(80100)]
    public function hasPrototype(): bool {}

    /**
     * Creates a ReflectionMethod from a single string of the form 'ClassName::methodName',
     * combining the class name and the method name that the two-argument constructor would
     * otherwise take separately.
     */
    #[Mago\AvailableSince(80300)]
    public static function createFromMethodName(string $method): static {}
}

/**
 * @template T of object = object
 */
class ReflectionClass implements Reflector
{
    /**
     * @var class-string<T>
     */
    public string $name;

    public const IS_IMPLICIT_ABSTRACT = 16;

    public const IS_EXPLICIT_ABSTRACT = 64;

    public const IS_FINAL = 32;

    public const IS_READONLY = 65536;

    public const int SKIP_INITIALIZATION_ON_SERIALIZE = 0;

    public const int SKIP_DESTRUCTOR = 0;

    /**
     * Constructs a ReflectionClass for a given class, accepting either the class name as a string
     * or an object instance, in which case that object's class is reflected. Throws
     * ReflectionException if the named class does not exist.
     *
     * @param class-string<T>|T $objectOrClass
     *
     * @throws ReflectionException
     */
    public function __construct(object|string $objectOrClass) {}

    /**
     * Returns a formatted, multi-line textual description of the class, intended for debugging
     * output. The text lists the class's name, parent, modifiers, constants, properties and
     * methods, in the same layout produced by the command-line reflection export, and is not meant
     * to be parsed.
     */
    public function __toString(): string {}

    /**
     * Returns the class's fully qualified name, without a leading namespace separator.
     *
     * @pure
     *
     * @return class-string<T>
     */
    public function getName(): string {}

    /**
     * Returns whether the class is built into PHP or provided by a compiled extension, rather than
     * declared in PHP source code.
     *
     * @pure
     */
    public function isInternal(): bool {}

    /**
     * Returns whether the class was declared in PHP source code, the exact opposite of isInternal.
     *
     * @pure
     */
    public function isUserDefined(): bool {}

    /**
     * Returns whether an instance of the class can be created with the new operator. This is false
     * for interfaces, traits and enums, for abstract classes, and also for any otherwise concrete
     * class whose constructor is private or protected — a case isAbstract cannot detect, since such
     * a class is not declared abstract at all.
     *
     * @pure
     */
    public function isInstantiable(): bool {}

    /**
     * Returns whether an instance of the class can be duplicated with the clone operator. This is
     * false for interfaces and traits, which cannot be instantiated in the first place, and false
     * for any class whose __clone method is declared private or protected.
     *
     * @pure
     */
    public function isCloneable(): bool {}

    /**
     * Returns the absolute path of the file the class was declared in, or false for an internal
     * class or one with no associated file, such as a class defined via eval.
     *
     * @pure
     */
    public function getFileName(): string|false {}

    /**
     * Returns the line number on which the class declaration begins, or false for an internal
     * class.
     *
     * @pure
     */
    public function getStartLine(): int|false {}

    /**
     * Returns the line number on which the class declaration ends, or false for an internal class.
     *
     * @pure
     */
    public function getEndLine(): int|false {}

    /**
     * Returns the class's own doc comment exactly as written, with its delimiters intact, or false
     * if the class has none.
     *
     * @pure
     */
    public function getDocComment(): string|false {}

    /**
     * Returns a ReflectionMethod for the constructor that governs instances of this class,
     * following inheritance to an ancestor's constructor when the class declares none of its own,
     * or null if no class in the hierarchy defines one.
     *
     * @pure
     */
    public function getConstructor(): ?ReflectionMethod {}

    /**
     * Returns whether the class, one of its ancestors, or a trait it uses declares a method of the
     * given name. The match is case-insensitive, as PHP method names are.
     *
     * @pure
     */
    public function hasMethod(string $name): bool {}

    /**
     * Returns a ReflectionMethod for the named method, searching the class, its ancestors and any
     * traits it uses. Throws ReflectionException if no method of that name exists anywhere in the
     * hierarchy. The match is case-insensitive.
     *
     * @throws ReflectionException
     *
     * @return ReflectionMethod<T>
     *
     * @pure
     */
    public function getMethod(string $name): ReflectionMethod {}

    /**
     * Returns every method visible on the class as a plain list of ReflectionMethod objects,
     * including those inherited from parent classes and those brought in from traits. Passing a
     * filter restricts the list to methods matching at least one of the given
     * ReflectionMethod::IS_* flags — IS_PUBLIC, IS_PROTECTED, IS_PRIVATE, IS_STATIC, IS_ABSTRACT or
     * IS_FINAL, ORed together — and omitting it returns methods of every visibility and kind.
     *
     * @code
     * $class->getMethods(ReflectionMethod::IS_PUBLIC | ReflectionMethod::IS_STATIC);
     * @endcode
     *
     * @return list<ReflectionMethod<T>>
     *
     * @pure
     */
    public function getMethods(?int $filter = null): array {}

    /**
     * Returns whether the class or one of its ancestors declares a property of the given name.
     * Unlike method names, property names are matched case-sensitively.
     *
     * @pure
     */
    public function hasProperty(string $name): bool {}

    /**
     * Returns a ReflectionProperty for the named property, searching the class and its ancestors.
     * Throws ReflectionException if no property of that name is declared anywhere in the hierarchy.
     *
     * @return ReflectionProperty<T>
     *
     * @throws ReflectionException
     *
     * @pure
     */
    public function getProperty(string $name): ReflectionProperty {}

    /**
     * Returns every property visible on the class as a plain list of ReflectionProperty objects,
     * including those inherited from parent classes. Passing a filter restricts the list to
     * properties matching at least one of the given ReflectionProperty::IS_* flags — IS_PUBLIC,
     * IS_PROTECTED, IS_PRIVATE or IS_STATIC, ORed together — and omitting it returns properties of
     * every visibility.
     *
     * @return list<ReflectionProperty<T>>
     *
     * @pure
     */
    public function getProperties(?int $filter = null): array {}

    /**
     * Returns a ReflectionClassConstant for the named constant, searching the class, its ancestors
     * and any interfaces it implements, or false if no constant of that name exists anywhere in the
     * hierarchy.
     *
     * @return false|ReflectionClassConstant<T>
     *
     * @pure
     */
    public function getReflectionConstant(string $name): ReflectionClassConstant|false {}

    /**
     * Returns every constant visible on the class as a plain list of ReflectionClassConstant
     * objects, including those inherited from parent classes and those declared on implemented
     * interfaces. Passing a filter restricts the list by visibility using the
     * ReflectionClassConstant::IS_* flags.
     *
     * @return list<ReflectionClassConstant<T>>
     *
     * @pure
     */
    public function getReflectionConstants(?int $filter = null): array {}

    /**
     * Returns whether the class, one of its ancestors, or an interface it implements declares a
     * constant of the given name.
     *
     * @pure
     */
    public function hasConstant(string $name): bool {}

    /**
     * Returns an associative array, keyed by constant name, of every constant's value, including
     * constants inherited from parent classes and constants declared on implemented interfaces.
     * Passing a filter restricts the result by visibility using the ReflectionClassConstant::IS_*
     * flags.
     *
     * @return array<string, mixed>
     *
     * @pure
     */
    public function getConstants(?int $filter = null): array {}

    /**
     * Returns the value of the named constant, searching the class, its ancestors and any
     * interfaces it implements. Returns false if no constant of that name exists anywhere in the
     * hierarchy, which is indistinguishable from a constant whose real value is false; call
     * hasConstant first to tell the two apart.
     *
     * @code
     * $class->hasConstant('FOO') ? $class->getConstant('FOO') : null;
     * @endcode
     *
     * @pure
     */
    public function getConstant(string $name): mixed {}

    /**
     * Returns every interface implemented by the class, directly or through a parent class or
     * another extended interface, as an associative array keyed by each interface's fully qualified
     * name.
     *
     * @return array<interface-string, ReflectionClass>
     *
     * @pure
     */
    public function getInterfaces(): array {}

    /**
     * Returns the fully qualified names of every interface implemented by the class as a plain
     * list, including interfaces implemented only by a parent class or reached by extension from
     * another interface.
     *
     * @return list<interface-string>
     */
    public function getInterfaceNames(): array {}

    /**
     * Returns whether the class is an anonymous class created with an inline new class declaration
     * rather than a named one.
     *
     * @pure
     */
    public function isAnonymous(): bool {}

    /**
     * Returns whether the reflected type is an interface rather than a concrete or abstract class.
     *
     * @pure
     */
    public function isInterface(): bool {}

    /**
     * Returns the traits used directly in this class's own declaration, as an associative array
     * keyed by each trait's fully qualified name. Traits used by a parent class are not included,
     * since a class's own use of a trait is not itself inherited — only the methods and properties
     * the trait contributes are.
     *
     * @return array<trait-string, ReflectionClass>
     *
     * @pure
     */
    public function getTraits(): array {}

    /**
     * Returns the fully qualified names of the traits used directly in this class's own
     * declaration, as a plain list. Like getTraits, it does not include traits used only by a
     * parent class.
     *
     * @return list<trait-string>
     *
     * @pure
     */
    public function getTraitNames(): array {}

    /**
     * Returns the method aliases created where this class uses a trait with an as clause, as an
     * associative array mapping each alias name to the string TraitName::originalMethodName it
     * resolves to.
     *
     * @return array<string, string>
     *
     * @pure
     */
    public function getTraitAliases(): array {}

    /**
     * Returns whether the reflected type is a trait rather than a class.
     *
     * @pure
     */
    public function isTrait(): bool {}

    /**
     * Returns whether the class itself is declared with the abstract keyword. An interface or trait
     * is not considered abstract by this check even though neither can be instantiated; use
     * isInstantiable for that broader question.
     *
     * @pure
     */
    public function isAbstract(): bool {}

    /**
     * Returns whether the class is declared with the final keyword, preventing it from being
     * extended.
     *
     * @pure
     */
    public function isFinal(): bool {}

    /**
     * Returns whether the class itself is declared with the readonly modifier, which makes every
     * typed property declared directly in the class implicitly readonly.
     *
     * @pure
     */
    #[Mago\AvailableSince(80200)]
    public function isReadOnly(): bool {}

    /**
     * Returns a bitmask encoding the class's declaration modifiers, such as abstract, final and
     * readonly. The individual bits are only meaningful when tested against the corresponding
     * ReflectionClass::IS_* class constants, since their positions are an implementation detail
     * rather than a stable value to compare directly.
     *
     * @pure
     */
    public function getModifiers(): int {}

    /**
     * Returns whether the given object is an instance of the reflected class or one of its
     * subclasses, the same test the instanceof operator performs.
     *
     * @pure
     *
     * @assert-if-true T $object
     */
    public function isInstance(object $object): bool {}

    /**
     * Creates a new instance of the class, passing the given arguments to its constructor exactly
     * as a normal new call would. Throws ReflectionException if the class cannot be instantiated,
     * for example because it is abstract or its constructor is private or protected.
     *
     * @return T
     *
     * @throws ReflectionException
     */
    public function newInstance(...$args) {}

    /**
     * Creates a new instance of the class without invoking its constructor at all, leaving typed
     * properties uninitialised. Unlike newInstance, it bypasses a private or protected constructor
     * rather than being blocked by it, which is why it is used for cases such as singletons and
     * deserialisation. Throws ReflectionException if the class cannot be instantiated at all, such
     * as an abstract class or an interface.
     *
     * @return T
     *
     * @throws ReflectionException
     */
    public function newInstanceWithoutConstructor(): object {}

    /**
     * Creates a new instance of the class and invokes its constructor, identically to newInstance
     * except that the constructor arguments are passed as a single array rather than as a variadic
     * argument list.
     * @return T
     *
     * @throws ReflectionException
     */
    public function newInstanceArgs(array $args = []): object {}

    /**
     * Returns a ReflectionClass for the class's immediate parent, or false if the class has no
     * parent.
     * @pure
     */
    public function getParentClass(): ReflectionClass|false {}

    /**
     * Returns whether the class descends from, or implements, the given class or interface. It
     * answers false when the given class is the reflected class itself — this checks strict
     * ancestry, not equality — so it is not equivalent to an instanceof check against the same
     * class.
     * @pure
     */
    public function isSubclassOf(ReflectionClass|string $class): bool {}

    /**
     * Returns an associative array, keyed by property name, of the current values of every static
     * property declared by the class or inherited from its ancestors.
     *
     * @pure
     */
    public function getStaticProperties(): array {}

    /**
     * Returns the current value of the named static property, searching the class and its ancestors
     * for the declaration. If $default is supplied, it is returned instead of throwing when the
     * property cannot be read, for example because it does not exist.
     *
     * @pure
     */
    public function getStaticPropertyValue(string $name, mixed $default = UNKNOWN): mixed {}

    /**
     * Assigns a new value to the named static property, searching the class and its ancestors for
     * the declaration and bypassing its visibility. Throws if no static property of that name is
     * declared anywhere in the hierarchy.
     */
    public function setStaticPropertyValue(string $name, mixed $value): void {}

    /**
     * Returns an associative array, keyed by property name, of the default values written in the
     * declarations of the class's properties, not the current values held by any particular
     * instance. A typed property declared without a default value is omitted, since it has none to
     * report.
     *
     * @pure
     */
    public function getDefaultProperties(): array {}

    /**
     * Returns whether the class implements the Traversable interface, making its instances usable
     * in a foreach loop. This is a deprecated alias for isIterable and behaves identically to it.
     *
     * @pure
     */
    public function isIterateable(): bool {}

    /**
     * Returns whether the class implements the Traversable interface, directly or through a parent
     * class or an extended interface, making its instances usable in a foreach loop.
     *
     * @pure
     */
    public function isIterable(): bool {}

    /**
     * Returns whether the class implements the given interface, whether declared directly or
     * inherited from a parent class.
     *
     * @pure
     */
    public function implementsInterface(ReflectionClass|string $interface): bool {}

    /**
     * Returns a ReflectionExtension for the compiled extension that defines the class, or null for
     * a user-defined class or one for which no extension information is available.
     *
     * @pure
     */
    public function getExtension(): ?ReflectionExtension {}

    /**
     * Returns the name of the compiled extension that defines the class, or false for a user-
     * defined class.
     *
     * @pure
     */
    public function getExtensionName(): string|false {}

    /**
     * Returns whether the class's name contains a namespace, that is, whether it was declared
     * inside a namespace block rather than in the global namespace.
     *
     * @pure
     */
    public function inNamespace(): bool {}

    /**
     * Returns the class's namespace, without a leading or trailing separator and without the
     * class's own short name. Returns an empty string for a class in the global namespace.
     *
     * @pure
     */
    public function getNamespaceName(): string {}

    /**
     * Returns the class's name with its namespace prefix removed.
     *
     * @pure
     */
    public function getShortName(): string {}

    /**
     * Returns the class's own attributes as a plain list of ReflectionAttribute objects. Passing
     * $name restricts the list to attributes of that class; by default this is an exact match, but
     * passing the ReflectionAttribute::IS_INSTANCEOF flag also matches attributes that are
     * subclasses of $name.
     *
     * @code
     * $class->getAttributes(SomeAttribute::class, ReflectionAttribute::IS_INSTANCEOF);
     * @endcode
     *
     * @template A
     *
     * @param class-string<A>|null $name
     *
     * @return list<ReflectionAttribute<A>>
     *
     * @pure
     */
    public function getAttributes(?string $name = null, int $flags = 0): array {}

    /**
     * Private and never called, so that cloning a ReflectionClass instance is disallowed; PHP
     * raises an Error if a clone is attempted.
     */
    private function __clone(): void {}

    /**
     * Returns whether the reflected type is an enum rather than a class.
     *
     * @pure
     */
    #[Mago\AvailableSince(80100)]
    public function isEnum(): bool {}

    /**
     * Creates a lazy ghost: a new instance of the class whose properties stay uninitialised until
     * first accessed, at which point $initializer is called with the instance to populate them.
     * $options is a bitmask of flags governing the object's behaviour before that first access,
     * such as under serialisation or destruction, and defaults to none.
     */
    #[Mago\AvailableSince(80400)]
    public function newLazyGhost(callable $initializer, int $options = 0): object {}

    /**
     * Creates a lazy proxy: a new instance of the class that forwards every property access and
     * method call to a separate, real instance produced on first access by calling $factory with
     * the proxy itself. $options is a bitmask of flags governing the proxy's behaviour before that
     * real instance is created.
     *
     * @return T
     */
    #[Mago\AvailableSince(80400)]
    public function newLazyProxy(callable $factory, int $options = 0): object {}

    /**
     * Turns an already-constructed object back into an uninitialised lazy ghost, discarding
     * whatever property values it currently holds, so that $initializer runs again the next time
     * one of its properties is accessed.
     *
     * @param T $object
     */
    #[Mago\AvailableSince(80400)]
    public function resetAsLazyGhost(object $object, callable $initializer, int $options = 0): void {}

    /**
     * Turns an already-constructed object into a lazy proxy backed by $factory, discarding whatever
     * state it currently holds.
     *
     * @param T $object
     */
    #[Mago\AvailableSince(80400)]
    public function resetAsLazyProxy(object $object, callable $factory, int $options = 0): void {}

    /**
     * Forces a lazy ghost or lazy proxy to initialise immediately, running its initializer or
     * factory now rather than waiting for the first property access, and returns the object itself.
     *
     * @param T $object
     */
    #[Mago\AvailableSince(80400)]
    public function initializeLazyObject(object $object): object {}

    /**
     * Returns whether the given object is a lazy ghost or lazy proxy whose initializer or factory
     * has not yet run.
     *
     * @param T $object
     *
     * @pure
     */
    #[Mago\AvailableSince(80400)]
    public function isUninitializedLazyObject(object $object): bool {}

    /**
     * Marks a lazy ghost or lazy proxy as initialised without running its initializer or factory,
     * and returns the object itself. This is for populating a ghost's properties by hand and
     * telling the object it no longer needs to call the initializer.
     *
     * @param T $object
     */
    #[Mago\AvailableSince(80400)]
    public function markLazyObjectAsInitialized(object $object): object {}

    /**
     * Returns the initializer or factory callable associated with the given lazy object, or null if
     * the object is not a lazy object, or if it has already been initialised.
     *
     * @param T $object
     */
    #[Mago\AvailableSince(80400)]
    public function getLazyInitializer(object $object): ?callable {}
}

/**
 * @template T of object = object
 *
 * @extends ReflectionClass<T>
 */
class ReflectionObject extends ReflectionClass
{
    /**
     * @param T $object
     */
    public function __construct(object $object) {}
}

/**
 * @extends ReflectionClass<UnitEnum|BackedEnum>
 */
#[Mago\AvailableSince(80100)]
class ReflectionEnum extends ReflectionClass
{
    /**
     * Reports whether the enum declares a case with the given name.
     */
    public function hasCase(string $name): bool {}

    /**
     * Returns the enum's cases as a plain, zero-indexed list in declaration order, not keyed by
     * name. Each element is a ReflectionEnumBackedCase if the case carries a scalar value, or a
     * ReflectionEnumUnitCase if it is a pure case with no value.
     *
     * @return list<ReflectionEnumUnitCase|ReflectionEnumBackedCase>
     */
    public function getCases(): array {}

    /**
     * Returns the reflection of the case with the given name. Throws ReflectionException if no such
     * case exists.
     *
     * @throws ReflectionException
     */
    public function getCase(string $name): ReflectionEnumUnitCase {}

    /**
     * Reports whether the enum's cases carry a scalar backing value, meaning the enum was declared
     * with a backing type such as int or string.
     *
     * @pure
     */
    public function isBacked(): bool {}

    /**
     * Returns the enum's declared backing type. Returns null if the enum is a pure enum with no
     * backing type.
     *
     * @pure
     */
    public function getBackingType(): ?ReflectionNamedType {}
}

/**
 * @extends ReflectionClassConstant<UnitEnum>
 */
#[Mago\AvailableSince(80100)]
class ReflectionEnumUnitCase extends ReflectionClassConstant
{
    /**
     * Returns the enum case itself, as an instance of the enum it belongs to, not its backing
     * value.
     *
     * @pure
     */
    public function getValue(): UnitEnum {}

    /**
     * Returns a reflection of the enum that declares this case.
     *
     * @pure
     */
    public function getEnum(): ReflectionEnum {}
}

/**
 * @extends ReflectionEnumUnitCase<BackedEnum>
 */
#[Mago\AvailableSince(80100)]
class ReflectionEnumBackedCase extends ReflectionEnumUnitCase
{
    /**
     * Returns the case's scalar backing value, the literal assigned after the equals sign in the
     * enum's declaration.
     *
     * @pure
     */
    public function getBackingValue(): int|string {}
}
