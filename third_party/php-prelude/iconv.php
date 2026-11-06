<?php

/**
 * Converts a string from one character encoding to another. Returns false if the string contains a
 * byte sequence that is illegal or unmappable in the source or target encoding.
 *
 * @pure
 */
function iconv(string $from_encoding, string $to_encoding, string $string): string|false {}

/**
 * Converts buffered script output between the character encodings configured with
 * iconv_set_encoding(), for use as an output buffering callback passed to ob_start(). $status
 * carries the PHP_OUTPUT_HANDLER_* flags describing which stage of the buffering sequence this call
 * represents.
 *
 * @pure
 */
function ob_iconv_handler(string $contents, int $status): string {}

/**
 * Returns iconv's current internal character-encoding settings, either all three at once or a
 * single one selected by $type.
 *
 * @param 'all'|'input_encoding'|'output_encoding'|'internal_encoding' $type
 *
 * @return false|string|array{input_encoding: string, output_encoding: string, internal_encoding: string}
 *
 * @pure
 */
function iconv_get_encoding(string $type = 'all'): array|string|false {}

/**
 * Sets one of the three default character encodings used by the iconv extension for the rest of the
 * request. $type selects which default to change — input_encoding, output_encoding or
 * internal_encoding — and $encoding gives the character encoding to use for it. Returns false if
 * $type is not one of those three names.
 */
function iconv_set_encoding(string $type, string $encoding): bool {}

/**
 * Returns the number of characters in string according to the given character encoding, or the
 * internal encoding if $encoding is omitted, counting multibyte characters as one character rather
 * than counting bytes. Returns false if the encoding is unsupported or the string contains an
 * invalid byte sequence.
 *
 * @return false|int<0, max>
 *
 * @pure
 */
function iconv_strlen(string $string, ?string $encoding = null): int|false {}

/**
 * Extracts the portion of string starting at character offset offset and length characters long,
 * treating string as encoded in encoding (or the internal encoding if omitted). Unlike substr,
 * offset and length count characters of that encoding rather than bytes. Returns false if the
 * encoding is unsupported or offset is out of range.
 *
 * @pure
 */
function iconv_substr(string $string, int $offset, ?int $length = null, ?string $encoding = null): string|false {}

/**
 * Returns the offset of the first occurrence of needle in haystack, counting multibyte characters
 * rather than bytes, or false if needle does not occur. offset, if given, is also a character count
 * rather than a byte count.
 *
 * @pure
 */
function iconv_strpos(string $haystack, string $needle, int $offset = 0, ?string $encoding = null): int|false {}

/**
 * Finds the offset of the last occurrence of needle in haystack, counting characters in the given
 * encoding (or the internal encoding, if omitted) rather than bytes as strrpos does. Returns false
 * if needle does not occur in haystack.
 *
 * @pure
 */
function iconv_strrpos(string $haystack, string $needle, ?string $encoding = null): int|false {}

/**
 * Composes a MIME header field, such as an email Subject line, from field_name and field_value,
 * encoded per RFC 2047. Within options, scheme selects B (Base64) or Q (quoted-printable) encoding,
 * input-charset and output-charset control the source and target character sets, and line-length
 * and line-break-chars control how the encoded header is wrapped across lines.
 *
 * @code
 * iconv_mime_encode('Subject', $subject, ['scheme' => 'Q', 'output-charset' => 'UTF-8']);
 * @endcode
 *
 * @param array{'scheme'?: string, 'input-charset'?: string, 'output-charset'?: string, 'line-length'?: int, 'line-break-chars'?: string} $options
 *
 * @pure
 */
function iconv_mime_encode(string $field_name, string $field_value, array $options = []): string|false {}

/**
 * Decodes a single MIME header field, such as an email subject line, converting any encoded words
 * to the given target encoding. With mode 0 (the default), decoding stops at the first malformed
 * header field and returns false; a strict flag enforces full RFC compliance, and a continue-on-
 * error flag skips past malformed fields instead of aborting.
 *
 * @param int<0, 3> $mode
 *
 * @pure
 */
function iconv_mime_decode(string $string, int $mode = 0, ?string $encoding = null): string|false {}

/**
 * Decodes every MIME encoded-word header in a raw block of mail headers at once — the multi-header
 * counterpart of iconv_mime_decode() — returning them as an associative array keyed by header name.
 * A header that occurs more than once collects all of its values into a list instead of the later
 * occurrence overwriting the earlier one. $mode is a bitmask: ICONV_MIME_DECODE_STRICT requires
 * headers to conform strictly to RFC 2047, and ICONV_MIME_DECODE_CONTINUE_ON_ERROR skips a
 * malformed header instead of making the whole call fail. Returns false on failure.
 *
 * @code
 * iconv_mime_decode_headers($headers, ICONV_MIME_DECODE_STRICT | ICONV_MIME_DECODE_CONTINUE_ON_ERROR);
 * @endcode
 *
 * @param int<0, 3> $mode
 *
 * @return array<string, string|non-empty-list<string>>|false
 *
 * @pure
 */
function iconv_mime_decode_headers(string $headers, int $mode = 0, ?string $encoding = null): array|false {}

const ICONV_IMPL = 'libiconv';

const ICONV_VERSION = 2.17;

const ICONV_MIME_DECODE_STRICT = 1;

const ICONV_MIME_DECODE_CONTINUE_ON_ERROR = 2;
