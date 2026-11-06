<?php

/**
 * Checks whether every character in the string is a letter or a digit. Returns false for an empty
 * string, as all ctype_* functions do.
 *
 * @pure
 */
function ctype_alnum(string $text): bool {}

/**
 * Returns true if every character in $text is an alphabetic letter and $text is non-empty, and
 * false otherwise. An empty string always returns false, and what counts as a letter depends on the
 * current locale.
 *
 * @pure
 */
function ctype_alpha(string $text): bool {}

/**
 * Returns true if every character of $text is a control character, and false otherwise, including
 * for an empty string.
 *
 * @pure
 */
function ctype_cntrl(string $text): bool {}

/**
 * Determines whether every character in a string is a decimal digit (0-9). Returns false for an
 * empty string.
 *
 * @assert-if-true numeric-string $text
 *
 * @pure
 */
function ctype_digit(string $text): bool {}

/**
 * Checks whether every character in the string is a lowercase ASCII letter, returning false for an
 * empty string or for a string containing anything else.
 *
 * @pure
 */
function ctype_lower(string $text): bool {}

/**
 * Checks whether every character in text is a visible, printable character other than whitespace,
 * so letters, digits, punctuation and symbols pass but spaces, tabs and newlines do not. Returns
 * false for an empty string.
 *
 * @pure
 */
function ctype_graph(string $text): bool {}

/**
 * Checks whether every character in text is printable, meaning it produces visible output —
 * including a plain space — rather than being a control character.
 *
 * @pure
 */
function ctype_print(string $text): bool {}

/**
 * Reports whether every character in the given string is a printable punctuation character,
 * excluding whitespace, letters and digits.
 *
 * @pure
 */
function ctype_punct(string $text): bool {}

/**
 * Reports whether every character in text is whitespace (space, tab, newline, vertical tab, form
 * feed or carriage return). Returns false for an empty string.
 *
 * @pure
 */
function ctype_space(string $text): bool {}

/**
 * Returns true only if every character in text is an uppercase letter, and false for an empty
 * string or one containing any non-uppercase character.
 *
 * @pure
 */
function ctype_upper(string $text): bool {}

/**
 * Checks whether every character in the string is a hexadecimal digit (0-9, a-f, A-F). Returns
 * false for an empty string.
 *
 * @pure
 */
function ctype_xdigit(string $text): bool {}
