<?php

class SimpleXMLElement implements Traversable, ArrayAccess, Countable, Iterator, Stringable, RecursiveIterator
{
    /**
     * Constructs a new element from well-formed XML data. $data is XML markup by default, or a file
     * path or URL to load it from when $dataIsURL is true. $options accepts a bitmask of libxml
     * constants such as LIBXML_NOCDATA and LIBXML_NOERROR. Throws Exception if the XML cannot be
     * parsed.
     *
     * @throws Exception
     */
    public function __construct(
        string $data,
        int $options = 0,
        bool $dataIsURL = false,
        string $namespaceOrPrefix = '',
        bool $isPrefix = false,
    ) {}

    /**
     * Returns the child element or elements with the given name, invoked automatically by PHP when
     * a property is read on the object, such as $xml->title. If only one child has that name the
     * returned object wraps that child directly; if several share the name, the returned object
     * represents all of them together for iteration.
     *
     * @code
     * $title = $xml->title;
     * foreach ($xml->item as $item) {
     *     // one iteration per <item> child
     * }
     * @endcode
     *
     * @param string $name child name
     *
     * @return static
     */
    private function __get($name) {}

    /**
     * Returns the well-formed XML representation of this element and its children as a string. Pass
     * $filename to write the XML to that file instead, returning true on success or false on
     * failure.
     */
    public function asXML(?string $filename = null): string|bool {}

    /**
     * Alias of asXML(). Returns the well-formed XML representation of this element and its children
     * as a string, or writes it to $filename and returns a boolean when a filename is given.
     */
    public function saveXML(?string $filename = null): string|bool {}

    /**
     * Evaluates the given XPath 1.0 expression relative to this node and returns the matching
     * elements. Returns an empty array if nothing matches, and false if the expression could not be
     * evaluated.
     *
     * @return list<SimpleXMLElement>|false|null
     */
    public function xpath(string $expression): array|false|null {}

    /**
     * Binds a namespace prefix to a URI for use in subsequent xpath() calls, letting an expression
     * reference elements whose namespace has no corresponding prefix in the source document.
     * Returns true on success, false on failure.
     */
    public function registerXPathNamespace(string $prefix, string $namespace): bool {}

    /**
     * Returns the attributes of this element as an iterable object, or null if it has none. As with
     * other methods that return an element, the result wraps the same underlying document rather
     * than a copy, so changing an attribute through it changes the original data. Pass
     * $namespaceOrPrefix to restrict the result to a particular namespace.
     */
    public function attributes(?string $namespaceOrPrefix = null, bool $isPrefix = false): ?static {}

    /**
     * Returns the children of this element, optionally restricted to a given namespace or prefix,
     * as an iterable object, or null if it has none. Like attributes(), the result is a view over
     * the same underlying document rather than a copy.
     */
    public function children(?string $namespaceOrPrefix = null, bool $isPrefix = false): ?static {}

    /**
     * Returns the namespaces in scope for this element, as prefix => URI pairs. Set $recursive to
     * true to also include namespaces declared on descendant elements.
     *
     * @return array<string, string>
     */
    public function getNamespaces(bool $recursive = false): array {}

    /**
     * Returns the namespaces declared in the document, as prefix => URI pairs, rather than only
     * those in scope for this element. Set $recursive to also collect namespaces declared on
     * descendant nodes, and $fromRoot to false to consider only nodes from this element downwards,
     * as though it were the document root. Returns false on failure.
     *
     * @return array<string, string>|false
     */
    public function getDocNamespaces(bool $recursive = false, bool $fromRoot = true): array|false {}

    /**
     * Returns the local, unqualified tag name of this element, without any namespace prefix.
     */
    public function getName(): string {}

    /**
     * Appends a new child element with the given qualified name to this element and returns it, or
     * null on failure. Pass $value to give the new element text content, and $namespace to create
     * it in a namespace other than its parent's. The value is inserted as raw text without
     * escaping, so characters such as & or < should be passed through htmlspecialchars() first to
     * avoid producing malformed XML.
     *
     * @code
     * $el->addChild('price', htmlspecialchars('5 & dime'));
     * @endcode
     */
    public function addChild(string $qualifiedName, ?string $value = null, ?string $namespace = null): ?static {}

    /**
     * Adds an attribute with the given qualified name and value to this element. Pass $namespace to
     * create the attribute in a namespace other than its parent's.
     */
    public function addAttribute(string $qualifiedName, string $value, ?string $namespace = null): void {}

    /**
     * Returns the text content of this element, i.e. its character data concatenated together,
     * without any of its children's markup.
     */
    public function __toString(): string {}

    /**
     * Returns the number of child elements of this node. Called by PHP when the object is passed to
     * count().
     *
     * @return int<0, max>
     */
    public function count(): int {}

    /**
     * Returns whether an attribute exists, for a string offset, or whether a same-named sibling
     * element exists at that position, for an integer offset. Called implicitly by isset() and
     * empty() on array-style access such as isset($element['id']) or isset($element[0]).
     *
     * @param string|int $offset
     *
     * @return bool
     */
    public function offsetExists($offset) {}

    /**
     * Returns the attribute's value for a string offset, or the same-named sibling element at that
     * position for an integer offset, wrapping the same underlying document rather than a copy.
     * Called implicitly by array-style access such as $element['id'] or $element[0].
     *
     * @code
     * $id = $element['id'];
     * $second = $element[1]; // second sibling sharing this element's name
     * @endcode
     *
     * @param string|int $offset
     *
     * @return static
     */
    public function offsetGet($offset) {}

    /**
     * Sets the attribute named by a string offset to $value, or replaces the text content of the
     * same-named sibling element at an integer offset. Called implicitly by array-style assignment
     * such as $element['id'] = $value.
     *
     * @param string|int $offset
     *
     * @param mixed $value
     */
    public function offsetSet($offset, $value) {}

    /**
     * Removes the attribute named by a string offset, or the same-named sibling element at an
     * integer offset. Called implicitly by unset($element['id']).
     *
     * @param string|int $offset
     *
     * @return void
     */
    public function offsetUnset($offset) {}

    /**
     * Resets the internal iteration pointer to this element's first child. Called by PHP at the
     * start of a foreach loop over the object.
     */
    public function rewind(): void {}

    /**
     * Returns whether the internal pointer still points to a valid child element. Called by PHP
     * after rewind() and after each next() to decide whether to continue a foreach loop.
     */
    public function valid(): bool {}

    /**
     * Returns the child element the internal pointer currently points to, or null if the pointer is
     * not on a valid element. Called by PHP to obtain the value on each iteration of a foreach
     * loop.
     *
     * @return SimpleXMLElement|null
     */
    public function current(): ?static {}

    /**
     * Returns the tag name of the child element the internal pointer currently points to. Called by
     * PHP to obtain the key on each iteration of a foreach loop.
     */
    public function key(): string {}

    /**
     * Advances the internal pointer to the next child element. Called by PHP between iterations of
     * a foreach loop.
     */
    public function next(): void {}

    /**
     * Returns whether the element the iterator currently points to has any child elements. Called
     * by PHP during recursive iteration with RecursiveIteratorIterator, to decide whether to
     * descend into it.
     */
    public function hasChildren(): bool {}

    /**
     * Returns the children of the element the iterator currently points to, for recursive
     * iteration, or null if it has none. Called by PHP after hasChildren() returns true, to
     * continue iterating one level deeper.
     */
    public function getChildren(): ?SimpleXMLElement {}
}

class SimpleXMLIterator extends SimpleXMLElement implements RecursiveIterator, Countable, Stringable
{
    /**
     * Resets the internal iteration pointer to this element's first child. Called by PHP at the
     * start of a foreach loop over the object.
     *
     * @return void
     */
    public function rewind() {}

    /**
     * Returns whether the internal pointer still points to a valid child element. Called by PHP
     * after rewind() and after each next() to decide whether to continue a foreach loop.
     *
     * @return bool
     */
    public function valid() {}

    /**
     * Returns the child element the internal pointer currently points to, or null if the pointer is
     * not on a valid element. Called by PHP to obtain the value on each iteration of a foreach
     * loop.
     *
     * @return static|null
     */
    public function current() {}

    /**
     * Returns the tag name of the child element the internal pointer currently points to, or false
     * if the pointer is not on a valid element. Called by PHP to obtain the key on each iteration
     * of a foreach loop.
     *
     * @return string|false
     */
    public function key() {}

    /**
     * Advances the internal pointer to the next child element. Called by PHP between iterations of
     * a foreach loop.
     *
     * @return void
     */
    public function next() {}

    /**
     * Returns whether the element the iterator currently points to has any child elements. Called
     * by PHP during recursive iteration with RecursiveIteratorIterator, to decide whether to
     * descend into it.
     *
     * @return bool
     */
    public function hasChildren() {}

    /**
     * Returns the children of the element the iterator currently points to, for recursive
     * iteration. Called by PHP after hasChildren() returns true, to continue iterating one level
     * deeper.
     *
     * @return SimpleXMLIterator
     */
    public function getChildren() {}

    /**
     * Returns the text content of the current element, i.e. its character data concatenated
     * together, without any of its children's markup.
     *
     * @return string
     */
    public function __toString() {}

    /**
     * Returns the number of child elements of the current node. Called by PHP when the object is
     * passed to count().
     */
    public function count() {}
}

/**
 * Loads and parses the XML document at $filename into a SimpleXMLElement (or an instance of
 * $class_name, a subclass of it), returning false if the file cannot be read or parsed. $options is
 * a bitmask of libxml constants such as LIBXML_NOCDATA or LIBXML_NOBLANKS.
 *
 * @template T of SimpleXMLElement
 *
 * @param class-string<T>|null $class_name
 *
 * @return T|false
 */
function simplexml_load_file(
    string $filename,
    ?string $class_name = SimpleXMLElement::class,
    int $options = 0,
    string $namespace_or_prefix = '',
    bool $is_prefix = false,
): SimpleXMLElement|false {}

/**
 * Parses an XML string into a SimpleXMLElement object, or an instance of the given subclass.
 * Returns false if the string cannot be parsed as XML.
 *
 * @template T of SimpleXMLElement
 *
 * @param class-string<T>|null $class_name
 *
 * @return T|false
 */
function simplexml_load_string(
    string $data,
    ?string $class_name = SimpleXMLElement::class,
    int $options = 0,
    string $namespace_or_prefix = '',
    bool $is_prefix = false,
): SimpleXMLElement|false {}

/**
 * Converts the given DOM node into a SimpleXMLElement, or into an instance of the given subclass if
 * class_name is provided, returning null if the node cannot be converted.
 *
 * @template T of SimpleXMLElement
 *
 * @param class-string<T>|null $class_name
 *
 * @return T|null
 */
function simplexml_import_dom(object $node, ?string $class_name = SimpleXMLElement::class): ?SimpleXMLElement {}
