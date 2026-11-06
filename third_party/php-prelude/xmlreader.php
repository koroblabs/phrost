<?php

class XMLReader
{
    public const int NONE = UNKNOWN;
    public const int ELEMENT = UNKNOWN;
    public const int ATTRIBUTE = UNKNOWN;
    public const int TEXT = UNKNOWN;
    public const int CDATA = UNKNOWN;
    public const int ENTITY_REF = UNKNOWN;
    public const int ENTITY = UNKNOWN;
    public const int PI = UNKNOWN;
    public const int COMMENT = UNKNOWN;
    public const int DOC = UNKNOWN;
    public const int DOC_TYPE = UNKNOWN;
    public const int DOC_FRAGMENT = UNKNOWN;
    public const int NOTATION = UNKNOWN;
    public const int WHITESPACE = UNKNOWN;
    public const int SIGNIFICANT_WHITESPACE = UNKNOWN;
    public const int END_ELEMENT = UNKNOWN;
    public const int END_ENTITY = UNKNOWN;
    public const int XML_DECLARATION = UNKNOWN;
    public const int LOADDTD = UNKNOWN;
    public const int DEFAULTATTRS = UNKNOWN;
    public const int VALIDATE = UNKNOWN;
    public const int SUBST_ENTITIES = UNKNOWN;

    public int $attributeCount;

    public string $baseURI;

    public int $depth;

    public bool $hasAttributes;

    public bool $hasValue;

    public bool $isDefault;

    public bool $isEmptyElement;

    public string $localName;

    public string $name;

    public string $namespaceURI;

    public int $nodeType;

    public string $prefix;

    public string $value;

    public string $xmlLang;

    /**
     * Closes the reader and releases the underlying libxml resources. The reader is left unusable
     * until open(), XML(), or one of the fromX() constructors is called again.
     */
    public function close(): true {}

    /**
     * Returns the value of the attribute named $name on the current element, or null if no such
     * attribute exists or the current node is not an element. Unlike moveToAttribute(), this does
     * not move the cursor.
     */
    public function getAttribute(string $name): ?string {}

    /**
     * Returns the value of the attribute at the zero-based $index on the current element, or null
     * if the index is out of range. This does not move the cursor.
     */
    public function getAttributeNo(int $index): ?string {}

    /**
     * Returns the value of the attribute named $name in the namespace $namespace on the current
     * element, or null if no such attribute exists. This does not move the cursor.
     */
    public function getAttributeNs(string $name, string $namespace): ?string {}

    /**
     * Returns whether the parser property $property, one of XMLReader::LOADDTD, DEFAULTATTRS,
     * VALIDATE or SUBST_ENTITIES, is currently enabled.
     */
    public function getParserProperty(int $property): bool {}

    /**
     * Reports whether the document read so far conforms to whichever DTD or schema validation has
     * been enabled, via setParserProperty(XMLReader::VALIDATE, true), setSchema(), or a RelaxNG
     * schema. The result is meaningless if no validation was enabled.
     */
    public function isValid(): bool {}

    /**
     * Returns the namespace URI bound to $prefix in the scope of the current node, or null if the
     * prefix is not bound there.
     */
    public function lookupNamespace(string $prefix): ?string {}

    /**
     * Moves the cursor onto the attribute named $name of the current element, so that subsequent
     * reads of the node's name and value apply to the attribute rather than the element. Returns
     * false and leaves the cursor on the element if no such attribute exists. Call moveToElement()
     * to return to the element afterwards.
     */
    public function moveToAttribute(string $name): bool {}

    /**
     * Moves the cursor onto the attribute at the zero-based $index of the current element. Returns
     * false and leaves the cursor unchanged if the index is out of range.
     */
    public function moveToAttributeNo(int $index): bool {}

    /**
     * Moves the cursor onto the attribute named $name in the namespace $namespace of the current
     * element. Returns false and leaves the cursor unchanged if no such attribute exists.
     */
    public function moveToAttributeNs(string $name, string $namespace): bool {}

    /**
     * Moves the cursor back from an attribute to the element that owns it. Returns false if the
     * cursor is not currently positioned on an attribute.
     */
    public function moveToElement(): bool {}

    /**
     * Moves the cursor to the first attribute of the current element. Returns false if the current
     * node has no attributes.
     */
    public function moveToFirstAttribute(): bool {}

    /**
     * Moves the cursor to the next attribute of the current element, following a previous
     * moveToFirstAttribute() or moveToAttribute() call. Returns false once there are no more
     * attributes.
     */
    public function moveToNextAttribute(): bool {}

    /**
     * Reads the next node in the document and moves the cursor onto it, discarding whatever the
     * previous read() left behind. Returns false at the end of the document or on a parse error.
     * This is the loop that drives parsing: almost every other query method reports on whatever
     * node the most recent read() left the cursor on.
     */
    public function read(): bool {}

    /**
     * Skips over the current node's children entirely and moves the cursor to its next sibling,
     * rather than descending into it as read() would. When $name is given, skips forward to the
     * next sibling element with that name. Returns false once there is no such sibling.
     */
    public function next(?string $name = null): bool {}

    /**
     * Creates an XMLReader for the document at $uri, a local path or any URI understood by a
     * registered stream wrapper, or returns false on failure. $flags accepts the LIBXML_* constants
     * understood by libxml2, such as LIBXML_NOENT or LIBXML_DTDLOAD. Parse errors are reported
     * through libxml_get_errors() when libxml_use_internal_errors() has been enabled, and as a PHP
     * warning otherwise. fromUri() is a newer alternative returning a properly typed instance and
     * throwing instead of returning false.
     *
     * @return bool|XMLReader
     */
    public static function open(string $uri, ?string $encoding = null, int $flags = 0) {}

    /**
     * Creates an XMLReader for the document at $uri, exactly as open() does, except that it returns
     * a properly typed XMLReader instance and throws on failure instead of returning false.
     */
    #[Mago\AvailableSince(80400)]
    public static function fromUri(string $uri, ?string $encoding = null, int $flags = 0): static {}

    /**
     * Creates an XMLReader that reads from the already-open stream resource $stream rather than a
     * URI. $documentUri, if given, sets the base URI used to resolve any relative references within
     * the document. Throws on failure instead of returning false.
     *
     * @param resource $stream
     */
    #[Mago\AvailableSince(80400)]
    public static function fromStream(
        $stream,
        ?string $encoding = null,
        int $flags = 0,
        ?string $documentUri = null,
    ): static {}

    /**
     * Returns the XML markup of the current node's children as a string, without the node's own
     * start and end tags. The cursor is not moved. Returns an empty string if the current node has
     * no children.
     */
    public function readInnerXml(): string {}

    /**
     * Returns the XML markup of the current node together with its own start and end tags and all
     * of its children, as a string. The cursor is not moved.
     */
    public function readOuterXml(): string {}

    /**
     * Returns the current node's text content, its own text together with that of any descendants,
     * with all markup stripped, as a string. The cursor is not moved.
     */
    public function readString(): string {}

    /**
     * Enables validation of the document against the XML Schema (XSD) at $filename, or disables
     * schema validation if $filename is null. Must be called before the first read(). Returns false
     * on failure.
     */
    public function setSchema(?string $filename): bool {}

    /**
     * Enables or disables the parser property $property, one of XMLReader::LOADDTD, DEFAULTATTRS,
     * VALIDATE or SUBST_ENTITIES, for the remainder of parsing. Returns false on failure.
     */
    public function setParserProperty(int $property, bool $value): bool {}

    /**
     * Enables validation of the document against the RELAX NG schema at $filename, or disables it
     * if $filename is null. Must be called before the first read().
     */
    public function setRelaxNGSchema(?string $filename): bool {}

    /**
     * Enables validation of the document against a RELAX NG schema given as the literal string
     * $source, rather than a file, or disables it if $source is null. Must be called before the
     * first read().
     */
    public function setRelaxNGSchemaSource(?string $source): bool {}

    /**
     * Creates an XMLReader that parses the XML document given directly as the string $source,
     * rather than a URI, or returns false on failure. fromString() is a newer alternative returning
     * a properly typed instance and throwing instead of returning false.
     *
     * @return bool|XMLReader
     */
    public static function XML(string $source, ?string $encoding = null, int $flags = 0) {}

    /**
     * Creates an XMLReader that parses the XML document given directly as the string $source,
     * exactly as XML() does, except that it returns a properly typed instance and throws on failure
     * instead of returning false.
     */
    #[Mago\AvailableSince(80400)]
    public static function fromString(string $source, ?string $encoding = null, int $flags = 0): static {}

    /**
     * Returns the current node, together with all of its descendants, as a DOMNode, without moving
     * the cursor; a subsequent read() continues from where the reader was. Returns false on
     * failure. When $baseNode is given, the returned node belongs to that node's owner document
     * instead of a new standalone one, so it can be inserted directly into an existing DOM tree.
     *
     * @code
     * $node = $reader->expand($dom);
     * $dom->appendChild($node);
     * @endcode
     */
    public function expand(?DOMNode $baseNode = null): DOMNode|false {}
}
