<?php

/**
 * Creates a new XMLWriter that writes its output directly to the given URI, such as a file path or
 * php://output, rather than buffering it in memory for later retrieval. Returns false on failure.
 */
function xmlwriter_open_uri(string $uri): XMLWriter|false {}

/**
 * Creates a new XMLWriter instance that buffers its output in memory rather than writing to a file
 * or URI. Call xmlwriter_output_memory() to retrieve the buffered document text. Returns false on
 * failure.
 */
function xmlwriter_open_memory(): XMLWriter|false {}

/**
 * Turns pretty-printed indentation of writer's output on or off; the string used for each indent
 * level is set separately with xmlwriter_set_indent_string().
 */
function xmlwriter_set_indent(XMLWriter $writer, bool $enable): bool {}

/**
 * Sets the string written for each level of indentation once indenting has been enabled on the
 * writer.
 */
function xmlwriter_set_indent_string(XMLWriter $writer, string $indentation): bool {}

/**
 * Opens a new XML comment node on $writer; whatever is written until the matching
 * xmlwriter_end_comment() call becomes the comment's content.
 */
function xmlwriter_start_comment(XMLWriter $writer): bool {}

/**
 * Closes the XML comment most recently opened with xmlwriter_start_comment(). Returns false on
 * failure.
 */
function xmlwriter_end_comment(XMLWriter $writer): bool {}

/**
 * Starts a new attribute named $name on the currently open element; the procedural equivalent of
 * XMLWriter::startAttribute(). The attribute is completed by calling xmlwriter_end_attribute().
 */
function xmlwriter_start_attribute(XMLWriter $writer, string $name): bool {}

/**
 * Closes the attribute most recently opened with xmlwriter_start_attribute(), writing its closing
 * quote.
 */
function xmlwriter_end_attribute(XMLWriter $writer): bool {}

/**
 * Writes a complete attribute -- name and value together -- onto the element currently open for
 * writing, the procedural equivalent of XMLWriter::writeAttribute(). To build an attribute's value
 * up incrementally instead, use xmlwriter_start_attribute(), xmlwriter_text() and
 * xmlwriter_end_attribute().
 */
function xmlwriter_write_attribute(XMLWriter $writer, string $name, string $value): bool {}

/**
 * Starts a namespaced attribute on the currently open element, to be finished with
 * xmlwriter_end_attribute(). $prefix may be null to write the attribute without a namespace prefix.
 */
function xmlwriter_start_attribute_ns(XMLWriter $writer, ?string $prefix, string $name, ?string $namespace): bool {}

/**
 * Writes a complete namespaced attribute in one call, equivalent to starting the attribute, writing
 * its text content and ending it, using $prefix and $namespace as the attribute's namespace prefix
 * and URI and $value as its content.
 *
 * @code
 * xmlwriter_write_attribute_ns($writer, 'xsi', 'type', 'http://www.w3.org/2001/XMLSchema-instance', 'string');
 * @endcode
 */
function xmlwriter_write_attribute_ns(
    XMLWriter $writer,
    ?string $prefix,
    string $name,
    ?string $namespace,
    string $value,
): bool {}

/**
 * Procedural form of XMLWriter::startElement: opens a new element named $name in the document being
 * written by $writer, to be closed later with xmlwriter_end_element or xmlwriter_full_end_element.
 */
function xmlwriter_start_element(XMLWriter $writer, string $name): bool {}

/**
 * Closes the innermost element still open on $writer, writing its end tag. This is the procedural
 * equivalent of XMLWriter::endElement().
 */
function xmlwriter_end_element(XMLWriter $writer): bool {}

function xmlwriter_full_end_element(XMLWriter $writer): bool {}

/**
 * Opens a new namespaced element named name, bound to namespace under prefix; procedural equivalent
 * of XMLWriter::startElementNs. Returns false on failure.
 */
function xmlwriter_start_element_ns(XMLWriter $writer, ?string $prefix, string $name, ?string $namespace): bool {}

/**
 * Writes a complete element in a single call: the opening tag, the given text content, and the
 * closing tag. If content is omitted, the element is written empty.
 */
function xmlwriter_write_element(XMLWriter $writer, string $name, ?string $content = null): bool {}

/**
 * Writes a complete namespaced element in one call -- start tag with the given prefix, namespace
 * URI and name, optional text content, and matching end tag -- equivalent to pairing
 * xmlwriter_start_element_ns() and xmlwriter_end_element() by hand.
 */
function xmlwriter_write_element_ns(
    XMLWriter $writer,
    ?string $prefix,
    string $name,
    ?string $namespace,
    ?string $content = null,
): bool {}

/**
 * Opens a processing instruction with the given target name, such as xml-stylesheet, which must
 * later be closed with xmlwriter_end_pi() after its content is written.
 */
function xmlwriter_start_pi(XMLWriter $writer, string $target): bool {}

/**
 * Closes the processing instruction most recently opened with xmlwriter_start_pi.
 */
function xmlwriter_end_pi(XMLWriter $writer): bool {}

/**
 * Writes a complete XML processing instruction, <?$target $content?>, in a single call, rather than
 * pairing separate start and end calls.
 */
function xmlwriter_write_pi(XMLWriter $writer, string $target, string $content): bool {}

/**
 * Opens a CDATA section in the document writer is building; the section must later be closed with
 * xmlwriter_end_cdata(). Returns false on failure.
 */
function xmlwriter_start_cdata(XMLWriter $writer): bool {}

/**
 * Closes a CDATA section previously opened with xmlwriter_start_cdata(); the procedural equivalent
 * of XMLWriter::endCData().
 */
function xmlwriter_end_cdata(XMLWriter $writer): bool {}

/**
 * Writes a complete CDATA section containing $content in one call, equivalent to starting a CDATA
 * section, writing the text, and ending it.
 */
function xmlwriter_write_cdata(XMLWriter $writer, string $content): bool {}

/**
 * Writes a text node into whichever element or attribute is currently open, the procedural
 * equivalent of XMLWriter::text(). Special characters are escaped automatically.
 */
function xmlwriter_text(XMLWriter $writer, string $content): bool {}

/**
 * Writes the given content directly into the document as raw markup, without escaping any special
 * characters, unlike xmlwriter_text().
 */
function xmlwriter_write_raw(XMLWriter $writer, string $content): bool {}

/**
 * Writes the XML declaration that opens a document, with the given version, encoding and standalone
 * attributes. Typically the first call made on a writer.
 */
function xmlwriter_start_document(
    XMLWriter $writer,
    ?string $version = '1.0',
    ?string $encoding = null,
    ?string $standalone = null,
): bool {}

/**
 * Procedural form of XMLWriter::endDocument: closes any elements $writer still has open and ends
 * the document.
 */
function xmlwriter_end_document(XMLWriter $writer): bool {}

/**
 * Writes a complete XML comment containing $content in a single call, without needing separate
 * start, text and end steps. This is the procedural equivalent of XMLWriter::writeComment().
 */
function xmlwriter_write_comment(XMLWriter $writer, string $content): bool {}

/**
 * Starts a DTD (Document Type Definition) section for qualifiedName, to be closed with
 * xmlwriter_end_dtd(). $publicId and $systemId, if given, add an external PUBLIC or SYSTEM
 * reference to the declaration.
 */
function xmlwriter_start_dtd(
    XMLWriter $writer,
    string $qualifiedName,
    ?string $publicId = null,
    ?string $systemId = null,
): bool {}

/**
 * Closes a DTD block previously opened with xmlwriter_start_dtd. Returns false on failure.
 */
function xmlwriter_end_dtd(XMLWriter $writer): bool {}

/**
 * Writes a DOCTYPE declaration for the given root element name, with optional public and system
 * identifiers and inline declaration content.
 */
function xmlwriter_write_dtd(
    XMLWriter $writer,
    string $name,
    ?string $publicId = null,
    ?string $systemId = null,
    ?string $content = null,
): bool {}

/**
 * Opens an element declaration inside a document type definition, to be closed with
 * xmlwriter_end_dtd_element(); xmlwriter_write_dtd_element() is the one-call shorthand when there
 * is no content to add in between.
 */
function xmlwriter_start_dtd_element(XMLWriter $writer, string $qualifiedName): bool {}

/**
 * Closes a DTD element declaration previously opened with xmlwriter_start_dtd_element(), as
 * distinct from xmlwriter_write_dtd_element(), which writes a complete element declaration in one
 * call and needs no matching end.
 */
function xmlwriter_end_dtd_element(XMLWriter $writer): bool {}

/**
 * Writes a complete DTD element declaration with the given name and content in a single call.
 */
function xmlwriter_write_dtd_element(XMLWriter $writer, string $name, string $content): bool {}

/**
 * Opens an ATTLIST declaration, <!ATTLIST $name ...>, inside a DTD on $writer; the attribute
 * definitions written until the matching xmlwriter_end_dtd_attlist() call become its content.
 */
function xmlwriter_start_dtd_attlist(XMLWriter $writer, string $name): bool {}

/**
 * Closes the DTD attribute-list declaration most recently opened with
 * xmlwriter_start_dtd_attlist(). Returns false on failure.
 */
function xmlwriter_end_dtd_attlist(XMLWriter $writer): bool {}

/**
 * Writes a complete ATTLIST declaration into the document's DTD, of the form <!ATTLIST name
 * content>. $content is written verbatim, with no validation or escaping.
 */
function xmlwriter_write_dtd_attlist(XMLWriter $writer, string $name, string $content): bool {}

/**
 * Starts a DTD entity declaration; the entity's replacement text is written afterwards with
 * xmlwriter_text() and the declaration is closed with xmlwriter_end_dtd_entity(). $isParam declares
 * a parameter entity (the % form) instead of a general entity.
 */
function xmlwriter_start_dtd_entity(XMLWriter $writer, string $name, bool $isParam): bool {}

/**
 * Closes a DTD entity declaration previously opened with xmlwriter_start_dtd_entity(), the
 * procedural equivalent of XMLWriter::endDtdEntity().
 */
function xmlwriter_end_dtd_entity(XMLWriter $writer): bool {}

/**
 * Writes a complete DTD entity declaration. Setting $isParam declares a parameter entity, usable
 * only within the DTD itself, rather than a general entity usable in the document body.
 */
function xmlwriter_write_dtd_entity(
    XMLWriter $writer,
    string $name,
    string $content,
    bool $isParam = false,
    ?string $publicId = null,
    ?string $systemId = null,
    ?string $notationData = null,
): bool {}

/**
 * Returns the XML markup written so far to an XMLWriter created with xmlwriter_open_memory(). When
 * $flush is true, the default, the internal buffer is also cleared, so a later call returns only
 * content written since the previous call; passing false leaves the buffer intact and returns the
 * same accumulated content again.
 */
function xmlwriter_output_memory(XMLWriter $writer, bool $flush = true): string {}

/**
 * Procedural form of XMLWriter::flush. For a writer opened with xmlwriter_open_memory this returns
 * the markup generated so far as a string; for one opened with xmlwriter_open_uri it returns the
 * number of bytes written. Leaving $empty at its default of true clears the internal buffer
 * afterwards, so repeated calls each return only what was written since the previous flush.
 */
function xmlwriter_flush(XMLWriter $writer, bool $empty = true): string|int {}

class XMLWriter
{
    public function openUri(string $uri): bool {}

    #[Mago\AvailableSince(80400)]
    public static function toUri(string $uri): static {}

    public function openMemory(): bool {}

    public static function toMemory(): static {}

    /** @param resource $stream */
    #[Mago\AvailableSince(80400)]
    public static function toStream($stream): static {}

    /**
     * Turns indentation of the generated XML on or off. Only affects output written after this
     * call, and works together with setIndentString, which chooses the string used for each level.
     */
    public function setIndent(bool $enable): bool {}

    /**
     * Sets the string used for one level of indentation, such as two spaces or a tab. Has no effect
     * unless indentation has been turned on with setIndent.
     */
    public function setIndentString(string $indentation): bool {}

    /**
     * Opens an XML comment. Any text or raw content written afterwards becomes the comment's body
     * until endComment closes it.
     */
    public function startComment(): bool {}

    /**
     * Closes the comment opened by startComment.
     */
    public function endComment(): bool {}

    /**
     * Opens a new attribute with the given name on the currently open element. Text written
     * afterwards becomes the attribute's value until endAttribute closes it.
     */
    public function startAttribute(string $name): bool {}

    /**
     * Closes the attribute opened by startAttribute.
     */
    public function endAttribute(): bool {}

    /**
     * Writes a complete attribute, name and value, on the currently open element in a single call.
     */
    public function writeAttribute(string $name, string $value): bool {}

    /**
     * Opens a namespaced attribute, under the given prefix and namespace URI, on the currently open
     * element. Text written afterwards becomes its value until endAttribute closes it.
     */
    public function startAttributeNs(?string $prefix, string $name, ?string $namespace): bool {}

    /**
     * Writes a complete namespaced attribute, prefix, name, namespace URI and value, on the
     * currently open element in a single call.
     */
    public function writeAttributeNs(?string $prefix, string $name, ?string $namespace, string $value): bool {}

    /**
     * Opens a new element with the given name. Whatever is written before it is closed, with
     * endElement or fullEndElement, becomes its attributes and content.
     */
    public function startElement(string $name): bool {}

    /**
     * Closes the most recently opened element, writing it self-closed if nothing was written inside
     * it, or with a separate closing tag otherwise.
     */
    public function endElement(): bool {}

    /**
     * Closes the most recently opened element with an explicit closing tag, even when it is empty,
     * rather than the self-closed form endElement would use.
     */
    public function fullEndElement(): bool {}

    /**
     * Opens an element under the given prefix and namespace URI, writing the corresponding xmlns
     * declaration. Closed the same way as any other element, with endElement or fullEndElement.
     */
    public function startElementNs(?string $prefix, string $name, ?string $namespace): bool {}

    /**
     * Writes a complete element in a single call. The element is self-closed when content is null,
     * and written with a separate closing tag around the given text otherwise.
     */
    public function writeElement(string $name, ?string $content = null): bool {}

    /**
     * Namespaced form of writeElement: writes a complete element under the given prefix and
     * namespace URI in a single call, self-closed when content is null.
     */
    public function writeElementNs(?string $prefix, string $name, ?string $namespace, ?string $content = null): bool {}

    /**
     * Opens a processing instruction for the given target, such as xml-stylesheet. Content written
     * afterwards becomes its body until endPi closes it.
     */
    public function startPi(string $target): bool {}

    /**
     * Closes the processing instruction opened by startPi.
     */
    public function endPi(): bool {}

    /**
     * Writes a complete processing instruction, target and content, in a single call.
     */
    public function writePi(string $target, string $content): bool {}

    /**
     * Opens a CDATA section. Text or raw content written afterwards is placed inside it unescaped,
     * until endCdata closes it.
     */
    public function startCdata(): bool {}

    /**
     * Closes the CDATA section opened by startCdata.
     */
    public function endCdata(): bool {}

    /**
     * Writes a complete CDATA section wrapping the given content in a single call.
     */
    public function writeCdata(string $content): bool {}

    /**
     * Writes text at the current position, escaping characters such as less-than and ampersand so
     * the string is safe as XML character data.
     */
    public function text(string $content): bool {}

    /**
     * Writes the given string at the current position exactly as given, with no escaping, so any
     * markup it contains is emitted verbatim.
     *
     * @code
     * $w->writeRaw('<b>already escaped or literal markup</b>');
     * @endcode
     */
    public function writeRaw(string $content): bool {}

    /**
     * Writes the XML declaration and must be the first call made on a fresh writer. Encoding and
     * standalone are left out of the declaration when null, and version defaults to 1.0.
     */
    public function startDocument(
        ?string $version = '1.0',
        ?string $encoding = null,
        ?string $standalone = null,
    ): bool {}

    /**
     * Ends the document, closing any elements, comments, or other constructs that were left open.
     */
    public function endDocument(): bool {}

    /**
     * Writes a complete comment wrapping the given content in a single call.
     */
    public function writeComment(string $content): bool {}

    /**
     * Opens a DOCTYPE declaration for the given root element name, with optional public and system
     * identifiers. Element, attribute-list and entity declarations can be written before endDtd
     * closes it.
     */
    public function startDtd(string $qualifiedName, ?string $publicId = null, ?string $systemId = null): bool {}

    /**
     * Closes the DOCTYPE declaration opened by startDtd.
     */
    public function endDtd(): bool {}

    /**
     * Writes a complete DOCTYPE declaration in a single call, with optional public and system
     * identifiers and an optional internal subset given as content.
     */
    public function writeDtd(
        string $name,
        ?string $publicId = null,
        ?string $systemId = null,
        ?string $content = null,
    ): bool {}

    /**
     * Opens an element declaration for the named element inside a DTD's internal subset. The
     * content model written afterwards is closed with endDtdElement.
     */
    public function startDtdElement(string $qualifiedName): bool {}

    /**
     * Closes the element declaration opened by startDtdElement.
     */
    public function endDtdElement(): bool {}

    /**
     * Writes a complete DTD element declaration, name and content model, in a single call.
     */
    public function writeDtdElement(string $name, string $content): bool {}

    /**
     * Opens an attribute-list declaration for the named element inside a DTD's internal subset,
     * closed with endDtdAttlist.
     */
    public function startDtdAttlist(string $name): bool {}

    /**
     * Closes the attribute-list declaration opened by startDtdAttlist.
     */
    public function endDtdAttlist(): bool {}

    /**
     * Writes a complete DTD attribute-list declaration, element name and attribute definitions, in
     * a single call.
     */
    public function writeDtdAttlist(string $name, string $content): bool {}

    /**
     * Opens an entity declaration inside a DTD's internal subset, closed with endDtdEntity. isParam
     * true declares a parameter entity, usable only within the DTD, and false declares a general
     * entity usable in the document.
     */
    public function startDtdEntity(string $name, bool $isParam): bool {}

    /**
     * Closes the entity declaration opened by startDtdEntity.
     */
    public function endDtdEntity(): bool {}

    /**
     * Writes a complete DTD entity declaration in a single call. isParam selects a parameter entity
     * over a general one, and publicId, systemId and notationData describe an external entity
     * rather than the inline value given by content.
     */
    public function writeDtdEntity(
        string $name,
        string $content,
        bool $isParam = false,
        ?string $publicId = null,
        ?string $systemId = null,
        ?string $notationData = null,
    ): bool {}

    /**
     * Returns the XML written so far by a writer opened with openMemory. The internal buffer is
     * then emptied by default, so a later call only returns what was written since; pass false to
     * keep the buffer and get the whole document again next time.
     */
    public function outputMemory(bool $flush = true): string {}

    /**
     * Flushes the buffered XML. A writer opened with openUri writes the buffered data to its target
     * and returns the number of bytes written; a writer opened with openMemory returns the buffered
     * XML as a string instead, the same as outputMemory. The buffer is emptied afterwards by
     * default; pass false to keep it.
     *
     * @code
     * $out = $w->flush(); // int when opened with openUri, string when opened with openMemory
     * @endcode
     */
    public function flush(bool $empty = true): string|int {}
}
