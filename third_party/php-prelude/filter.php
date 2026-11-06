<?php

/**
 * Retrieves a single variable named var_name from the specified external input source, such as
 * INPUT_GET or INPUT_POST, optionally passing it through filter. Returns null if the variable does
 * not exist in that source, and false if filtering fails.
 *
 * @pure
 */
function filter_input(int $type, string $var_name, int $filter = FILTER_DEFAULT, array|int $options = 0): mixed {}

/**
 * Filters, and optionally sanitises, a value according to the given filter. Returns false when
 * validation fails, unless FILTER_NULL_ON_FAILURE is set, in which case it returns null instead.
 *
 * @pure
 */
function filter_var(mixed $value, int $filter = FILTER_DEFAULT, array|int $options = 0): mixed {}

/**
 * Filters a whole set of external input variables (from sources such as GET, POST, COOKIE, SERVER
 * or ENV, selected by the type constant) in one call. options may be a single filter constant
 * applied to every value, or an associative array mapping each variable name to a filter id or to a
 * filter/flags/options specification, mirroring filter_var_array. When add_empty is true, variable
 * names with no corresponding input are still present in the result with a null value; when false,
 * they are omitted. Returns false on failure, and in some cases null, when no external variables of
 * the requested type are available.
 *
 * @pure
 */
function filter_input_array(int $type, array|int $options = FILTER_DEFAULT, bool $add_empty = true): array|false|null {}

/**
 * Runs a batch of values through filter_var(), either applying one filter to every element of
 * $array or a different filter per key when $options is an array of per-key filter definitions,
 * each in the same shape filter_var() itself accepts. Returns false if $array is not an array.
 * $add_empty (default true) controls whether keys named in the filter definitions but absent from
 * $array are added to the result as null, versus left out entirely when set to false.
 *
 * @pure
 */
function filter_var_array(
    array $array,
    array|int $options = FILTER_DEFAULT,
    bool $add_empty = true,
): array|false|null {}

/**
 * Returns the names of every filter that filter_var(), filter_input() and related functions accept,
 * such as "int", "boolean", "email" and "url".
 *
 * @return array<array-key, string>
 *
 * @pure
 */
function filter_list(): array {}

/**
 * Reports whether a variable of the given name exists in the specified external input source, such
 * as INPUT_GET or INPUT_POST, without validating or returning its value.
 *
 * @pure
 */
function filter_has_var(int $input_type, string $var_name): bool {}

/**
 * Returns the internal filter ID for a named filter as used by the filter extension, the same name
 * accepted by filter_var(), or false if no filter with that name exists.
 *
 * @pure
 */
function filter_id(string $name): int|false {}

const INPUT_POST = 0;

const INPUT_GET = 1;

const INPUT_COOKIE = 2;

const INPUT_ENV = 4;

const INPUT_SERVER = 5;

const FILTER_FLAG_NONE = 0;

const FILTER_REQUIRE_SCALAR = 33554432;

const FILTER_REQUIRE_ARRAY = 16777216;

const FILTER_FORCE_ARRAY = 67108864;

const FILTER_NULL_ON_FAILURE = 134217728;

const FILTER_VALIDATE_INT = 257;

#[Mago\AvailableSince(80000)]
const FILTER_VALIDATE_BOOLEAN = 258;

#[Mago\AvailableSince(80100)]
const FILTER_VALIDATE_BOOL = 258;

const FILTER_VALIDATE_FLOAT = 259;

const FILTER_VALIDATE_REGEXP = 272;

const FILTER_VALIDATE_DOMAIN = 277;

const FILTER_VALIDATE_URL = 273;

const FILTER_VALIDATE_EMAIL = 274;

const FILTER_VALIDATE_IP = 275;

const FILTER_VALIDATE_MAC = 276;

const FILTER_DEFAULT = 516;

const FILTER_SANITIZE_ADD_SLASHES = 523;

const FILTER_UNSAFE_RAW = 516;

/**
 * @deprecated
 */
const FILTER_SANITIZE_STRING = 513;

/**
 * @deprecated
 */
const FILTER_SANITIZE_STRIPPED = 513;

const FILTER_SANITIZE_ENCODED = 514;

const FILTER_SANITIZE_SPECIAL_CHARS = 515;

const FILTER_SANITIZE_FULL_SPECIAL_CHARS = 522;

const FILTER_SANITIZE_EMAIL = 517;

const FILTER_SANITIZE_URL = 518;

const FILTER_SANITIZE_NUMBER_INT = 519;

const FILTER_SANITIZE_NUMBER_FLOAT = 520;

const FILTER_CALLBACK = 1024;

const FILTER_FLAG_ALLOW_OCTAL = 1;

const FILTER_FLAG_ALLOW_HEX = 2;

const FILTER_FLAG_STRIP_LOW = 4;

const FILTER_FLAG_STRIP_HIGH = 8;

const FILTER_FLAG_STRIP_BACKTICK = 512;

const FILTER_FLAG_ENCODE_LOW = 16;

const FILTER_FLAG_ENCODE_HIGH = 32;

const FILTER_FLAG_ENCODE_AMP = 64;

const FILTER_FLAG_NO_ENCODE_QUOTES = 128;

const FILTER_FLAG_EMPTY_STRING_NULL = 256;

const FILTER_FLAG_ALLOW_FRACTION = 4096;

const FILTER_FLAG_ALLOW_THOUSAND = 8192;

const FILTER_FLAG_ALLOW_SCIENTIFIC = 16384;

const FILTER_FLAG_PATH_REQUIRED = 262144;

const FILTER_FLAG_QUERY_REQUIRED = 524288;

const FILTER_FLAG_IPV4 = 1048576;

const FILTER_FLAG_IPV6 = 2097152;

const FILTER_FLAG_NO_RES_RANGE = 4194304;

const FILTER_FLAG_NO_PRIV_RANGE = 8388608;

const FILTER_FLAG_HOSTNAME = 1048576;

const FILTER_FLAG_EMAIL_UNICODE = 1048576;

const FILTER_FLAG_GLOBAL_RANGE = 536870912;
