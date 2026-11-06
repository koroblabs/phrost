<?php

const MB_CASE_UPPER = 0;

const MB_CASE_LOWER = 1;

const MB_CASE_TITLE = 2;

#[Mago\AvailableSince(70300)]
const MB_CASE_FOLD = 3;

#[Mago\AvailableSince(70300)]
const MB_CASE_UPPER_SIMPLE = 4;

#[Mago\AvailableSince(70300)]
const MB_CASE_LOWER_SIMPLE = 5;

#[Mago\AvailableSince(70300)]
const MB_CASE_TITLE_SIMPLE = 6;

#[Mago\AvailableSince(70300)]
const MB_CASE_FOLD_SIMPLE = 7;

const MB_ONIGURUMA_VERSION = '6.9.9';

/**
 * Converts the case of $string according to $mode. MB_CASE_UPPER, MB_CASE_LOWER and MB_CASE_TITLE
 * give the obvious case changes, while MB_CASE_FOLD performs Unicode case folding for caseless
 * comparison rather than producing a specific case.
 *
 * @code
 * mb_convert_case('hello world', MB_CASE_TITLE)
 * @endcode
 *
 * @pure
 */
function mb_convert_case(string $string, int $mode, ?string $encoding = null): string {}

/**
 * Converts a string to upper case using multibyte-aware case rules for $encoding, rather than the
 * single-byte rules strtoupper() applies. Uses the internal encoding when $encoding is omitted.
 *
 * @return ($string is non-empty-string ? non-empty-uppercase-string : '')
 *
 * @pure
 */
function mb_strtoupper(string $string, ?string $encoding = null): string {}

/**
 * Converts string to lowercase, treating multibyte characters correctly according to $encoding, or
 * the internal encoding if omitted.
 *
 * @return ($string is non-empty-string ? non-empty-lowercase-string : '')
 *
 * @pure
 */
function mb_strtolower(string $string, ?string $encoding = null): string {}

/**
 * Gets or sets the language mbstring uses for mail-related functions such as mb_send_mail. When
 * $language is null, returns the current language as a string; otherwise sets it and returns true
 * on success or false on failure.
 */
function mb_language(?string $language = null): string|bool {}

/**
 * Gets or sets the internal character encoding used by default throughout the mbstring functions.
 * Called with no argument it returns the current encoding; called with an encoding name it sets it
 * and returns whether the encoding was recognised.
 *
 * @return ($encoding is null ? string : bool)
 */
function mb_internal_encoding(?string $encoding = null): string|bool {}

/**
 * Reports the character encoding mbstring detected in one of the raw HTTP inputs. type selects
 * which one: 'G', 'P' and 'C' for $_GET, $_POST and $_COOKIE respectively, and 'I' for the full
 * list of encodings detected across every input processed so far; omitting type returns whichever
 * input was processed last. Returns false if that input was never processed or no encoding could be
 * detected.
 *
 * @param 'G'|'P'|'C'|'S'|'L'|'I'|null $type
 *
 * @return ($type is 'G'|'P'|'C'|'S'|'L'|null ? string|false : list<string>|false)
 */
function mb_http_input(?string $type = null): array|string|false {}

/**
 * Gets or sets the character encoding mbstring's output buffer handler uses when converting output
 * for HTTP. Passing null returns the current output encoding; passing a value sets it and returns
 * true or false depending on whether the encoding was recognised.
 *
 * @return ($encoding is null ? string : bool)
 */
function mb_http_output(?string $encoding = null): string|bool {}

/**
 * Gets or sets the ordered list of character encodings that mb_detect_encoding tries when guessing
 * an unknown string's encoding. Called with no argument it returns the current order; called with
 * one it sets a new order and returns whether it was accepted.
 *
 * @return ($encoding is null ? list<string> : bool)
 */
function mb_detect_order(array|string|null $encoding = null): array|true {}

/**
 * Gets or sets the character substituted for input bytes that cannot be represented in the target
 * encoding during mbstring conversions. Called with no argument (or null), it returns the current
 * setting — a Unicode codepoint, or one of the special modes "none" (drop the character), "long"
 * (numeric character reference such as U+3042) or "entity" (HTML entity) — as a string or int;
 * called with an argument, it sets the substitute instead and returns a bool.
 *
 * @return ($substitute_character is null ? string|int : bool)
 */
function mb_substitute_character(string|int|null $substitute_character = null): string|int|bool {}

/**
 * Parses a URL-encoded query string into result the way parse_str() does, but decodes it using the
 * current mbstring internal encoding. Returns false on failure, and writes the parsed key-value
 * pairs into result by reference.
 *
 * @param-out array<string, string|array<string|array<mixed>>> $result
 *
 * @pure
 */
function mb_parse_str(string $string, array &$result): bool {}

/**
 * Serves as the output-buffering callback behind mbstring's HTTP output conversion, converting
 * buffered text to the encoding configured for output. It is invoked automatically by the output-
 * buffering system when registered with ob_start(), not called directly by application code.
 *
 * @pure
 */
function mb_output_handler(string $string, int $status): string {}

/**
 * Returns the preferred MIME charset name, suitable for a Content-Type header, for the given
 * internal character encoding name, or false if the encoding has no registered MIME name.
 *
 * @pure
 */
function mb_preferred_mime_name(string $encoding): string|false {}

/**
 * Returns the number of characters in $string under the given (or default internal) encoding,
 * counting characters rather than the bytes that strlen() counts.
 *
 * @return int<0, max>
 *
 * @pure
 */
function mb_strlen(string $string, ?string $encoding = null): int {}

/**
 * Finds the position of the first occurrence of $needle within $haystack, counting multibyte
 * characters rather than bytes. Both $offset and the returned position are in characters; returns
 * false if $needle does not occur.
 *
 * @return int<0,max>|false
 *
 * @pure
 */
function mb_strpos(string $haystack, string $needle, int $offset = 0, ?string $encoding = null): int|false {}

/**
 * Finds the position of the last occurrence of a substring in a string, counting multibyte
 * characters rather than bytes, according to the given or internal encoding. Returns false if the
 * substring is not found.
 *
 * @return int<0,max>|false
 *
 * @pure
 */
function mb_strrpos(string $haystack, string $needle, int $offset = 0, ?string $encoding = null): int|false {}

/**
 * Returns the offset of the first case-insensitive occurrence of $needle in $haystack, searching
 * from $offset onward, or false if it is not found. Like the rest of the mbstring functions, the
 * offset is counted in characters of $encoding (or the internal encoding if omitted), not bytes.
 *
 * @return int<0,max>|false
 *
 * @pure
 */
function mb_stripos(string $haystack, string $needle, int $offset = 0, ?string $encoding = null): int|false {}

/**
 * Case-insensitive, multibyte-aware equivalent of strripos: returns the character offset, not byte
 * offset, of the last occurrence of $needle in $haystack, or false if it does not occur. $offset
 * shifts where the search begins, counted in characters from the start, or, if negative, from the
 * end of the string.
 *
 * @return int<0,max>|false
 *
 * @pure
 */
function mb_strripos(string $haystack, string $needle, int $offset = 0, ?string $encoding = null): int|false {}

/**
 * Finds the first occurrence of $needle in $haystack using multibyte-aware comparison and returns
 * everything from that point to the end of $haystack, or, when $before_needle is true, everything
 * before it instead. Returns false if $needle does not occur in $haystack.
 *
 * @pure
 */
function mb_strstr(
    string $haystack,
    string $needle,
    bool $before_needle = false,
    ?string $encoding = null,
): string|false {}

/**
 * Finds the last occurrence of needle in haystack, multibyte-safe, and returns the matched text
 * through the end of haystack, or false if needle does not occur at all. When $before_needle is
 * true, returns the part of haystack before that last occurrence instead.
 *
 * @code
 * mb_strrchr('a/b/c', '/', true); // 'a/b'
 * @endcode
 *
 * @pure
 */
function mb_strrchr(
    string $haystack,
    string $needle,
    bool $before_needle = false,
    ?string $encoding = null,
): string|false {}

/**
 * Finds the first case-insensitive occurrence of needle in haystack and returns the portion of
 * haystack from that point onward — or, when $before_needle is true, everything before it instead.
 * Returns false if needle does not occur in haystack, regardless of case.
 *
 * @pure
 */
function mb_stristr(
    string $haystack,
    string $needle,
    bool $before_needle = false,
    ?string $encoding = null,
): string|false {}

/**
 * Returns the portion of haystack from the last case-insensitive occurrence of needle onward, or
 * false if needle does not occur. Set before_needle to true to return the portion of the string
 * that precedes the match instead.
 *
 * @pure
 */
function mb_strrichr(
    string $haystack,
    string $needle,
    bool $before_needle = false,
    ?string $encoding = null,
): string|false {}

/**
 * Counts how many times needle occurs in haystack, treating both as sequences of characters in the
 * given encoding (or the internal encoding) rather than bytes as substr_count does.
 *
 * @pure
 */
function mb_substr_count(string $haystack, string $needle, ?string $encoding = null): int {}

/**
 * Returns part of a string, counting $start and $length in characters rather than bytes, so it is
 * safe for multibyte encodings.
 *
 * @pure
 */
function mb_substr(string $string, int $start, ?int $length = null, ?string $encoding = null): string {}

/**
 * Extracts a substring of length bytes starting at byte offset start, in the manner of substr(),
 * but shifts the boundaries so a multi-byte character is never split in half. Unlike mb_substr(),
 * start and length here are measured in bytes rather than characters.
 *
 * @pure
 */
function mb_strcut(string $string, int $start, ?int $length = null, ?string $encoding = null): string {}

/**
 * Returns the display width of a string, counting each halfwidth character as one column and each
 * fullwidth character, such as most East Asian ideographs, as two.
 *
 * @pure
 */
function mb_strwidth(string $string, ?string $encoding = null): int {}

/**
 * Truncates $string, starting at character offset $start, to fit within display $width rather than
 * byte or character count — a full-width character such as CJK text counts as two. If truncation
 * occurs, $trim_marker is appended and itself counted within $width, so a longer marker eats
 * further into the content it marks.
 *
 * @code
 * mb_strimwidth('Hello World', 0, 10, '...');
 * @endcode
 *
 * @pure
 */
function mb_strimwidth(
    string $string,
    int $start,
    int $width,
    string $trim_marker = '',
    ?string $encoding = null,
): string {}

/**
 * Converts a string, or every string in an array, from one character encoding to another. When
 * from_encoding is omitted, the source encoding is detected from the encodings configured for
 * detection rather than assumed. Returns false if the conversion fails.
 *
 * @param string|array<string> $string
 * @param string|array<string>|null $from_encoding
 *
 * @return ($string is string ? string|false : array|false)
 *
 * @pure
 */
function mb_convert_encoding(
    array|string $string,
    string $to_encoding,
    array|string|null $from_encoding = null,
): array|string|false {}

/**
 * Attempts to determine which of the candidate encodings the string is encoded in, trying the
 * encodings in $encodings, or the order configured by mb_detect_order() when $encodings is omitted,
 * and returning the first plausible match. When $strict is true, it returns false instead of a
 * match whenever the result would be ambiguous.
 *
 * @param string|array<string>|null $encodings
 *
 * @pure
 */
function mb_detect_encoding(string $string, array|string|null $encodings = null, bool $strict = false): string|false {}

/**
 * Returns the names of every character encoding supported by mbstring.
 *
 * @return list<string>
 *
 * @pure
 */
function mb_list_encodings(): array {}

/**
 * Returns the alternative names by which a character encoding is also known.
 *
 * @return list<string>
 *
 * @pure
 */
function mb_encoding_aliases(string $encoding): array {}

/**
 * @pure
 */
function mb_convert_kana(string $string, string $mode = 'KV', ?string $encoding = null): string {}

/**
 * Encodes $string for safe use inside an email header by splitting it into MIME encoded-words, so
 * non-ASCII characters survive mail transport, wrapping the result at $indent columns using
 * $newline as the line break. $transfer_encoding selects the encoded-word scheme: "B" for base64 or
 * "Q" for quoted-printable.
 *
 * @pure
 */
function mb_encode_mimeheader(
    string $string,
    ?string $charset = null,
    ?string $transfer_encoding = null,
    string $newline = "\r\n",
    int $indent = 0,
): string {}

/**
 * Decodes a MIME encoded-word header value, such as an email Subject line written as
 * =?UTF-8?B?...?=, back into a plain string.
 *
 * @pure
 */
function mb_decode_mimeheader(string $string): string {}

/**
 * Converts the character encoding of one or more variables in place, working recursively through
 * arrays and object properties. $from_encoding may list several candidate encodings to detect from,
 * or name a single one; each variable is converted to $to_encoding accordingly. Returns whichever
 * encoding was detected as the source, or false on failure.
 *
 * @param string|string[] $from_encoding
 */
function mb_convert_variables(
    string $to_encoding,
    array|string $from_encoding,
    string|array|object &$var,
    string|array|object &...$vars,
): string|false {}

/**
 * Converts characters in string to HTML numeric character references (such as €) according to $map,
 * encoded using $encoding or the internal encoding if omitted. $map is a flat list of four-integer
 * groups - start codepoint, end codepoint, mask and base - each group naming a range of codepoints
 * to convert; codepoints outside every range are left untouched. $hex writes the entities in
 * hexadecimal (&#xNN;) instead of the default decimal (&#NN;) form.
 *
 * @code
 * mb_encode_numericentity($string, [0x80, 0xffff, 0, 0xffff], 'UTF-8');
 * @endcode
 *
 * @param int[] $map
 *
 * @pure
 */
function mb_encode_numericentity(string $string, array $map, ?string $encoding = null, bool $hex = false): string {}

/**
 * Decodes HTML/XML numeric character references (such as Σ or Σ) embedded in string into their
 * corresponding characters, but only for the Unicode code-point ranges listed in map; references
 * for code points outside those ranges are left untouched. string is treated as encoded in
 * encoding, or the internal encoding if omitted.
 *
 * @param int[] $map
 *
 * @pure
 */
function mb_decode_numericentity(string $string, array $map, ?string $encoding = null): string {}

/**
 * Sends an email in the same way as mail, but encodes the subject and message body for the detected
 * or configured mbstring encoding before doing so.
 */
function mb_send_mail(
    string $to,
    string $subject,
    string $message,
    array|string $additional_headers = [],
    ?string $additional_params = null,
): bool {}

/**
 * Returns mbstring's internal runtime configuration. With no argument, or 'all', every setting
 * comes back as one associative array; passing the name of a single setting -- any key of that
 * array, such as 'internal_encoding' or 'http_output' -- returns just that value.
 *
 * @code
 * mb_get_info('internal_encoding');
 * mb_get_info('all');
 * @endcode
 *
 * @return string|int|false|null|array{
 *     internal_encoding: string,
 *     http_input: string,
 *     http_output: string,
 *     http_output_conv_mimetypes: string,
 *     mail_charset: string,
 *     mail_header_encoding: string,
 *     mail_body_encoding: string,
 *     illegal_chars: string,
 *     encoding_translation: string,
 *     language: string,
 *     detect_order: string,
 *     substitute_character: string,
 *     strict_detection: string
 * }
 *
 * @pure
 */
function mb_get_info(string $type = 'all'): array|string|int|false|null {}

/**
 * Checks whether value is validly encoded as encoding, or as the internal encoding when encoding is
 * omitted, returning false as soon as it finds a malformed byte sequence rather than raising an
 * error.
 *
 * @param string|string[]|null $value
 *
 * @pure
 */
function mb_check_encoding(array|string|null $value = null, ?string $encoding = null): bool {}

/**
 * Gets or sets the character encoding used internally by the mbstring regular expression functions
 * (the mb_ereg family), independently of mb_internal_encoding.
 */
function mb_regex_encoding(?string $encoding = null): string|bool {}

/**
 * Sets the default option flags used by the mbstring regular expression functions (the mb_ereg
 * family) whenever they are not given their own, or returns the current defaults when $options is
 * null. Each character in the option string switches on one flag, for example "i" for case-
 * insensitive matching.
 */
function mb_regex_set_options(?string $options = null): string {}

/**
 * Matches a POSIX-style multibyte regular expression against a string using the mbstring internal
 * encoding, and returns whether it matched. On a successful match, the captured groups are written
 * into matches, with the whole match at index 0.
 *
 * @code
 * if (mb_ereg('([a-z]+)([0-9]+)', $string, $matches)) {
 *     // $matches[1], $matches[2]
 * }
 * @endcode
 *
 * @param string[] &$matches
 */
function mb_ereg(string $pattern, string $string, &$matches = null): bool {}

/**
 * Performs a case-insensitive match of the multibyte regular expression $pattern against $string,
 * using mbstring's own POSIX-style regular-expression engine rather than PCRE, and writes any
 * captured subpatterns into $matches by reference.
 *
 * @param string[] &$matches
 */
function mb_eregi(string $pattern, string $string, &$matches = null): bool {}

/**
 * Replaces every match of a multibyte-safe regular expression in $string with $replacement, using
 * mbstring's own regex engine rather than PCRE, and returns false if the pattern fails to compile.
 * The $options string can include i for case-insensitive matching.
 *
 * @pure
 */
function mb_ereg_replace(
    string $pattern,
    string $replacement,
    string $string,
    ?string $options = null,
): string|false|null {}

/**
 * Performs a multibyte regular expression search and replace where the replacement text for each
 * match is produced by calling $callback with the array of matched groups, the mbstring analogue of
 * preg_replace_callback(). Returns false or null on error, such as an invalid pattern.
 *
 * @code
 * mb_ereg_replace_callback('[a-z]+', fn($m) => strtoupper($m[0]), $text);
 * @endcode
 */
function mb_ereg_replace_callback(
    string $pattern,
    callable $callback,
    string $string,
    ?string $options = null,
): string|false|null {}

/**
 * Case-insensitive counterpart of mb_ereg_replace(): performs a multibyte-aware regular expression
 * search and replace using the mbregex (POSIX-extended) engine, matching regardless of case.
 * Returns false if the pattern is invalid.
 *
 * @pure
 */
function mb_eregi_replace(
    string $pattern,
    string $replacement,
    string $string,
    ?string $options = null,
): string|false|null {}

/**
 * Splits $string wherever it matches the regular expression $pattern and returns the pieces as an
 * array, or false if $pattern is invalid. The pattern uses the mbstring extension's own regular-
 * expression syntax, the same as mb_ereg(), rather than PCRE.
 *
 * @return string[]|false
 * @pure
 */
function mb_split(string $pattern, string $string, int $limit = -1): array|false {}

/**
 * Returns true if $string matches the multibyte regular expression $pattern in its entirety.
 * $options is a string of single-letter flags controlling the match, such as i for case-
 * insensitivity.
 *
 * @pure
 */
function mb_ereg_match(string $pattern, string $string, ?string $options = null): bool {}

/**
 * Runs a multibyte regular expression match against the string previously set with
 * mb_ereg_search_init(), using $pattern and $options if given, or the pattern and options set at
 * initialisation otherwise. Returns true on a match and false otherwise.
 *
 * @pure
 */
function mb_ereg_search(?string $pattern = null, ?string $options = null): bool {}

/**
 * Runs a regular-expression search against the string registered by mb_ereg_search_init(),
 * continuing from the current internal search position, and returns the match's byte offset and
 * length as a two-element array, or false if it does not match. Omitting $pattern reuses the
 * pattern from the most recent search call on this string.
 *
 * @return list<int>|false
 *
 * @pure
 */
function mb_ereg_search_pos(?string $pattern = null, ?string $options = null): array|false {}

/**
 * Executes a regular-expression match from the current mb_ereg_search position within the string
 * previously set by mb_ereg_search_init, and returns the substrings captured by that match — the
 * whole match at index 0 followed by each capturing group — or false if no further match is found.
 * Optional $pattern and $options replace the ones currently in effect for the search.
 *
 * @return list<string>|false
 *
 * @pure
 */
function mb_ereg_search_regs(?string $pattern = null, ?string $options = null): array|false {}

/**
 * Sets up a string to be scanned incrementally by the mb_ereg_search family of functions,
 * optionally installing a default regular expression pattern and options that later calls will use
 * if they omit their own.
 */
function mb_ereg_search_init(string $string, ?string $pattern = null, ?string $options = null): bool {}

/**
 * Returns the substrings captured by the most recent successful mb_ereg_search() match (or
 * mb_ereg_search_pos() / mb_ereg_search_regs()), in the same shape as the $regs array from
 * mb_ereg(): index 0 holds the whole match and each following index holds one parenthesised group.
 * Returns false if there is nothing to report.
 *
 * @return list<string>|false
 *
 * @pure
 */
function mb_ereg_search_getregs(): array|false {}

/**
 * Returns the position in the search string where the next mb_ereg_search() match attempt will
 * begin, as established by mb_ereg_search_init() and then advanced automatically after each
 * successful match.
 *
 * @pure
 */
function mb_ereg_search_getpos(): int {}

/**
 * Sets, in characters, the position in the search string where the next mb_ereg_search call will
 * start. Returns false if the offset is out of range.
 *
 * @pure
 */
function mb_ereg_search_setpos(int $offset): bool {}

/**
 * Returns the character, as a string in $encoding (or the current internal encoding if omitted),
 * represented by the Unicode codepoint $codepoint. Returns false if $codepoint is not a valid
 * codepoint or $encoding is not supported.
 *
 * @pure
 *
 * @ignore-falsable-return
 */
function mb_chr(int $codepoint, ?string $encoding = null): string|false {}

/**
 * Returns the Unicode code point of the first character of string, decoded using the given encoding
 * or the mbstring internal encoding if omitted. Returns false on failure, such as when string is
 * empty.
 *
 * @return int<0, max>|false
 *
 * @pure
 *
 * @ignore-falsable-return
 */
function mb_ord(string $string, ?string $encoding = null): int|false {}

/**
 * Rewrites $string in $encoding, or the internal encoding when omitted, replacing any ill-formed
 * byte sequences with the substitute character. It is shorthand for converting the string to itself
 * with mb_convert_encoding().
 *
 * @pure
 */
function mb_scrub(string $string, ?string $encoding = null): string {}

/**
 * Sets the byte offset at which the next mb_ereg_search() call resumes scanning the string
 * previously set with mbereg_search_init(). This is the legacy, unspaced alias of
 * mb_ereg_search_setpos().
 *
 * @pure
 * @deprecated
 */
function mbereg_search_setpos($position) {}

/**
 * Splits a string into an array of chunks of $length characters each, the multibyte-aware analogue
 * of str_split() -- chunks are counted in characters rather than bytes.
 *
 * @return ($string is non-empty-string ? list<non-empty-string> : list<string>)
 *
 * @pure
 */
function mb_str_split(string $string, int $length = 1, ?string $encoding = null): array {}

/**
 * Multibyte-aware version of str_pad(): pads a string with another string until it reaches the
 * given length, measured in characters rather than bytes. $pad_type selects which side is padded:
 * STR_PAD_RIGHT (the default), STR_PAD_LEFT, or STR_PAD_BOTH, which splits the padding between both
 * sides. A string already at or beyond the target length is returned unchanged.
 */
#[Mago\AvailableSince(80300)]
function mb_str_pad(
    string $string,
    int $length,
    string $pad_string = ' ',
    int $pad_type = STR_PAD_RIGHT,
    ?string $encoding = null,
): string {}

/**
 * Returns a copy of $string with its first character converted to uppercase using multibyte-aware
 * case conversion for $encoding, or the internal encoding if omitted, leaving the rest of the
 * string unchanged.
 */
#[Mago\AvailableSince(80400)]
function mb_ucfirst(string $string, ?string $encoding = null): string {}

/**
 * Multibyte-safe equivalent of lcfirst: lowercases the first character of $string according to
 * $encoding, or the internal encoding if omitted, rather than treating the first byte as the first
 * character.
 */
function mb_lcfirst(string $string, ?string $encoding = null): string {}

/**
 * Strips whitespace, or the characters listed in $characters, from both ends of a multibyte string.
 * Without $characters it trims a broader, Unicode-aware notion of whitespace than trim()'s fixed
 * set of ASCII characters.
 */
function mb_trim(string $string, ?string $characters = null, ?string $encoding = null): string {}

/**
 * Multibyte-safe equivalent of ltrim(): removes characters from the start of string, matching by
 * character rather than by byte. Unlike ltrim(), its default set of characters to strip, used when
 * $characters is omitted, covers Unicode whitespace such as non-breaking spaces, not only the ASCII
 * whitespace characters.
 */
#[Mago\AvailableSince(80400)]
function mb_ltrim(string $string, ?string $characters = null, ?string $encoding = null): string {}

/**
 * Strips whitespace — or the characters listed in $characters — from the end of a multibyte string,
 * treating string according to encoding (or the internal encoding if omitted).
 */
function mb_rtrim(string $string, ?string $characters = null, ?string $encoding = null): string {}
