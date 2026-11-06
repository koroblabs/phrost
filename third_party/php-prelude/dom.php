<?php

namespace {
    /**
     * @var int
     */
    const XML_ELEMENT_NODE = UNKNOWN;

    /**
     * @var int
     */
    const XML_ATTRIBUTE_NODE = UNKNOWN;

    /**
     * @var int
     */
    const XML_TEXT_NODE = UNKNOWN;

    /**
     * @var int
     */
    const XML_CDATA_SECTION_NODE = UNKNOWN;

    /**
     * @var int
     */
    const XML_ENTITY_REF_NODE = UNKNOWN;

    /**
     * @var int
     */
    const XML_ENTITY_NODE = UNKNOWN;

    /**
     * @var int
     */
    const XML_PI_NODE = UNKNOWN;

    /**
     * @var int
     */
    const XML_COMMENT_NODE = UNKNOWN;

    /**
     * @var int
     */
    const XML_DOCUMENT_NODE = UNKNOWN;

    /**
     * @var int
     */
    const XML_DOCUMENT_TYPE_NODE = UNKNOWN;

    /**
     * @var int
     */
    const XML_DOCUMENT_FRAG_NODE = UNKNOWN;

    /**
     * @var int
     */
    const XML_NOTATION_NODE = UNKNOWN;

    /**
     * @var int
     */
    const XML_HTML_DOCUMENT_NODE = UNKNOWN;

    /**
     * @var int
     */
    const XML_DTD_NODE = UNKNOWN;

    /**
     * @var int
     */
    const XML_ELEMENT_DECL_NODE = UNKNOWN;

    /**
     * @var int
     */
    const XML_ATTRIBUTE_DECL_NODE = UNKNOWN;

    /**
     * @var int
     */
    const XML_ENTITY_DECL_NODE = UNKNOWN;

    /**
     * @var int
     */
    const XML_NAMESPACE_DECL_NODE = UNKNOWN;

    /**
     * @var int
     */
    const XML_LOCAL_NAMESPACE = UNKNOWN;

    /**
     * @var int
     */
    const XML_ATTRIBUTE_CDATA = UNKNOWN;

    /**
     * @var int
     */
    const XML_ATTRIBUTE_ID = UNKNOWN;

    /**
     * @var int
     */
    const XML_ATTRIBUTE_IDREF = UNKNOWN;

    /**
     * @var int
     */
    const XML_ATTRIBUTE_IDREFS = UNKNOWN;

    /**
     * @var int
     */
    const XML_ATTRIBUTE_ENTITY = UNKNOWN;

    /**
     * @var int
     */
    const XML_ATTRIBUTE_NMTOKEN = UNKNOWN;

    /**
     * @var int
     */
    const XML_ATTRIBUTE_NMTOKENS = UNKNOWN;

    /**
     * @var int
     */
    const XML_ATTRIBUTE_ENUMERATION = UNKNOWN;

    /**
     * @var int
     */
    const XML_ATTRIBUTE_NOTATION = UNKNOWN;

    /**
     * @var int
     */
    const DOM_PHP_ERR = UNKNOWN;

    /**
     * @var int
     */
    const DOM_INDEX_SIZE_ERR = UNKNOWN;

    /**
     * @var int
     */
    const DOMSTRING_SIZE_ERR = UNKNOWN;

    /**
     * @var int
     */
    const DOM_HIERARCHY_REQUEST_ERR = UNKNOWN;

    /**
     * @var int
     */
    const DOM_WRONG_DOCUMENT_ERR = UNKNOWN;

    /**
     * @var int
     */
    const DOM_INVALID_CHARACTER_ERR = UNKNOWN;

    /**
     * @var int
     */
    const DOM_NO_DATA_ALLOWED_ERR = UNKNOWN;

    /**
     * @var int
     */
    const DOM_NO_MODIFICATION_ALLOWED_ERR = UNKNOWN;

    /**
     * @var int
     */
    const DOM_NOT_FOUND_ERR = UNKNOWN;

    /**
     * @var int
     */
    const DOM_NOT_SUPPORTED_ERR = UNKNOWN;

    /**
     * @var int
     */
    const DOM_INUSE_ATTRIBUTE_ERR = UNKNOWN;

    /**
     * @var int
     */
    const DOM_INVALID_STATE_ERR = UNKNOWN;

    /**
     * @var int
     */
    const DOM_SYNTAX_ERR = UNKNOWN;

    /**
     * @var int
     */
    const DOM_INVALID_MODIFICATION_ERR = UNKNOWN;

    /**
     * @var int
     */
    const DOM_NAMESPACE_ERR = UNKNOWN;

    /**
     * @var int
     */
    const DOM_INVALID_ACCESS_ERR = UNKNOWN;

    /**
     * @var int
     */
    const DOM_VALIDATION_ERR = UNKNOWN;

    class DOMDocumentType extends DOMNode
    {
        /**
         * @readonly
         */
        public string $name;

        /**
         * @readonly
         */
        public DOMNamedNodeMap $entities;

        /**
         * @readonly
         */
        public DOMNamedNodeMap $notations;

        /**
         * @readonly
         */
        public string $publicId;

        /**
         * @readonly
         */
        public string $systemId;

        /**
         * @readonly
         */
        public ?string $internalSubset;
    }

    class DOMCdataSection extends DOMText
    {
        public function __construct(string $data) {}
    }

    class DOMComment extends DOMCharacterData
    {
        public function __construct(string $data = '') {}
    }

    interface DOMParentNode
    {
        /** @param DOMNode|string $nodes */
        public function append(...$nodes): void;

        /**
         * Inserts the given nodes before this node's first child, making them its new first
         * children. Each argument is a DOMNode to insert directly, or a string to insert as a new
         * text node.
         *
         * @param DOMNode|string $nodes
         */
        public function prepend(...$nodes): void;

        /**
         * Removes all of this node's existing children and replaces them with the given nodes. Each
         * argument is a DOMNode to insert directly, or a string to insert as a new text node.
         *
         * @param DOMNode|string $nodes
         */
        public function replaceChildren(...$nodes): void;
    }

    interface DOMChildNode
    {
        /**
         * Removes this node from its parent.
         */
        public function remove(): void;

        /**
         * Inserts the given nodes as siblings immediately before this node, under the same parent.
         * Each argument is a DOMNode to insert directly, or a string to insert as a new text node.
         *
         * @param DOMNode|string $nodes
         */
        public function before(...$nodes): void;

        /**
         * Inserts the given nodes as siblings immediately after this node, under the same parent.
         * Each argument is a DOMNode to insert directly, or a string to insert as a new text node.
         *
         * @param DOMNode|string $nodes
         */
        public function after(...$nodes): void;

        /**
         * Replaces this node with the given nodes in its parent. Each argument is a DOMNode to
         * insert directly, or a string to insert as a new text node.
         *
         * @param DOMNode|string $nodes
         */
        public function replaceWith(...$nodes): void;
    }

    class DOMNode
    {
        public const int DOCUMENT_POSITION_DISCONNECTED = 0x01;
        public const int DOCUMENT_POSITION_PRECEDING = 0x02;
        public const int DOCUMENT_POSITION_FOLLOWING = 0x04;
        public const int DOCUMENT_POSITION_CONTAINS = 0x08;
        public const int DOCUMENT_POSITION_CONTAINED_BY = 0x10;
        public const int DOCUMENT_POSITION_IMPLEMENTATION_SPECIFIC = 0x20;

        /**
         * @readonly
         */
        public string $nodeName;

        public ?string $nodeValue;

        /**
         * @readonly
         */
        public int $nodeType;

        /**
         * @readonly
         */
        public ?DOMNode $parentNode;

        /**
         * @readonly
         */
        public ?DOMElement $parentElement;

        /**
         * @readonly
         * @var DOMNodeList<DOMNode>
         */
        public DOMNodeList $childNodes;

        /**
         * @readonly
         */
        public ?DOMNode $firstChild;

        /**
         * @readonly
         */
        public ?DOMNode $lastChild;

        /**
         * @readonly
         */
        public ?DOMNode $previousSibling;

        /**
         * @readonly
         */
        public ?DOMNode $nextSibling;

        /**
         * @readonly
         * @var DOMNamedNodeMap<DOMAttr>
         */
        public ?DOMNamedNodeMap $attributes;

        /**
         * @readonly
         */
        public bool $isConnected;

        /**
         * @readonly
         */
        public ?DOMDocument $ownerDocument;

        /**
         * @readonly
         */
        public ?string $namespaceURI;

        public string $prefix;

        /**
         * @readonly
         */
        public ?string $localName;

        /**
         * @readonly
         */
        public ?string $baseURI;

        public string $textContent;

        /**
         * Adds node as the last child of this node and returns it. If node already has a parent, it
         * is detached from its current position and moved here rather than copied. node must belong
         * to (or first be imported into, via DOMDocument::importNode) this node's document, or the
         * call fails. Returns false on failure.
         *
         * @template TNode of DOMNode
         *
         * @param TNode $node
         *
         * @return TNode|false
         */
        public function appendChild(DOMNode $node) {}

        /**
         * Returns this node and its subtree serialized as Canonical XML per the W3C XML-C14N
         * specification, or false on failure. exclusive selects Exclusive XML Canonicalization
         * instead of the full form; withComments keeps comment nodes in the output instead of
         * stripping them; xpath, when given, restricts canonicalization to the node set matched by
         * an XPath query, an array with a 'query' key holding the expression and, optionally, a
         * 'namespaces' key mapping prefixes to URIs for use within it; nsPrefixes lists the
         * namespace prefixes to treat as visible, and only matters when exclusive is true.
         *
         * @code
         * $doc->C14N(false, false, ['query' => '(//. | //@* | //namespace::*)']);
         * @endcode
         */
        public function C14N(
            bool $exclusive = false,
            bool $withComments = false,
            ?array $xpath = null,
            ?array $nsPrefixes = null,
        ): string|false {}

        /**
         * Canonicalizes this node and its subtree exactly as C14N does, but writes the result
         * directly to the file at uri instead of returning it as a string. Returns the number of
         * bytes written on success, or false on failure.
         */
        public function C14NFile(
            string $uri,
            bool $exclusive = false,
            bool $withComments = false,
            ?array $xpath = null,
            ?array $nsPrefixes = null,
        ): int|false {}

        /**
         * Creates and returns a copy of this node. When deep is true, all descendant nodes are
         * copied too; when false, only this node itself is copied, with an element's attributes
         * carried over but none of its children. The clone belongs to the same document but starts
         * out detached, with no parent, so it must be attached with appendChild or similar before
         * it appears anywhere. Returns false on failure.
         *
         * @return DOMNode|false
         */
        public function cloneNode(bool $deep = false) {}

        /**
         * Returns the line number in the original source where this node appeared, or 0 for a node
         * that was not parsed from a document, such as one created programmatically with
         * createElement.
         */
        public function getLineNo(): int {}

        /**
         * Returns an XPath-like expression describing this node's position within its document
         * tree, or null if the node is not part of a document, for example a freshly created or
         * already-removed node.
         */
        public function getNodePath(): ?string {}

        /**
         * Returns whether this node carries any attributes. Only a DOMElement can have attributes;
         * every other node kind, including DOMAttr itself, always returns false.
         */
        public function hasAttributes(): bool {}

        /**
         * Returns whether this node has at least one child node.
         */
        public function hasChildNodes(): bool {}

        /**
         * Inserts node into this node's list of children immediately before child, or appends it as
         * the new last child when child is null. As with appendChild, an existing node is moved
         * rather than copied, and it must belong to, or first be imported into, this node's
         * document. Returns the inserted node on success, or false on failure.
         *
         * @template TNode of DOMNode
         *
         * @param TNode $node
         *
         * @return TNode|false
         */
        public function insertBefore(DOMNode $node, ?DOMNode $child = null) {}

        /**
         * Returns whether namespace is the default namespace, the one with no prefix, in scope at
         * this node.
         */
        public function isDefaultNamespace(string $namespace): bool {}

        /**
         * Returns whether otherNode refers to the very same node as this one, by identity, not
         * merely one with equal content. Compare with isEqualNode for structural equality.
         */
        public function isSameNode(DOMNode $otherNode): bool {}

        /**
         * Returns whether otherNode is structurally equal to this node: the same node type, name
         * and value, and, recursively, the same attributes and child nodes, even if it is a
         * distinct object or belongs to a different document. Two different objects can satisfy
         * isEqualNode without satisfying isSameNode.
         */
        public function isEqualNode(?DOMNode $otherNode): bool {}

        /**
         * Reports whether the DOM implementation claims to support the given feature name and
         * version string, from the legacy DOM feature-string registry inherited from the
         * specification. Present for interface compatibility rather than as a capability check
         * anything in this library actually consults.
         */
        public function isSupported(string $feature, string $version): bool {}

        /**
         * Returns the namespace URI bound to prefix as seen from this node, searching up through
         * ancestor scope, or null if no such namespace is in scope. Pass null to look up the
         * default, no-prefix, namespace instead of a specific prefix.
         */
        public function lookupNamespaceURI(?string $prefix): ?string {}

        /**
         * Returns the prefix bound to namespace as seen from this node, searching up through
         * ancestor scope, or null if no prefix for that namespace is in scope.
         */
        public function lookupPrefix(string $namespace): ?string {}

        /**
         * Merges adjacent text nodes and removes empty text nodes throughout the subtree rooted at
         * this node, leaving it in the normalized form the DOM specification describes.
         */
        public function normalize(): void {}

        /**
         * Removes child from this node's list of children and returns it, now detached from the
         * tree. child must be a direct child of this node, or the call fails. Returns false on
         * failure.
         *
         * @template TNode of DOMNode
         *
         * @param TNode $child
         *
         * @return TNode|false
         */
        public function removeChild(DOMNode $child) {}

        /**
         * Replaces child with node in this node's list of children and returns child, now detached
         * from the tree. node must belong to, or first be imported into, this node's document.
         * Returns false on failure.
         *
         * @template TNode of DOMNode
         *
         * @param TNode $child
         *
         * @return TNode|false
         */
        public function replaceChild(DOMNode $node, DOMNode $child) {}

        /**
         * Returns whether other is this node itself or one of its descendants anywhere in the
         * subtree rooted at this node. Returns false when other is null.
         */
        public function contains(DOMNode|DOMNameSpaceNode|null $other): bool {}

        /**
         * Returns the topmost ancestor of this node: the owning DOMDocument if this node is
         * attached to one, or the highest node reachable by following parentNode otherwise.
         */
        public function getRootNode(?array $options = null): DOMNode {}

        /**
         * Returns a bitmask of DOCUMENT_POSITION_* constants describing where other sits relative
         * to this node: whether it precedes or follows this node in document order, contains it or
         * is contained by it, or belongs to a disconnected tree entirely.
         */
        public function compareDocumentPosition(DOMNode $other): int {}

        /**
         * Called by PHP when serialize() is invoked on this object. A DOM node wraps data outside
         * PHP's control and cannot be meaningfully serialized, so this does not hand back a usable
         * property list; serializing a DOMNode is not a supported operation.
         */
        public function __sleep(): array {}

        /**
         * Called by PHP when unserialize() is invoked. Since a DOMNode cannot genuinely be
         * serialized in the first place, this exists only to complete PHP's serialization protocol
         * and is not a supported way to reconstruct a node.
         */
        public function __wakeup(): void {}
    }

    class DOMNameSpaceNode
    {
        /**
         * @readonly
         */
        public string $nodeName;

        /**
         * @readonly
         */
        public ?string $nodeValue;

        /**
         * @readonly
         */
        public int $nodeType;

        /**
         * @readonly
         */
        public string $prefix;

        /**
         * @readonly
         */
        public ?string $localName;

        /**
         * @readonly
         */
        public ?string $namespaceURI;

        /**
         * @readonly
         */
        public bool $isConnected;

        /**
         * @readonly
         */
        public ?DOMDocument $ownerDocument;

        /**
         * @readonly
         */
        public ?DOMNode $parentNode;

        /**
         * @readonly
         */
        public ?DOMElement $parentElement;

        public function __sleep(): array {}

        public function __wakeup(): void {}
    }

    class DOMImplementation
    {
        public function getFeature(string $feature, string $version): never {}

        public function hasFeature(string $feature, string $version): bool {}

        /** @return DOMDocumentType|false */
        public function createDocumentType(string $qualifiedName, string $publicId = '', string $systemId = '') {}

        /**
         * Creates and returns a new DOMDocument, with a document element in the given namespace
         * named qualifiedName when one is supplied. Leave qualifiedName empty, the default, to
         * create a document with no root element yet. A doctype passed in, if any, becomes owned by
         * the new document, so it should not already belong to another one.
         */
        public function createDocument(
            ?string $namespace = null,
            string $qualifiedName = '',
            ?DOMDocumentType $doctype = null,
        ): DOMDocument {}
    }

    class DOMDocumentFragment extends DOMNode implements DOMParentNode
    {
        /**
         * @readonly
         */
        public ?DOMElement $firstElementChild;

        /**
         * @readonly
         */
        public ?DOMElement $lastElementChild;

        /**
         * @readonly
         */
        public int $childElementCount;

        public function __construct() {}

        public function appendXML(string $data): bool {}

        /**
         * Appends each of nodes, DOMNode instances and/or plain strings that are converted to text
         * nodes, to the end of this fragment's list of children, in the order given.
         *
         * @param DOMNode|string $nodes
         */
        public function append(...$nodes): void {}

        /**
         * Inserts each of nodes, DOMNode instances and/or plain strings converted to text nodes, at
         * the start of this fragment's list of children, before its current first child, preserving
         * the given order.
         *
         * @param DOMNode|string $nodes
         */
        public function prepend(...$nodes): void {}

        /**
         * Removes all of this fragment's existing children and replaces them with nodes, DOMNode
         * instances and/or plain strings converted to text nodes.
         *
         * @param DOMNode|string $nodes
         */
        public function replaceChildren(...$nodes): void {}
    }

    /**
     * @template-covariant TNode of DOMNode|DOMNameSpaceNode
     *
     * @template-implements IteratorAggregate<int, TNode>
     * @template-implements ArrayAccess<int, TNode>
     */
    class DOMNodeList implements IteratorAggregate, Countable, ArrayAccess
    {
        /**
         * @readonly
         */
        public int $length;

        public function count(): int {}

        /**
         * @return Iterator<int, TNode>
         */
        public function getIterator(): Iterator {}

        /**
         * Returns the node at the given 0-based index in the list, or null if index is out of
         * range. The list is a live view over the document, so the same call can return a different
         * result after the tree changes.
         *
         * @return TNode|null
         */
        public function item(int $index) {}

        /**
         * Returns whether an item exists at the given 0-based index, so isset($list[$offset]) works
         * as expected.
         *
         * @param int $offset
         *
         * @return bool
         */
        public function offsetExists($offset) {}

        /**
         * Returns the item at the given 0-based index, or null if none exists, so $list[$offset]
         * works as an alternative to item($offset).
         *
         * @param int $offset
         *
         * @return TNode|null
         */
        public function offsetGet($offset) {}

        /**
         * Not supported: a node list is a live, read-only view over the document, so items cannot
         * be assigned by index. Modify the tree through the owning node's own methods, such as
         * appendChild or replaceChild, instead.
         *
         * @param int $offset
         * @param TNode $value
         *
         * @return void
         */
        public function offsetSet($offset, $value) {}

        /**
         * Not supported, for the same reason as offsetSet: a node list is a live, read-only view,
         * so items cannot be removed by index. Use removeChild on the owning node instead.
         *
         * @param int $offset
         *
         * @return void
         */
        public function offsetUnset($offset) {}
    }

    class DOMCharacterData extends DOMNode implements DOMChildNode
    {
        public string $data;

        /**
         * @readonly
         */
        public int $length;

        /**
         * @readonly
         */
        public ?DOMElement $previousElementSibling;

        /**
         * @readonly
         */
        public ?DOMElement $nextElementSibling;

        public function appendData(string $data): true {}

        /**
         * Returns count characters of this node's character data starting at the 0-based offset, or
         * false if offset or count is out of range.
         *
         * @return string|false
         */
        public function substringData(int $offset, int $count) {}

        /**
         * Inserts data into this node's character data starting at the 0-based offset, shifting the
         * existing content after that point rather than overwriting it. Returns true on success,
         * false on failure.
         */
        public function insertData(int $offset, string $data): bool {}

        /**
         * Removes count characters from this node's character data starting at the 0-based offset.
         * Returns true on success, false on failure.
         */
        public function deleteData(int $offset, int $count): bool {}

        /**
         * Replaces count characters of this node's character data, starting at the 0-based offset,
         * with data; the replacement can be a different length than what it replaces. Returns true
         * on success, false on failure.
         */
        public function replaceData(int $offset, int $count, string $data): bool {}

        /**
         * Replaces this node, in its parent's list of children, with nodes, DOMNode instances
         * and/or plain strings converted to text nodes, then removes this node from the tree.
         *
         * @param DOMNode|string $nodes
         */
        public function replaceWith(...$nodes): void {}

        /**
         * Removes this node from its parent's list of children, detaching it from the tree.
         */
        public function remove(): void {}

        /**
         * Inserts each of nodes, DOMNode instances and/or plain strings converted to text nodes, as
         * siblings immediately before this node, in the order given.
         *
         * @param DOMNode|string $nodes
         */
        public function before(...$nodes): void {}

        /**
         * Inserts each of nodes, DOMNode instances and/or plain strings converted to text nodes, as
         * siblings immediately after this node, in the order given.
         *
         * @param DOMNode|string $nodes
         */
        public function after(...$nodes): void {}
    }

    class DOMAttr extends DOMNode
    {
        /**
         * @readonly
         */
        public string $name;

        /**
         * @readonly
         */
        public bool $specified = true;

        public string $value;

        /**
         * @readonly
         */
        public ?DOMElement $ownerElement;

        /**
         * @readonly
         */
        public mixed $schemaTypeInfo = null;

        public function __construct(string $name, string $value = '') {}

        public function isId(): bool {}
    }

    class DOMElement extends DOMNode implements \DOMParentNode, \DOMChildNode
    {
        /**
         * @readonly
         */
        public string $tagName;

        public string $className;

        public string $id;

        /**
         * @readonly
         */
        public mixed $schemaTypeInfo = null;

        /**
         * @readonly
         */
        public ?DOMElement $firstElementChild;

        /**
         * @readonly
         */
        public ?DOMElement $lastElementChild;

        /**
         * @readonly
         */
        public int $childElementCount;

        /**
         * @readonly
         */
        public ?DOMElement $previousElementSibling;

        /**
         * @readonly
         */
        public ?DOMElement $nextElementSibling;

        public function __construct(string $qualifiedName, ?string $value = null, string $namespace = '') {}

        public function getAttribute(string $qualifiedName): string {}

        public function getAttributeNames(): array {}

        public function getAttributeNS(?string $namespace, string $localName): string {}

        /** @return DOMAttr|DOMNameSpaceNode|false */
        public function getAttributeNode(string $qualifiedName) {}

        /**
         * Returns the attribute node identified by the given namespace URI and local name, or null
         * if the element has no matching attribute.
         *
         * @return DOMAttr|DOMNameSpaceNode|null
         */
        public function getAttributeNodeNS(?string $namespace, string $localName) {}

        /**
         * Returns a list of all descendant elements (not including the element itself) with the
         * given tag name, in document order; pass "*" to match every element. The returned
         * DOMNodeList is a snapshot taken at call time and does not update as the document changes
         * afterwards.
         *
         * @return DOMNodeList<DOMElement>
         */
        public function getElementsByTagName(string $qualifiedName): DOMNodeList {}

        /**
         * Returns a list of all descendant elements (not including the element itself) whose
         * namespace URI and local name match the given values, in document order; pass "*" for
         * either argument to match anything. The returned DOMNodeList is a snapshot taken at call
         * time and does not update as the document changes afterwards.
         *
         * @return DOMNodeList<DOMElement>
         */
        public function getElementsByTagNameNS(?string $namespace, string $localName): DOMNodeList {}

        /**
         * Returns whether the element has an attribute with the given qualified name.
         */
        public function hasAttribute(string $qualifiedName): bool {}

        /**
         * Returns whether the element has an attribute with the given namespace URI and local name.
         */
        public function hasAttributeNS(?string $namespace, string $localName): bool {}

        /**
         * Removes the attribute with the given qualified name from the element, returning true on
         * success or false on failure.
         */
        public function removeAttribute(string $qualifiedName): bool {}

        /**
         * Removes the attribute with the given namespace URI and local name from the element.
         */
        public function removeAttributeNS(?string $namespace, string $localName): void {}

        /**
         * Removes the given attribute node from the element and returns it, or false if attr is not
         * an attribute of this element.
         *
         * @return DOMAttr|false
         */
        public function removeAttributeNode(DOMAttr $attr) {}

        /**
         * Sets the value of the attribute with the given qualified name, creating the attribute if
         * it does not already exist.
         *
         * @return DOMAttr|bool
         */
        public function setAttribute(string $qualifiedName, string $value) {}

        /**
         * Sets the value of the attribute identified by namespace URI and qualified name, creating
         * the attribute, and declaring its namespace prefix on the element if needed, if it does
         * not already exist.
         */
        public function setAttributeNS(?string $namespace, string $qualifiedName, string $value): void {}

        /**
         * Adds the given attribute node to the element, replacing and returning any existing
         * attribute of the same name, or null if there was none. attr must not already be an
         * attribute of another element; passing one that is throws DOMException.
         *
         * @throws DOMException
         *
         * @return DOMAttr|null|false
         */
        public function setAttributeNode(DOMAttr $attr) {}

        /**
         * Adds the given attribute node to the element under its own namespace URI and qualified
         * name, replacing and returning any existing attribute of the same name, or null if there
         * was none. attr must not already be an attribute of another element; passing one that is
         * throws DOMException.
         *
         * @throws DOMException
         *
         * @return DOMAttr|null|false
         */
        public function setAttributeNodeNS(DOMAttr $attr) {}

        /**
         * Marks the attribute with the given qualified name as an ID-type attribute on this element
         * for the purposes of DOMDocument::getElementById(), independently of any ID declared in a
         * DTD; pass isId as false to remove the flag.
         */
        public function setIdAttribute(string $qualifiedName, bool $isId): void {}

        /**
         * Marks the attribute identified by namespace URI and qualified name as an ID-type
         * attribute on this element for the purposes of DOMDocument::getElementById(); pass isId as
         * false to remove the flag.
         */
        public function setIdAttributeNS(string $namespace, string $qualifiedName, bool $isId): void {}

        /**
         * Marks the given attribute node as an ID-type attribute on this element for the purposes
         * of DOMDocument::getElementById(); pass isId as false to remove the flag.
         */
        public function setIdAttributeNode(DOMAttr $attr, bool $isId): void {}

        /**
         * Adds the attribute with the given qualified name, with an empty string value, if it is
         * currently absent, or removes it if it is present. Pass force as true to always add it or
         * false to always remove it instead of toggling. Returns true if the attribute is present
         * on the element after the call, false otherwise.
         *
         * @code
         * $el->toggleAttribute('disabled'); // adds it if missing, removes it if present
         * $el->toggleAttribute('disabled', true); // always adds it
         * @endcode
         */
        public function toggleAttribute(string $qualifiedName, ?bool $force = null): bool {}

        /**
         * Removes the element from its parent node; does nothing if it currently has no parent.
         */
        public function remove(): void {}

        /**
         * Inserts the given nodes, each either a DOMNode or a string converted to a DOMText node,
         * as siblings immediately before this element and in the given order, moving rather than
         * copying any node that already belongs to the tree; does nothing if the element currently
         * has no parent.
         *
         * @param DOMNode|string $nodes
         */
        public function before(...$nodes): void {}

        /**
         * Inserts the given nodes, each either a DOMNode or a string converted to a DOMText node,
         * as siblings immediately after this element and in the given order, moving rather than
         * copying any node that already belongs to the tree; does nothing if the element currently
         * has no parent.
         *
         * @param DOMNode|string $nodes
         */
        public function after(...$nodes): void {}

        /**
         * Replaces this element, in its parent's list of children, with the given nodes, each
         * either a DOMNode or a string converted to a DOMText node, moving rather than copying any
         * node that already belongs to the tree; does nothing if the element currently has no
         * parent.
         *
         * @param DOMNode|string $nodes
         */
        public function replaceWith(...$nodes): void {}

        /**
         * Appends the given nodes, each either a DOMNode or a string converted to a DOMText node,
         * as the last children of this element, in the given order, moving rather than copying any
         * node that already belongs to the tree.
         *
         * @param DOMNode|string $nodes
         */
        public function append(...$nodes): void {}

        /**
         * Inserts the given nodes, each either a DOMNode or a string converted to a DOMText node,
         * as the first children of this element, in the given order, moving rather than copying any
         * node that already belongs to the tree.
         *
         * @param DOMNode|string $nodes
         */
        public function prepend(...$nodes): void {}

        /**
         * Removes all existing children of this element and replaces them with the given nodes,
         * each either a DOMNode or a string converted to a DOMText node, moving rather than copying
         * any that already belong to the tree.
         *
         * @param DOMNode|string $nodes
         */
        public function replaceChildren(...$nodes): void {}

        /**
         * Inserts element into the tree at a position relative to this element given by where:
         * "beforebegin" immediately before this element, "afterbegin" as this element's first
         * child, "beforeend" as this element's last child, "afterend" immediately after this
         * element. Moves element rather than copying it; if it already belongs elsewhere in the
         * tree it is detached from its current location first. Returns null instead of inserting
         * when "beforebegin" or "afterend" is requested but this element currently has no parent.
         */
        public function insertAdjacentElement(string $where, DOMElement $element): ?DOMElement {}

        /**
         * Inserts a new text node containing data into the tree at a position relative to this
         * element given by where, using the same four values as insertAdjacentElement:
         * "beforebegin", "afterbegin", "beforeend", "afterend". Does nothing when "beforebegin" or
         * "afterend" is requested but this element currently has no parent.
         */
        public function insertAdjacentText(string $where, string $data): void {}
    }

    class DOMDocument extends DOMNode implements DOMParentNode
    {
        /**
         * @readonly
         */
        public ?DOMDocumentType $doctype;

        /**
         * @readonly
         */
        public DOMImplementation $implementation;

        /**
         * @readonly
         */
        public ?DOMElement $documentElement;

        /**
         * @readonly
         * @deprecated
         */
        public ?string $actualEncoding;

        public ?string $encoding;

        /**
         * @readonly
         */
        public ?string $xmlEncoding;

        public bool $standalone;

        public bool $xmlStandalone;

        public ?string $version;

        public ?string $xmlVersion;

        public bool $strictErrorChecking;

        public ?string $documentURI;

        /**
         * @readonly
         * @deprecated
         */
        public mixed $config;

        public bool $formatOutput;

        public bool $validateOnParse;

        public bool $resolveExternals;

        public bool $preserveWhiteSpace;

        public bool $recover;

        public bool $substituteEntities;

        /**
         * @readonly
         */
        public ?DOMElement $firstElementChild;

        /**
         * @readonly
         */
        public ?DOMElement $lastElementChild;

        /**
         * @readonly
         */
        public int $childElementCount;

        public function __construct(string $version = '1.0', string $encoding = '') {}

        /**
         * Creates a new, detached attribute node with the given name. It is not attached to any
         * element until passed to a method such as DOMElement::setAttributeNode. Returns false if
         * the name is not a valid XML name.
         *
         * @return DOMAttr|false
         */
        public function createAttribute(string $localName) {}

        /**
         * Creates a new, detached attribute node with the given namespace URI and qualified name.
         * It is not attached to any element until passed to a method such as
         * DOMElement::setAttributeNodeNS. Returns false if the namespace URI or qualified name is
         * invalid or mismatched.
         *
         * @return DOMAttr|false
         */
        public function createAttributeNS(?string $namespace, string $qualifiedName) {}

        /**
         * Creates a new, detached CDATA section node containing the given text. Append it to the
         * tree with a method such as appendChild. Returns false on failure.
         *
         * @return DOMCdataSection|false
         */
        public function createCDATASection(string $data) {}

        /**
         * Creates a new, detached comment node containing the given text. Append it to the tree
         * with a method such as appendChild.
         */
        public function createComment(string $data): DOMComment {}

        /**
         * Creates a new, empty, detached document fragment, a lightweight container used to
         * assemble a group of nodes for insertion elsewhere in the tree.
         */
        public function createDocumentFragment(): DOMDocumentFragment {}

        /**
         * Creates a new, detached element node with the given tag name and, if $value is given, a
         * text node child holding it. It is not part of the tree until inserted with a method such
         * as appendChild. Returns false if $localName is not a valid XML name.
         *
         * @code
         * $el = $dom->createElement('p');
         * $dom->appendChild($el);
         * @endcode
         *
         * @return DOMElement|false
         */
        public function createElement(string $localName, string $value = '') {}

        /**
         * Creates a new, detached element node with the given namespace URI and qualified name,
         * and, if $value is given, a text node child holding it. Returns false if the namespace URI
         * or qualified name is invalid or mismatched.
         *
         * @return DOMElement|false
         */
        public function createElementNS(?string $namespace, string $qualifiedName, string $value = '') {}

        /**
         * Creates a new, detached entity reference node for the named entity. Returns false if the
         * name is not a valid XML name.
         *
         * @return DOMEntityReference|false
         */
        public function createEntityReference(string $name) {}

        /**
         * Creates a new, detached processing instruction node with the given target and content.
         * Returns false if the target is not a valid XML name.
         *
         * @return DOMProcessingInstruction|false
         */
        public function createProcessingInstruction(string $target, string $data = '') {}

        /**
         * Creates a new, detached text node containing the given text. Append it to the tree with a
         * method such as appendChild.
         */
        public function createTextNode(string $data): DOMText {}

        /**
         * Returns the element whose id attribute has the given value, or null if none matches. Only
         * recognises attributes actually declared with type ID, typically via a DTD; an attribute
         * merely named id is not enough unless the document declares or validates it as such, which
         * is a common source of surprise when a document is loaded with loadXML rather than
         * validated against its DTD.
         */
        public function getElementById(string $elementId): ?DOMElement {}

        /**
         * Returns a live list of every descendant element with the given tag name, in document
         * order. Pass * to match every element regardless of name. The list reflects later changes
         * to the document rather than being a fixed snapshot.
         *
         * @return DOMNodeList<DOMElement>
         */
        public function getElementsByTagName(string $qualifiedName): DOMNodeList {}

        /**
         * Returns a live list of every descendant element with the given namespace URI and local
         * name, in document order. Pass * for either argument to match any namespace or any local
         * name. The list reflects later changes to the document rather than being a fixed snapshot.
         *
         * @return DOMNodeList<DOMElement>
         */
        public function getElementsByTagNameNS(?string $namespace, string $localName): DOMNodeList {}

        /**
         * Copies a node from another document into this one so it can be inserted into this
         * document's tree; the returned node is a copy and is not yet attached anywhere. When $deep
         * is true the node's descendants are copied too; otherwise only the node itself is copied,
         * though an element's attributes are always copied regardless of $deep. The original node
         * and its owning document are left untouched. Returns false on failure.
         *
         * @return DOMNode|false
         */
        public function importNode(DOMNode $node, bool $deep = false) {}

        /**
         * Loads and parses an XML document from the given file path or URL, replacing the current
         * document's contents. $options is a bitmask of LIBXML_* constants controlling the parser,
         * such as LIBXML_NOBLANKS to strip insignificant whitespace or LIBXML_NONET to disable
         * network access. Returns false on failure; parser warnings and errors are reported through
         * libxml's error handling rather than the return value.
         */
        public function load(string $filename, int $options = 0): bool {}

        /**
         * Parses an XML document from the given string, replacing the current document's contents.
         * $options is a bitmask of LIBXML_* constants controlling the parser. Returns false on
         * failure; parser warnings and errors are reported through libxml's error handling rather
         * than the return value.
         */
        public function loadXML(string $source, int $options = 0): bool {}

        /**
         * Puts the document into a normal form, as though it had just been saved and reloaded:
         * adjacent text nodes are merged and empty text nodes are removed.
         */
        public function normalizeDocument(): void {}

        /**
         * Registers $extendedClass, a subclass of $baseClass, as the class this document
         * instantiates whenever it would otherwise create a plain instance of $baseClass, for
         * example DOMElement on every createElement call. Pass null for $extendedClass to remove a
         * previous registration and revert to the built-in class. Always returns true.
         *
         * @code
         * $dom->registerNodeClass(DOMElement::class, MyElement::class);
         * @endcode
         */
        public function registerNodeClass(string $baseClass, ?string $extendedClass): true {}

        /**
         * Serialises the document as XML and writes it to the given file path. Returns the number
         * of bytes written, or false on failure.
         */
        public function save(string $filename, int $options = 0): int|false {}

        /**
         * Parses an HTML document from the given string using libxml's lenient HTML parser,
         * replacing the current document's contents. $options is a bitmask of LIBXML_* constants,
         * such as LIBXML_HTML_NOIMPLIED to suppress the automatically added html and body elements
         * or LIBXML_HTML_NODEFDTD to suppress the default doctype. Malformed markup is generally
         * recovered from rather than rejected, with problems reported through libxml's error
         * handling. Returns false on failure.
         */
        public function loadHTML(string $source, int $options = 0): bool {}

        /**
         * Parses an HTML document from the given file path or URL using libxml's lenient HTML
         * parser, replacing the current document's contents. Accepts the same LIBXML_* $options as
         * loadHTML. Returns false on failure.
         */
        public function loadHTMLFile(string $filename, int $options = 0): bool {}

        /**
         * Serialises the document, or just the given node and its descendants when $node is
         * provided, as HTML and returns the markup as a string. Returns false on failure.
         */
        public function saveHTML(?DOMNode $node = null): string|false {}

        /**
         * Serialises the document as HTML and writes it to the given file path. Returns the number
         * of bytes written, or false on failure.
         */
        public function saveHTMLFile(string $filename): int|false {}

        /**
         * Serialises the document, or just the given node and its descendants when $node is
         * provided, as XML and returns the markup as a string. $options is a bitmask of LIBXML_*
         * constants, such as LIBXML_NOEMPTYTAG to force explicit closing tags instead of self-
         * closed empty elements. Returns false on failure.
         */
        public function saveXML(?DOMNode $node = null, int $options = 0): string|false {}

        /**
         * Validates the document against the XML Schema (XSD) at the given file path or URL.
         * Returns true if the document is valid; validation errors are reported through libxml's
         * error handling rather than the return value.
         */
        public function schemaValidate(string $filename, int $flags = 0): bool {}

        /**
         * Validates the document against the XML Schema (XSD) given as a string. Returns true if
         * the document is valid; validation errors are reported through libxml's error handling
         * rather than the return value.
         */
        public function schemaValidateSource(string $source, int $flags = 0): bool {}

        /**
         * Validates the document against the RELAX NG schema at the given file path or URL. Returns
         * true if the document is valid; validation errors are reported through libxml's error
         * handling rather than the return value.
         */
        public function relaxNGValidate(string $filename): bool {}

        /**
         * Validates the document against the RELAX NG schema given as a string. Returns true if the
         * document is valid; validation errors are reported through libxml's error handling rather
         * than the return value.
         */
        public function relaxNGValidateSource(string $source): bool {}

        /**
         * Validates the document against the DOCTYPE (DTD) declaration it was loaded with. Returns
         * false if there is no DTD or the document does not conform to it.
         */
        public function validate(): bool {}

        /**
         * Processes every XInclude directive in the document, replacing each xi:include element
         * with the content it references. $options is a bitmask of the same LIBXML_* parser
         * constants accepted by load. Returns the number of substitutions performed, or false on
         * failure.
         */
        public function xinclude(int $options = 0): int|false {}

        /**
         * Moves a node from another document into this one, changing its ownerDocument, without
         * inserting it anywhere in this document's tree; the caller must still attach it, for
         * example with appendChild. Unlike importNode, the original node is taken over rather than
         * copied, and it is removed from its previous parent, if it had one. Returns false on
         * failure.
         */
        public function adoptNode(DOMNode $node): DOMNode|false {}

        /**
         * Appends each given node, or string wrapped in a new text node, as a new child at the end
         * of the document, in the order given.
         *
         * @param DOMNode|string $nodes
         */
        public function append(...$nodes): void {}

        /**
         * Inserts each given node, or string wrapped in a new text node, as a new child at the
         * start of the document, in the order given.
         *
         * @param DOMNode|string $nodes
         */
        public function prepend(...$nodes): void {}

        /**
         * Removes all of the document's existing children and replaces them with the given nodes
         * and strings, each string wrapped in a new text node, in the order given.
         *
         * @param DOMNode|string $nodes
         */
        public function replaceChildren(...$nodes): void {}
    }

    final class DOMException extends Exception
    {
        /**
         * @var int
         */
        public $code = 0;
    }

    class DOMText extends DOMCharacterData
    {
        /**
         * @readonly
         */
        public string $wholeText;

        public function __construct(string $data = '') {}

        public function isWhitespaceInElementContent(): bool {}

        public function isElementContentWhitespace(): bool {}

        /** @return DOMText|false */
        public function splitText(int $offset) {}
    }

    /**
     * @template-covariant TNode of DOMNode
     * @implements IteratorAggregate<string, TNode>
     */
    class DOMNamedNodeMap implements IteratorAggregate, Countable
    {
        /**
         * @readonly
         */
        public int $length;

        /** @return null|TNode */
        public function getNamedItem(string $qualifiedName): ?DOMNode {}

        /**
         * Returns the node in the map whose namespace URI and local name match, or null if none is
         * found.
         *
         * @return null|TNode
         */
        public function getNamedItemNS(?string $namespace, string $localName): ?DOMNode {}

        /**
         * Returns the node at the given position in the map, or null if the index is out of range.
         * The index is 0-based; use $length or iterate the map itself rather than assuming a
         * particular ordering.
         *
         * @return null|TNode
         */
        public function item(int $index): ?DOMNode {}

        /**
         * Returns the number of nodes in the map, the same value as the length property. Implements
         * the Countable interface, so count() works directly on a DOMNamedNodeMap.
         */
        public function count(): int {}

        /**
         * Returns an iterator over the map's nodes, keyed by node name, used implicitly by foreach.
         * Implements IteratorAggregate.
         *
         * @return Iterator<string, TNode>
         */
        public function getIterator(): Iterator {}
    }

    class DOMEntity extends DOMNode
    {
        /**
         * @readonly
         */
        public ?string $publicId;

        /**
         * @readonly
         */
        public ?string $systemId;

        /**
         * @readonly
         */
        public ?string $notationName;

        /**
         * @readonly
         * @deprecated
         */
        public ?string $actualEncoding = null;

        /**
         * @readonly
         * @deprecated
         */
        public ?string $encoding = null;

        /**
         * @readonly
         * @deprecated
         */
        public ?string $version = null;
    }

    class DOMEntityReference extends DOMNode
    {
        public function __construct(string $name) {}
    }

    class DOMNotation extends DOMNode
    {
        /**
         * @readonly
         */
        public string $publicId;

        /**
         * @readonly
         */
        public string $systemId;
    }

    class DOMProcessingInstruction extends DOMNode
    {
        /**
         * @readonly
         */
        public string $target;

        public string $data;

        public function __construct(string $name, string $value = '') {}
    }

    class DOMXPath
    {
        /**
         * @readonly
         */
        public DOMDocument $document;

        public bool $registerNodeNamespaces;

        public function __construct(DOMDocument $document, bool $registerNodeNS = true) {}

        public function evaluate(
            string $expression,
            ?DOMNode $contextNode = null,
            bool $registerNodeNS = true,
        ): mixed {}

        /**
         * @return DOMNodeList<DOMNode>|false
         */
        public function query(string $expression, ?DOMNode $contextNode = null, bool $registerNodeNS = true): mixed {}

        public function registerNamespace(string $prefix, string $namespace): bool {}

        public function registerPhpFunctions(string|array|null $restrict = null): void {}

        public function registerPhpFunctionNS(string $namespaceURI, string $name, callable $callable): void {}

        public static function quote(string $str): string {}
    }

    function dom_import_simplexml(object $node): DOMElement|DOMAttr {}
}

namespace Dom {
    use Countable;
    use Iterator;
    use IteratorAggregate;
    use Mago;

    /**
     * @var int
     */
    const INDEX_SIZE_ERR = UNKNOWN;

    /**
     * @var int
     */
    const STRING_SIZE_ERR = UNKNOWN;

    /**
     * @var int
     */
    const HIERARCHY_REQUEST_ERR = UNKNOWN;

    /**
     * @var int
     */
    const WRONG_DOCUMENT_ERR = UNKNOWN;

    /**
     * @var int
     */
    const INVALID_CHARACTER_ERR = UNKNOWN;

    /**
     * @var int
     */
    const NO_DATA_ALLOWED_ERR = UNKNOWN;

    /**
     * @var int
     */
    const NO_MODIFICATION_ALLOWED_ERR = UNKNOWN;

    /**
     * @var int
     */
    const NOT_FOUND_ERR = UNKNOWN;

    /**
     * @var int
     */
    const NOT_SUPPORTED_ERR = UNKNOWN;

    /**
     * @var int
     */
    const INUSE_ATTRIBUTE_ERR = UNKNOWN;

    /**
     * @var int
     */
    const INVALID_STATE_ERR = UNKNOWN;

    /**
     * @var int
     */
    const SYNTAX_ERR = UNKNOWN;

    /**
     * @var int
     */
    const INVALID_MODIFICATION_ERR = UNKNOWN;

    /**
     * @var int
     */
    const NAMESPACE_ERR = UNKNOWN;

    /**
     * @var int
     */
    const VALIDATION_ERR = UNKNOWN;

    /**
     * @var int
     */
    const HTML_NO_DEFAULT_NS = UNKNOWN;

    interface ParentNode
    {
        public function append(Node|string ...$nodes): void;

        public function prepend(Node|string ...$nodes): void;

        public function replaceChildren(Node|string ...$nodes): void;

        public function querySelector(string $selectors): ?Element;

        public function querySelectorAll(string $selectors): NodeList;
    }

    interface ChildNode
    {
        public function remove(): void;

        public function before(Node|string ...$nodes): void;

        public function after(Node|string ...$nodes): void;

        public function replaceWith(Node|string ...$nodes): void;
    }

    /**
     * @strict-properties
     */
    class Implementation
    {
        public function createDocumentType(string $qualifiedName, string $publicId, string $systemId): DocumentType {}

        public function createDocument(
            ?string $namespace,
            string $qualifiedName,
            ?DocumentType $doctype = null,
        ): XMLDocument {}

        public function createHTMLDocument(?string $title = null): HTMLDocument {}
    }

    /** @strict-properties */
    class Node
    {
        final private function __construct() {}

        /**
         * @readonly
         */
        public int $nodeType;

        /**
         * @readonly
         */
        public string $nodeName;

        /**
         * @readonly
         */
        public string $baseURI;

        /**
         * @readonly
         */
        public bool $isConnected;

        /**
         * @readonly
         */
        public ?Document $ownerDocument;

        public function getRootNode(array $options = []): Node {}

        /**
         * @readonly
         */
        public ?Node $parentNode;

        /**
         * @readonly
         */
        public ?Element $parentElement;

        public function hasChildNodes(): bool {}

        /**
         * @readonly
         * @var NodeList<Node>
         */
        public NodeList $childNodes;

        /**
         * @readonly
         */
        public ?Node $firstChild;

        /**
         * @readonly
         */
        public ?Node $lastChild;

        /**
         * @readonly
         */
        public ?Node $previousSibling;

        /**
         * @readonly
         */
        public ?Node $nextSibling;

        public ?string $nodeValue;
        public ?string $textContent;

        public function normalize(): void {}

        public function cloneNode(bool $deep = false): Node {}

        public function isEqualNode(?Node $otherNode): bool {}

        public function isSameNode(?Node $otherNode): bool {}

        public const int DOCUMENT_POSITION_DISCONNECTED = 0x01;
        public const int DOCUMENT_POSITION_PRECEDING = 0x02;
        public const int DOCUMENT_POSITION_FOLLOWING = 0x04;
        public const int DOCUMENT_POSITION_CONTAINS = 0x08;
        public const int DOCUMENT_POSITION_CONTAINED_BY = 0x10;
        public const int DOCUMENT_POSITION_IMPLEMENTATION_SPECIFIC = 0x20;

        public function compareDocumentPosition(Node $other): int {}

        public function contains(?Node $other): bool {}

        public function lookupPrefix(?string $namespace): ?string {}

        public function lookupNamespaceURI(?string $prefix): ?string {}

        public function isDefaultNamespace(?string $namespace): bool {}

        public function insertBefore(Node $node, ?Node $child): Node {}

        public function appendChild(Node $node): Node {}

        public function replaceChild(Node $node, Node $child): Node {}

        public function removeChild(Node $child): Node {}

        public function getLineNo(): int {}

        public function getNodePath(): string {}

        public function C14N(
            bool $exclusive = false,
            bool $withComments = false,
            ?array $xpath = null,
            ?array $nsPrefixes = null,
        ): string|false {}

        public function C14NFile(
            string $uri,
            bool $exclusive = false,
            bool $withComments = false,
            ?array $xpath = null,
            ?array $nsPrefixes = null,
        ): int|false {}

        public function __sleep(): array {}

        public function __wakeup(): void {}
    }

    /**
     * @template-covariant TNode as Node
     *
     * @template-implements IteratorAggregate<int, TNode>
     */
    class NodeList implements IteratorAggregate, Countable
    {
        /**
         * @readonly
         */
        public int $length;

        public function count(): int {}

        /**
         * @return iterable<int, TNode>
         */
        public function getIterator(): Iterator {}

        /**
         * @return TNode
         */
        public function item(int $index): ?Node {}
    }

    /**
     * @implements IteratorAggregate<array-key, Attr>
     */
    #[Mago\AvailableSince(80400)]
    class NamedNodeMap implements IteratorAggregate, Countable
    {
        /**
         * @readonly
         */
        public int $length;

        public function item(int $index): ?Attr {}

        public function getNamedItem(string $qualifiedName): ?Attr {}

        public function getNamedItemNS(?string $namespace, string $localName): ?Attr {}

        public function count(): int {}

        /**
         * @return Iterator<array-key, Attr>
         */
        public function getIterator(): Iterator {}
    }

    /**
     * @implements IteratorAggregate<string, Entity|Notation>
     */
    class DtdNamedNodeMap implements IteratorAggregate, Countable
    {
        /**
         * @readonly
         */
        public int $length;

        public function item(int $index): Entity|Notation|null {}

        public function getNamedItem(string $qualifiedName): Entity|Notation|null {}

        public function getNamedItemNS(?string $namespace, string $localName): Entity|Notation|null {}

        public function count(): int {}

        /**
         * @return Iterator<string, Entity|Notation>
         */
        public function getIterator(): Iterator {}
    }

    /**
     * @implements IteratorAggregate<array-key, Element>
     */
    #[Mago\AvailableSince(80400)]
    class HTMLCollection implements IteratorAggregate, Countable
    {
        /**
         * @readonly
         */
        public int $length;

        public function item(int $index): ?Element {}

        public function namedItem(string $key): ?Element {}

        public function count(): int {}

        /**
         * @return Iterator<array-key, Element>
         */
        public function getIterator(): Iterator {}
    }

    enum AdjacentPosition: string
    {
        case BeforeBegin = 'beforebegin';
        case AfterBegin = 'afterbegin';
        case BeforeEnd = 'beforeend';
        case AfterEnd = 'afterend';
    }

    class Element extends Node implements ParentNode, ChildNode
    {
        /**
         * @readonly
         */
        public ?string $namespaceURI;

        /**
         * @readonly
         */
        public ?string $prefix;

        /**
         * @readonly
         */
        public string $localName;

        /**
         * @readonly
         */
        public string $tagName;

        public string $id;
        public string $className;

        /**
         * @readonly
         */
        public TokenList $classList;

        public function hasAttributes(): bool {}

        /**
         * @readonly
         */
        public NamedNodeMap $attributes;

        public function getAttributeNames(): array {}

        public function getAttribute(string $qualifiedName): ?string {}

        public function getAttributeNS(?string $namespace, string $localName): ?string {}

        public function setAttribute(string $qualifiedName, string $value): void {}

        public function setAttributeNS(?string $namespace, string $qualifiedName, string $value): void {}

        public function removeAttribute(string $qualifiedName): void {}

        public function removeAttributeNS(?string $namespace, string $localName): void {}

        public function toggleAttribute(string $qualifiedName, ?bool $force = null): bool {}

        public function hasAttribute(string $qualifiedName): bool {}

        public function hasAttributeNS(?string $namespace, string $localName): bool {}

        public function getAttributeNode(string $qualifiedName): ?Attr {}

        public function getAttributeNodeNS(?string $namespace, string $localName): ?Attr {}

        public function setAttributeNode(Attr $attr): ?Attr {}

        public function setAttributeNodeNS(Attr $attr): ?Attr {}

        public function removeAttributeNode(Attr $attr): Attr {}

        public function getElementsByTagName(string $qualifiedName): HTMLCollection {}

        public function getElementsByTagNameNS(?string $namespace, string $localName): HTMLCollection {}

        public function insertAdjacentElement(AdjacentPosition $where, Element $element): ?Element {}

        public function insertAdjacentText(AdjacentPosition $where, string $data): void {}

        /**
         * @readonly
         */
        public ?Element $firstElementChild;

        /**
         * @readonly
         */
        public ?Element $lastElementChild;

        /**
         * @readonly
         */
        public int $childElementCount;

        /**
         * @readonly
         */
        public ?Element $previousElementSibling;

        /**
         * @readonly
         */
        public ?Element $nextElementSibling;

        public function setIdAttribute(string $qualifiedName, bool $isId): void {}

        public function setIdAttributeNS(?string $namespace, string $qualifiedName, bool $isId): void {}

        public function setIdAttributeNode(Attr $attr, bool $isId): void {}

        public function remove(): void {}

        public function before(Node|string ...$nodes): void {}

        public function after(Node|string ...$nodes): void {}

        public function replaceWith(Node|string ...$nodes): void {}

        public function append(Node|string ...$nodes): void {}

        public function prepend(Node|string ...$nodes): void {}

        public function replaceChildren(Node|string ...$nodes): void {}

        public function querySelector(string $selectors): ?Element {}

        public function querySelectorAll(string $selectors): NodeList {}

        public function closest(string $selectors): ?Element {}

        public function matches(string $selectors): bool {}

        public string $innerHTML;

        public string $substitutedNodeValue;

        /** @return list<NamespaceInfo> */
        public function getInScopeNamespaces(): array {}

        /** @return list<NamespaceInfo> */
        public function getDescendantNamespaces(): array {}

        public function rename(?string $namespaceURI, string $qualifiedName): void {}
    }

    #[Mago\AvailableSince(80400)]
    class HTMLElement extends Element {}

    class Attr extends Node
    {
        /**
         * @readonly
         */
        public ?string $namespaceURI;

        /**
         * @readonly
         */
        public ?string $prefix;

        /**
         * @readonly
         */
        public string $localName;

        /**
         * @readonly
         */
        public string $name;

        public string $value;

        /**
         * @readonly
         */
        public ?Element $ownerElement;

        /**
         * @readonly
         */
        public bool $specified = true;

        public function isId(): bool {}

        public function rename(?string $namespaceURI, string $qualifiedName): void {}
    }

    class CharacterData extends Node implements ChildNode
    {
        /**
         * @readonly
         */
        public ?Element $previousElementSibling;

        /**
         * @readonly
         */
        public ?Element $nextElementSibling;

        public string $data;

        /**
         * @readonly
         */
        public int $length;

        public function substringData(int $offset, int $count): string {}

        public function appendData(string $data): void {}

        public function insertData(int $offset, string $data): void {}

        public function deleteData(int $offset, int $count): void {}

        public function replaceData(int $offset, int $count, string $data): void {}

        public function remove(): void {}

        public function before(Node|string ...$nodes): void {}

        public function after(Node|string ...$nodes): void {}

        public function replaceWith(Node|string ...$nodes): void {}
    }

    class Text extends CharacterData
    {
        public function splitText(int $offset): Text {}

        /**
         * @readonly
         */
        public string $wholeText;
    }

    class CDATASection extends Text {}

    class ProcessingInstruction extends CharacterData
    {
        /**
         * @readonly
         */
        public string $target;
    }

    class Comment extends CharacterData {}

    class DocumentType extends Node implements ChildNode
    {
        /**
         * @readonly
         */
        public string $name;

        /**
         * @readonly
         */
        public DtdNamedNodeMap $entities;

        /**
         * @readonly
         */
        public DtdNamedNodeMap $notations;

        /**
         * @readonly
         */
        public string $publicId;

        /**
         * @readonly
         */
        public string $systemId;

        /**
         * @readonly
         */
        public ?string $internalSubset;

        public function remove(): void {}

        public function before(Node|string ...$nodes): void {}

        public function after(Node|string ...$nodes): void {}

        public function replaceWith(Node|string ...$nodes): void {}
    }

    class DocumentFragment extends Node implements ParentNode
    {
        /**
         * @readonly
         */
        public ?Element $firstElementChild;

        /**
         * @readonly
         */
        public ?Element $lastElementChild;

        /**
         * @readonly
         */
        public int $childElementCount;

        public function appendXml(string $data): bool {}

        public function append(Node|string ...$nodes): void {}

        public function prepend(Node|string ...$nodes): void {}

        public function replaceChildren(Node|string ...$nodes): void {}

        public function querySelector(string $selectors): ?Element {}

        public function querySelectorAll(string $selectors): NodeList {}
    }

    class Entity extends Node
    {
        /**
         * @readonly
         */
        public ?string $publicId;

        /**
         * @readonly
         */
        public ?string $systemId;

        /**
         * @readonly
         */
        public ?string $notationName;
    }

    class EntityReference extends Node {}

    class Notation extends Node
    {
        /**
         * @readonly
         */
        public string $publicId;

        /**
         * @readonly
         */
        public string $systemId;
    }

    abstract class Document extends Node implements ParentNode
    {
        /**
         * @readonly
         */
        public Implementation $implementation;
        public string $URL;
        public string $documentURI;
        public string $characterSet;
        public string $charset;
        public string $inputEncoding;

        /**
         * @readonly
         */
        public ?DocumentType $doctype;

        /**
         * @readonly
         */
        public ?Element $documentElement;

        public function getElementsByTagName(string $qualifiedName): HTMLCollection {}

        public function getElementsByTagNameNS(?string $namespace, string $localName): HTMLCollection {}

        public function createElement(string $localName): Element {}

        public function createElementNS(?string $namespace, string $qualifiedName): Element {}

        public function createDocumentFragment(): DocumentFragment {}

        public function createTextNode(string $data): Text {}

        public function createCDATASection(string $data): CDATASection {}

        public function createComment(string $data): Comment {}

        public function createProcessingInstruction(string $target, string $data): ProcessingInstruction {}

        public function importNode(?Node $node, bool $deep = false): Node {}

        public function adoptNode(Node $node): Node {}

        public function createAttribute(string $localName): Attr {}

        public function createAttributeNS(?string $namespace, string $qualifiedName): Attr {}

        /**
         * @readonly
         */
        public ?Element $firstElementChild;

        /**
         * @readonly
         */
        public ?Element $lastElementChild;

        /**
         * @readonly
         */
        public int $childElementCount;

        public function getElementById(string $elementId): ?Element {}

        public function registerNodeClass(string $baseClass, ?string $extendedClass): void {}

        public function schemaValidate(string $filename, int $flags = 0): bool {}

        public function schemaValidateSource(string $source, int $flags = 0): bool {}

        public function relaxNgValidate(string $filename): bool {}

        public function relaxNgValidateSource(string $source): bool {}

        public function append(Node|string ...$nodes): void {}

        public function prepend(Node|string ...$nodes): void {}

        public function replaceChildren(Node|string ...$nodes): void {}

        public function importLegacyNode(\DOMNode $node, bool $deep = false): Node {}

        public function querySelector(string $selectors): ?Element {}

        public function querySelectorAll(string $selectors): NodeList {}

        public ?HTMLElement $body;

        /**
         * @readonly
         */
        public ?HTMLElement $head;
        public string $title;
    }

    #[Mago\AvailableSince(80400)]
    final class HTMLDocument extends Document
    {
        #[Mago\AvailableSince(80400)]
        public static function createEmpty(string $encoding = 'UTF-8'): HTMLDocument {}

        #[Mago\AvailableSince(80400)]
        public static function createFromFile(
            string $path,
            int $options = 0,
            ?string $overrideEncoding = null,
        ): HTMLDocument {}

        #[Mago\AvailableSince(80400)]
        public static function createFromString(
            string $source,
            int $options = 0,
            ?string $overrideEncoding = null,
        ): HTMLDocument {}

        public function saveXml(?Node $node = null, int $options = 0): string|false {}

        public function saveXmlFile(string $filename, int $options = 0): int|false {}

        #[Mago\AvailableSince(80400)]
        public function saveHtml(?Node $node = null): string {}

        public function saveHtmlFile(string $filename): int|false {}
    }

    #[Mago\AvailableSince(80400)]
    final class XMLDocument extends Document
    {
        #[Mago\AvailableSince(80400)]
        public static function createEmpty(string $version = '1.0', string $encoding = 'UTF-8'): XMLDocument {}

        #[Mago\AvailableSince(80400)]
        public static function createFromFile(
            string $path,
            int $options = 0,
            ?string $overrideEncoding = null,
        ): XMLDocument {}

        #[Mago\AvailableSince(80400)]
        public static function createFromString(
            string $source,
            int $options = 0,
            ?string $overrideEncoding = null,
        ): XMLDocument {}

        /**
         * @readonly
         */
        public string $xmlEncoding;

        public bool $xmlStandalone;

        public string $xmlVersion;

        public bool $formatOutput;

        public function createEntityReference(string $name): EntityReference {}

        public function validate(): bool {}

        public function xinclude(int $options = 0): int {}

        public function saveXml(?Node $node = null, int $options = 0): string|false {}

        public function saveXmlFile(string $filename, int $options = 0): int|false {}
    }

    #[Mago\AvailableSince(80400)]
    final class TokenList implements IteratorAggregate, Countable
    {
        private function __construct() {}

        /**
         * @readonly
         */
        public int $length;

        public function item(int $index): ?string {}

        public function contains(string $token): bool {}

        public function add(string ...$tokens): void {}

        public function remove(string ...$tokens): void {}

        public function toggle(string $token, ?bool $force = null): bool {}

        public function replace(string $token, string $newToken): bool {}

        public function supports(string $token): bool {}

        public string $value;

        public function count(): int {}

        public function getIterator(): Iterator {}
    }

    final readonly class NamespaceInfo
    {
        public ?string $prefix;
        public ?string $namespaceURI;
        public Element $element;

        private function __construct() {}
    }

    final class XPath
    {
        /**
         * @readonly
         */
        public Document $document;

        public bool $registerNodeNamespaces;

        public function __construct(Document $document, bool $registerNodeNS = true) {}

        public function evaluate(
            string $expression,
            ?Node $contextNode = null,
            bool $registerNodeNS = true,
        ): null|bool|float|string|NodeList {}

        public function query(string $expression, ?Node $contextNode = null, bool $registerNodeNS = true): NodeList {}

        public function registerNamespace(string $prefix, string $namespace): bool {}

        public function registerPhpFunctions(string|array|null $restrict = null): void {}

        public function registerPhpFunctionNS(string $namespaceURI, string $name, callable $callable): void {}

        public static function quote(string $str): string {}
    }

    function import_simplexml(object $node): Element {}
}
