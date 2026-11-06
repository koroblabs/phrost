<?php

interface JsonSerializable
{
    /**
     * Called automatically by json_encode() when it encounters an object implementing this
     * interface, in place of encoding the object's own public properties. Must return a value that
     * json_encode() can serialise on its own, such as an array, not a JSON-formatted string.
     *
     * @code
     * public function jsonSerialize(): mixed
     * {
     *     return ['id' => $this->id, 'name' => $this->name];
     * }
     * @endcode
     */
    public function jsonSerialize(): mixed;
}

class JsonException extends Exception {}

/**
 * Returns a JSON-encoded string representation of the given value. Returns false on failure; check
 * json_last_error() for the reason -- unless JSON_THROW_ON_ERROR was passed, which makes it throw
 * a JsonException instead and so never return false.
 *
 * @return ($flags is 4194304 ? non-empty-string : non-empty-string|false)
 */
function json_encode(mixed $value, int $flags = 0, int $depth = 512): string|false {}

/**
 * Decodes a JSON-encoded string into a PHP value. Returns null for both malformed JSON and a
 * literal JSON null, so json_last_error() should be checked to tell the two apart.
 */
function json_decode(string $json, ?bool $associative = null, int $depth = 512, int $flags = 0): mixed {}

/**
 * Returns the error code from the most recent json_encode() or json_decode() call.
 *
 * @pure
 */
function json_last_error(): int {}

/**
 * Returns a human-readable message describing the most recent JSON encode or decode error.
 *
 * @pure
 */
function json_last_error_msg(): string {}

/**
 * Returns true if the given string is syntactically valid JSON. Cheaper than decoding when only
 * validity matters, since it never builds the decoded value.
 *
 * @pure
 *
 * @assert-if-true non-empty-string $json
 */
#[Mago\AvailableSince(80300)]
function json_validate(string $json, int $depth = 512, int $flags = 0): bool {}

const JSON_HEX_TAG = 1;

const JSON_HEX_AMP = 2;

const JSON_HEX_APOS = 4;

const JSON_HEX_QUOT = 8;

const JSON_FORCE_OBJECT = 16;

const JSON_NUMERIC_CHECK = 32;

const JSON_UNESCAPED_SLASHES = 64;

const JSON_PRETTY_PRINT = 128;

const JSON_UNESCAPED_UNICODE = 256;

#[Mago\AvailableSince(80000)]
const JSON_PARTIAL_OUTPUT_ON_ERROR = 512;

const JSON_ERROR_STATE_MISMATCH = 2;

const JSON_ERROR_CTRL_CHAR = 3;

const JSON_ERROR_UTF8 = 5;

const JSON_ERROR_RECURSION = 6;

const JSON_ERROR_INF_OR_NAN = 7;

const JSON_ERROR_UNSUPPORTED_TYPE = 8;

const JSON_ERROR_NONE = 0;

const JSON_ERROR_DEPTH = 1;

const JSON_ERROR_SYNTAX = 4;

const JSON_OBJECT_AS_ARRAY = 1;

const JSON_PARSER_NOTSTRICT = 4;

const JSON_BIGINT_AS_STRING = 2;

const JSON_PRESERVE_ZERO_FRACTION = 1024;

const JSON_UNESCAPED_LINE_TERMINATORS = 2048;

const JSON_INVALID_UTF8_IGNORE = 1048576;

const JSON_INVALID_UTF8_SUBSTITUTE = 2097152;

const JSON_ERROR_INVALID_PROPERTY_NAME = 9;

const JSON_ERROR_UTF16 = 10;

#[Mago\AvailableSince(80000)]
const JSON_THROW_ON_ERROR = 4194304;

#[Mago\AvailableSince(80300)]
const JSON_ERROR_NON_BACKED_ENUM = 11;
