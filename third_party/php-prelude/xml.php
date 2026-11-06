<?php

final class XMLParser {}

/**
 * Creates a new XML parser that markup can be fed to incrementally with xml_parse(). The optional
 * $encoding names the character encoding used for the character data passed to the parser's
 * callback functions; when omitted, the encoding declared by the document itself is used.
 *
 * @pure
 */
function xml_parser_create(?string $encoding = null): XMLParser {}

/**
 * Creates a new XML parser with namespace support. Reported element and attribute names combine
 * their namespace URI and local name joined by $separator, instead of the local name alone.
 *
 * @pure
 */
function xml_parser_create_ns(?string $encoding = null, string $separator = ':'): XMLParser {}

/**
 * Associates $object with an XML parser so that subsequent xml_set_*_handler() calls can register
 * that object's methods as callbacks instead of free functions.
 *
 * @deprecated
 */
function xml_set_object(XMLParser $parser, object $object): bool {}

/**
 * Registers callbacks that $parser calls at the start and end of every element while xml_parse
 * processes the document. Either handler may be any callable, not only the name of a global
 * function.
 */
function xml_set_element_handler(
    XMLParser $parser,
    callable|string|null $start_handler,
    callable|string|null $end_handler,
): bool {}

/**
 * Registers $handler to be called whenever $parser encounters character data — plain text — between
 * tags. Passing null removes any handler already set.
 */
function xml_set_character_data_handler(XMLParser $parser, callable|string|null $handler): bool {}

/**
 * Registers handler to be called whenever parser encounters a processing instruction, such as
 * <?xml-stylesheet ... ?>, receiving its target and data. Does not fire for the mandatory <?xml ...
 * ?> declaration itself, which the parser handles separately.
 */
function xml_set_processing_instruction_handler(XMLParser $parser, callable|string|null $handler): bool {}

/**
 * Registers handler to be called for any XML markup the parser does not dispatch to a more specific
 * handler — for example the XML declaration or a DOCTYPE declaration. Passing null removes the
 * current default handler.
 */
function xml_set_default_handler(XMLParser $parser, callable|string|null $handler): bool {}

/**
 * Registers the handler invoked when the XML parser encounters a DTD entity declaration with
 * unparsed content, that is, an entity declared with a notation for referencing external non-XML
 * data such as a binary file. The handler can be cleared by passing null.
 */
function xml_set_unparsed_entity_decl_handler(XMLParser $parser, callable|string|null $handler): bool {}

/**
 * Registers handler to be invoked whenever parser encounters a notation declaration in the
 * document's DTD. Rarely needed outside code that inspects or validates a document's DTD directly.
 */
function xml_set_notation_decl_handler(XMLParser $parser, callable|string|null $handler): bool {}

/**
 * Registers handler to be invoked whenever parser encounters a reference to an external entity,
 * such as one declared in an external DTD, letting the handler decide how, or whether, to resolve
 * it.
 */
function xml_set_external_entity_ref_handler(XMLParser $parser, callable|string|null $handler): bool {}

/**
 * Registers the callback invoked by the XML parser when it encounters the start of a namespace
 * declaration during xml_parse. Passing null removes the handler.
 */
function xml_set_start_namespace_decl_handler(XMLParser $parser, callable|string|null $handler): bool {}

/**
 * Registers $handler to be called by $parser whenever it reaches the end of an XML namespace
 * declaration's scope — the counterpart of xml_set_start_namespace_decl_handler() — for parsers
 * created with the namespace-aware xml_parser_create_ns(). Passing null removes the handler.
 */
function xml_set_end_namespace_decl_handler(XMLParser $parser, callable|string|null $handler): bool {}

/**
 * Feeds a chunk of data to parser, invoking whatever handlers were registered on it, such as via
 * xml_set_element_handler(). Returns 1 on success and 0 on a parse error, whose details can then be
 * read with xml_get_error_code() and xml_error_string(). is_final should be true on the last chunk,
 * so the parser can detect a truncated document.
 */
function xml_parse(XMLParser $parser, string $data, bool $is_final = false): int {}

/**
 * Parses the XML in $data with $parser, writing the parsed elements into $values and, into $index,
 * a map from each tag name to the positions in $values where it occurs. Returns 1 on success and 0
 * on failure, not a boolean.
 */
function xml_parse_into_struct(XMLParser $parser, string $data, array &$values, array &$index = null): int {}

/**
 * Returns the current error code for $parser as one of the XML_ERROR_* constants; pass it to
 * xml_error_string() for a human-readable message.
 *
 * @pure
 */
function xml_get_error_code(XMLParser $parser): int {}

/**
 * Returns a human-readable description of an XML parser error code, such as one returned by
 * xml_get_error_code(), or null if the code is not recognised.
 *
 * @pure
 */
function xml_error_string(int $error_code): ?string {}

/**
 * Returns the line number the given parser is currently at within the document being parsed.
 * Intended to be called from inside a handler registered on the parser, to report where in the
 * source an error or event occurred.
 *
 * @pure
 */
function xml_get_current_line_number(XMLParser $parser): int {}

/**
 * Returns the column number the underlying XML parser is currently positioned at, valid only when
 * called from inside one of that parser's own handler callbacks during parsing.
 *
 * @pure
 */
function xml_get_current_column_number(XMLParser $parser): int {}

/**
 * Returns how many bytes of the document $parser has consumed so far, meaningful only while called
 * from inside one of that parser's own handler callbacks.
 *
 * @pure
 */
function xml_get_current_byte_index(XMLParser $parser): int {}

/**
 * Releases the resources held by $parser. Since PHP 8.0, an XMLParser is also freed automatically
 * once it is garbage collected, so this call is optional but still safe to make explicitly.
 */
#[Deprecated(since: '8.5', message: 'as it has no effect since PHP 8.0')]
function xml_parser_free(XMLParser $parser): bool {}

/**
 * Sets an option, such as XML_OPTION_CASE_FOLDING or XML_OPTION_TARGET_ENCODING, on parser. Returns
 * false if the option is unknown or cannot be changed given the parser's current state, for example
 * after parsing has already begun.
 */
function xml_parser_set_option(XMLParser $parser, int $option, mixed $value): bool {}

/**
 * Returns the current value of the given parser option, such as XML_OPTION_CASE_FOLDING or
 * XML_OPTION_TARGET_ENCODING, as previously set with xml_parser_set_option or defaulted when the
 * parser was created.
 *
 * @pure
 */
function xml_parser_get_option(XMLParser $parser, int $option): string|int|bool {}

const XML_ERROR_NONE = 0;

const XML_ERROR_NO_MEMORY = 1;

const XML_ERROR_SYNTAX = 2;

const XML_ERROR_NO_ELEMENTS = 3;

const XML_ERROR_INVALID_TOKEN = 4;

const XML_ERROR_UNCLOSED_TOKEN = 5;

const XML_ERROR_PARTIAL_CHAR = 6;

const XML_ERROR_TAG_MISMATCH = 7;

const XML_ERROR_DUPLICATE_ATTRIBUTE = 8;

const XML_ERROR_JUNK_AFTER_DOC_ELEMENT = 9;

const XML_ERROR_PARAM_ENTITY_REF = 10;

const XML_ERROR_UNDEFINED_ENTITY = 11;

const XML_ERROR_RECURSIVE_ENTITY_REF = 12;

const XML_ERROR_ASYNC_ENTITY = 13;

const XML_ERROR_BAD_CHAR_REF = 14;

const XML_ERROR_BINARY_ENTITY_REF = 15;

const XML_ERROR_ATTRIBUTE_EXTERNAL_ENTITY_REF = 16;

const XML_ERROR_MISPLACED_XML_PI = 17;

const XML_ERROR_UNKNOWN_ENCODING = 18;

const XML_ERROR_INCORRECT_ENCODING = 19;

const XML_ERROR_UNCLOSED_CDATA_SECTION = 20;

const XML_ERROR_EXTERNAL_ENTITY_HANDLING = 21;

const XML_OPTION_CASE_FOLDING = 1;

const XML_OPTION_TARGET_ENCODING = 2;

const XML_OPTION_SKIP_TAGSTART = 3;

const XML_OPTION_SKIP_WHITE = 4;

const XML_OPTION_PARSE_HUGE = 5;

const XML_SAX_IMPL = 'libxml';
