<?php

const PREG_PATTERN_ORDER = 1;

const PREG_JIT_STACKLIMIT_ERROR = 6;

const PREG_SET_ORDER = 2;

const PREG_OFFSET_CAPTURE = 256;

const PREG_SPLIT_NO_EMPTY = 1;

const PREG_SPLIT_DELIM_CAPTURE = 2;

const PREG_SPLIT_OFFSET_CAPTURE = 4;

const PREG_GREP_INVERT = 1;

const PREG_NO_ERROR = 0;

const PREG_INTERNAL_ERROR = 1;

const PREG_BACKTRACK_LIMIT_ERROR = 2;

const PREG_RECURSION_LIMIT_ERROR = 3;

const PREG_BAD_UTF8_ERROR = 4;

const PREG_BAD_UTF8_OFFSET_ERROR = 5;

#[Mago\AvailableSince(80000)]
const PREG_UNMATCHED_AS_NULL = 512;

const PCRE_VERSION = '8.31 2012-07-06';

const PCRE_VERSION_MAJOR = 10;

const PCRE_VERSION_MINOR = 42;

const PCRE_JIT_SUPPORT = 1;

/**
 * Searches $subject for a match to $pattern, filling $matches with the captured groups. Returns 1
 * if it matches, 0 if it does not, or false on a regex error.
 *
 * @param-out array<string> $matches
 *
 * @return 0|1|false
 */
function preg_match(string $pattern, string $subject, &$matches = [], int $flags = 0, int $offset = 0): int|false {}

/**
 * Finds every match of $pattern in $subject and returns how many were found. By default $matches
 * groups results by pattern (PREG_PATTERN_ORDER); PREG_SET_ORDER groups them by each full match
 * instead.
 *
 * @param-out array<array<string>> $matches
 *
 * @return non-negative-int|false
 */
function preg_match_all(string $pattern, string $subject, &$matches = null, int $flags = 0, int $offset = 0): int|false {}

/**
 * Performs a regular-expression search and replace, accepting arrays for $pattern, $replacement and
 * $subject to apply several replacements at once. Returns null if a regular-expression error
 * occurs.
 *
 * @param string|array<string> $pattern
 * @param string|array<string> $replacement
 * @param string|array<string> $subject
 *
 * @param-out non-negative-int $count
 *
 * @return ($subject is string ? string|null : array<string>|null)
 *
 * @pure
 */
function preg_replace(
    array|string $pattern,
    array|string $replacement,
    array|string $subject,
    int $limit = -1,
    &$count = null,
): array|string|null {}

/**
 * Performs a regular-expression search and replace, calling back with each match so the callback
 * can compute its replacement. Returns null if a regular-expression error occurs.
 *
 * @param string|array<string> $pattern
 * @param (callable(array<string>): string) $callback
 * @param string|array<string> $subject
 *
 * @param-out int $count
 *
 * @return ($subject is string ? string|null : array<string>|null)
 */
function preg_replace_callback(
    array|string $pattern,
    callable $callback,
    array|string $subject,
    int $limit = -1,
    &$count = null,
    int $flags = 0,
): array|string|null {}

/**
 * Runs several regular-expression search-and-replace passes over subject in one call, using a
 * different callback for each pattern. Each key of pattern is a regular expression and its value is
 * the callback invoked to build the replacement for matches of that pattern.
 *
 * @code
 * preg_replace_callback_array(['/foo/' => fn($m) => strtoupper($m[0])], $subject);
 * @endcode
 *
 * @param array<string, (callable(array<string>): string)> $pattern
 * @param string|array<string> $subject
 *
 * @param-out int $count
 *
 * @return ($subject is string ? string|null : array<string>|null)
 */
function preg_replace_callback_array(
    array $pattern,
    array|string $subject,
    int $limit = -1,
    &$count = null,
    int $flags = 0,
): array|string|null {}

/**
 * Behaves like preg_replace(), except it keeps only the subjects that pattern actually matched,
 * dropping every one that did not instead of returning it unchanged. Given an array subject, the
 * result contains just the matched-and-replaced entries, empty if none matched; given a string
 * subject with no match, the result is null. limit caps replacements per subject, and count
 * receives the total number of replacements made across every subject.
 *
 * @code
 * preg_filter('/^a/', 'X', ['apple', 'banana']); // ['Xpple'], key 0 -- 'banana' is dropped, not kept as-is
 * @endcode
 *
 * @param string|array<string> $pattern
 * @param string|array<string> $replacement
 * @param string|array<string> $subject
 *
 * @param-out int $count
 *
 * @return ($subject is string ? string|null : array<string>|null)
 */
function preg_filter(
    array|string $pattern,
    array|string $replacement,
    array|string $subject,
    int $limit = -1,
    &$count = null,
): array|string|null {}

/**
 * Splits $subject wherever it matches $pattern, returning an array of the pieces in between.
 *
 * @return ($subject is non-empty-string ? list<non-empty-string>|false : list<string>|false)
 *
 * @pure
 */
function preg_split(string $pattern, string $subject, int $limit = -1, int $flags = 0): array|false {}

/**
 * Escapes every character with special meaning in a regular expression so that str can be matched
 * literally. Passing delimiter additionally escapes that character, needed when it is not already
 * one of the characters preg_quote() treats as special.
 *
 * @code
 * preg_quote($str, '/');
 * @endcode
 *
 * @return ($str is non-empty-string ? non-empty-string : string)
 *
 * @pure
 */
function preg_quote(string $str, ?string $delimiter = null): string {}

/**
 * Returns the elements of the array whose value matches the given regular expression, preserving
 * the original keys. The invert flag reverses this, returning only the elements that do not match.
 * Returns false if the pattern is invalid.
 *
 * @template K of array-key
 *
 * @param array<K, string> $array
 *
 * @return array<K, string>|false
 *
 * @pure
 */
function preg_grep(string $pattern, array $array, int $flags = 0): array|false {}

/**
 * Returns the error code from the most recent PCRE function call (preg_match(), preg_replace(), and
 * so on) — PREG_NO_ERROR when the last call succeeded, or a code such as
 * PREG_BACKTRACK_LIMIT_ERROR, PREG_RECURSION_LIMIT_ERROR or PREG_BAD_UTF8_ERROR when it did not.
 */
function preg_last_error(): int {}

/**
 * Returns a human-readable description of the error from the most recent PCRE regular expression
 * function call, such as "No error" when the last call succeeded. It is the message form of
 * preg_last_error(), which returns the same information as a PREG_*_ERROR constant.
 */
#[Mago\AvailableSince(80000)]
function preg_last_error_msg(): string {}
