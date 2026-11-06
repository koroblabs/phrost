<?php

const __COMPILER_HALT_OFFSET__ = 0;

const CONNECTION_ABORTED = 1;

const CONNECTION_NORMAL = 0;

const CONNECTION_TIMEOUT = 2;

const INI_USER = 1;

const INI_PERDIR = 2;

const INI_SYSTEM = 4;

const INI_ALL = 7;

const INI_SCANNER_NORMAL = 0;

const INI_SCANNER_TYPED = 2;

const INI_SCANNER_RAW = 1;

const PHP_URL_SCHEME = 0;

const PHP_URL_HOST = 1;

const PHP_URL_PORT = 2;

const PHP_URL_USER = 3;

const PHP_URL_PASS = 4;

const PHP_URL_PATH = 5;

const PHP_URL_QUERY = 6;

const PHP_URL_FRAGMENT = 7;

const M_E = 2.718281828459;

const M_LOG2E = 1.442695040889;

const M_LOG10E = 0.43429448190325;

const M_LN2 = 0.69314718055995;

const M_LN10 = 2.302585092994;

const M_PI = 3.1415926535898;

const M_PI_2 = 1.5707963267949;

const M_PI_4 = 0.78539816339745;

const M_1_PI = 0.31830988618379;

const M_2_PI = 0.63661977236758;

const M_SQRTPI = 1.7724538509055;

const M_2_SQRTPI = 1.1283791670955;

const M_LNPI = 1.1447298858494;

const M_EULER = 0.57721566490153;

const M_SQRT2 = 1.4142135623731;

const M_SQRT1_2 = 0.70710678118655;

const M_SQRT3 = 1.7320508075689;

const INF = (float) INF;

const NAN = (float) NAN;

const PHP_ROUND_HALF_UP = 1;

const PHP_ROUND_HALF_DOWN = 2;

const PHP_ROUND_HALF_EVEN = 3;

const PHP_ROUND_HALF_ODD = 4;

const INFO_GENERAL = 1;

const INFO_CREDITS = 2;

const INFO_CONFIGURATION = 4;

const INFO_MODULES = 8;

const INFO_ENVIRONMENT = 16;

const INFO_VARIABLES = 32;

const INFO_LICENSE = 64;

const INFO_ALL = 4294967295;

const CREDITS_GROUP = 1;

const CREDITS_GENERAL = 2;

const CREDITS_SAPI = 4;

const CREDITS_MODULES = 8;

const CREDITS_DOCS = 16;

const CREDITS_FULLPAGE = 32;

const CREDITS_QA = 64;

const CREDITS_ALL = 4294967295;

const HTML_SPECIALCHARS = 0;

const HTML_ENTITIES = 1;

const ENT_COMPAT = 2;

const ENT_QUOTES = 3;

const ENT_NOQUOTES = 0;

const ENT_IGNORE = 4;

const STR_PAD_LEFT = 0;

const STR_PAD_RIGHT = 1;

const STR_PAD_BOTH = 2;

const PATHINFO_DIRNAME = 1;

const PATHINFO_BASENAME = 2;

const PATHINFO_EXTENSION = 4;

const PATHINFO_FILENAME = 8;

const PATHINFO_ALL = 15;

const CHAR_MAX = 127;

const LC_CTYPE = 0;

const LC_NUMERIC = 1;

const LC_TIME = 2;

const LC_COLLATE = 3;

const LC_MONETARY = 4;

const LC_ALL = 6;

const LC_MESSAGES = 5;

const SEEK_SET = 0;

const SEEK_CUR = 1;

const SEEK_END = 2;

const LOCK_SH = 1;

const LOCK_EX = 2;

const LOCK_UN = 3;

const LOCK_NB = 4;

const STREAM_NOTIFY_CONNECT = 2;

const STREAM_NOTIFY_AUTH_REQUIRED = 3;

const STREAM_NOTIFY_AUTH_RESULT = 10;

const STREAM_NOTIFY_MIME_TYPE_IS = 4;

const STREAM_NOTIFY_FILE_SIZE_IS = 5;

const STREAM_NOTIFY_REDIRECTED = 6;

const STREAM_NOTIFY_PROGRESS = 7;

const STREAM_NOTIFY_FAILURE = 9;

const STREAM_NOTIFY_COMPLETED = 8;

const STREAM_NOTIFY_RESOLVE = 1;

const STREAM_NOTIFY_SEVERITY_INFO = 0;

const STREAM_NOTIFY_SEVERITY_WARN = 1;

const STREAM_NOTIFY_SEVERITY_ERR = 2;

const STREAM_FILTER_READ = 1;

const STREAM_FILTER_WRITE = 2;

const STREAM_FILTER_ALL = 3;

const STREAM_CLIENT_PERSISTENT = 1;

const STREAM_CLIENT_ASYNC_CONNECT = 2;

const STREAM_CLIENT_CONNECT = 4;

const STREAM_SHUT_RD = 0;

const STREAM_SHUT_WR = 1;

const STREAM_SHUT_RDWR = 2;

const STREAM_PF_INET = 2;

const STREAM_PF_INET6 = 10;

const STREAM_PF_UNIX = 1;

const STREAM_IPPROTO_IP = 0;

const STREAM_IPPROTO_TCP = 6;

const STREAM_IPPROTO_UDP = 17;

const STREAM_IPPROTO_ICMP = 1;

const STREAM_IPPROTO_RAW = 255;

const STREAM_SOCK_STREAM = 1;

const STREAM_SOCK_DGRAM = 2;

const STREAM_SOCK_RAW = 3;

const STREAM_SOCK_SEQPACKET = 5;

const STREAM_SOCK_RDM = 4;

const STREAM_PEEK = 2;

const STREAM_OOB = 1;

const STREAM_SERVER_BIND = 4;

const STREAM_SERVER_LISTEN = 8;

const FILE_USE_INCLUDE_PATH = 1;

const FILE_IGNORE_NEW_LINES = 2;

const FILE_SKIP_EMPTY_LINES = 4;

const FILE_APPEND = 8;

const FILE_NO_DEFAULT_CONTEXT = 16;

/**
 * @deprecated
 */
const FILE_TEXT = 0;

const FILE_BINARY = 0;

const FNM_NOESCAPE = 2;

const FNM_PATHNAME = 1;

const FNM_PERIOD = 4;

const FNM_CASEFOLD = 16;

const PSFS_PASS_ON = 2;

const PSFS_FEED_ME = 1;

const PSFS_ERR_FATAL = 0;

const PSFS_FLAG_NORMAL = 0;

const PSFS_FLAG_FLUSH_INC = 1;

const PSFS_FLAG_FLUSH_CLOSE = 2;

const ABDAY_1 = 131072;

const ABDAY_2 = 131073;

const ABDAY_3 = 131074;

const ABDAY_4 = 131075;

const ABDAY_5 = 131076;

const ABDAY_6 = 131077;

const ABDAY_7 = 131078;

const DAY_1 = 131079;

const DAY_2 = 131080;

const DAY_3 = 131081;

const DAY_4 = 131082;

const DAY_5 = 131083;

const DAY_6 = 131084;

const DAY_7 = 131085;

const ABMON_1 = 131086;

const ABMON_2 = 131087;

const ABMON_3 = 131088;

const ABMON_4 = 131089;

const ABMON_5 = 131090;

const ABMON_6 = 131091;

const ABMON_7 = 131092;

const ABMON_8 = 131093;

const ABMON_9 = 131094;

const ABMON_10 = 131095;

const ABMON_11 = 131096;

const ABMON_12 = 131097;

const MON_1 = 131098;

const MON_2 = 131099;

const MON_3 = 131100;

const MON_4 = 131101;

const MON_5 = 131102;

const MON_6 = 131103;

const MON_7 = 131104;

const MON_8 = 131105;

const MON_9 = 131106;

const MON_10 = 131107;

const MON_11 = 131108;

const MON_12 = 131109;

const AM_STR = 131110;

const PM_STR = 131111;

const D_T_FMT = 131112;

const D_FMT = 131113;

const T_FMT = 131114;

const T_FMT_AMPM = 131115;

const ERA = 131116;

const ERA_D_T_FMT = 131120;

const ERA_D_FMT = 131118;

const ERA_T_FMT = 131121;

const ALT_DIGITS = 131119;

const CRNCYSTR = 262159;

const RADIXCHAR = 65536;

const THOUSEP = 65537;

const YESEXPR = 327680;

const NOEXPR = 327681;

const YESSTR = 327682;

const NOSTR = 327683;

const CODESET = 14;

const CRYPT_SALT_LENGTH = 123;

const CRYPT_STD_DES = 1;

const CRYPT_EXT_DES = 1;

const CRYPT_MD5 = 1;

const CRYPT_BLOWFISH = 1;

const CRYPT_SHA256 = 1;

const CRYPT_SHA512 = 1;

/** @var non-falsy-string */
const DIRECTORY_SEPARATOR = DIRECTORY_SEPARATOR;

/** @var non-falsy-string */
const PATH_SEPARATOR = PATH_SEPARATOR;

const GLOB_BRACE = 1024;

const GLOB_MARK = 2;

const GLOB_NOSORT = 4;

const GLOB_NOCHECK = 16;

const GLOB_NOESCAPE = 64;

const GLOB_ERR = 1;

const GLOB_ONLYDIR = 1073741824;

const GLOB_AVAILABLE_FLAGS = 1073741911;

const EXTR_OVERWRITE = 0;

const EXTR_SKIP = 1;

const EXTR_PREFIX_SAME = 2;

const EXTR_PREFIX_ALL = 3;

const EXTR_PREFIX_INVALID = 4;

const EXTR_PREFIX_IF_EXISTS = 5;

const EXTR_IF_EXISTS = 6;

const EXTR_REFS = 256;

const SORT_ASC = 4;

const SORT_DESC = 3;

const SORT_REGULAR = 0;

const SORT_NUMERIC = 1;

const SORT_STRING = 2;

const SORT_LOCALE_STRING = 5;

const CASE_LOWER = 0;

const CASE_UPPER = 1;

const COUNT_NORMAL = 0;

const COUNT_RECURSIVE = 1;

const ASSERT_ACTIVE = 1;

const ASSERT_CALLBACK = 2;

const ASSERT_BAIL = 3;

const ASSERT_WARNING = 4;

const ASSERT_EXCEPTION = 5;

const STREAM_USE_PATH = 1;

const STREAM_IGNORE_URL = 2;

const STREAM_ENFORCE_SAFE_MODE = 4;

const STREAM_REPORT_ERRORS = 8;

const STREAM_MUST_SEEK = 16;

const STREAM_URL_STAT_LINK = 1;

const STREAM_URL_STAT_QUIET = 2;

const STREAM_MKDIR_RECURSIVE = 1;

const STREAM_IS_URL = 1;

const STREAM_OPTION_BLOCKING = 1;

const STREAM_OPTION_READ_TIMEOUT = 4;

const STREAM_OPTION_READ_BUFFER = 2;

const STREAM_OPTION_WRITE_BUFFER = 3;

const STREAM_BUFFER_NONE = 0;

const STREAM_BUFFER_LINE = 1;

const STREAM_BUFFER_FULL = 2;

const STREAM_CAST_AS_STREAM = 0;

const STREAM_CAST_FOR_SELECT = 3;

const IMAGETYPE_GIF = 1;

const IMAGETYPE_JPEG = 2;

const IMAGETYPE_PNG = 3;

const IMAGETYPE_SWF = 4;

const IMAGETYPE_PSD = 5;

const IMAGETYPE_BMP = 6;

const IMAGETYPE_TIFF_II = 7;

const IMAGETYPE_TIFF_MM = 8;

const IMAGETYPE_JPC = 9;

const IMAGETYPE_JP2 = 10;

const IMAGETYPE_JPX = 11;

const IMAGETYPE_JB2 = 12;

const IMAGETYPE_SWC = 13;

const IMAGETYPE_IFF = 14;

const IMAGETYPE_WBMP = 15;

const IMAGETYPE_JPEG2000 = 9;

const IMAGETYPE_XBM = 16;

const IMAGETYPE_ICO = 17;

const IMAGETYPE_WEBP = 18;

const IMAGETYPE_UNKNOWN = 0;

const IMAGETYPE_COUNT = 20;

const IMAGETYPE_AVIF = 19;

const DNS_A = 1;

const DNS_CAA = 8192;

const DNS_NS = 2;

const DNS_CNAME = 16;

const DNS_SOA = 32;

const DNS_PTR = 2048;

const DNS_HINFO = 4096;

const DNS_MX = 16384;

const DNS_TXT = 32768;

const DNS_SRV = 33554432;

const DNS_NAPTR = 67108864;

const DNS_AAAA = 134217728;

const DNS_A6 = 16777216;

const DNS_ANY = 268435456;

const DNS_ALL = 251721779;

const PHP_QUERY_RFC1738 = 1;

const PHP_QUERY_RFC3986 = 2;

const SID = 'name=ID';

const PHP_SESSION_DISABLED = 0;

const PHP_SESSION_NONE = 1;

const PHP_SESSION_ACTIVE = 2;

const ENT_SUBSTITUTE = 8;

const ENT_DISALLOWED = 128;

const ENT_HTML401 = 0;

const ENT_XML1 = 16;

const ENT_XHTML = 32;

const ENT_HTML5 = 48;

const SCANDIR_SORT_ASCENDING = 0;

const SCANDIR_SORT_DESCENDING = 1;

const SCANDIR_SORT_NONE = 2;

const SORT_NATURAL = 6;

const SORT_FLAG_CASE = 8;

const STREAM_META_TOUCH = 1;

const STREAM_META_OWNER = 3;

const STREAM_META_OWNER_NAME = 2;

const STREAM_META_GROUP = 5;

const STREAM_META_GROUP_NAME = 4;

const STREAM_META_ACCESS = 6;

const STREAM_CRYPTO_METHOD_SSLv2_CLIENT = 3;

const STREAM_CRYPTO_METHOD_SSLv3_CLIENT = 5;

const STREAM_CRYPTO_METHOD_SSLv23_CLIENT = 57;

const STREAM_CRYPTO_METHOD_TLS_CLIENT = 121;

const STREAM_CRYPTO_METHOD_SSLv2_SERVER = 2;

const STREAM_CRYPTO_METHOD_SSLv3_SERVER = 4;

const STREAM_CRYPTO_METHOD_SSLv23_SERVER = 120;

const STREAM_CRYPTO_METHOD_TLS_SERVER = 120;

const STREAM_CRYPTO_METHOD_ANY_CLIENT = 127;

const STREAM_CRYPTO_METHOD_ANY_SERVER = 126;

const STREAM_CRYPTO_METHOD_TLSv1_0_CLIENT = 9;

const STREAM_CRYPTO_METHOD_TLSv1_0_SERVER = 8;

const STREAM_CRYPTO_METHOD_TLSv1_1_CLIENT = 17;

const STREAM_CRYPTO_METHOD_TLSv1_1_SERVER = 16;

const STREAM_CRYPTO_METHOD_TLSv1_2_CLIENT = 33;

const STREAM_CRYPTO_METHOD_TLSv1_2_SERVER = 32;

const STREAM_CRYPTO_METHOD_TLSv1_3_CLIENT = 65;

const STREAM_CRYPTO_METHOD_TLSv1_3_SERVER = 64;

const STREAM_CRYPTO_PROTO_SSLv3 = 4;

const STREAM_CRYPTO_PROTO_TLSv1_0 = 8;

const STREAM_CRYPTO_PROTO_TLSv1_1 = 16;

const STREAM_CRYPTO_PROTO_TLSv1_2 = 32;

const STREAM_CRYPTO_PROTO_TLSv1_3 = 64;

const MT_RAND_MT19937 = 0;

const MT_RAND_PHP = 1;

const LOG_EMERG = 0;

const LOG_ALERT = 1;

const LOG_CRIT = 2;

const LOG_ERR = 3;

const LOG_WARNING = 4;

const LOG_NOTICE = 5;

const LOG_INFO = 6;

const LOG_DEBUG = 7;

const LOG_KERN = 0;

const LOG_USER = 8;

const LOG_MAIL = 16;

const LOG_DAEMON = 24;

const LOG_AUTH = 32;

const LOG_SYSLOG = 40;

const LOG_LPR = 48;

const LOG_NEWS = 56;

const LOG_UUCP = 64;

const LOG_CRON = 72;

const LOG_AUTHPRIV = 80;

const LOG_LOCAL0 = 128;

const LOG_LOCAL1 = 136;

const LOG_LOCAL2 = 144;

const LOG_LOCAL3 = 152;

const LOG_LOCAL4 = 160;

const LOG_LOCAL5 = 168;

const LOG_LOCAL6 = 176;

const LOG_LOCAL7 = 184;

const LOG_PID = 1;

const LOG_CONS = 2;

const LOG_ODELAY = 4;

const LOG_NDELAY = 8;

const LOG_NOWAIT = 16;

const LOG_PERROR = 32;

const DECIMAL_POINT = 65536;

const THOUSANDS_SEP = 65537;

const GROUPING = 65538;

const ERA_YEAR = 131117;

const INT_CURR_SYMBOL = 262144;

const CURRENCY_SYMBOL = 262145;

const MON_DECIMAL_POINT = 262146;

const MON_THOUSANDS_SEP = 262147;

const MON_GROUPING = 262148;

const POSITIVE_SIGN = 262149;

const NEGATIVE_SIGN = 262150;

const INT_FRAC_DIGITS = 262151;

const FRAC_DIGITS = 262152;

const P_CS_PRECEDES = 262153;

const P_SEP_BY_SPACE = 262154;

const N_CS_PRECEDES = 262155;

const N_SEP_BY_SPACE = 262156;

const P_SIGN_POSN = 262157;

const N_SIGN_POSN = 262158;

const PASSWORD_DEFAULT = '2y';

const PASSWORD_BCRYPT_DEFAULT_COST = 12;

const PASSWORD_BCRYPT = '2y';

const PASSWORD_ARGON2I = 'argon2i';

const PASSWORD_ARGON2ID = 'argon2id';

const PASSWORD_ARGON2_DEFAULT_MEMORY_COST = 65536;

const PASSWORD_ARGON2_DEFAULT_TIME_COST = 4;

const PASSWORD_ARGON2_DEFAULT_THREADS = 1;

const PASSWORD_ARGON2_PROVIDER = 'standard';

/**
 * Returns information about the given password hash: the algorithm identifier and name it was
 * created with, and the options, such as cost for bcrypt or memory and time cost for Argon2, that
 * were used to create it.
 *
 * @return array{
 *   'algo': non-empty-string,
 *   'algoName': non-empty-string,
 *   'options': array{'salt'?: int, 'cost'?: int, 'memory_cost'?: int, 'time_cost'?: int, 'threads'?: int, ...},
 * }
 *
 * @pure
 */
function password_get_info(string $hash): array {}

/**
 * Creates a password hash using the given algorithm and options, suitable for storing and later
 * verifying with password_verify(). The resulting string already encodes the algorithm and its
 * parameters, so it should be stored and compared as an opaque value.
 *
 * @param array{'salt'?: int, 'cost'?: int, 'memory_cost'?: int, 'time_cost'?: int, 'threads'?: int, ...} $options
 *
 * @return non-empty-string
 *
 * @pure
 */
function password_hash(string $password, string|int|null $algo, array $options = []): string {}

/**
 * Reports whether a password hash was produced with different settings than algo and options now
 * specify, meaning it should be regenerated so future verifications use the current cost, algorithm
 * or other parameters. Does not verify the password itself.
 *
 * @code
 * if (password_verify($password, $hash) && password_needs_rehash($hash, PASSWORD_DEFAULT)) {
 *     $hash = password_hash($password, PASSWORD_DEFAULT);
 * }
 * @endcode
 *
 * @param array{'salt'?:int, 'cost'?: int, 'memory_cost'?: int, 'time_cost'?: int, 'threads'?: int, ...} $options
 *
 * @pure
 */
function password_needs_rehash(string $hash, string|int|null $algo, array $options = []): bool {}

/**
 * Verifies that $password matches the hash produced by password_hash().
 *
 * @pure
 */
function password_verify(string $password, string $hash): bool {}

/**
 * Returns the identifiers of the password hashing algorithms, such as bcrypt and the Argon2
 * variants, available in the current build for use with password_hash() and password_verify().
 *
 * @return list<non-empty-string>
 *
 * @pure
 */
function password_algos(): array {}

/**
 * Loads a PHP extension shared library at runtime by filename. Only works under SAPIs such as the
 * CLI, and only when the enable_dl ini directive is on; returns false otherwise or on failure.
 */
function dl(string $extension_filename): bool {}

/**
 * Sets the process title shown by tools such as ps, where the SAPI and platform support it. Returns
 * false, and raises a warning, if changing the title is not supported.
 */
function cli_set_process_title(string $title): bool {}

/**
 * Returns the current process title as previously set by cli_set_process_title(), or null if the
 * CLI SAPI or the underlying platform does not support reading it back.
 *
 * @pure
 */
function cli_get_process_title(): ?string {}

/**
 * Converts a string assumed to be encoded in ISO-8859-1 (Latin-1) to UTF-8, byte for byte, with no
 * way to detect or validate the input encoding, so passing anything other than genuine Latin-1
 * produces mojibake.
 *
 * @deprecated
 * @pure
 */
function utf8_encode(string $string): string {}

/**
 * Converts a UTF-8 encoded string to a single-byte ISO-8859-1 (Latin-1) string, substituting a
 * placeholder character for any code point with no Latin-1 representation. Superseded by
 * mb_convert_encoding() or iconv().
 *
 * @pure
 * @deprecated
 */
function utf8_decode(string $string): string {}

/**
 * Clears the last error recorded for error_get_last(), so that function returns null again until a
 * new error occurs.
 */
function error_clear_last(): void {}

/**
 * Windows-only: returns a code page number. $kind selects which one: an empty string for the code
 * page PHP itself is running with, 'ansi' or 'oem' for the corresponding system code page, or
 * 'console_input' or 'console_output' for the current console's code pages.
 */
function sapi_windows_cp_get(string $kind = ''): int {}

/**
 * Sets the code page used for console output by the SAPI process on Windows. Has no effect, and
 * returns false, when not running under Windows.
 */
function sapi_windows_cp_set(int $codepage): bool {}

/**
 * Available only on Windows. Converts $subject from the code page $in_codepage to $out_codepage,
 * each given as a numeric code page or a recognised name such as UTF-8 or ANSI. Returns null if the
 * conversion is unavailable or fails.
 */
function sapi_windows_cp_conv(int|string $in_codepage, int|string $out_codepage, string $subject): ?string {}

/**
 * Reports whether the SAPI's current Windows code page is set to UTF-8. This is a Windows-only
 * function and has no defined effect on other platforms.
 */
function sapi_windows_cp_is_utf8(): bool {}

/**
 * Queries, or with $enable, sets whether a Windows console stream has VT100 (ANSI escape sequence)
 * support enabled. Only meaningful on Windows; has no effect on other platforms.
 *
 * @param resource $stream
 */
function sapi_windows_vt100_support($stream, ?bool $enable = null): bool {}

/**
 * Registers $handler to be invoked whenever the console process receives a Ctrl+C or Ctrl+Break
 * signal, or unregisters it when $add is false. Available only on Windows; returns false if the
 * handler could not be (un)registered.
 */
function sapi_windows_set_ctrl_handler(?callable $handler, bool $add = true): bool {}

/**
 * On Windows, generates a console control event equivalent to a Ctrl+C or Ctrl+Break keypress for
 * the given process group, for processes sharing that console. Returns whether the event was
 * generated successfully.
 */
function sapi_windows_generate_ctrl_event(int $event, int $pid = 0): bool {}

/**
 * @template-covariant TKey
 * @template-covariant TValue
 * @template TSend
 * @template-covariant TReturn
 *
 * @template-implements Iterator<TKey, TValue>
 */
class Generator implements Iterator
{
    /**
     * @return ?TValue
     *
     * @psalm-ignore-nullable-return
     */
    public function current() {}

    /**
     * Resumes the generator's execution until the next yield expression, or until it finishes. If
     * the generator has not started yet, this both starts it and advances past the first yield.
     * Equivalent to send(null), except it discards rather than returns the newly yielded value.
     *
     * @return void
     */
    public function next() {}

    /**
     * Returns the key associated with the current yielded value, the same key foreach would expose
     * from yield $key => $value. If the generator yields without explicit keys, returns sequential
     * integers starting at 0.
     *
     * @return TKey
     */
    public function key() {}

    /**
     * Reports whether the generator still has a value to yield, i.e. whether it has not yet run to
     * completion. foreach calls this after rewind() and after each next() to decide whether to keep
     * iterating.
     *
     * @return bool
     */
    public function valid() {}

    /**
     * Starts the generator's execution up to its first yield, if it has not already started.
     * Calling it after execution has advanced past the first yield throws an Exception, because a
     * generator's execution cannot be replayed from an earlier point.
     *
     * @return void
     */
    public function rewind() {}

    /**
     * Returns the value the generator produced with a return statement after it finished executing.
     * Calling it before the generator has finished, for example while valid() still returns true,
     * throws an Exception.
     *
     * @return TReturn
     */
    public function getReturn() {}

    /**
     * Resumes execution, making $value the result of the currently suspended yield expression, and
     * runs until the next yield or the generator's end. Returns the newly yielded value, or null
     * once the generator finishes. If the generator has not started yet, it is first advanced to
     * its first yield before $value is delivered, so calling send() immediately after creating the
     * generator behaves like next() and $value is discarded.
     *
     * @code
     * $gen->send(1); // on a fresh generator this just starts it, like next()
     * @endcode
     *
     * @param TSend $value
     *
     * @return ?TValue
     *
     * @psalm-ignore-nullable-return
     */
    public function send($value) {}

    /**
     * Throws $exception at the generator's current suspended yield, as if it had been thrown there,
     * and resumes execution until the next yield, completion, or the exception escaping uncaught.
     * Returns the next yielded value, or null on completion. If the generator has not started, or
     * has already finished, the exception is thrown in the caller instead of inside the generator.
     *
     * @return ?TValue
     *
     * @psalm-ignore-nullable-return
     */
    public function throw(Throwable $exception) {}
}

function hex2bin(string $string): string|false {}

function http_response_code(int $response_code = 0): int|bool {}

final class __PHP_Incomplete_Class
{
    /**
     * @var string
     */
    public $__PHP_Incomplete_Class_Name;
}

class php_user_filter
{
    public string $filtername;
    public mixed $params;
    public $stream;

    /**
     * @param resource $in
     * @param resource $out
     * @param int &$consumed
     */
    public function filter($in, $out, &$consumed, bool $closing): int {}

    /**
     * Called by the streams API when the filter is attached to a stream, for example via
     * stream_filter_append() or stream_filter_prepend(), before any data passes through it, so the
     * filter can perform its own setup. Returning false prevents the filter from being attached.
     */
    public function onCreate(): bool {}

    /**
     * Called by the streams API when the filter is detached, for example when the stream it is
     * attached to is closed, so the filter can release any resources acquired in onCreate().
     */
    public function onClose(): void {}
}

#[Mago\AvailableSince(80400)]
final class StreamBucket
{
    public $bucket;
    public string $data;
    public int $datalen;
    public int $dataLength;
}

class Directory
{
    public readonly string $path;

    /**
     * @var resource
     */
    public readonly mixed $handle;

    /**
     * Closes the directory handle, the object-oriented equivalent of closedir().
     */
    public function close(): void {}

    /**
     * Resets the directory handle to its first entry, the object-oriented equivalent of
     * rewinddir().
     */
    public function rewind(): void {}

    /**
     * Reads the next entry's name from the directory handle, the object-oriented equivalent of
     * readdir(), including the '.' and '..' entries. Returns false once there are no more entries
     * to read.
     */
    public function read(): string|false {}
}

/**
 * Looks up the value of a constant given its name as a string, including a class constant written
 * as 'Class::CONST', which is useful when the constant's name is itself not known until runtime.
 *
 * @throws Error
 *
 * @pure
 */
function constant(string $name): mixed {}

/**
 * Converts binary data into its lowercase hexadecimal string representation.
 *
 * @pure
 *
 * @return ($string is non-empty-string ? non-empty-string : '')
 */
function bin2hex(string $string): string {}

/**
 * Delays script execution for the given number of seconds. Returns 0 on success, or the number of
 * seconds still remaining if a signal interrupted the sleep.
 *
 * @param int<0, max> $seconds
 */
function sleep(int $seconds): int {}

/**
 * Suspends execution of the current script for the given number of microseconds.
 *
 * @param int<0, max> $microseconds
 */
function usleep(int $microseconds): void {}

/**
 * Suspends execution for the given number of seconds and nanoseconds. Returns true on completion,
 * the remaining unslept time if a signal interrupts the sleep early, and false if either argument
 * is invalid.
 *
 * @param positive-int $seconds
 * @param positive-int $nanoseconds
 *
 * @return bool|array{seconds: int, nanoseconds: int}
 */
function time_nanosleep(int $seconds, int $nanoseconds): array|bool {}

/**
 * Sleeps until the given absolute Unix timestamp is reached, rather than for a relative duration
 * like sleep or usleep, which lets the delay be specified with microsecond precision. Returns false
 * on failure.
 */
function time_sleep_until(float $timestamp): bool {}

/**
 * Parses $timestamp against a strftime()-style $format and returns its components — hour, minute,
 * second, day, month, year, weekday, day of year, and any unparsed trailing text — instead of a
 * Unix timestamp. Returns false if the string does not match the format. Deprecated since PHP 8.1,
 * and not implemented on Windows at all.
 *
 * @return false|array{tm_sec: int, tm_min: int, tm_hour: int, tm_mday: int, tm_mon: int, tm_year: int, tm_wday: int, tm_yday: int, unparsed: string}
 */
function strptime(string $timestamp, string $format): array|false {}

/**
 * Flushes any output PHP has buffered so far, sending it towards the client immediately. Whether
 * this has any visible effect depends on the SAPI and on any further buffering layers, such as a
 * proxy, sitting between PHP and the client.
 */
function flush(): void {}

/**
 * Wraps string so that no line exceeds width characters, inserting break at each wrap point. A word
 * longer than width is left intact and allowed to exceed it unless cut_long_words is true, which
 * forces a break inside the word itself; the width is counted in bytes, not multibyte characters.
 *
 * @pure
 */
function wordwrap(string $string, int $width = 75, string $break = "\n", bool $cut_long_words = false): string {}

/**
 * Converts special characters such as ampersands, quotation marks and angle brackets into their
 * corresponding HTML entities, so they are not interpreted as markup.
 *
 * @pure
 */
function htmlspecialchars(
    string $string,
    int $flags = ENT_QUOTES | ENT_SUBSTITUTE,
    ?string $encoding = null,
    bool $double_encode = true,
): string {}

/**
 * Converts every character that has an HTML entity equivalent, including accented and other non-
 * ASCII letters, unlike htmlspecialchars(), which only touches a handful of characters such as <,
 * >, & and quotes. $flags controls which quotes are converted (ENT_QUOTES for both, ENT_COMPAT for
 * double only, ENT_NOQUOTES for neither) and, with ENT_SUBSTITUTE, replaces invalid byte sequences
 * with a substitute character instead of returning an empty string.
 *
 * @pure
 */
function htmlentities(
    string $string,
    int $flags = ENT_QUOTES | ENT_SUBSTITUTE,
    ?string $encoding = null,
    bool $double_encode = true,
): string {}

/**
 * Decodes HTML entities in $string back to their literal characters, the inverse of htmlentities().
 * $flags controls which quote entities are decoded (ENT_QUOTES for both single and double,
 * ENT_COMPAT for double quotes only, ENT_NOQUOTES for neither), which document type's entity table
 * is used (ENT_HTML401, ENT_HTML5, ENT_XML1, ENT_XHTML), and how malformed byte sequences are
 * handled (ENT_SUBSTITUTE replaces them with U+FFFD, ENT_IGNORE drops them silently).
 *
 * @pure
 */
function html_entity_decode(
    string $string,
    int $flags = ENT_QUOTES | ENT_SUBSTITUTE,
    ?string $encoding = null,
): string {}

/**
 * Converts the small set of HTML entities that htmlspecialchars() produces -- for the ampersand,
 * the two angle brackets, the double quote and, depending on flags, the single quote -- back into
 * their literal characters. The $flags bitmask controls which quote entities are decoded and which
 * document type's entity set is recognised.
 *
 * @pure
 */
function htmlspecialchars_decode(string $string, int $flags = ENT_QUOTES | ENT_SUBSTITUTE): string {}

/**
 * Returns the character-to-entity translation table used internally by htmlspecialchars() ($table =
 * HTML_SPECIALCHARS) or htmlentities() ($table = HTML_ENTITIES), as an array keyed by the raw
 * character and valued by its encoded entity string. $flags selects the quote style, the same as
 * the corresponding encoding function accepts.
 *
 * @code
 * get_html_translation_table(HTML_ENTITIES, ENT_QUOTES);
 * // ['"' => '"', '<' => '<', '>' => '>']
 * @endcode
 *
 * @pure
 */
function get_html_translation_table(
    int $table = 0,
    int $flags = ENT_QUOTES | ENT_SUBSTITUTE,
    string $encoding = 'UTF-8',
): array {}

/**
 * Returns the SHA-1 hash of $string as a 40-character lowercase hexadecimal string, or, when
 * $binary is true, as the raw 20-byte binary digest instead.
 *
 * @pure
 * @return non-falsy-string
 */
function sha1(string $string, bool $binary = false): string {}

/**
 * Computes the SHA-1 hash of the contents of the file $filename, returned as a 40-character
 * hexadecimal string, or as raw 20-byte binary data when $binary is true. Returns false if the file
 * cannot be read.
 *
 * @return non-falsy-string|false
 */
function sha1_file(string $filename, bool $binary = false): string|false {}

/**
 * Returns the MD5 hash of a string, as a 32-character hex string or 16-byte binary when the binary
 * flag is true. MD5 is cryptographically broken and unsuitable for security-sensitive use.
 *
 * @pure
 * @return non-falsy-string
 */
function md5(string $string, bool $binary = false): string {}

/**
 * Computes the MD5 hash of a file's contents without reading the whole file into memory at once.
 * Returns the hash as a 32-character hexadecimal string, or as a raw 16-byte binary string when
 * $binary is true. Returns false if the file cannot be read.
 *
 * @return non-falsy-string|false
 */
function md5_file(string $filename, bool $binary = false): string|false {}

/**
 * Calculates the CRC32 polynomial checksum of string, for detecting accidental data corruption
 * rather than for cryptographic use. On 32-bit builds of PHP the result can appear as a negative
 * integer, since the checksum itself is an unsigned 32-bit value but PHP integers are signed.
 *
 * @pure
 */
function crc32(string $string): int {}

/**
 * Parses a binary IPTC block, such as the "APP13" segment returned by getimagesize, into its
 * individual tags, or returns false if the block contains no recognisable data. Keys look like
 * "2#005", combining the IPTC record number and dataset number, and each value is an array of one
 * or more strings for that tag.
 *
 * @code
 * $info = getimagesize($file);
 * $iptc = iptcparse($info['APP13']);
 * @endcode
 *
 * @pure
 */
function iptcparse(string $iptc_block): array|false {}

/**
 * Embeds binary IPTC metadata into a JPEG image and returns the resulting image data. Depending on
 * the spool argument, it can instead write the modified image directly to the given file and return
 * a boolean indicating success.
 */
function iptcembed(string $iptc_data, string $filename, int $spool = 0): string|bool {}

/**
 * Determines an image's size and format by reading just enough of its header, without decoding the
 * whole file. Returns false if filename cannot be read or is not a recognised image format. Index 0
 * and 1 are the width and height in pixels, 2 is one of the IMAGETYPE_* constants identifying the
 * format, and 3 is a ready-to-use 'width="..." height="..."' string for an HTML img tag; bits and
 * channels describe colour depth and channel count where the format records them, and mime is the
 * corresponding MIME type. image_info, when given, is filled with the image's raw APP markers as
 * used by JPEG for embedded metadata such as IPTC, rather than anything getimagesize interprets
 * itself.
 *
 * @code
 * [$width, $height, $type, $attr] = getimagesize($path);
 * @endcode
 *
 * @param-out ?array $image_info
 *
 * @return false|array{0: int, 1: int, 2: int, 3: string, bits: int, channels: int, mime: string}
 */
function getimagesize(string $filename, ?array &$image_info = null): array|false {}

/**
 * Returns the MIME type string for one of the IMAGETYPE_* constants, such as the values returned by
 * getimagesize(), falling back to application/octet-stream for a type it does not recognise.
 *
 * @pure
 */
function image_type_to_mime_type(int $image_type): string {}

/**
 * Converts an IMAGETYPE_* constant to the file extension conventionally used for that image format.
 * The dot is included in the result unless include_dot is set to false. Returns false for an image
 * type with no known extension.
 *
 * @pure
 */
function image_type_to_extension(int $image_type, bool $include_dot = true): string|false {}

/**
 * Outputs information about the current PHP configuration, loaded extensions, and environment.
 */
function phpinfo(int $flags = INFO_ALL): bool {}

/**
 * Returns the version string of the running PHP interpreter, or of a loaded extension when
 * $extension is given. Returns false if the named extension is not loaded or carries no version
 * information.
 *
 * @pure
 *
 * @return ($extension is null ? non-empty-string : non-empty-string|false)
 */
function phpversion(?string $extension = null): string|false {}

/**
 * Prints an HTML page of PHP credits and always returns true. flags is a bitmask of CREDITS_*
 * constants (CREDITS_GENERAL, CREDITS_GROUP, CREDITS_SAPI, CREDITS_MODULES, CREDITS_DOCS,
 * CREDITS_QA, among others) selecting which sections to print, and CREDITS_FULLPAGE additionally
 * wraps the output in a complete HTML document.
 */
function phpcredits(int $flags = CREDITS_ALL): bool {}

/**
 * Returns an identifier for the Server API PHP is running under, distinguishing, for example, a
 * command-line script from one served by php-fpm or the Apache module, or false if the interface
 * cannot be determined.
 *
 * @return 'cli'|'phpdbg'|'embed'|'apache'|'apache2handler'|'cgi-fcgi'|'cli-server'|'fpm-fcgi'|'litespeed'|false
 * @pure
 */
function php_sapi_name(): string|false {}

/**
 * Returns information about the operating system PHP is running under. $mode selects which piece is
 * returned: 'a' (the default) combines all of the below into one string, 's' the OS name, 'n' the
 * host name, 'r' the release name, 'v' the version information, and 'm' the machine or architecture
 * type.
 *
 * @pure
 */
function php_uname(string $mode = 'a'): string {}

/**
 * Returns a comma-separated list of the additional php.ini files parsed from the configured scan
 * directory, or false if no scan directory is configured.
 *
 * @pure
 */
function php_ini_scanned_files(): string|false {}

/**
 * Returns the full path of the php.ini file that was loaded for the current SAPI, or false if no
 * configuration file was loaded.
 *
 * @pure
 */
function php_ini_loaded_file(): string|false {}

/**
 * Compares $string1 and $string2 using natural ordering, so embedded numbers compare by numeric
 * value rather than digit by digit, e.g. "img2" sorts before "img10". Returns a value less than,
 * equal to, or greater than zero like strcmp(), and the comparison is case-sensitive.
 *
 * @pure
 */
function strnatcmp(string $string1, string $string2): int {}

/**
 * Compares $string1 and $string2 using a case-insensitive natural-order algorithm, so that, for
 * example, item2 sorts before item10, and returns a value less than, equal to, or greater than zero
 * accordingly.
 *
 * @pure
 */
function strnatcasecmp(string $string1, string $string2): int {}

/**
 * Counts non-overlapping occurrences of $needle in $haystack, so 'aaa' counts one occurrence of
 * 'aa', not two. $offset and $length restrict the search to part of $haystack, both counted in
 * bytes rather than characters.
 *
 * @return int<0,max>
 *
 * @pure
 */
function substr_count(string $haystack, string $needle, int $offset = 0, ?int $length = null): int {}

/**
 * Returns the length, in bytes, of the initial segment of string that consists entirely of
 * characters found in characters, starting at $offset and optionally limited to $length bytes.
 *
 * @pure
 */
function strspn(string $string, string $characters, int $offset = 0, ?int $length = null): int {}

/**
 * Returns the length of the initial segment of string made up entirely of characters not found in
 * characters — the complement of strspn. Length is counted in bytes, not multibyte characters.
 *
 * @code
 * strcspn('hello world', 'ow'); // 4
 * @endcode
 *
 * @pure
 */
function strcspn(string $string, string $characters, int $offset = 0, ?int $length = null): int {}

/**
 * Splits a string into tokens delimited by any of the characters in token. The first call supplies
 * both the string and its delimiters; each further call, to continue tokenizing the same string, is
 * made with only a new set of delimiter characters. Returns false once no further token is found.
 *
 * @code
 * $tok = strtok($string, ',');
 * while ($tok !== false) {
 *     $tok = strtok(',');
 * }
 * @endcode
 *
 * @pure
 */
function strtok(string $string, ?string $token = null): string|false {}

/**
 * Converts the ASCII alphabetic characters in a string to uppercase. Leaves accented or multibyte
 * characters unchanged; use mb_strtoupper() for those.
 *
 * @return ($string is non-empty-string ? non-empty-uppercase-string : '')
 *
 * @pure
 */
function strtoupper(string $string): string {}

/**
 * Converts the ASCII letters A-Z in a string to lowercase. Multibyte characters are left unchanged,
 * so it is not safe for non-ASCII text; use mb_strtolower instead.
 *
 * @return ($string is non-empty-string ? non-empty-lowercase-string : '')
 *
 * @pure
 */
function strtolower(string $string): string {}

/**
 * Returns the position of the first occurrence of a substring within a string, or false if it is
 * not found. It works byte-by-byte rather than by multibyte character, and the result must be
 * compared with === since a match at position 0 is falsy.
 *
 * @return non-negative-int|false
 *
 * @pure
 */
function strpos(string $haystack, string $needle, int $offset = 0): int|false {}

/**
 * Finds the position of the first case-insensitive occurrence of $needle in $haystack. Returns
 * false when not found, so the result must be compared with ===, since a match at position 0 is
 * falsy.
 *
 * @return int<0, max>|false
 *
 * @pure
 */
function stripos(string $haystack, string $needle, int $offset = 0): int|false {}

/**
 * Returns the position of the last occurrence of $needle in $haystack, searching from $offset.
 * Returns false if $needle is not found.
 *
 * @return int<0, max>|false
 *
 * @pure
 */
function strrpos(string $haystack, string $needle, int $offset = 0): int|false {}

/**
 * Case-insensitively finds the offset of the last occurrence of needle in haystack, counting bytes
 * rather than characters. A positive offset skips that many bytes from the start of haystack before
 * searching; a negative offset starts that many bytes before the end and searches backwards from
 * there. Returns false if needle does not occur.
 *
 * @return int<0, max>|false
 *
 * @pure
 */
function strripos(string $haystack, string $needle, int $offset = 0): int|false {}

/**
 * Returns string with its bytes in reverse order. Because it operates on bytes rather than
 * characters, reversing a multi-byte UTF-8 string this way corrupts it.
 *
 * @return ($string is non-empty-string ? non-empty-string : string)
 *
 * @pure
 */
function strrev(string $string): string {}

/**
 * Converts a logically-ordered Hebrew string into visual order for correct right-to-left display.
 * When max_chars_per_line is greater than zero, the text is also wrapped so that no line exceeds
 * that many characters; 0 means no wrapping.
 *
 * @pure
 */
function hebrev(string $string, int $max_chars_per_line = 0): string {}

/**
 * Inserts an HTML line-break tag before every newline in a string, in its XHTML form unless
 * $use_xhtml is false.
 *
 * @pure
 */
function nl2br(string $string, bool $use_xhtml = true): string {}

/**
 * Returns the trailing filename component of $path, optionally stripping a trailing $suffix from
 * it.
 *
 * @pure
 *
 * @return ($path is non-empty-string ? non-empty-string : string)
 */
function basename(string $path, string $suffix = ''): string {}

/**
 * Returns the parent directory portion of $path. $levels controls how many levels of the path to
 * strip; values greater than 1 go up multiple directories.
 *
 * @pure
 *
 * @return ($path is non-empty-string ? non-empty-string : string)
 */
function dirname(string $path, int $levels = 1): string {}

/**
 * Returns information about a file path - directory name, basename, extension and filename. Pass a
 * single PATHINFO_* flag to get just that component as a string instead of the full array.
 *
 * @param 1|2|4|8|15 $flags
 *
 * @return ($flags is 15 ? array{dirname?: string, basename: string, extension?: string, filename: string} : string)
 */
function pathinfo(string $path, int $flags = PATHINFO_ALL): array|string {}

/**
 * Removes the backslashes added by addslashes(), un-escaping quotes, backslashes and NUL bytes — it
 * is not a general-purpose way to make untrusted input safe for SQL, shell or HTML output.
 *
 * @pure
 */
function stripslashes(string $string): string {}

/**
 * Reverses addcslashes() by interpreting C-style backslash escape sequences in string, such as \n,
 * \t and octal or hexadecimal \xHH escapes, back into their literal characters.
 *
 * @pure
 */
function stripcslashes(string $string): string {}

/**
 * Returns the portion of $haystack from the first occurrence of $needle onwards, or, when
 * $before_needle is true, the portion before it instead. The comparison is case-sensitive, and it
 * returns false if $needle does not occur in $haystack.
 *
 * @pure
 */
function strstr(string $haystack, string $needle, bool $before_needle = false): string|false {}

/**
 * Returns the portion of $haystack starting at the first case-insensitive occurrence of $needle, or
 * the portion before it when $before_needle is true, or false if $needle does not occur anywhere in
 * $haystack.
 *
 * @pure
 */
function stristr(string $haystack, string $needle, bool $before_needle = false): string|false {}

/**
 * Finds the last occurrence of a character in a string and returns everything from that point to
 * the end. Only the first character of $needle is used, even if a longer string is passed. When
 * $before_needle is true, it instead returns everything in $haystack before that last occurrence.
 * Returns false if the character is not found.
 *
 * @pure
 */
function strrchr(string $haystack, string $needle, bool $before_needle = false): string|false {}

/**
 * Randomly reorders the characters of a string, byte by byte, and returns the result as a new
 * string of the same length. It is not suitable for cryptographic use, and because it shuffles
 * bytes rather than characters, applying it to a multibyte-encoded string can corrupt multibyte
 * characters.
 *
 * @template T as string
 *
 * @param T $string
 *
 * @return (T is non-empty-string ? non-empty-string : string)
 *
 * @pure
 */
function str_shuffle(string $string): string {}

/**
 * Counts or extracts the words in $string. $format controls the shape of the result: 0 (the
 * default) returns the word count as an integer, 1 returns the words themselves as a list, and 2
 * returns the words as an array keyed by the byte offset at which each occurs in $string.
 * $characters, if given, lists extra characters besides letters, apostrophes and hyphens that
 * should be treated as part of a word.
 *
 * @code
 * str_word_count($string, 0); // word count
 * str_word_count($string, 1); // list of words
 * str_word_count($string, 2); // words keyed by offset
 * @endcode
 *
 * @template T as 0|1|2
 *
 * @param T $format
 *
 * @return (T is 0 ? int<0, max> : (
 *   T is 1 ? list<non-empty-string> : (
 *     T is 2 ? array<int<0, max>, non-empty-string> : (
 *       int<0, max>|list<non-empty-string>|array<int<0, max>, non-empty-string>
 *     )
 *   )
 * ))
 *
 * @pure
 */
function str_word_count(string $string, int $format = 0, ?string $characters = null): array|int {}

/**
 * Splits $string into an array of chunks of $length. It operates on bytes, not multibyte
 * characters, so use mb_str_split() for multibyte strings.
 *
 * @param int<1, max> $length
 *
 * @return list<string>
 *
 * @pure
 */
function str_split(string $string, int $length = 1): array {}

/**
 * Searches $string for the first occurrence of any character in $characters and returns the
 * remainder of $string from that character onward, or false if none of the characters appear.
 *
 * @pure
 */
function strpbrk(string $string, string $characters): string|false {}

/**
 * Compares $needle against the portion of $haystack starting at $offset — counted from the end when
 * negative — and, if given, no longer than $length bytes. $case_insensitive makes the comparison
 * ignore ASCII case. Returns a value less than, equal to, or greater than zero, in the same style
 * as strcmp().
 *
 * @pure
 */
function substr_compare(
    string $haystack,
    string $needle,
    int $offset,
    ?int $length = null,
    bool $case_insensitive = false,
): int {}

/**
 * Compares string1 and string2 using the collation rules of the current LC_COLLATE locale,
 * returning a value less than, equal to, or greater than zero. Behaves like strcmp() unless a
 * locale has been set with setlocale().
 *
 * @pure
 */
function strcoll(string $string1, string $string2): int {}

/**
 * Returns the portion of $string starting at $offset with the given $length. A negative $offset or
 * $length counts from the end of the string, and the operation is on bytes, not multibyte
 * characters.
 *
 * @pure
 */
function substr(string $string, int $offset, ?int $length = null): string {}

/**
 * Replaces the portion of string given by offset and length with replace. A negative offset counts
 * from the end of the string, and a negative length leaves that many characters at the end of the
 * string untouched rather than replacing them; omitting length replaces through to the end of the
 * string. When string is an array, replace, offset and length may also be arrays, applied element
 * by element to each string in parallel.
 *
 * @code
 * substr_replace(['aaa', 'bbb'], 'X', 1, 1); // ['aXa', 'bXb']
 * @endcode
 *
 * @template K as array-key
 *
 * @param string|array<K, string> $string
 * @param string|array<string> $replace
 * @param int|array<int> $offset
 * @param null|int<0, max>|array<int<0, max>> $length
 *
 * @return ($string is string ? string : array<K, string>)
 *
 * @pure
 */
function substr_replace(
    array|string $string,
    array|string $replace,
    array|int $offset,
    array|int|null $length = null,
): array|string {}

/**
 * Escapes the characters . \ + * ? [ ^ ] $ ( ) with a backslash, so they can be used literally
 * where they would otherwise have special meaning, such as in a regular expression.
 *
 * @pure
 */
function quotemeta(string $string): string {}

/**
 * Converts the first character of $string to uppercase, leaving the rest of the string unchanged.
 *
 * @pure
 *
 * @return ($string is non-empty-string ? non-empty-string : string)
 */
function ucfirst(string $string): string {}

/**
 * Lowercases the first character of a string, leaving the rest unchanged.
 *
 * @pure
 *
 * @return ($string is non-empty-string ? non-empty-string : string)
 */
function lcfirst(string $string): string {}

/**
 * Uppercases the first character of each word in a string, where a word boundary is any character
 * in $separators. Only ASCII letters are affected.
 *
 * @pure
 *
 * @return ($string is non-empty-string ? non-empty-string : string)
 */
function ucwords(string $string, string $separators = " \t\r\n\f\v"): string {}

/**
 * Translates characters or substrings within a string according to a replacement mapping. With two
 * string arguments it maps character-by-character; with an array, longer keys take precedence over
 * shorter ones regardless of array order.
 *
 * @param string|array<string, string> $from
 *
 * @pure
 */
function strtr(string $string, string|array $from, string $to = ''): string {}

/**
 * Escapes characters that have special meaning in an SQL statement or a C-style string literal --
 * single quote, double quote, backslash and the NUL byte -- by prefixing each with a backslash. Not
 * a safe way to escape values for a SQL query by itself; use the database driver's own escaping, or
 * better, parameterised queries.
 *
 * @pure
 *
 * @return ($string is non-empty-string ? non-empty-string : string)
 */
function addslashes(string $string): string {}

/**
 * Backslash-escapes every occurrence of the characters listed in characters within string.
 * characters may include ranges written with two dots, such as a..z; control characters like
 * newline, carriage return, and tab are emitted using their usual short escape form, and any other
 * escaped byte outside the printable ASCII range is emitted as an octal sequence.
 *
 * @code
 * addcslashes($string, 'a..zA..Z');
 * @endcode
 *
 * @pure
 *
 * @return ($string is non-empty-string ? non-empty-string : string)
 */
function addcslashes(string $string, string $characters): string {}

/**
 * Strips whitespace, or the given $characters, from the end of $string.
 *
 * @pure
 */
function rtrim(string $string, string $characters = " \n\r\t\v\0"): string {}

/**
 * Replaces every occurrence of $search with $replace in $subject. Comparison is case-sensitive and
 * matching is binary-safe.
 *
 * @param string|array<string> $search
 * @param string|array<string> $replace
 * @param string|array<string> $subject
 *
 * @param-out int $count
 *
 * @return ($subject is string ? string : array<string>)
 *
 * @pure
 */
function str_replace(
    array|string $search,
    array|string $replace,
    array|string $subject,
    ?int &$count = null,
): array|string {}

/**
 * Replaces occurrences of a search string with a replacement, matching case-insensitively.
 *
 * @param string|array<string> $search
 * @param string|array<string> $replace
 * @param string|array<string> $subject
 *
 * @param-out int $count
 *
 * @return ($subject is string ? string : array<string>)
 *
 * @pure
 */
function str_ireplace(
    array|string $search,
    array|string $replace,
    array|string $subject,
    ?int &$count = null,
): array|string {}

/**
 * Returns a new string consisting of the input string repeated the given number of times.
 *
 * @pure
 *
 * @param non-negative-int $times
 * @return (
 *   $string is non-empty-string
 *     ? ($times is positive-int ? non-empty-string : string)
 *     : string
 * )
 */
function str_repeat(string $string, int $times): string {}

/**
 * Counts how often each byte value occurs in the string. With mode 0 (the default) it returns an
 * array of all 256 byte values with their counts, including zeros; mode 1 keeps only the byte
 * values that occur at least once, and mode 2 keeps only those that never occur. Modes 3 and 4
 * return a string instead of an array: mode 3 is built from every distinct byte value present, and
 * mode 4 is built from every byte value that never occurs.
 *
 * @pure
 *
 * @return non-empty-list<non-negative-int>
 */
function count_chars(string $string, int $mode = 0): array|string {}

/**
 * Splits $string into chunks of $length characters and appends $separator (default CRLF) after
 * every chunk, including the last one, for output formats such as base64-encoded MIME bodies that
 * expect fixed-width lines. It operates on bytes, so a multibyte character can end up split across
 * two chunks.
 *
 * @pure
 */
function chunk_split(string $string, int $length = 76, string $separator = "\r\n"): string {}

/**
 * Strips whitespace, or a given set of characters, from both ends of a string.
 *
 * @pure
 */
function trim(string $string, string $characters = " \n\r\t\v\0"): string {}

/**
 * Strips whitespace, or a given set of characters, from the start of a string.
 *
 * @pure
 */
function ltrim(string $string, string $characters = " \n\r\t\v\0"): string {}

/**
 * Strips HTML and PHP tags from a string, keeping only tags listed in $allowed_tags. It is not a
 * full HTML parser and is not a reliable defence against XSS.
 *
 * @param array<string>|string|null $allowed_tags
 *
 * @pure
 */
function strip_tags(string $string, string|array|null $allowed_tags = null): string {}

/**
 * Returns the number of matching characters between string1 and string2, and is case-sensitive.
 * When percent is given by reference, it receives how similar the two strings are, expressed as a
 * percentage.
 *
 * @code
 * similar_text('World', 'Word', $percent);
 * @endcode
 *
 * @pure
 */
function similar_text(string $string1, string $string2, &$percent = null): int {}

/**
 * Splits a string into an array of substrings using the given delimiter. The delimiter cannot be an
 * empty string, which raises a ValueError.
 *
 * @return non-empty-list<string>
 *
 * @pure
 */
function explode(string $separator, string $string, int $limit = PHP_INT_MAX): array {}

/**
 * Joins the elements of $array into a single string, with $separator placed between each element.
 *
 * @param array<string>|string $separator
 * @param array<int|string|float|bool|null|Stringable>|null $array
 *
 * @return (
 *     $array is non-empty-array<non-empty-literal-string|literal-int|literal-float>
 *     ? ($separator is literal-string ? non-empty-literal-string : non-empty-string)
 *     : ($array is non-empty-array<non-empty-string|int|float>
 *         ? non-empty-string
 *         : string
 *     )
 * )
 *
 * @pure
 */
function implode(array|string $separator = '', ?array $array = null): string {}

/**
 * Joins the elements of an array into a string, with each element separated by the given separator;
 * an alias of implode().
 *
 * @param array<string>|string $separator
 * @param array<int|string|float|bool|null|Stringable>|null $array
 *
 * @return (
 *     $separator is non-empty-string
 *     ? ($array is non-empty-array
 *         ? ($array is array<literal-string|literal-int>
 *             ? ($separator is literal-string ? non-empty-literal-string : non-empty-string)
 *             : non-empty-string
 *         )
 *         : string)
 *     : ($array is non-empty-array
 *         ? ($array is array<non-empty-literal-string|non-empty-string>
 *             ? ($array is array<non-empty-literal-string> ? non-empty-literal-string : non-empty-string)
 *             : string
 *         )
 *         : string)
 * )
 *
 * @pure
 */
function join(array|string $separator = '', ?array $array = null): string {}

/**
 * Tries each candidate locale in turn, from $locales and the additional $rest arguments, and sets
 * the first one the platform accepts for the given category, returning the name that was actually
 * set, or false if none of them were valid. An empty string as a candidate asks the system to use
 * the locale named by the environment's LANG/LC_* variables, and passing 0 leaves the current
 * locale unchanged and simply returns it.
 *
 * @code
 * setlocale(LC_ALL, ['en_US.UTF-8', 'en_US', 'en']);
 * @endcode
 *
 * @param string|array<string>|int $locales
 * @param string|array<string> ...$rest
 */
function setlocale(int $category, string|int|array $locales, string|array ...$rest): string|false {}

/**
 * Returns the numeric and monetary formatting conventions of the locale currently set with
 * setlocale().
 *
 * @return array{
 *   decimal_point: string,
 *   thousands_sep: string,
 *   grouping: array<int, int>,
 *   int_curr_symbol: string,
 *   currency_symbol: string,
 *   mon_decimal_point: string,
 *   mon_thousands_sep: string,
 *   mon_grouping: string,
 *   positive_sign: string,
 *   negative_sign: string,
 *   int_frac_digits: string,
 *   frac_digits: string,
 *   p_cs_precedes: bool,
 *   p_sep_by_space: bool,
 *   n_cs_precedes: bool,
 *   n_sep_by_space: bool,
 *   p_sign_posn: int,
 *   n_sign_posn: int
 * }
 *
 * @pure
 */
function localeconv(): array {}

/**
 * Returns the first element for which the callback returns true, or null if no element matches.
 *
 * @template K
 * @template V
 *
 * @param array<K, V> $array
 * @param (callable(V, K): bool) $callback
 *
 * @return V|null
 *
 * @since 8.4
 */
#[Mago\AvailableSince(80400)]
function array_find(array $array, callable $callback): mixed {}

/**
 * Returns the key of the first element of $array for which $callback returns true, given that
 * element's value and key, or null if no element matches.
 *
 * @template K
 * @template V
 *
 * @param array<K, V> $array
 * @param (callable(V, K): bool) $callback
 *
 * @return ($array is array{} ? null : K|null)
 *
 * @since 8.4
 */
#[Mago\AvailableSince(80400)]
function array_find_key(array $array, callable $callback): mixed {}

/**
 * Returns true if $callback returns true for at least one element of $array.
 *
 * @template K
 * @template V
 *
 * @param array<K, V> $array
 * @param (callable(V, K): bool) $callback
 *
 * @since 8.4
 */
#[Mago\AvailableSince(80400)]
function array_any(array $array, callable $callback): bool {}

/**
 * Returns true if the callback returns a truthy value for every element of the array. Returns true
 * for an empty array, since there is no element for the callback to fail on.
 *
 * @template K
 * @template V
 *
 * @param array<K, V> $array
 * @param (callable(V, K): bool) $callback
 *
 * @since 8.4
 */
#[Mago\AvailableSince(80400)]
function array_all(array $array, callable $callback): bool {}

/**
 * Returns the raw response headers, including the status line, from the most recent HTTP request
 * made through a stream wrapper, such as a file_get_contents() call on an http:// or https:// URL.
 * It is the functional equivalent of the automatic $http_response_header variable, and returns null
 * if no HTTP request has been made yet in the current scope.
 *
 * @return null|list<non-empty-string>
 *
 * @since 8.4
 */
function http_get_last_response_headers(): ?array {}

/**
 * @since 8.4
 */
#[Mago\AvailableSince(80400)]
function http_clear_last_response_headers(): void {}

/**
 * Parses the raw request body from php://input according to its Content-Type header and returns a
 * two-element list of the decoded fields followed by any uploaded files, mirroring $_POST and
 * $_FILES. This is chiefly useful for methods such as PUT or PATCH, for which PHP does not populate
 * those superglobals automatically.
 *
 * @code
 * [$fields, $files] = request_parse_body();
 * @endcode
 *
 * @since 8.4
 * @param array|null $options
 * @return array<int, array>
 */
function request_parse_body(?array $options = null): array {}

/**
 * Raises $num to the power of $exponent and always returns the result as a float, unlike pow(),
 * which returns an int when both arguments are integers and the result fits in one.
 */
function fpow(float $num, float $exponent): float {}

#[Mago\AvailableSince(80400)]
enum RoundingMode implements UnitEnum
{
    case HalfAwayFromZero;
    case HalfTowardsZero;
    case HalfEven;
    case HalfOdd;
    case TowardsZero;
    case AwayFromZero;
    case NegativeInfinity;
    case PositiveInfinity;
}

#[Mago\AvailableSince(80600)]
enum SortDirection implements UnitEnum
{
    case Ascending;
    case Descending;
}

/**
 * Returns locale-specific formatting information for the given item, such as date and time strings,
 * currency symbols or the decimal point, according to the active locale. Not implemented on
 * Windows.
 *
 * @pure
 */
function nl_langinfo(int $item): string|false {}

/**
 * Calculates the soundex key of string, a four-character code approximating its English
 * pronunciation, so that similarly-sounding words (such as "Robert" and "Rupert") share the same
 * key.
 *
 * @pure
 */
function soundex(string $string): string {}

/**
 * Calculates the Levenshtein distance between two strings, the minimum number of single-character
 * edits needed to turn one into the other.
 */
function levenshtein(
    string $string1,
    string $string2,
    int $insertion_cost = 1,
    int $replacement_cost = 1,
    int $deletion_cost = 1,
): int {}

/**
 * Returns a one-byte string containing the character for the given byte value, wrapping modulo 256
 * for values outside the range 0-255.
 *
 * @pure
 */
function chr(int $codepoint): string {}

/**
 * Returns the byte value, 0 to 255, of the first byte of character. Because it works on bytes
 * rather than characters, a multi-byte UTF-8 character yields only the value of its first byte.
 * Returns 0 for an empty string.
 *
 * @param string $character
 * @return int<0, 255>
 *
 * @pure
 */
function ord(string $character): int {}

/**
 * Parses string as a URL query string, using the same syntax as $_GET, and writes the resulting
 * variables into result by reference, replacing whatever it previously held. Keys using bracket
 * syntax such as a[b] produce nested arrays in result.
 *
 * @param-out array<string, string|array<array-key, string|array<array-key, mixed>>> $result
 *
 * @return void
 */
function parse_str(string $string, &$result): void {}

/**
 * Parses a single line of CSV-formatted text into an array of fields, using the given field
 * separator, enclosure and escape characters.
 *
 * @pure
 */
function str_getcsv(string $string, string $separator = ',', string $enclosure = '"', string $escape = "\\"): array {}

/**
 * Pads $string with $pad_string until it reaches $length, adding to the right by default. No
 * padding takes place if $string is already at least that long.
 *
 * @pure
 */
function str_pad(string $string, int $length, string $pad_string = ' ', int $pad_type = STR_PAD_RIGHT): string {}

/**
 * Alias of rtrim(): strips whitespace, or the characters listed in $characters, from the end of
 * $string only, leaving the start untouched.
 *
 * @pure
 */
function chop(string $string, string $characters = " \n\r\t\v\0"): string {}

/**
 * Alias of strstr(): finds the first occurrence of needle in haystack and returns everything from
 * that point onward, or everything before it when before_needle is true. Returns false if needle is
 * not found; the comparison is case-sensitive.
 *
 * @pure
 */
function strchr(string $haystack, string $needle, bool $before_needle = false): string|false {}

/**
 * Returns a string built from $format with each conversion specifier replaced by the corresponding
 * value. Unlike printf(), the result is returned rather than printed.
 *
 * @param string|int|float|bool|null|Stringable ...$values
 *
 * @pure
 */
function sprintf(string $format, mixed ...$values): string {}

/**
 * Writes a formatted string built from $format and $values directly to output and returns the
 * number of bytes written, unlike sprintf(), which returns the string instead of printing it.
 *
 * @param string|int|float|bool|null|Stringable ...$values
 *
 * @return int<0, max>
 */
function printf(string $format, mixed ...$values): int {}

/**
 * Formats $format with the values in $values the same way printf() would with them as separate
 * arguments, and writes the result to output. Returns the number of bytes written, not the number
 * of characters.
 *
 * @param array<string|int|float|bool|null|Stringable> $values
 */
function vprintf(string $format, array $values): int {}

/**
 * Returns a string formatted according to $format, taking its substitution values from the $values
 * array instead of from separate arguments -- the array-argument analogue of sprintf().
 *
 * @param array<string|int|float|bool|null|Stringable> $values
 *
 * @pure
 */
function vsprintf(string $format, array $values): string {}

/**
 * Writes a formatted string, built the same way as sprintf(), to the given stream and returns the
 * number of bytes written.
 *
 * @param resource $stream
 * @param string|int|float|bool|null|Stringable ...$values
 */
function fprintf($stream, string $format, mixed ...$values): int {}

/**
 * Writes a formatted string, built from $format and the positional arguments in $values the same
 * way sprintf() would build it, to $stream. Returns the number of bytes written, not characters.
 *
 * @param resource $stream
 * @param array<string|int|float|bool|null|Stringable> $values
 */
function vfprintf($stream, string $format, array $values): int {}

/**
 * Parses $string according to the sprintf-style $format. Called with only the two required
 * arguments it returns the parsed values as an array; passed further arguments by reference, it
 * assigns the values to them instead and returns how many were assigned.
 *
 * @code
 * sscanf('age: 42', 'age: %d', $age);
 * $parsed = sscanf('bob 42', '%s %d');
 * @endcode
 */
function sscanf(string $string, string $format, mixed &...$vars): array|int|null {}

/**
 * Parses input read from $stream according to a sscanf()-style $format string. Called with just
 * $stream and $format, it returns the parsed values as an array; given additional variables by
 * reference, it assigns each parsed value to one of them instead and returns the count of values
 * assigned.
 *
 * @param resource $stream
 */
function fscanf($stream, string $format, mixed &...$vars): array|int|false|null {}

/**
 * Parses a URL into its components - scheme, host, path, query and so on. It does not validate the
 * URL; it only returns false for input malformed enough to be unparseable.
 *
 * @return ($component is -1
 *     ? array{scheme?: string, host?: string, port?: int, user?: string, pass?: string, path?: string, query?: string, fragment?: string}|false
 *     : ($component is 2 ? int|false|null : string|false|null))
 *
 * @pure
 */
function parse_url(string $url, int $component = -1): array|string|int|false|null {}

/**
 * Encodes string for use in a URL query string, following the application/x-www-form-urlencoded
 * convention: a space becomes a plus sign and other non-alphanumeric bytes become %XX sequences,
 * unlike rawurlencode(), which encodes a space as %20 per RFC 3986.
 *
 * @pure
 */
function urlencode(string $string): string {}

/**
 * Decodes an application/x-www-form-urlencoded string: percent-encoded bytes (%XX) are converted
 * back to their raw byte, and a literal + is converted to a space. Use rawurldecode instead when +
 * should be kept literal, as in a URL path rather than a query string.
 *
 * @pure
 */
function urldecode(string $string): string {}

/**
 * Returns $string with every character except unreserved ones percent-encoded per RFC 3986. Unlike
 * urlencode(), spaces are encoded as %20 rather than a plus sign.
 *
 * @pure
 */
function rawurlencode(string $string): string {}

/**
 * Decodes a percent-encoded (RFC 3986) URL string. Unlike urldecode(), it does not convert a plus
 * sign into a space.
 *
 * @pure
 */
function rawurldecode(string $string): string {}

/**
 * Builds a URL-encoded query string from the given array or object, joining pairs with
 * arg_separator (or the ini default when omitted). Numeric array keys are prefixed with
 * numeric_prefix, since a bare numeric key is not a valid variable name in the result.
 * encoding_type selects between RFC 1738 encoding, which encodes spaces as +, and RFC 3986
 * encoding, which encodes them as %20.
 *
 * @pure
 */
function http_build_query(
    object|array $data,
    string $numeric_prefix = '',
    ?string $arg_separator = null,
    int $encoding_type = PHP_QUERY_RFC1738,
): string {}

/**
 * Returns the target a symbolic link points to, exactly as stored in the link -- which may be a
 * relative path -- rather than the canonical absolute path realpath() would produce. Returns false
 * if path is not a valid symbolic link.
 */
function readlink(string $path): string|false {}

/**
 * Returns the device number from the filesystem entry at path without following it, so it succeeds
 * even for a symbolic link whose target is missing, unlike stat(), which follows the link and fails
 * when the target does not exist. Returns false only when there is no entry at all, not even a
 * broken link, at path; always returns 0 on Windows.
 */
function linkinfo(string $path): int|false {}

/**
 * Creates a symbolic link. The argument order is target then link name: the first argument is the
 * file or directory being pointed to, and the second is the path of the link to create.
 *
 * @code
 * symlink('/var/www/shared/upload.php', '/var/www/site/upload.php');
 * @endcode
 */
function symlink(string $target, string $link): bool {}

/**
 * Creates a hard link named $link pointing at the already-existing file $target; both names then
 * refer to the same inode, so writes through either are visible via the other, and the underlying
 * data is only freed once every link to it is removed. $target and $link must reside on the same
 * filesystem, unlike symlink(), which has no such restriction.
 *
 * @code
 * link('/var/data/report.csv', '/var/data/report-hardlink.csv');
 * @endcode
 */
function link(string $target, string $link): bool {}

/**
 * Deletes a file from the filesystem.
 *
 * @param null|resource $context
 */
function unlink(string $filename, mixed $context = null): bool {}

/**
 * Executes an external program and returns the last line of its output.
 *
 * @param-out list<string> $output
 * @param-out int $result_code
 */
function exec(string $command, ?array &$output = null, int|int &$result_code = null): string|false {}

/**
 * Executes command as an external program, streaming its output directly rather than capturing it,
 * and returns the last line of that output. Returns false if the command could not be executed;
 * when result_code is given by reference, it receives the command's exit status.
 *
 * @code
 * $lastLine = system('ls -la', $exitCode);
 * @endcode
 *
 * @param-out int $result_code
 */
function system(string $command, ?int &$result_code = null): string|false {}

/**
 * Escapes any characters in $command that a shell would otherwise treat specially, so the whole
 * command line can be passed safely to an exec()-family function. Unlike escapeshellarg(), it
 * escapes the string in place rather than wrapping it in quotes, so it is meant for a full command
 * line rather than a single argument.
 *
 * @pure
 */
function escapeshellcmd(string $command): string {}

/**
 * Escapes a string so it can be safely passed as a single argument to a shell command. Wraps the
 * value in quotes appropriate to the operating system.
 *
 * @pure
 */
function escapeshellarg(string $arg): string {}

/**
 * Runs an external program and streams its raw output directly to PHP's own output rather than
 * capturing it, making it suitable for commands that produce binary data. Returns false if the
 * command could not be executed, or null otherwise; the command's exit status is written to
 * $result_code, not returned.
 *
 * @param-out int $result_code
 */
function passthru(string $command, &$result_code = null): ?false {}

/**
 * Runs $command through the shell and returns everything the command wrote to standard output, the
 * function-call form of the backtick operator. Returns null if the command runs but produces no
 * output, and false if the command could not be executed at all.
 */
function shell_exec(string $command): string|false|null {}

/**
 * Spawns a process for the given command and opens pipes or files for the descriptors requested in
 * $descriptor_spec, without waiting for the command to finish. When $command is given as an array
 * rather than a string, the program is executed directly and no shell is invoked, so shell
 * metacharacters are not interpreted. Reading and writing the resulting pipes in the wrong order
 * can deadlock a process whose own buffers fill up, so a long-running exchange with a child process
 * typically needs non-blocking reads or a separate read/write loop.
 *
 * @code
 * proc_open('sort', [0 => ['pipe', 'r'], 1 => ['pipe', 'w']], $pipes);
 * @endcode
 *
 * @param array<string>|string $command
 * @param array{
 *   0?: resource|object|array{0: 'pipe', 1: 'r'|'w'}|array{0: 'file', 1: non-empty-string, 2?: 'r'|'w'},
 *   1?: resource|object|array{0: 'pipe', 1: 'r'|'w'}|array{0: 'file', 1: non-empty-string, 2?: 'r'|'w'},
 *   2?: resource|object|array{0: 'pipe', 1: 'r'|'w'}|array{0: 'file', 1: non-empty-string, 2?: 'r'|'w'},
 * } $descriptor_spec
 * @param non-empty-string|null $cwd
 * @param null|array<string, string> $env_vars
 * @param null|array{
 *   suppress_errors?: bool,
 *   bypass_shell?: bool,
 *   blocking_pipes?: bool,
 *   create_process_group?: bool,
 *   create_new_console?: bool,
 * } $options
 *
 * @param-out array{
 *   0?: resource,
 *   1?: resource,
 *   2?: resource,
 * } $pipes
 *
 * @return open-resource|false
 */
function proc_open(
    array|string $command,
    array $descriptor_spec,
    ?array &$pipes,
    ?string $cwd = null,
    ?array $env_vars = null,
    ?array $options = null,
) {}

/**
 * Waits for the process opened by proc_open() to terminate and returns its exit code, or -1 if one
 * could not be obtained. Should only be called after every pipe returned alongside the process has
 * been read to completion and closed, since the child may be blocked writing to a full pipe that
 * nothing is draining, which would make this call hang indefinitely.
 *
 * @param resource $process
 */
function proc_close($process): int {}

/**
 * Sends $signal, SIGTERM by default, to the process behind $process, opened with proc_open, without
 * waiting to see whether it actually exits. On Windows $signal is ignored and the process is simply
 * killed. Returns whether the signal was sent, not whether the process terminated.
 *
 * @param resource $process
 */
function proc_terminate($process, int $signal = 15): bool {}

/**
 * Retrieves the current status of a process opened with proc_open(). running turns false once the
 * process has exited, and exitcode holds its exit code only on the first status check taken after
 * that happens — every call after the first reads -1 for it, so the code must be captured
 * immediately. signaled and termsig, and stopped and stopsig, report the same kind of information
 * for a process killed or suspended by a signal.
 *
 * @code
 * $status = proc_get_status($process);
 * $code = $status['exitcode']; // -1 on every call after the first one taken post-exit
 * @endcode
 *
 * @param resource $process
 *
 * @return array{
 *  'command': string,
 *  'pid': int,
 *  'running': bool,
 *  'signaled': bool,
 *  'stopped': bool,
 *  'exitcode': int,
 *  'termsig': int,
 *  'stopsig': int,
 * }
 */
function proc_get_status($process): array {}

/**
 * Changes the scheduling priority (niceness) of the current process by $priority; a positive value
 * lowers priority and a negative value raises it, the latter typically requiring elevated
 * privileges. Returns false, with a warning, on failure. Not implemented on Windows.
 */
function proc_nice(int $priority): bool {}

/**
 * Looks up the port number for $service under the given $protocol (typically "tcp" or "udp") in the
 * system's services database, returning false if no matching entry exists.
 */
function getservbyname(string $service, string $protocol): int|false {}

/**
 * Looks up the service name associated with the given port number for the given protocol, typically
 * tcp or udp, using the system's services database. Returns false if no matching entry exists.
 *
 * @pure
 */
function getservbyport(int $port, string $protocol): string|false {}

/**
 * Looks up a network protocol's number from the system's protocol database by name, such as 'tcp'
 * or 'udp'. Returns false if the name is not recognised.
 *
 * @pure
 */
function getprotobyname(string $protocol): int|false {}

/**
 * Looks up the protocol name, such as tcp or udp, registered for protocol number in the system's
 * protocol database, returning false if no matching entry exists.
 *
 * @pure
 */
function getprotobynumber(int $protocol): string|false {}

/**
 * Returns the numeric user id associated with the running script, typically that of the process
 * owner, or false on error.
 *
 * @pure
 */
function getmyuid(): int|false {}

/**
 * Returns the real group ID the current PHP process is running as. Returns false if this cannot be
 * determined, such as on Windows, where the concept does not apply.
 *
 * @pure
 */
function getmygid(): int|false {}

/**
 * Returns the process ID of the current PHP process. Returns false in the rare cases where the ID
 * cannot be determined, such as under some non-standard SAPI builds.
 *
 * @pure
 */
function getmypid(): int|false {}

/**
 * Returns the inode number of the current script file, or false if it cannot be determined, such as
 * on a filesystem that has no concept of inodes.
 *
 * @pure
 */
function getmyinode(): int|false {}

/**
 * Returns the Unix timestamp of the last modification time of the main script file being executed,
 * or false on failure.
 */
function getlastmod(): int|false {}

/**
 * Decodes a base64-encoded string back to binary data. With $strict false (the default) invalid
 * characters are silently discarded; with $strict true, invalid input makes it return false.
 *
 * @return ($strict is false ? string : string|false)
 *
 * @pure
 */
function base64_decode(string $string, bool $strict = false): string|false {}

/**
 * Encodes binary data using base64, producing an ASCII string safe for transport in text-based
 * formats.
 *
 * @pure
 *
 * @return ($string is non-empty-string ? non-empty-string : '')
 */
function base64_encode(string $string): string {}

/**
 * Encodes a string using the uuencode algorithm, historically used to transmit binary data safely
 * over 7-bit text channels such as email.
 *
 * @pure
 */
function convert_uuencode(string $string): string {}

/**
 * Decodes a uuencoded string back into its original binary form. Returns false if the string is not
 * valid uuencoded data.
 *
 * @pure
 */
function convert_uudecode(string $string): string|false {}

/**
 * Returns the absolute (unsigned) value of $num.
 *
 * @return ($num is int ? int : float)
 *
 * @pure
 */
function abs(int|float $num): int|float {}

/**
 * Rounds a number up to the nearest integer.
 *
 * @pure
 */
function ceil(int|float $num): float {}

/**
 * Rounds a number down to the nearest integer.
 *
 * @pure
 */
function floor(int|float $num): float {}

/**
 * Rounds a number to the given decimal precision, returning a float. By default, exact halfway
 * values round away from zero; the mode argument selects a different tie-breaking rule.
 *
 * @pure
 */
function round(int|float $num, int $precision = 0, RoundingMode|int $mode = 0): float {}

/**
 * Returns the sine of $num, given in radians.
 *
 * @pure
 */
function sin(float $num): float {}

/**
 * Returns the cosine of $num, taken to be in radians.
 *
 * @pure
 */
function cos(float $num): float {}

/**
 * Returns the tangent of $num, given in radians.
 *
 * @pure
 */
function tan(float $num): float {}

/**
 * Returns the arc sine of num, in radians.
 *
 * @pure
 */
function asin(float $num): float {}

/**
 * Returns the arc cosine of num, in radians. num must be between -1 and 1 inclusive; outside that
 * range the result is NAN.
 *
 * @pure
 */
function acos(float $num): float {}

/**
 * Returns the arc tangent of num, in radians.
 *
 * @pure
 */
function atan(float $num): float {}

/**
 * Returns the inverse hyperbolic tangent of num.
 *
 * @pure
 */
function atanh(float $num): float {}

/**
 * Returns the angle in radians between the positive x-axis and the point (x, y), using the sign of
 * both coordinates to pick the correct quadrant. Note the argument order: y is given first, then x.
 *
 * @code
 * atan2($y, $x);
 * @endcode
 *
 * @pure
 */
function atan2(float $y, float $x): float {}

/**
 * Returns the hyperbolic sine of the given number.
 *
 * @pure
 */
function sinh(float $num): float {}

/**
 * Returns the hyperbolic cosine of $num.
 *
 * @pure
 */
function cosh(float $num): float {}

/**
 * Returns the hyperbolic tangent of num.
 *
 * @pure
 */
function tanh(float $num): float {}

/**
 * Returns the inverse hyperbolic sine of $num.
 *
 * @pure
 */
function asinh(float $num): float {}

/**
 * Returns the inverse hyperbolic cosine of $num. Only defined for $num >= 1; smaller values yield
 * NAN.
 *
 * @pure
 */
function acosh(float $num): float {}

/**
 * Returns exp($num) minus 1, computed by a method that stays accurate even when $num is very close
 * to zero, unlike evaluating exp($num) - 1 directly, which loses precision to cancellation there.
 *
 * @pure
 */
function expm1(float $num): float {}

/**
 * Returns the natural logarithm of 1 plus the given number, computed so as to remain accurate when
 * the number is very close to zero, where computing log(1 + num) directly would lose precision.
 *
 * @pure
 */
function log1p(float $num): float {}

/**
 * Returns an approximation of the mathematical constant π.
 *
 * @pure
 */
function pi(): float {}

/**
 * Checks whether a floating-point value is finite, meaning neither infinite nor NAN.
 *
 * @pure
 */
function is_finite(float $num): bool {}

/**
 * Returns true if $num is NaN, not a number, which typically results from an undefined floating-
 * point operation.
 *
 * @pure
 */
function is_nan(float $num): bool {}

/**
 * Divides $num1 by $num2 and truncates the result toward zero, unlike the / operator, which would
 * return a float. Throws DivisionByZeroError when $num2 is zero, and ArithmeticError for the one
 * division that would overflow a native int: PHP_INT_MIN divided by -1.
 *
 * @pure
 *
 * @throws DivisionByZeroError
 * @throws ArithmeticError
 */
function intdiv(int $num1, int $num2): int {}

/**
 * Checks whether num is INF or -INF. Returns false for NAN.
 *
 * @pure
 */
function is_infinite(float $num): bool {}

/**
 * Returns the given number raised to the power of the exponent. Returns a float instead of an int
 * once the result would overflow the integer range.
 *
 * @pure
 */
function pow(mixed $num, mixed $exponent): object|int|float {}

/**
 * Returns e raised to the power of num.
 *
 * @pure
 */
function exp(float $num): float {}

/**
 * Returns the logarithm of $num to the given $base, or the natural logarithm when no base is given.
 *
 * @pure
 */
function log(float $num, float $base = M_E): float {}

/**
 * Returns the base-10 logarithm of num.
 *
 * @pure
 */
function log10(float $num): float {}

/**
 * Returns the square root of num.
 *
 * @pure
 */
function sqrt(float $num): float {}

/**
 * Returns the length of the hypotenuse of a right triangle with legs x and y, equivalent to
 * sqrt(x**2 + y**2) but computed so as to avoid overflow or underflow on very large or very small
 * inputs.
 *
 * @pure
 */
function hypot(float $x, float $y): float {}

/**
 * Converts a number of degrees to the equivalent number of radians.
 *
 * @pure
 */
function deg2rad(float $num): float {}

/**
 * Converts an angle given in radians to the equivalent in degrees.
 *
 * @pure
 */
function rad2deg(float $num): float {}

/**
 * Converts a binary string to its decimal equivalent, returning a float instead of an int when the
 * value is too large to fit. Characters other than 0 and 1 in binary_string are silently ignored
 * rather than causing an error.
 *
 * @pure
 */
function bindec(string $binary_string): int|float {}

/**
 * Converts a hexadecimal string to its decimal value, silently ignoring any characters that are not
 * valid hexadecimal digits. Returns a float instead of an int when the value is too large for PHP's
 * native integer size.
 *
 * @pure
 */
function hexdec(string $hex_string): int|float {}

/**
 * Converts an octal number given as a string to an integer, or to a float if the value is too large
 * to fit in an int. Characters that are not valid octal digits are ignored rather than causing an
 * error.
 *
 * @pure
 */
function octdec(string $octal_string): int|float {}

/**
 * Returns the binary representation of $num as a string of 0s and 1s. A negative number is rendered
 * as its two's-complement form at the platform's native integer width, rather than with a leading
 * minus sign.
 *
 * @pure
 */
function decbin(int $num): string {}

/**
 * Converts a decimal integer to a string containing its octal representation. A negative number is
 * rendered as the octal digits of its underlying two's-complement bit pattern rather than with a
 * leading minus sign.
 *
 * @pure
 */
function decoct(int $num): string {}

/**
 * Returns the hexadecimal representation of $num as a string. A negative $num is converted from its
 * native-width two's-complement bit pattern, so on a typical 64-bit build dechex(-1) returns a
 * string of sixteen f characters rather than a value with a minus sign.
 *
 * @pure
 */
function dechex(int $num): string {}

/**
 * Converts the digit string $num from $from_base to $to_base, where both bases are between 2 and 36
 * and digits above 9 are the letters a to z. Because the conversion goes through floating-point
 * arithmetic internally, precision is lost for numbers too large to be represented exactly as a
 * float.
 *
 * @pure
 */
function base_convert(string $num, int $from_base, int $to_base): string {}

/**
 * Formats a number with grouped thousands and a fixed number of decimal places, rounding to the
 * given precision.
 *
 * @pure
 */
function number_format(
    float $num,
    int $decimals = 0,
    ?string $decimal_separator = '.',
    ?string $thousands_separator = ',',
): string {}

/**
 * Returns the floating-point remainder of dividing $num1 by $num2, with the sign of $num1,
 * following the same convention as C's fmod() rather than the integer-truncating % operator.
 *
 * @pure
 */
function fmod(float $num1, float $num2): float {}

/**
 * Divides num1 by num2 and returns the result as a float. Division by zero yields INF, -INF or NAN
 * according to sign, instead of throwing the DivisionByZeroError that the / operator raises.
 *
 * @code
 * fdiv(1, 0); // INF
 * 1 / 0; // throws DivisionByZeroError
 * @endcode
 *
 * @pure
 */
#[Mago\AvailableSince(80000)]
function fdiv(float $num1, float $num2): float {}

/**
 * Converts a packed binary IPv4 or IPv6 address, such as one produced by inet_pton or read raw from
 * a socket, into its human-readable string representation, or false if the input is not a valid
 * packed address.
 *
 * @pure
 */
function inet_ntop(string $ip): string|false {}

/**
 * Converts a human-readable IPv4 or IPv6 address into its packed binary in_addr representation.
 * Returns false if the address is not valid.
 *
 * @pure
 */
function inet_pton(string $ip): string|false {}

/**
 * Converts a dotted-quad IPv4 address string into its 32-bit unsigned integer representation.
 * Returns false if ip is not a valid IPv4 address; IPv6 addresses are not supported.
 *
 * @pure
 */
function ip2long(string $ip): int|false {}

/**
 * Converts a 32-bit integer representation of an IPv4 address, such as one produced by ip2long(),
 * into its dotted-quad string notation.
 *
 * @pure
 */
function long2ip(int $ip): string {}

/**
 * Returns the value of the environment variable $name, or every environment variable as an array
 * when $name is omitted.
 *
 * @return ($name is null ? array<string, string> : string|false)
 */
function getenv(?string $name = null, bool $local_only = false): array|string|false {}

/**
 * Sets an environment variable for the current process and any child processes it spawns, from a
 * single NAME=value string. It does not affect the environment of the parent shell that launched
 * PHP. Returns false on failure.
 */
function putenv(string $assignment): bool {}

/**
 * Parses the command-line arguments against $short_options — a string in which an option letter may
 * be followed by ":" to require a value or "::" for an optional one — and $long_options, whose
 * entries follow the same ":"/"::" convention for --style options. An option given more than once
 * on the command line collects its values into a list rather than the last occurrence overwriting
 * the earlier ones. Returns false on a malformed invocation, such as a required value that was not
 * supplied.
 *
 * @code
 * getopt('vo:', ['verbose', 'output::']);
 * @endcode
 *
 * @param string $short_options
 * @param list<string> $long_options
 *
 * @param-out int $rest_index
 *
 * @return array<string, string>|false
 */
function getopt(string $short_options, array $long_options = [], ?int &$rest_index = null): array|false {}

/**
 * Returns the system load averages for the past 1, 5 and 15 minutes, in that order, as a three-
 * element array of floats. Returns false on platforms that do not expose load averages, such as
 * Windows.
 */
function sys_getloadavg(): array|false {}

/**
 * Returns the current Unix time with microsecond precision: as a float when $as_float is true,
 * otherwise as a string with the fractional seconds first followed by the whole seconds.
 *
 * @return ($as_float is true ? float : ($as_float is false ? string : string|float))
 */
function microtime(bool $as_float = false): string|float {}

/**
 * Returns the current time as an array with the seconds and microseconds since the Unix epoch, the
 * number of minutes the local timezone is west of UTC, and whether daylight-saving time is in
 * effect, or, when $as_float is true, as a single float counting seconds since the epoch instead.
 *
 * @return ($as_float is true ? float : (
 *   $ast_float is false ? array{sec: int, usec: int, minuteswest: int, dsttime: int} : array{sec: int, usec: int, minuteswest: int, dsttime: int}|float
 * ))
 */
function gettimeofday(bool $as_float = false): array|float {}

/**
 * Returns resource-usage statistics for the current process as reported by the operating system's
 * getrusage() call, keyed by field names such as ru_utime.tv_sec, ru_utime.tv_usec and ru_maxrss
 * that mirror the native rusage struct; the exact set of keys is platform-dependent. $mode selects
 * whose usage is reported: 0 (the default) for the current process, 1 for its terminated children.
 * Returns false on failure.
 *
 * @return array<string, int>|false
 */
function getrusage(int $mode = 0): array|false {}

/**
 * Returns a unique identifier string based on the current time in microseconds. It is not
 * cryptographically secure and is not guaranteed unique across parallel processes.
 *
 * @return non-falsy-string
 */
function uniqid(string $prefix = '', bool $more_entropy = false): string {}

/**
 * Decodes a string that was encoded with quoted-printable transfer encoding, the reverse of
 * quoted_printable_encode().
 *
 * @pure
 */
function quoted_printable_decode(string $string): string {}

/**
 * Encodes a string using the quoted-printable transfer encoding used in MIME email, replacing non-
 * printable and 8-bit characters, and literal equals signs, with =XX hexadecimal escapes.
 *
 * @pure
 */
function quoted_printable_encode(string $string): string {}

/**
 * Returns the name of the owner of the file containing the currently executing script, which is not
 * necessarily the operating-system user the PHP process itself is running as.
 */
function get_current_user(): string {}

/**
 * Sets the script's maximum execution time to $seconds, restarting the countdown from the moment it
 * is called. Time spent on system calls, sleeping, or waiting on external resources does not count
 * against it.
 */
function set_time_limit(int $seconds): bool {}

/**
 * Returns the value $option was given in the loaded php.ini configuration, or false if the option
 * is not set there. It reflects the configuration file's value even after ini_set has changed the
 * setting at runtime; use ini_get for the setting's current effective value.
 *
 * @pure
 */
function get_cfg_var(string $option): array|string|false {}

/**
 * Always returns false. Magic quotes were removed from the language; the function survives only for
 * code written before that removal.
 *
 * @deprecated
 */
function get_magic_quotes_runtime(): int {}

/**
 * Sends message to a logging destination chosen by $message_type: 0 (the default) sends it to the
 * system logger, 1 emails it to the address in $destination using the extra headers in
 * $additional_headers, 3 appends it to the file named by $destination, and 4 sends it to the SAPI's
 * own logging handler, for example the webserver's error log.
 */
function error_log(
    string $message,
    int $message_type = 0,
    ?string $destination = null,
    ?string $additional_headers = null,
): bool {}

/**
 * Casts value to bool, equivalent to (bool) $value, but usable as a callable — for example as the
 * callback passed to array_filter — where the cast operator cannot be used directly.
 *
 * @pure
 */
function boolval(mixed $value): bool {}

/**
 * Converts a value to an integer. $base only applies when $value is a string, letting it be parsed
 * in a base other than decimal.
 *
 * @pure
 */
function intval(mixed $value, int $base = 10): int {}

/**
 * Converts a value to its float representation, using the same coercion rules as a (float) cast.
 *
 * @pure
 */
function floatval(mixed $value): float {}

/**
 * Returns the float value of a variable, converting it the same way a float cast would. An alias of
 * floatval().
 *
 * @pure
 */
function doubleval(mixed $value): float {}

/**
 * Returns the string representation of value, following PHP's usual scalar conversion rules: true
 * becomes '1', while false and null both become an empty string, and an array always becomes the
 * literal string 'Array' regardless of its contents. An object converts via its __toString()
 * method, or raises an error if it does not define one.
 */
function strval(mixed $value): string {}

/**
 * Returns the name of a value's type as a string. The strings used are PHP's historical type names,
 * such as integer, double and boolean, rather than the spelling used in type declarations;
 * get_debug_type offers the latter.
 *
 * @return 'boolean'|'integer'|'double'|'string'|'array'|'object'|'resource'|'NULL'|'unknown type'|'resource (closed)'
 *
 * @pure
 */
function gettype(mixed $value): string {}

/**
 * Converts $var to $type in place, overwriting its current value and type, and reports whether the
 * conversion succeeded.
 *
 * @param 'bool'|'boolean'|'int'|'integer'|'float'|'double'|'string'|'array'|'object'|'null' $type
 */
function settype(mixed &$var, string $type): bool {}

/**
 * Returns true if $value is null.
 *
 * @assert-if-true null $value
 *
 * @return ($value is null ? true : false)
 *
 * @pure
 */
function is_null(mixed $value): bool {}

/**
 * Returns true if value is a resource that is still open. A resource that has already been closed
 * or freed no longer counts as one, even though the variable may still hold its old value.
 *
 * @assert-if-true open-resource $value
 *
 * @return ($value is open-resource ? true : false)
 *
 * @pure
 */
function is_resource(mixed $value): bool {}

/**
 * Determines whether a value is a boolean.
 *
 * @assert-if-true bool $value
 *
 * @return ($value is bool ? true : false)
 *
 * @pure
 */
function is_bool(mixed $value): bool {}

/**
 * Reports whether $value is of type int; an alias of is_int().
 *
 * @assert-if-true int $value
 *
 * @return ($value is int ? true : false)
 *
 * @pure
 */
function is_long(mixed $value): bool {}

/**
 * Checks whether a value is of type float.
 *
 * @assert-if-true float $value
 *
 * @return ($value is float ? true : false)
 *
 * @pure
 */
function is_float(mixed $value): bool {}

/**
 * Determines whether a value is an integer.
 *
 * @assert-if-true int $value
 *
 * @return ($value is int ? true : false)
 *
 * @pure
 */
function is_int(mixed $value): bool {}

/**
 * Alias of is_int(). Returns true if $value's type is int.
 *
 * @assert-if-true int $value
 *
 * @return ($value is int ? true : false)
 *
 * @pure
 */
function is_integer(mixed $value): bool {}

/**
 * Returns whether $value is of type float; an alias of is_float().
 *
 * @assert-if-true float $value
 *
 * @return ($value is float ? true : false)
 *
 * @pure
 */
function is_double(mixed $value): bool {}

/**
 * Alias of is_float(): checks whether a value is of type float. Deprecated; use is_float() instead.
 *
 * @assert-if-true float $value
 *
 * @return ($value is float ? true : false)
 *
 * @pure
 * @deprecated
 */
function is_real(mixed $var): bool {}

/**
 * Returns true if $value is an int, float, or a numeric string.
 *
 * @assert-if-true numeric $value
 *
 * @return ($value is numeric ? true : false)
 *
 * @pure
 */
function is_numeric(mixed $value): bool {}

/**
 * Returns true if $value is of type string.
 *
 * @assert-if-true string $value
 *
 * @return ($value is string ? true : false)
 *
 * @pure
 */
function is_string(mixed $value): bool {}

/**
 * Determines whether a value is an array.
 *
 * @assert-if-true array<array-key, mixed> $value
 *
 * @return ($value is array ? true : false)
 *
 * @pure
 */
function is_array(mixed $value): bool {}

/**
 * Returns true if the array's keys are the integers 0 through n-1, in that order, with no gaps.
 *
 * @template V
 *
 * @param array<V> $array
 *
 * @assert-if-true list<V> $array
 *
 * @return ($array is list<V> ? true : false)
 *
 * @pure
 */
#[Mago\AvailableSince(80100)]
function array_is_list(array $array): bool {}

/**
 * Returns true if $value is an array or an object implementing Traversable, i.e. anything that can
 * be passed to foreach, and false otherwise.
 *
 * @assert-if-true iterable $value
 *
 * @return ($values is iterable ? true : false)
 *
 * @pure
 */
function is_iterable(mixed $value): bool {}

/**
 * Returns true if $value is an object.
 *
 * @assert-if-true object $value
 *
 * @return ($value is object ? true : false)
 *
 * @pure
 */
function is_object(mixed $value): bool {}

/**
 * Determines whether a value is a scalar: an int, float, string, or bool.
 *
 * @assert-if-true scalar $value
 *
 * @return ($value is scalar ? true : false)
 *
 * @pure
 */
function is_scalar(mixed $value): bool {}

/**
 * Returns true if $value can be called as a function. By default this also checks that the named
 * function or method actually exists, while $syntax_only limits the check to the value's shape
 * alone.
 *
 * @param mixed $value
 * @param bool $syntax_only
 *
 * @param-out string $callable_name
 *
 * @assert-if-true callable $value
 *
 * @pure
 */
function is_callable(mixed $value, bool $syntax_only = false, &$callable_name = null): bool {}

/**
 * Determines whether a value can be passed to count(): an array, or an object implementing
 * Countable.
 *
 * @pure
 */
function is_countable(mixed $value): bool {}

/**
 * Closes a pipe opened with popen, waiting for the underlying process to finish, and returns its
 * exit status, or -1 if $handle is not a valid pipe.
 *
 * @param resource $handle
 *
 * @return int<-1, max>
 */
function pclose($handle): int {}

/**
 * Opens a pipe to the process running $command, returning a stream that can be read from or written
 * to depending on whether $mode is 'r' or 'w' — never both at once. Close it with pclose() rather
 * than fclose(). Returns false if the process cannot be started.
 *
 * @return open-resource|false
 */
function popen(string $command, string $mode) {}

/**
 * Reads the file at filename and writes its contents directly to the output buffer.
 * $use_include_path, when true, searches the include path for the file. Returns the number of bytes
 * read, or false on failure.
 *
 * @param null|resource $context
 */
function readfile(string $filename, bool $use_include_path = false, $context = null): int|false {}

/**
 * Moves a file pointer's position back to the start of the stream. Also clears any end-of-file flag
 * that was set.
 *
 * @param resource $stream
 */
function rewind($stream): bool {}

/**
 * Removes a directory. Fails if the directory is not empty.
 *
 * @param null|resource $context
 */
function rmdir(string $directory, $context = null): bool {}

/**
 * Returns the current umask used to mask file-creation permissions. If $mask is given, sets the
 * umask to it and returns the umask's previous value, rather than the one just set.
 */
function umask(?int $mask = null): int {}

/**
 * Closes an open file pointer.
 *
 * @param resource $stream
 */
function fclose($stream): bool {}

/**
 * Reports whether the file pointer is at end-of-file or has encountered an error, for a stream
 * opened for reading.
 *
 * @param resource $stream
 */
function feof($stream): bool {}

/**
 * Reads and returns a single byte from stream, advancing the file pointer by one. Returns false at
 * end of file or on error.
 *
 * @param resource $stream
 */
function fgetc($stream): string|false {}

/**
 * Reads a line from the file pointer $stream, stopping at a newline, end of file, or after $length
 * minus 1 bytes if $length is given. Returns false when there is nothing left to read.
 *
 * @param resource $stream
 */
function fgets($stream, ?int $length = null): string|false {}

/**
 * Reads up to length bytes from stream and returns them, stopping earlier if end of file is reached
 * or, for a stream such as a network socket that is not a plain local file, whenever no more data
 * happens to be immediately available. Returns false on failure.
 *
 * @param resource $stream
 */
function fread($stream, int $length): string|false {}

/**
 * Opens a file or URL and returns a handle for use with the other filesystem functions. Returns
 * false, rather than throwing, if the file cannot be opened.
 *
 * @param resource|null $context
 *
 * @return open-resource|false
 *
 * @ignore-falsable-return
 */
function fopen(string $filename, string $mode, bool $use_include_path = false, $context = null) {}

/**
 * Reads all remaining data from an open file pointer, from the current position to the end of the
 * file, and writes it directly to the output buffer. Returns the number of bytes passed through.
 *
 * @param resource $stream
 */
function fpassthru($stream): int {}

/**
 * Truncates the file behind the open stream $stream to exactly $size bytes, or extends it and pads
 * the new region with null bytes when $size is larger than the current length. Returns false on
 * failure.
 *
 * @param resource $stream
 */
function ftruncate($stream, int $size): bool {}

/**
 * Returns file status information for an open stream, operating like stat() but on an already-open
 * stream instead of a filename. Returns false if the stream does not support it or the call
 * otherwise fails.
 *
 * @param resource $stream
 *
 * @return false|array{
 *   'dev': int<0, max>,
 *   'ino': int<0, max>,
 *   'mode': int<0, max>,
 *   'nlink': int<0, max>,
 *   'uid': int<0, max>,
 *   'gid': int<0, max>,
 *   'rdev': int<0, max>,
 *   'size': int<0, max>,
 *   'atime': int<1750171087, max>,
 *   'mtime': int<1750171087, max>,
 *   'ctime': int<1750171087, max>,
 *   'blksize': int<0, max>,
 *   'blocks': int<0, max>,
 * }
 */
function fstat($stream): array|false {}

/**
 * Moves the file pointer of $stream by $offset, interpreted relative to the start of the file, the
 * current position, or the end of the file according to $whence (SEEK_SET, SEEK_CUR or SEEK_END).
 * Returns 0 on success and -1 on failure, not a boolean.
 *
 * @code
 * fseek($stream, -1, SEEK_END);
 * @endcode
 *
 * @param resource $stream
 */
function fseek($stream, int $offset, int $whence = SEEK_SET): int {}

/**
 * Returns the current byte offset of the read/write pointer on $stream, or false on error.
 *
 * @param resource $stream
 *
 * @return int<0, max>|false
 */
function ftell($stream): int|false {}

/**
 * Flushes any buffered output for an open file pointer or stream to the underlying storage or OS
 * immediately, rather than waiting for the buffer to fill or the stream to be closed.
 *
 * @param resource $stream
 */
function fflush($stream): bool {}

/**
 * Flushes the operating system's write buffers for the given stream to the underlying storage,
 * ensuring that data already written with fwrite() is durably persisted rather than sitting in the
 * OS cache. Returns false if the underlying platform or stream does not support it.
 *
 * @param resource $stream
 */
#[Mago\AvailableSince(80100)]
function fsync($stream): bool {}

/**
 * Flushes $stream's userspace buffer and forces the operating system to write its data to disk,
 * without necessarily flushing metadata that fsync() would also flush, such as the modification
 * time. Returns false on failure.
 *
 * @param resource $stream
 */
function fdatasync($stream): bool {}

/**
 * Writes $data to the file pointer $stream, stopping after $length bytes if given. It may write
 * fewer bytes than requested, particularly on network streams, so one call is not guaranteed to
 * write everything.
 *
 * @param resource $stream
 *
 * @return int<0, max>|false
 */
function fwrite($stream, string $data, ?int $length = null): int|false {}

/**
 * Alias of fwrite: writes $data to $stream, stopping after $length bytes if given, and returns the
 * number of bytes actually written, or false on failure.
 *
 * @param resource $stream
 *
 * @return int<0, max>|false
 */
function fputs($stream, string $data, ?int $length = null): int|false {}

/**
 * Creates the directory $directory, and, when $recursive is true, any missing parent directories as
 * well. The requested $permissions are modified by the process's umask.
 *
 * @param null|resource $context
 */
function mkdir(string $directory, int $permissions = 0777, bool $recursive = false, $context = null): bool {}

/**
 * Renames or moves $from to $to, overwriting an existing file at $to on most platforms. Moving
 * between two different stream wrappers is generally not supported; both paths should use the same
 * one. Returns false on failure, such as insufficient permissions.
 *
 * @param null|resource $context
 */
function rename(string $from, string $to, $context = null): bool {}

/**
 * Copies a file to a new destination, overwriting the destination if it already exists.
 *
 * @param null|resource $context
 */
function copy(string $from, string $to, $context = null): bool {}

/**
 * Creates a uniquely named, empty temporary file in the given directory and returns its path.
 * Returns false on failure; the caller is responsible for deleting the file afterwards.
 *
 * @return non-empty-string|false
 */
function tempnam(string $directory, string $prefix): string|false {}

/**
 * Creates a temporary file with a unique name, opened for reading and writing, and returns a handle
 * to it. The file is deleted automatically when the handle is closed or the script ends. Returns
 * false on failure.
 *
 * @return resource|false
 */
function tmpfile() {}

/**
 * Reads an entire file into an array, with each element holding one line. Returns false on failure,
 * and each line keeps its trailing newline unless FILE_IGNORE_NEW_LINES is passed.
 *
 * @param null|resource $context
 *
 * @return array<int, string>|false
 */
function file(string $filename, int $flags = 0, $context = null): array|false {}

/**
 * Reads the entire contents of a file or URL into a string. Returns false on failure rather than
 * raising an exception.
 *
 * @param null|resource $context
 */
function file_get_contents(
    string $filename,
    bool $use_include_path = false,
    $context = null,
    int $offset = 0,
    ?int $length = null,
): string|false {}

/**
 * Writes data to a file, creating it if necessary and overwriting its existing contents by default.
 * Returns the number of bytes written, or false on failure; pass FILE_APPEND to append instead of
 * overwrite.
 *
 * @param null|resource $context
 *
 * @return int<0, max>|false
 */
function file_put_contents(string $filename, mixed $data, int $flags = 0, $context = null): int|false {}

/**
 * Returns the most recent error, warning or notice raised during execution, or null if none has
 * occurred yet. The type field holds one of PHP's E_* error-level constants, such as E_WARNING or
 * E_DEPRECATED.
 *
 * @return array{type: int, message: string, file: string, line: int}|null
 *
 * @pure
 */
function error_get_last(): ?array {}

/**
 * Calls the given callback, passing the remaining arguments to it individually, unlike
 * call_user_func_array, which takes them as a single array. Arguments cannot be passed to the
 * callback by reference through this function.
 *
 * @template I
 * @template R
 *
 * @param callable(I...): R|callable-string|string $callback
 * @param I ...$args
 *
 * @return R
 */
function call_user_func(callable|string $callback, mixed ...$args): mixed {}

/**
 * Calls a callback, passing the elements of the given array as its arguments. String keys in the
 * array are passed as named arguments rather than positionally.
 *
 * @template I
 * @template R
 *
 * @param callable(I...): R|callable-string|string $callback
 * @param array<I> $args
 *
 * @return R
 */
function call_user_func_array(callable|string $callback, array $args): mixed {}

/**
 * Calls callback the way call_user_func() would, but when callback names a static method it
 * preserves the calling context's own late static binding (static::) scope instead of resetting it
 * to the class named in callback. Meant to be used inside __callStatic() to relay a call onward
 * without losing track of which subclass is actually running.
 *
 * @template I
 * @template R
 *
 * @param callable(I...): R|callable-string|string $callback
 * @param I ...$args
 *
 * @return R
 */
function forward_static_call(callable|string $callback, mixed ...$args): mixed {}

/**
 * Calls callback with the arguments given in args, the same way as call_user_func_array(), but
 * forwards the calling scope's late static binding, static::, instead of resetting it, which
 * matters when callback resolves a parent::-style call inside a class method.
 *
 * @template I
 * @template R
 *
 * @param callable(I...): R|callable-string|string $callback
 * @param array<I> $args
 *
 * @return R
 */
function forward_static_call_array(callable|string $callback, array $args): mixed {}

/**
 * Returns a string representation of $value that unserialize() can later restore.
 *
 * @return non-falsy-string
 */
function serialize(mixed $value): string {}

/**
 * Reconstructs a PHP value from a string previously produced by serialize(). Returns false on
 * failure, though false can also be a legitimately unserialized value, so the two cases cannot be
 * told apart from the return value alone.
 */
function unserialize(string $data, array $options = []): mixed {}

/**
 * Outputs the type and value of each given variable directly, recursing into arrays and objects and
 * showing property visibility. Unlike print_r, it does not return its output as a string, and it
 * distinguishes values that print_r's output does not, such as the boolean true from the integer 1.
 */
function var_dump(mixed $value, mixed ...$values): void {}

/**
 * Outputs (or, when $return is true, returns instead of printing) a string of PHP code that, when
 * evaluated, reconstructs $value — like var_dump(), but producing valid PHP syntax, which makes it
 * useful for writing generated config or cache files. An exported object round-trips through
 * ClassName::__set_state(array(...)), so the class must define a __set_state() method for that code
 * to actually work when run.
 *
 * @code
 * class Point { public static function __set_state(array $props): self { return new self(); } }
 * @endcode
 *
 * @return ($return is true ? non-empty-string : null)
 */
function var_export(mixed $value, bool $return = false): ?string {}

/**
 * Dumps the type, value and internal reference count of each given value, similar to var_dump() but
 * exposing the reference-count information var_dump() hides.
 */
function debug_zval_dump(mixed $value, mixed ...$values): void {}

/**
 * Prints human-readable information about a variable, showing the structure of arrays and objects.
 * Returns the output as a string instead of printing it when return is true.
 *
 * @return ($return is true ? non-empty-string : bool)
 */
function print_r(mixed $value, bool $return = false): string|bool {}

/**
 * Returns the amount of memory currently allocated to the PHP script, in bytes. When $real_usage is
 * true, it reports the real size of memory allocated from the system, including unused space
 * reserved in allocated blocks; otherwise it reports only the memory actually in use.
 */
function memory_get_usage(bool $real_usage = false): int {}

/**
 * Returns the peak number of bytes ever allocated to the running script. When $real_usage is true
 * this counts the memory blocks the PHP memory manager has actually claimed from the system,
 * including unused space within them; when false it counts only the memory the script's own
 * allocations are using.
 */
function memory_get_peak_usage(bool $real_usage = false): int {}

/**
 * Resets the peak memory usage figure reported by memory_get_peak_usage() back down to the current
 * usage, so later peaks are measured from this point rather than from the start of the script.
 */
#[Mago\AvailableSince(80200)]
function memory_reset_peak_usage(): void {}

/**
 * Registers a callback to run automatically when script execution ends, whether that is normal
 * completion, an explicit exit(), or a fatal error, optionally passing it extra arguments. Multiple
 * registered callbacks run in the order they were registered, which is why it is commonly paired
 * with error_get_last() to catch a fatal error that no exception handler can.
 *
 * @template I
 *
 * @param (callable(I...)) $callback
 * @param I ...$args
 *
 * @return bool|null
 */
function register_shutdown_function(callable $callback, mixed ...$args): void {}

/**
 * Registers $callback, called with $args as its arguments, to run every time PHP executes the
 * number of low-level statements set by declare(ticks=...) inside the block where ticks are active.
 * Multiple registered functions are all called, in the order they were registered.
 *
 * @template I
 *
 * @param (callable(I...)) $callback
 * @param I ...$args
 */
function register_tick_function(callable $callback, mixed ...$args): bool {}

/**
 * Removes $callback from the set of functions previously registered with register_tick_function, so
 * it is no longer called on each tick.
 */
function unregister_tick_function(callable $callback): void {}

/**
 * Outputs the source of $filename with the same syntax colouring highlight_string() applies. When
 * $return is true, the coloured markup is returned as a string instead of being printed. Returns
 * false if $filename cannot be opened.
 *
 * @return ($return is true ? string|false : bool)
 */
function highlight_file(string $filename, bool $return = false): string|bool {}

/**
 * Outputs syntax-highlighted HTML markup of the PHP source in filename, equivalent to
 * highlight_file(). When $return is true, the markup is returned as a string instead of being
 * printed.
 *
 * @return ($return is true ? string|false : bool)
 */
function show_source(string $filename, bool $return = false): string|bool {}

/**
 * Produces a syntax-highlighted HTML rendering of the given PHP source code, using the colours
 * configured by the highlight.* ini directives. By default it echoes the markup and returns true
 * (or false on failure); with $return true it returns the markup as a string instead of echoing it.
 *
 * @return ($return is true ? string|false : bool)
 */
function highlight_string(string $string, bool $return = false): string|bool {}

/**
 * Returns a reading from a monotonic, high-resolution timer that is unaffected by changes to the
 * system clock. By default it returns a two-element array of [seconds, nanoseconds]; passing true
 * returns the same reading as a single nanosecond count instead. Returns false if no such timer is
 * available on the platform.
 *
 * @code
 * [$seconds, $nanoseconds] = hrtime();
 * $nanoseconds = hrtime(true);
 * @endcode
 *
 * @return ($as_number is true ? int|float|false : list{int, int}|false)
 *
 * @mutation-free
 *
 * @psalm-ignore-falsable-return
 */
function hrtime(bool $as_number = false): array|int|float|false {}

/**
 * Returns the source code of filename with PHP comments and non-essential whitespace removed, as
 * php -w once produced. Does not execute the file.
 */
function php_strip_whitespace(string $filename): string {}

/**
 * Returns the current value of option, or false if no such configuration option exists. The value
 * is always returned as a string, even for options that are conceptually boolean or numeric, so a
 * disabled boolean setting typically reads back as an empty string or '0'.
 */
function ini_get(string $option): string|false {}

/**
 * Returns every php.ini configuration option and its value, or only those belonging to the given
 * extension. Restricting to an extension that is not loaded returns false. With details true (the
 * default) each option is reported as its global_value, local_value (the value in the current
 * context) and access (where it may be set); with details false, the result is a flat map of option
 * name to its current value only.
 *
 * @return array{'global_value': string, 'local_value': string, 'access': int}|false
 */
function ini_get_all(?string $extension = null, bool $details = true): array|false {}

/**
 * Sets a configuration option's value for the duration of the current script. Returns the option's
 * previous value, or false if it could not be set.
 */
function ini_set(string $option, string|int|float|bool|null $value): string|false {}

/**
 * Alias of ini_set(): changes the value of configuration option $option for the rest of the current
 * request and returns its previous value, or false if the option does not exist or cannot be
 * changed at runtime.
 */
function ini_alter(string $option, string|int|float|bool|null $value): string|false {}

/**
 * Restores option to the value it had when PHP started, undoing any changes made to it with
 * ini_set() during the current request.
 */
function ini_restore(string $option): void {}

/**
 * Parses an ini-style size shorthand such as "128M" or "512K" into a plain byte count, using the
 * same rules PHP's engine applies to settings like memory_limit.
 */
#[Mago\AvailableSince(80200)]
function ini_parse_quantity(string $shorthand): int {}

/**
 * Returns the current value of the include_path configuration directive, or false on failure.
 */
function get_include_path(): string|false {}

/**
 * Sets the include_path configuration directive for the remainder of the request. Returns the
 * include path that was in effect before the call, or false on failure.
 */
function set_include_path(string $include_path): string|false {}

/**
 * Sends a Set-Cookie header to define a cookie in the client's browser. Like header(), it must be
 * called before any output is sent, or it fails and returns false. $expires_or_options can be given
 * as a Unix timestamp, as in older PHP versions, or as an array of options such as 'expires',
 * 'path', 'domain', 'secure', 'httponly' and 'samesite', which is the only way to set SameSite. To
 * delete a cookie, call setcookie() again with the same name, path and domain and an expiration
 * time in the past.
 *
 * @code
 * setcookie('token', $value, ['expires' => time() + 3600, 'path' => '/', 'samesite' => 'Strict']);
 * @endcode
 */
function setcookie(
    string $name,
    string $value = '',
    array|int $expires_or_options = 0,
    string $path = '',
    string $domain = '',
    bool $secure = false,
    bool $httponly = false,
): bool {}

/**
 * Sends a cookie exactly like setcookie, except $value is sent to the browser as-is rather than
 * being URL-encoded, so the caller is responsible for encoding it if that is needed.
 *
 * @param string $value
 * @param int $expires_or_options
 * @param string $path
 * @param string $domain
 * @param bool $secure
 * @param bool $httponly
 */
function setrawcookie(
    string $name,
    $value = '',
    $expires_or_options = 0,
    $path = '',
    $domain = '',
    $secure = false,
    $httponly = false,
): bool {}

/**
 * Sends a cookie exactly like setcookie, except $value is sent to the browser as-is rather than
 * being URL-encoded, so the caller is responsible for encoding it if that is needed.
 *
 * @param string $value
 */
function setrawcookie(string $name, $value = '', array $options = []): bool {}

/**
 * Sends a raw HTTP header to the client. Must be called before any output is produced, or it has no
 * effect and triggers a warning.
 */
function header(string $header, bool $replace = true, int $response_code = 0): void {}

/**
 * Removes an HTTP response header previously set with header(). Given no $name, it removes every
 * header set so far for the current response.
 */
function header_remove(?string $name = null): void {}

/**
 * Determines whether HTTP headers have already been sent. When they have, $filename and $line are
 * set to where the output started.
 *
 * @param-out string $filename
 * @param-out int $line
 */
function headers_sent(&$filename = null, &$line = null): bool {}

/**
 * Returns the response headers that have been queued or already sent for the current request, each
 * as a full 'Name: value' string.
 *
 * @return list<string>
 */
function headers_list(): array {}

/**
 * Returns all HTTP request headers sent by the client for the current request, keyed by header
 * name, or false if they cannot be retrieved under the running SAPI.
 *
 * @return array<string, string>|false
 */
function apache_request_headers(): false|array {}

/**
 * Returns all HTTP request headers as an associative array, with each header's name kept in the
 * case the client actually sent it rather than normalised. Returns false if the headers could not
 * be retrieved.
 *
 * @return array<string, string>|false
 */
function getallheaders(): false|array {}

/**
 * Reports whether the client has disconnected during the current request: 1 if the connection was
 * aborted, 0 if it is still active. Meant to be polled periodically in a long-running script,
 * typically alongside ignore_user_abort().
 *
 * @return 0|1
 */
function connection_aborted(): int {}

/**
 * Returns the connection status of the current request as a bitmask: 0 means the client is still
 * normally connected, 1 means the connection was aborted by the client, 2 means the script's time
 * limit was reached, and 3 means both happened.
 */
function connection_status(): int {}

/**
 * Sets whether the script should keep running after the client disconnects, returning the previous
 * setting. Passing null leaves the setting unchanged and simply returns its current value.
 */
function ignore_user_abort(?bool $enable = null): int {}

/**
 * Parses an INI file into an array of its settings. With process_sections true, each section header
 * becomes a key holding a nested array of that section's settings; with it false, all settings from
 * every section are merged into a single flat array. scanner_mode controls how values are
 * interpreted: INI_SCANNER_NORMAL applies PHP's normal INI value parsing and substitutes any
 * constants defined with define, INI_SCANNER_RAW leaves every value as its literal string with no
 * parsing or constant substitution, and INI_SCANNER_TYPED converts recognisable values to their
 * proper bool, int, float or null type instead of returning everything as a string. Returns false
 * if the file cannot be read or parsed.
 *
 * @code
 * parse_ini_file($path, true);  // sections become nested arrays
 * parse_ini_file($path, false); // sections are merged into one flat array
 * @endcode
 */
function parse_ini_file(
    string $filename,
    bool $process_sections = false,
    int $scanner_mode = INI_SCANNER_NORMAL,
): array|false {}

/**
 * Parses $ini_string as if it were the contents of a php.ini-style file — the same format
 * parse_ini_file() reads from disk — rather than reading it from a path. With $process_sections
 * true, the result is grouped into a nested array keyed by each [section] header instead of one
 * flat array. $scanner_mode controls how values are interpreted: INI_SCANNER_NORMAL (the default)
 * converts the special words true/false/yes/no/on/off/none/null to their boolean or null
 * equivalents but leaves everything else, including numbers, as a string; INI_SCANNER_RAW disables
 * that conversion and returns every value exactly as written; INI_SCANNER_TYPED additionally
 * converts numeric strings to int or float. Returns false on a parse error.
 */
function parse_ini_string(
    string $ini_string,
    bool $process_sections = false,
    int $scanner_mode = INI_SCANNER_NORMAL,
): array|false {}

/**
 * Returns true if filename names a file that was uploaded via HTTP POST in the current request,
 * guarding against code that trusts a user-supplied path as though it were a genuine upload.
 */
function is_uploaded_file(string $filename): bool {}

/**
 * Moves a file uploaded via HTTP POST from its temporary location $from to $to, first verifying
 * that $from really was produced by a file upload. It refuses to move (and returns false for) a
 * path that was not actually created by an upload, which is what makes it safe to use with user-
 * supplied paths.
 */
function move_uploaded_file(string $from, string $to): bool {}

/**
 * Returns details of the system's network interfaces keyed by interface name, or false if the
 * information could not be retrieved.
 *
 * @return false|array<non-empty-string, array{
 *   'description'?: string,
 *   'mac'?: string,
 *   'mtu'?: int,
 *   'unicast': list<array{
 *     'flags': int,
 *     'family': int,
 *     'address'?: string,
 *     'netmask'?: string,
 *     'broadcast'?: string,
 *   }>,
 *   'up': bool,
 * }>
 */
function net_get_interfaces(): array|false {}

/**
 * Performs a reverse DNS lookup and returns the host name associated with the given IP address, or
 * false if no host name can be resolved.
 */
function gethostbyaddr(string $ip): string|false {}

/**
 * Resolves a hostname to its IPv4 address via DNS. On failure it returns the hostname unchanged,
 * not false, so the return value must be compared against the original argument to detect a lookup
 * failure.
 *
 * @code
 * $ip = gethostbyname('example.invalid');
 * // $ip === 'example.invalid' means the lookup failed
 * @endcode
 */
function gethostbyname(string $hostname): string {}

/**
 * Returns every IPv4 address associated with $hostname via DNS, or false if the lookup fails.
 * Unlike gethostbyname(), which returns only one address, this returns the complete list; it
 * resolves IPv4 addresses only.
 *
 * @return list<non-empty-string>|false
 */
function gethostbynamel(string $hostname): array|false {}

/**
 * Returns the host name of the machine the script is running on, or false if it cannot be
 * determined.
 *
 * @return non-empty-string|false
 */
function gethostname(): string|false {}

/**
 * Checks whether DNS records of type $type exist for $hostname, defaulting to MX records. This is a
 * more clearly named alias of checkdnsrr().
 */
function dns_check_record(string $hostname, string $type = 'MX'): bool {}

/**
 * Checks whether any DNS records of type $type exist for hostname, returning true if at least one
 * is found. $type defaults to 'MX' (mail exchanger records); other common values include 'A',
 * 'AAAA', 'NS' and 'TXT'.
 */
function checkdnsrr(string $hostname, string $type = 'MX'): bool {}

/**
 * Looks up the MX (mail exchanger) records for hostname, filling $hosts with the mail-exchanger
 * hostnames and $weights with their corresponding priority values in matching order, and returns
 * true if any records were found, or false if none exist or the lookup fails.
 *
 * @code
 * dns_get_mx('example.com', $hosts, $weights);
 * // $hosts[0] and $weights[0] describe the same record
 * @endcode
 *
 * @param-out list<non-empty-string> $hosts
 * @param-out array $weights
 */
function dns_get_mx(string $hostname, &$hosts, &$weights = null): bool {}

/**
 * Looks up the DNS MX records for hostname, filling hosts with the mail exchanger hostnames found
 * and weights with their corresponding priority values. Returns false if no MX records exist for
 * the hostname.
 *
 * @code
 * getmxrr('example.com', $hosts, $weights);
 * @endcode
 *
 * @param-out list<non-empty-string> $hosts
 * @param-out array $weights
 */
function getmxrr(string $hostname, &$hosts, &$weights = null): bool {}

/**
 * Queries DNS for records of the given type for hostname (or, by default, whatever DNS_ANY causes
 * the resolver to return), giving back an array of associative records or false on failure.
 * authoritative_name_servers and additional_records, when passed, are filled with the authority and
 * additional sections of the response in the same shape as the main result. Setting raw to true
 * interprets type as a raw DNS type ID rather than a DNS_* constant, and every returned record then
 * carries an unparsed data field instead of the usual type-specific ones.
 *
 * @param-out array $authoritative_name_servers
 * @param-out array $additional_records
 */
function dns_get_record(
    string $hostname,
    int $type = DNS_ANY,
    &$authoritative_name_servers = null,
    &$additional_records = null,
    bool $raw = false,
): array|false {}

/**
 * Waits, for up to seconds and microseconds, or indefinitely when seconds is null, for one of the
 * streams in read, write, or except to become ready for the corresponding operation, then rewrites
 * each of those arrays in place to hold only the streams that are actually ready. Returns the total
 * number of ready streams across all three arrays, 0 on timeout, or false on error.
 *
 * @template R of null|array<array-key, resource>
 * @template W of null|array<array-key, resource>
 * @template E of null|array<array-key, resource>
 *
 * @param R $read
 * @param W $write
 * @param E $except
 *
 * @param-out (R is null ? null : array<array-key, resource>) $read
 * @param-out (W is null ? null : array<array-key, resource>) $write
 * @param-out (E is null ? null : array<array-key, resource>) $except
 *
 * @return false|int<0, max>
 */
function stream_select(
    ?array &$read,
    ?array &$write,
    ?array &$except,
    ?int $seconds,
    ?int $microseconds = null,
): int|false {}

/**
 * Creates a stream context resource that carries wrapper-specific options and notification
 * parameters for use with stream functions such as fopen, file_get_contents or
 * stream_socket_client. options is keyed by wrapper name, each holding that wrapper's own settings;
 * params carries context-wide settings such as a notification callback.
 *
 * @code
 * stream_context_create(['http' => ['method' => 'POST', 'header' => 'Content-Type: application/json']]);
 * @endcode
 *
 * @return resource
 */
function stream_context_create(?array $options = null, ?array $params = null): mixed {}

/**
 * Sets parameters — as distinct from wrapper-specific options, which stream_context_set_option()
 * handles — on a stream context, or on the context of an existing stream, socket or file resource.
 * In practice the only key $params supports is "notification", whose value is a callable invoked as
 * the stream reports progress and status events such as connecting, redirecting or resolving a MIME
 * type.
 *
 * @param resource $context
 */
function stream_context_set_params($context, array $params): bool {}

/**
 * Returns the options and notification setting currently held by a stream context, the same
 * information passed to stream_context_create() and set with stream_context_set_option().
 *
 * @param resource $context
 *
 * @return array{notification: string, options: array}
 */
function stream_context_get_params($context): array {}

/**
 * Sets one or more options on a stream context, or on a stream that already has a context attached.
 * $options is a nested array keyed first by wrapper name and then by option name.
 *
 * @code
 * stream_context_set_option($context, ['http' => ['method' => 'POST']]);
 * stream_context_set_option($context, 'http', 'method', 'POST');
 * @endcode
 *
 * @param resource $context
 */
function stream_context_set_option(
    $context,
    array|string $wrapper_or_options,
    ?string $option_name = null,
    mixed $value = UNKNOWN,
): bool {}

/**
 * Sets multiple stream context options at once from a nested array keyed by wrapper name and then
 * option name, equivalent to calling the single-option setter once per entry.
 *
 * @code
 * stream_context_set_options($context, ['http' => ['method' => 'POST']]);
 * @endcode
 *
 * @param resource $context
 */
#[Mago\AvailableSince(80300)]
function stream_context_set_options($context, array $options): bool {}

/**
 * Returns the options currently set on a stream, wrapper, or context, in the same nested shape
 * accepted by stream_context_create(): an array keyed by wrapper name, each value an associative
 * array of that wrapper's options.
 *
 * @param resource $stream_or_context
 */
function stream_context_get_options($stream_or_context): array {}

/**
 * Returns the default stream context, used by stream functions called without an explicit context
 * argument. When $options is given, those options are applied to the default context before it is
 * returned.
 *
 * @return resource
 */
function stream_context_get_default(?array $options = null) {}

/**
 * Merges $options into the process-wide default stream context, creating it if it does not exist
 * yet, and returns that context, which is what stream functions use implicitly when called without
 * an explicit context argument.
 *
 * @return resource
 */
function stream_context_set_default(array $options) {}

/**
 * Attaches a stream filter at the front of $stream's filter chain, which makes it the first filter
 * applied on read and, perhaps counterintuitively, the last one applied on write. $mode selects
 * whether it runs on the read chain, the write chain, or both, and $params is passed through to the
 * filter, its meaning depending on which filter $filter_name names. Returns false if the filter
 * cannot be attached.
 *
 * @param resource $stream
 *
 * @return resource
 */
function stream_filter_prepend($stream, string $filter_name, int $mode = 0, mixed $params = null) {}

/**
 * Attaches the filter named $filter_name to stream, so it transforms data as it is read from,
 * written to, or both, depending on $mode. Left at its default of 0, the direction is inferred from
 * how the stream was opened; STREAM_FILTER_READ, STREAM_FILTER_WRITE or STREAM_FILTER_ALL override
 * that. Returns a filter resource usable with stream_filter_remove(), or false on failure.
 *
 * @param resource $stream
 *
 * @return resource|false
 */
function stream_filter_append($stream, string $filter_name, int $mode = 0, mixed $params = null) {}

/**
 * Detaches a filter previously attached to a stream by stream_filter_append or
 * stream_filter_prepend, given the resource those functions returned. Returns false if the filter
 * is not currently attached.
 *
 * @param resource $stream_filter
 */
function stream_filter_remove($stream_filter): bool {}

/**
 * Opens a network or Unix-domain socket connection to the given address, such as tcp://host:port or
 * unix:///path/to.sock, and returns the resulting stream, or false on failure. On failure,
 * error_code and error_message are filled by reference with the low-level connection error. timeout
 * limits how long the connection attempt may take, in seconds, defaulting to the
 * default_socket_timeout ini setting. flags controls the connection mode, such as connecting
 * synchronously, asynchronously, or persistently.
 *
 * @code
 * $conn = stream_socket_client('tcp://example.com:80', $errno, $errstr, 30);
 * @endcode
 *
 * @param null|resource $context
 *
 * @param-out null|int $error_code
 * @param-out null|string $error_message
 *
 * @return resource|false
 */
function stream_socket_client(
    string $address,
    &$error_code = null,
    &$error_message = null,
    ?float $timeout = null,
    int $flags = STREAM_CLIENT_CONNECT,
    $context = null,
) {}

/**
 * Creates a stream-based server socket bound to address, such as 'tcp://0.0.0.0:8080' or
 * 'unix:///tmp/my.sock', and by default both binds and listens on it, returning the resulting
 * resource or false on failure. Pass just STREAM_SERVER_BIND in flags for a datagram (UDP) server,
 * which has no listening state. error_code and error_message are filled in with the underlying
 * system error on failure.
 *
 * @param null|resource $context
 *
 * @param-out null|int $error_code
 * @param-out null|string $error_message
 *
 * @return resource|false
 */
function stream_socket_server(
    string $address,
    &$error_code = null,
    &$error_message = null,
    int $flags = STREAM_SERVER_BIND | STREAM_SERVER_LISTEN,
    $context = null,
) {}

/**
 * Accepts an incoming connection on socket, a stream created with stream_socket_server(), waiting
 * up to timeout seconds, or the socket's default timeout when omitted, and writing the remote
 * address into peer_name by reference. Returns a new stream for the accepted connection, or false
 * on failure or timeout.
 *
 * @param resource $socket
 *
 * @param-out string $peer_name
 *
 * @return resource|false
 */
function stream_socket_accept($socket, ?float $timeout = null, &$peer_name = null) {}

/**
 * Returns the address and port of a socket stream's local end, or of the remote peer when remote is
 * true. Returns false on failure.
 *
 * @param resource $socket
 */
function stream_socket_get_name($socket, bool $remote): string|false {}

/**
 * Reads up to $length bytes from $socket without requiring, or disturbing, an established
 * connection, which makes it usable with connectionless protocols such as UDP. $address is set, by
 * reference, to the address the data came from. $flags may include STREAM_OOB, to read out-of-band
 * data, and/or STREAM_PEEK, to read the data without removing it from the socket's receive queue so
 * a later read sees it again. Returns false on failure.
 *
 * @param resource $socket
 *
 * @param-out string $address
 */
function stream_socket_recvfrom($socket, int $length, int $flags = 0, &$address = null): string|false {}

/**
 * Sends data over socket without requiring the socket to be connected first, as used with
 * connectionless protocols such as UDP. address gives the destination and is required for an
 * unconnected socket. Returns the number of bytes sent, or false on failure.
 *
 * @param resource $socket
 */
function stream_socket_sendto($socket, string $data, int $flags = 0, string $address = ''): int|false {}

/**
 * Enables or disables encryption on an already-connected stream, such as upgrading a plain socket
 * to TLS. On a non-blocking stream, if there is not yet enough data to complete the negotiation, it
 * returns 0 rather than true or false, and should be called again once more data is available.
 *
 * @param resource $stream
 * @param null|resource $session_stream
 */
function stream_socket_enable_crypto(
    $stream,
    bool $enable,
    ?int $crypto_method = null,
    $session_stream = null,
): int|bool {}

/**
 * Shuts down a full-duplex socket stream in one or both directions without closing the underlying
 * stream resource, the way fclose() would. $mode is one of STREAM_SHUT_RD to stop receiving,
 * STREAM_SHUT_WR to stop sending, or STREAM_SHUT_RDWR for both.
 *
 * @param resource $stream
 */
function stream_socket_shutdown($stream, int $mode): bool {}

/**
 * Creates a pair of connected, indistinguishable stream sockets for local inter-process
 * communication, the userland equivalent of the Unix socketpair() system call -- typically used to
 * hand one end to a child process while the parent keeps the other. Returns false on failure.
 *
 * @return list{resource, resource}|false
 */
function stream_socket_pair(int $domain, int $type, int $protocol): array|false {}

/**
 * Copies data from one open stream to another, starting at the given byte offset in the source and
 * continuing for $length bytes, or until end of file if $length is omitted. Returns the number of
 * bytes copied, or false on failure.
 *
 * @param resource $from
 * @param resource $to
 */
function stream_copy_to_stream($from, $to, ?int $length = null, int $offset = 0): int|false {}

/**
 * Reads the remaining contents of $stream (or up to $length bytes) into a string, optionally
 * seeking to $offset first. Returns false on failure.
 *
 * @param resource $stream
 */
function stream_get_contents($stream, ?int $length = null, int $offset = -1): string|false {}

/**
 * Returns whether $stream supports locking through flock().
 *
 * @param resource $stream
 */
function stream_supports_lock($stream): bool {}

/**
 * Reads a line from a file pointer and parses it as CSV fields, returning them as an array. Returns
 * false at end of file or on error.
 *
 * @param resource $stream
 */
function fgetcsv(
    $stream,
    ?int $length = null,
    string $separator = ',',
    string $enclosure = '"',
    string $escape = '\\',
): array|false {}

/**
 * Formats $fields as one line of CSV and writes it to the open file pointer $stream. Returns the
 * number of bytes written, or false on failure.
 *
 * @param resource $stream
 */
function fputcsv(
    $stream,
    array $fields,
    string $separator = ',',
    string $enclosure = '"',
    string $escape = "\\",
    string $eol = PHP_EOL,
): int|false {}

/**
 * Applies or releases an advisory lock on the open stream $stream. $operation is LOCK_SH for a
 * shared, read, lock, LOCK_EX for an exclusive, write, lock, or LOCK_UN to release the current
 * lock, optionally combined with LOCK_NB to fail immediately instead of blocking. When LOCK_NB is
 * used and the lock could not be acquired, $would_block is set to 1.
 *
 * @code
 * flock($stream, LOCK_EX | LOCK_NB, $wouldBlock)
 * @endcode
 *
 * @param resource $stream
 * @param-out int $would_block
 *
 * @return bool
 */
function flock($stream, int $operation, &$would_block = null): bool {}

/**
 * Reads $filename far enough to collect its <meta name=... content=...> tags, stopping once it
 * reaches </head>; tags placed after the head are not seen. The result maps each name — lower-
 * cased, with special characters replaced by underscores — to its content. Returns false if the
 * file cannot be opened.
 *
 * @return array<string, string>|false
 */
function get_meta_tags(string $filename, bool $use_include_path = false): array|false {}

/**
 * Sets the write buffer size, in bytes, used for stream; passing 0 disables buffering so writes are
 * flushed immediately. Returns 0 on success.
 *
 * @param resource $stream
 */
function stream_set_write_buffer($stream, int $size): int {}

/**
 * Sets the size in bytes of the buffer used for read operations on stream; passing 0 requests
 * unbuffered reads. Returns zero on success, or a non-zero value if the stream does not support
 * changing its buffer size.
 *
 * @param resource $stream
 */
function stream_set_read_buffer($stream, int $size): int {}

/**
 * Sets the write buffer size, in bytes, for the given stream; an alias of stream_set_write_buffer.
 * Passing 0 makes writes to the stream unbuffered. Returns 0 on success.
 *
 * @param resource $stream
 */
function set_file_buffer($stream, int $size): int {}

/**
 * Switches stream between blocking and non-blocking mode for reads and writes. Returns true on
 * success.
 *
 * @param resource $stream
 */
function stream_set_blocking($stream, bool $enable): bool {}

/**
 * A legacy alias for stream_set_blocking(): puts stream into blocking mode when enable is true, and
 * non-blocking mode when it is false.
 *
 * @param resource $stream
 */
function socket_set_blocking($stream, bool $enable): bool {}

/**
 * Returns metadata about an open stream, such as its wrapper and read mode, whether it is seekable,
 * and whether it is currently blocking or at end-of-file.
 *
 * @param resource $stream
 *
 * @return array{
 *   timed_out: bool,
 *   blocked: bool,
 *   eof: bool,
 *   unread_bytes: int,
 *   stream_type: string,
 *   wrapper_type?: string,
 *   wrapper_data: mixed,
 *   mode: string,
 *   seekable: bool,
 *   uri: string,
 *   crypto: array,
 *   mediatype: string,
 * }
 */
function stream_get_meta_data($stream): array {}

/**
 * Reads from $stream until $ending is found, $length bytes have been read, or EOF is reached,
 * whichever happens first — like fgets(), but with an arbitrary, possibly multi-character line
 * ending instead of being limited to a newline. Unlike fgets(), $ending itself is not included in
 * the returned string. Returns false on failure.
 *
 * @code
 * stream_get_line($socket, 8192, "\r\n\r\n");
 * @endcode
 *
 * @param resource $stream
 */
function stream_get_line($stream, int $length, string $ending = ''): string|false {}

/**
 * Registers protocol as a custom stream wrapper implemented by class, so that fopen() and related
 * functions recognise URLs of the form protocol://.... Passing STREAM_IS_URL in flags marks the
 * protocol as URL-style, with a host/authority part, rather than a local-file-style path.
 */
function stream_wrapper_register(string $protocol, string $class, int $flags = 0): bool {}

/**
 * Registers a class implementing a stream wrapper for the given URI protocol, so that streams
 * opened with that protocol are handled by the class instead of PHP's built-in filesystem
 * functions. It is an alias of stream_wrapper_register().
 */
function stream_register_wrapper(string $protocol, string $class, int $flags = 0): bool {}

/**
 * Resolves $filename against include_path the same way include or require would, and returns the
 * resulting path, or false if the file cannot be found.
 */
function stream_resolve_include_path(string $filename): string|false {}

/**
 * Unregisters the stream wrapper for the given protocol so it is no longer available. To bring back
 * a built-in wrapper that this replaced, use stream_wrapper_restore() rather than registering it
 * again.
 */
function stream_wrapper_unregister(string $protocol): bool {}

/**
 * Restores a built-in stream wrapper, such as http or file, that was previously removed with
 * stream_wrapper_unregister().
 */
function stream_wrapper_restore(string $protocol): bool {}

/**
 * Returns the names of all stream wrappers currently registered, such as those used for file, http
 * and other URL-style resource access.
 *
 * @return list<string>
 */
function stream_get_wrappers(): array {}

/**
 * Returns the names of the socket transports currently registered, such as tcp, udp or ssl, which
 * is the set of schemes usable with stream_socket_* functions.
 *
 * @return list<string>
 */
function stream_get_transports(): array {}

/**
 * Determines whether $stream refers to a local resource such as a plain file, as opposed to a
 * network stream such as an FTP or HTTP URL.
 *
 * @param string|resource $stream
 */
function stream_is_local($stream): bool {}

/**
 * Sends a request for url and returns the response headers, discarding the body. Returns them as a
 * plain indexed array of header lines by default, or as an associative array keyed by header name
 * when $associative is true; a header repeated in the response becomes a list of values under that
 * key. Returns false on failure.
 *
 * @param null|resource $context
 */
function get_headers(string $url, bool $associative = false, $context = null): array|false {}

/**
 * Sets the timeout, in seconds plus microseconds, after which a blocking read or write operation on
 * stream gives up and returns rather than waiting indefinitely for data. After a timeout,
 * stream_get_meta_data($stream)['timed_out'] is true.
 *
 * @param resource $stream
 */
function stream_set_timeout($stream, int $seconds, int $microseconds = 0): bool {}

/**
 * Sets the timeout for read and write operations on the given stream; an alias of
 * stream_set_timeout. After it elapses, an operation that would otherwise block instead returns
 * having timed out, which stream_get_meta_data reports.
 *
 * @param resource $stream
 */
#[Deprecated(since: '8.5', message: 'use stream_set_timeout() instead')]
function socket_set_timeout($stream, int $seconds, int $microseconds = 0): bool {}

/**
 * Returns metadata about stream -- the same information stream_get_meta_data() returns, such as
 * whether the last read timed out or is blocked and how many bytes remain unread -- kept mainly for
 * backward compatibility.
 *
 * @param resource $stream
 */
function socket_get_status($stream): array {}

/**
 * Resolves a relative or symbolic path to its canonicalised absolute form. Returns false if the
 * target does not exist or cannot be accessed.
 *
 * @return non-empty-string|false
 */
function realpath(string $path): string|false {}

/**
 * Matches filename against pattern, written as a shell wildcard glob pattern rather than a regular
 * expression. flags is a bitmask: FNM_NOESCAPE stops backslash from acting as an escape character,
 * FNM_PATHNAME requires a literal slash in pattern to match a slash in filename, FNM_PERIOD
 * requires a leading period in filename to be matched explicitly rather than by a wildcard, and
 * FNM_CASEFOLD makes the match case-insensitive.
 */
function fnmatch(string $pattern, string $filename, int $flags = 0): bool {}

/**
 * Opens a network or Unix domain socket connection and returns it as a stream. hostname may carry a
 * transport scheme such as udp://, unix:// or ssl:// to select something other than a plain TCP
 * connection; for a unix:// socket the port is ignored and should be left at its default of -1. If
 * timeout is not given, the default_socket_timeout ini setting is used. Returns false on failure,
 * with the by-reference error_code and error_message describing why.
 *
 * @code
 * fsockopen('unix:///var/run/php-fpm.sock', -1, $errno, $errstr);
 * @endcode
 *
 * @param string $hostname <p>
 * @param int $port
 *
 * @param float|null $timeout
 *
 * @param-out int $error_code
 * @param-out string $error_message
 *
 * @return resource|false
 */
function fsockopen(
    string $hostname,
    int $port = -1,
    &$error_code = null,
    &$error_message = null,
    ?float $timeout = null,
) {}

/**
 * Opens a persistent Internet or Unix domain socket connection — the persistent counterpart of
 * fsockopen() — which PHP keeps open and reuses across requests within the same server process
 * instead of closing it when the script ends, avoiding the cost of reconnecting every time.
 * $error_code and $error_message are set, by reference, when the connection fails. Returns a stream
 * resource on success, or false on failure.
 *
 * @param string $hostname
 * @param int $port
 * @param float|null $timeout
 *
 * @param-out int $error_code
 * @param-out string $error_message
 *
 * @return resource|false
 */
function pfsockopen(
    string $hostname,
    int $port = -1,
    &$error_code = null,
    &$error_message = null,
    ?float $timeout = null,
) {}

/**
 * Packs the given values into a binary string according to format, a sequence of type codes, such
 * as N for a big-endian unsigned 32-bit integer or a for a null-padded string, each optionally
 * followed by a repeat count. Its inverse is unpack().
 *
 * @pure
 */
function pack(string $format, mixed ...$values): string {}

/**
 * Unpacks binary data from $string starting at byte $offset according to the pack()-style codes in
 * $format, returning the extracted values indexed from 1 rather than 0. Giving a code a name in
 * $format, such as "Nname", uses that name as the array key instead of a position.
 *
 * @code
 * unpack('Nsize/Nchecksum', $header);
 * @endcode
 *
 * @return ($format is 'a'|'A'|'h'|'H' ? array{1: string}|false : (
 *   $format is 'c' ? array{1: int<-128, 127>}|false : (
 *     $format is 'C' ? array{1: int<0, 255>}|false : (
 *       $format is 's' ? array{1: int<-32768, 32767>}|false : (
 *         $format is 'S'|'n'|'v' ? array{1: int<0, 65535>}|false : (
 *           $format is 'l' ? array{1: int<-2147483648, 2147483647>}|false : (
 *             $format is 'L'|'N'|'V' ? array{1: int<0, 4294967295>}|false : (
 *               $format is 'q'|'Q'|'J'|'P' ? array{1: int}|false : (
 *                 $format is 'f'|'g'|'G'|'d'|'e'|'E' ? array{1: float}|false : (
 *                   array<int>|false
 *                 )
 *               )
 *             )
 *           )
 *         )
 *       )
 *     )
 *   )
 * ))
 *
 * @pure
 *
 * @psalm-ignore-falsable-return
 */
function unpack(string $format, string $string, int $offset = 0): array|false {}

/**
 * Looks up browser capability information for a user agent string in the browscap database, using
 * the current request's user agent when $user_agent is null. Returns an stdClass object by default,
 * or an associative array when $return_array is true, or false if the browscap ini directive is not
 * configured or the lookup fails.
 */
function get_browser(?string $user_agent = null, bool $return_array = false): object|array|false {}

/**
 * Hashes $string using the DES, MD5, SHA-256, SHA-512 or bcrypt algorithm selected by the format of
 * $salt itself -- a salt beginning with $2y$, for instance, selects bcrypt. For passwords, prefer
 * password_hash(), which chooses and manages the salt automatically.
 *
 * @pure
 */
function crypt(string $string, string $salt): string {}

/**
 * Opens a handle on a directory for use with readdir(), rewinddir() and closedir(). Returns false
 * if the path cannot be opened, for example because it does not exist or is not a directory.
 *
 * @param null|resource $context
 *
 * @return resource|false
 */
function opendir(string $directory, $context = null) {}

/**
 * Closes the directory handle $dir_handle, previously opened with opendir(). If omitted, closes the
 * last directory handle that opendir() opened.
 *
 * @param null|resource $dir_handle
 */
function closedir($dir_handle = null): void {}

/**
 * Changes PHP's current working directory to $directory for the rest of the process, returning
 * false if the directory does not exist or is not accessible.
 */
function chdir(string $directory): bool {}

/**
 * Changes the root directory of the current process to $directory, confining all further filesystem
 * access to that subtree. Available only under the CLI SAPI on Unix-like systems, and typically
 * requires root privileges; the change cannot be undone within the process. Returns false on
 * failure.
 */
function chroot(string $directory): bool {}

/**
 * Returns the current working directory. Returns false if it cannot be determined, for example
 * because a parent directory is not readable.
 *
 * @return non-empty-string|false
 */
function getcwd(): string|false {}

/**
 * Resets the directory pointer identified by dir_handle back to the first entry, so a following
 * readdir call starts over from the beginning.
 *
 * @param resource|null $dir_handle
 */
function rewinddir($dir_handle = null): void {}

/**
 * Returns the name of the next entry in the directory handle opened by opendir, or false once every
 * entry has been returned. The special entries . and .. are included, and entries come back in
 * whatever order the filesystem provides them, not sorted alphabetically. If dir_handle is omitted,
 * the handle from the most recent opendir call is used.
 *
 * @param resource|null $dir_handle
 *
 * @return non-empty-string|false
 */
function readdir($dir_handle = null): string|false {}

/**
 * Opens directory for reading and returns it as a Directory object, offering ->read(), ->rewind()
 * and ->close() methods as an object-oriented alternative to opendir(). Returns false on failure.
 *
 * @param null|resource $context
 */
function dir(string $directory, $context = null): Directory|false {}

/**
 * @param null|resource $context
 */
function getdir(string $directory, $context = null): Directory|false {}

/**
 * Returns a list of the files and directories inside the given directory. The list includes the
 * special entries . and .., and returns false on failure.
 *
 * @param resource|null $context
 *
 * @return list<non-empty-string>|false
 */
function scandir(string $directory, int $sorting_order = 0, $context = null): array|false {}

/**
 * Returns an array of file paths matching the shell-style wildcard $pattern. Returns an empty array
 * if nothing matches, and false only if the glob operation itself fails.
 *
 * @return list<non-empty-string>|false
 */
function glob(string $pattern, int $flags = 0): array|false {}

/**
 * Returns the Unix timestamp of the last time the file's contents were accessed, or false on
 * failure.
 *
 * @return int<1750595956, max>|false
 */
function fileatime(string $filename): int|false {}

/**
 * Returns the time a file's inode metadata — permissions, ownership, link count, not its content —
 * was last changed; this is commonly, and incorrectly, assumed to be the file's creation time. Use
 * filemtime() for the content's last-modified time and fileatime() for the last-access time.
 * Returns false on failure, such as the file not existing.
 *
 * @return int<1750595956, max>|false
 */
function filectime(string $filename): int|false {}

/**
 * Returns the numeric group ID that owns filename; use posix_getgrgid() to resolve it to a group
 * name. Returns false on error, and the result is cached the same way stat() results are, so a
 * change made after the first call needs clearstatcache() to be picked up.
 *
 * @return int<0, max>|false
 */
function filegroup(string $filename): int|false {}

/**
 * Returns the inode number of $filename, or false on failure. Like other filesystem functions, the
 * result is cached per path until cleared with clearstatcache(), so a later change to the same file
 * may not be reflected without clearing the cache first.
 *
 * @return int<0, max>|false
 */
function fileinode(string $filename): int|false {}

/**
 * Returns the Unix timestamp of the last time $filename's contents were modified, or false on
 * failure. The result is cached by PHP's stat cache; call clearstatcache() to see a change made
 * since the cache was last populated.
 *
 * @return int<1750595956, max>|false
 */
function filemtime(string $filename): int|false {}

/**
 * Returns the numeric user ID that owns the given file, or false if the file does not exist or its
 * owner cannot be determined. Resolve the ID into a username with posix_getpwuid().
 *
 * @return int<0, max>|false
 */
function fileowner(string $filename): int|false {}

/**
 * Returns the raw file mode of the given path, or false on failure. The value includes the file-
 * type bits alongside the permission bits, so it is not the same as the octal permissions shown by
 * tools like chmod; mask it with 0777 to get just the permission bits. Like other stat-family
 * functions, the result is cached per file for the rest of the request until clearstatcache() is
 * called.
 *
 * @code
 * decoct(fileperms($path) & 0777); // e.g. '644'
 * @endcode
 *
 * @return int<0, max>|false
 */
function fileperms(string $filename): int|false {}

/**
 * Returns the size of the file in bytes, or false if the file does not exist or cannot be stat'd.
 * Results are cached the same way as stat(), so a file that changes size during the same request
 * may need clearstatcache() before this is called again. On builds where integers are 32-bit, sizes
 * above 2 GiB overflow and are reported incorrectly.
 *
 * @return int<0, max>|false
 */
function filesize(string $filename): int|false {}

/**
 * Returns the type of the filesystem entry at $filename.
 *
 * @return 'fifo'|'char'|'dir'|'block'|'link'|'file'|'socket'|'unknown'|false
 */
function filetype(string $filename): string|false {}

/**
 * Checks whether a file or directory exists at the given path. Results are cached by PHP's stat
 * cache, so a change made by another process may not be seen until clearstatcache() is called.
 */
function file_exists(string $filename): bool {}

/**
 * Reports whether the file or directory at $filename is writable by the current process, based on
 * filesystem permissions and PHP's stat cache — call clearstatcache() first if permissions changed
 * since the last check on that path. On Windows this only reflects the file's read-only attribute,
 * not its access control list.
 */
function is_writable(string $filename): bool {}

/**
 * Alias of is_writable(): checks whether filename exists and is writable. On Windows this only
 * reflects the file's read-only attribute, not full ACL permissions.
 */
function is_writeable(string $filename): bool {}

/**
 * Checks whether a file or directory exists and is readable by PHP.
 */
function is_readable(string $filename): bool {}

/**
 * Checks whether filename exists and is executable by the current user.
 */
function is_executable(string $filename): bool {}

/**
 * Returns true if the given path exists and is a regular file. Returns false for directories and
 * for paths that do not exist, including broken symlinks.
 */
function is_file(string $filename): bool {}

/**
 * Checks whether a given path refers to a directory.
 */
function is_dir(string $filename): bool {}

/**
 * Reports whether the given path is a symbolic link.
 */
function is_link(string $filename): bool {}

/**
 * Returns filesystem status information for filename, or false if it cannot be inspected. dev and
 * ino identify the device and inode, mode is the permission and file-type bitmask, nlink is the
 * hard-link count, uid and gid are the owner and group, rdev is the device type for a special file,
 * size is the size in bytes, atime, mtime and ctime are the access, modification and inode-change
 * timestamps, and blksize and blocks describe the preferred I/O block size and block count. Results
 * for a given path are cached for the rest of the request; call clearstatcache() if the file may
 * have changed since it was last stat'd.
 *
 * @return array{
 *   'dev': int,
 *   'ino': int,
 *   'mode': int,
 *   'nlink': int,
 *   'uid': int,
 *   'gid': int,
 *   'rdev': int,
 *   'size': int,
 *   'atime': int,
 *   'mtime': int,
 *   'ctime': int,
 *   'blksize': int,
 *   'blocks': int,
 * }|false
 */
function stat(string $filename): array|false {}

/**
 * Returns filesystem status information for filename without following it when it is a symbolic
 * link, unlike stat(), which follows the link through to the file it points at. For the symlink
 * itself, the size field is the length of the path text stored in the link, not the size of
 * whatever it points to.
 *
 * @return array{
 *   'dev': int,
 *   'ino': int,
 *   'mode': int,
 *   'nlink': int,
 *   'uid': int,
 *   'gid': int,
 *   'rdev': int,
 *   'size': int,
 *   'atime': int,
 *   'mtime': int,
 *   'ctime': int,
 *   'blksize': int,
 *   'blocks': int,
 * }|false
 */
function lstat(string $filename): array|false {}

/**
 * Changes the owner of a file to the given username or numeric user id. Returns false on failure;
 * on most systems this only succeeds when the script is running with superuser privileges.
 */
function chown(string $filename, string|int $user): bool {}

/**
 * Changes the group ownership of $filename to $group, given as either a group name or a numeric
 * group ID. On most systems, only the superuser, or a user who already belongs to the target group,
 * can do this. Returns false on failure.
 */
function chgrp(string $filename, string|int $group): bool {}

/**
 * Changes the owner of filename to user, given as a username or a numeric user ID. Unlike chown(),
 * if filename is a symbolic link the link itself is changed rather than the file it points to.
 */
function lchown(string $filename, string|int $user): bool {}

/**
 * Changes the group ownership of $filename itself when it is a symbolic link, rather than the file
 * or directory it points to, unlike chgrp(). It is not available on Windows.
 */
function lchgrp(string $filename, string|int $group): bool {}

/**
 * Changes the permissions of $filename. $permissions is interpreted as an octal number, so it must
 * be written with a leading zero (0755, not 755) or a plain decimal literal will set the wrong
 * mode.
 *
 * @code
 * chmod('/path/to/script.sh', 0755);
 * @endcode
 */
function chmod(string $filename, int $permissions): bool {}

/**
 * Sets a file's last-modified and last-accessed times, creating an empty file first if it does not
 * already exist. When $atime is omitted it takes the value given for $mtime, and when both are
 * omitted the current time is used for each.
 *
 * @code
 * touch($file, $mtime); // atime becomes $mtime too when omitted
 * @endcode
 */
function touch(string $filename, ?int $mtime = null, ?int $atime = null): bool {}

/**
 * Clears PHP's internal cache of file metadata, such as size, modification time and permissions,
 * that stat() and related functions would otherwise keep returning stale for the rest of the
 * request even after the underlying file changes. Setting $clear_realpath_cache also clears the
 * separate cache of resolved absolute paths, either entirely or, if $filename is given, for that
 * path alone.
 */
function clearstatcache(bool $clear_realpath_cache = false, string $filename = ''): void {}

/**
 * Returns the total size, in bytes, of the filesystem or disk partition containing $directory, or
 * false if it cannot be determined.
 */
function disk_total_space(string $directory): float|false {}

/**
 * Returns the number of bytes of free space on the filesystem or disk partition containing
 * $directory, or false on failure. The result is a float rather than an int because it can exceed
 * what a 32-bit integer can represent.
 */
function disk_free_space(string $directory): float|false {}

/**
 * Returns the number of bytes of free space on the filesystem or disk partition containing
 * $directory. This is an alias of disk_free_space(). Returns false on failure.
 */
function diskfreespace(string $directory): float|false {}

/**
 * Sends an email using the transport configured in php.ini, typically a local sendmail binary or an
 * SMTP relay. Returns true only if the message was successfully handed off for delivery, not that
 * it was actually delivered.
 *
 * @param string|array<string, string> $additional_headers
 */
function mail(
    string $to,
    string $subject,
    string $message,
    array|string $additional_headers = [],
    string $additional_params = '',
): bool {}

/**
 * Opens a connection to the system logger, for use by subsequent syslog() calls.
 */
function openlog(string $prefix, int $flags, int $facility): bool {}

const ARRAY_FILTER_USE_BOTH = 1;

const ARRAY_FILTER_USE_KEY = 2;

/**
 * Merges one or more arrays like array_merge, except that when two arrays share the same string
 * key, their values are merged into each other recursively instead of the later array overwriting
 * the earlier one; if both values at that key are not themselves arrays, they are combined into an
 * array holding both rather than one replacing the other. Integer keys are never treated this way —
 * they are always renumbered and appended, exactly as array_merge does.
 *
 * @code
 * array_merge_recursive(['colour' => 'red'], ['colour' => 'blue']);
 * // ['colour' => ['red', 'blue']]
 * @endcode
 *
 * @template K as array-key
 * @template V
 *
 * @param array<K, V> ...$arrays
 *
 * @return array<K, V>
 *
 * @no-named-arguments
 * @pure
 */
function array_merge_recursive(array ...$arrays) {}

/**
 * Replaces values in the first array with values from later arrays that share the same key,
 * appending any new keys. Unlike array_merge, keys - including integer ones - are never renumbered.
 *
 * @template K of array-key
 * @template V
 *
 * @param array<K, V> $array
 * @param array<K, V> ...$replacements
 *
 * @return array<K, V>
 *
 * @no-named-arguments
 * @pure
 */
function array_replace(array $array, array ...$replacements): array {}

/**
 * Like array_replace, but merges nested arrays recursively instead of overwriting them wholesale.
 *
 * @template K of array-key
 * @template V
 *
 * @param array<K, V> $array
 * @param array<K, V> ...$replacements
 *
 * @return array<K, V>
 *
 * @no-named-arguments
 * @pure
 */
function array_replace_recursive(array $array, array ...$replacements): array {}

/**
 * Returns the keys of an array. When $filter_value is given, only the keys whose values match it
 * are returned, compared loosely unless $strict is true.
 *
 * @template K as array-key
 * @template V
 *
 * @param array<K, V> $array
 * @param V $filter_value
 * @param bool $strict
 *
 * @return ($array is non-empty-array|non-empty-list ? non-empty-list<K> : list<K>)
 *
 * @pure
 */
function array_keys(array $array, mixed $filter_value = null, bool $strict = false): array {}

/**
 * Returns all the values of an array, re-indexed with sequential integer keys starting at 0.
 *
 * @template K as array-key
 * @template V
 *
 * @param array<K, V> $array
 *
 * @return ($array is non-empty-array|non-empty-list ? non-empty-list<V> : list<V>)
 *
 * @pure
 */
function array_values(array $array): array {}

/**
 * Counts how many times each distinct value occurs in array, returning an array keyed by those
 * values with the corresponding counts.
 *
 * @template K as array-key
 * @template V as array-key
 *
 * @param array<K, V> $array
 *
 * @return array<V, int>
 *
 * @pure
 */
function array_count_values(array $array): array {}

/**
 * Returns the values of a single column from an array of arrays or objects, optionally indexed by
 * another column. Rows that do not contain $column_key are silently omitted.
 *
 * @template K as array-key
 * @template V
 *
 * @param array<array-key, array<K, V>|object>|list<array<K,V>|object> $array
 * @param K|null $column_key
 * @param K|null $index_key
 *
 * @return array<array-key, V>
 *
 * @pure
 */
function array_column(array $array, string|int|null $column_key, string|int|null $index_key = null): array {}

/**
 * Reverses the order of an array's elements. String keys are always kept, but numeric keys are only
 * kept when $preserve_keys is true.
 *
 * @template K as array-key
 * @template V
 *
 * @param array<K, V> $array
 * @param bool $preserve_keys
 *
 * @return ($array is list<V> ? ($preserve_keys is true ? array<K, V> : list<V>) : array<K, V>)
 *
 * @pure
 */
function array_reverse(array $array, bool $preserve_keys = false): array {}

/**
 * Reduces $array to a single value by applying $callback to each element together with the value
 * carried from the previous call, starting from $initial.
 *
 * @template K as array-key
 * @template V
 * @template I
 *
 * @param array<K, V> $array
 * @param (callable(I, V): I) $callback
 * @param null|I $initial
 *
 * @return I
 */
function array_reduce(array $array, callable $callback, mixed $initial = null): mixed {}

/**
 * Copies array and pads it with value until it has abs(length) elements, adding to the end for a
 * positive length or the beginning for a negative one. If array already has at least abs(length)
 * elements, an unchanged copy is returned.
 *
 * @code
 * array_pad([1, 2], -4, 0); // [0, 0, 1, 2]
 * array_pad([1, 2], 4, 0);  // [1, 2, 0, 0]
 * @endcode
 *
 * @template K as array-key
 * @template V
 * @template T
 *
 * @param array<K, V> $array
 * @param T $value
 *
 * @return ($array is list<V>
 *     ? ($length is int<1, max> ? non-empty-list<V|T> : list<V|T>)
 *     : ($length is int<1, max> ? non-empty-array<K, V|T> : array<K, V|T>))
 *
 * @pure
 */
function array_pad(array $array, int $length, mixed $value): array {}

/**
 * Exchanges the keys and values of an array. If a value occurs more than once, only the last key
 * using that value is kept.
 *
 * @template K as array-key
 * @template V as array-key
 *
 * @param array<K, V> $array
 *
 * @return ($array is non-empty-array|non-empty-list ? non-empty-array<V, K> : array<V, K>)
 *
 * @pure
 */
function array_flip(array $array): array {}

/**
 * Returns a copy of array with every string key converted to lower case, CASE_LOWER being the
 * default, or upper case, CASE_UPPER; integer keys are left unchanged, and the conversion is ASCII-
 * only, so it does not affect accented or multi-byte characters.
 *
 *
 * @template V
 *
 * @param array<string, V> $array
 * @param int $case
 *
 * @return ($array is non-empty-array|non-empty-list ? non-empty-array<string, V> : array<string, V>)
 *
 * @pure
 */
function array_change_key_case(array $array, int $case = CASE_LOWER): array {}

/**
 * Picks one or more random keys from the array without replacement, using num to choose how many.
 * With num equal to 1 (the default) a single key is returned; otherwise an array of num distinct
 * keys is returned. Like PHP's other non-cryptographic random functions, it must not be used where
 * the choice needs to be unguessable.
 *
 * @template K as array-key
 *
 * @param array<K, mixed> $array
 * @param int<0, max> $num
 *
 * @return ($num is 1 ? K : array<K>)
 *
 * @pure
 */
function array_rand(array $array, int $num = 1): array|string|int {}

/**
 * Removes duplicate values from an array, keeping the first occurrence of each and the original
 * keys. Values are compared as strings by default, not strictly.
 *
 * @template K as array-key
 * @template V
 *
 * @param array<K, V> $array
 * @param int<0, 5> $flags
 *
 * @return ($array is non-empty-array|non-empty-list ? non-empty-array<K, V> : array<K, V>)
 *
 * @pure
 */
function array_unique(array $array, int $flags = SORT_STRING): array {}

/**
 * Returns the entries of $array whose value also occurs, in every one of the $arrays, keeping the
 * keys from $array. Values are compared by their string representation rather than with loose ==,
 * so two values are treated as equal whenever (string) $a === (string) $b — a narrower and
 * different rule from the one most other array_* functions use.
 *
 * @template K as array-key
 * @template V
 *
 * @param array<K, V> $array
 * @param array ...$arrays
 *
 * @return array<K, V>
 * @pure
 */
function array_intersect(array $array, array ...$arrays): array {}

/**
 * Returns the entries of $array whose keys also exist in every array given in $arrays, keeping
 * $array's original values.
 *
 * @template K as array-key
 * @template V
 *
 * @param array<K, V> $array
 * @param array ...$arrays
 *
 * @return array<K, V>
 *
 * @pure
 */
function array_intersect_key(array $array, array ...$arrays): array {}

/**
 * Returns the entries of array that are also present, under the same key, in every array given in
 * arrays. Values are compared by casting both sides to string, not by strict type or identity.
 *
 * @template K as array-key
 * @template V
 *
 * @param array<K, V> $array
 * @param array ...$arrays
 *
 * @return array<K, V>
 * @pure
 */
function array_intersect_assoc(array $array, array ...$arrays): array {}

/**
 * Computes the values in $array that are not present in any of $arrays, comparing values as
 * strings. Keys from $array are preserved in the result.
 *
 * @template K as array-key
 * @template V as scalar|Stringable|null|resource
 *
 * @param array<K, V> $array
 * @param array<array-key, V> ...$arrays
 *
 * @return array<K, V>
 * @pure
 */
function array_diff(array $array, array ...$arrays): array {}

/**
 * Returns the entries of the first array whose keys do not exist in any of the other arrays. Only
 * keys are compared; the values themselves are ignored.
 *
 * @template K as array-key
 * @template V
 *
 * @param array<K, V> $array
 * @param array ...$arrays
 *
 * @return array<K, V>
 * @pure
 */
function array_diff_key(array $array, array ...$arrays): array {}

/**
 * Returns the entries of $array whose key and value together do not occur in any of $arrays,
 * comparing both keys and values, unlike array_diff(), which compares values alone. Two values are
 * considered equal only if their string representations match.
 *
 * @template K as array-key
 * @template V
 *
 * @param array<K, V> $array
 * @param array ...$arrays
 *
 * @return array<K, V>
 *
 * @pure
 */
function array_diff_assoc(array $array, array ...$arrays): array {}

/**
 * Returns the sum of the values in $array.
 *
 * @param array<numeric> $array
 *
 * @return ($array is array<int> ? int : ($array is array<float> ? float : int|float))
 *
 * @pure
 */
function array_sum(array $array): int|float {}

/**
 * Returns the product of all values in $array, as an int where the result fits and a float
 * otherwise. Returns 1 for an empty array.
 *
 * @param array<int|float> $array
 *
 * @pure
 */
function array_product(array $array): int|float {}

/**
 * Filters an array, keeping only the elements for which the callback returns true, or removing
 * falsy values when no callback is given. The original keys are preserved in the result.
 *
 * @template K as array-key
 * @template V
 *
 * @param array<K, V> $array
 * @param null|(callable(V, K): bool)|(callable(V): bool)|(callable(K): bool) $callback
 *
 * @return array<K, V>
 */
function array_filter(array $array, ?callable $callback = null, int $mode = 0): array {}

/**
 * Applies a callback to each element of one or more arrays and returns an array of the results.
 * Keys are preserved when mapping a single array, but discarded and renumbered when mapping
 * multiple arrays together.
 *
 * @template K as array-key
 * @template V
 * @template S
 * @template U
 *
 * @param (callable(V): U)|(callable(V, S): U)|null $callback
 * @param array<K, V> $array
 * @param array<S> ...$arrays
 *
 * @return ($array is list<V>
 *     ? ($array is non-empty-list<V> ? non-empty-list<U> : list<U>)
 *     : ($array is non-empty-array<K, V> ? non-empty-array<K, U> : array<K, U>))
 */
function array_map(?callable $callback, array $array, array ...$arrays): array {}

/**
 * Splits an array into a list of smaller arrays, each with at most the given length. Keys are
 * discarded and the chunks reindexed from zero unless preserve_keys is true.
 *
 * @template K as array-key
 * @template V
 *
 * @param array<K, V> $array
 * @param int<1, max> $length
 *
 * @return ($preserve_keys is true ? list<array<K, V>> : list<list<V>>)
 *
 * @pure
 */
function array_chunk(array $array, int $length, bool $preserve_keys = false): array {}

/**
 * Builds a new array using one array's values as keys and another's values as the corresponding
 * values. Both arrays must have the same number of elements.
 *
 * @template K as array-key
 * @template V
 *
 * @param array<K> $keys
 * @param array<V> $values
 *
 * @return ($keys is non-empty-array|non-empty-list ? non-empty-array<K, V> : array<K, V>)
 *
 * @pure
 */
function array_combine(array $keys, array $values): array {}

/**
 * Checks whether a given key or index exists in an array. Unlike isset(), it returns true even when
 * the value stored at that key is null.
 *
 * @pure
 */
function array_key_exists(string|int|float|bool $key, array $array): bool {}

/**
 * Returns the first key of an array. Returns null if the array is empty, and unlike reset(), it
 * does not move the array's internal pointer.
 *
 * @template K as array-key
 *
 * @param array<K, mixed> $array
 *
 * @return ($array is non-empty-array|non-empty-list ? K : null)
 *
 * @pure
 */
function array_key_first(array $array): string|int|null {}

/**
 * Returns the value of the first element of $array regardless of its key, or null if the array is
 * empty. Unlike reset(), it does not move the array's internal pointer.
 *
 * @template V
 *
 * @param array<array-key, V> $array
 *
 * @return ($array is non-empty-array|non-empty-list ? V : null)
 *
 * @pure
 */
#[Mago\AvailableSince(80500)]
function array_first(array $array): mixed {}

/**
 * Returns the last key of an array, or null if the array is empty.
 *
 * @template K as array-key
 *
 * @param array<K, mixed> $array
 *
 * @return ($array is non-empty-array|non-empty-list ? K : null)
 *
 * @pure
 */
function array_key_last(array $array): string|int|null {}

/**
 * Returns the last element of an array, in iteration order rather than by key. Returns null if the
 * array is empty.
 *
 * @template V
 *
 * @param array<array-key, V> $array
 *
 * @return ($array is non-empty-array|non-empty-list ? V : null)
 *
 * @pure
 */
#[Mago\AvailableSince(80500)]
function array_last(array $array): mixed {}

/**
 * Alias of current(): returns the element the array's or object's internal pointer currently
 * references, without moving the pointer, or false if the pointer is past the last element.
 *
 * @pure
 */
function pos(object|array $array): mixed {}

/**
 * Alias of count: returns the number of elements in $value. With the default COUNT_NORMAL it counts
 * only the top level; COUNT_RECURSIVE also counts the elements of every nested array.
 *
 * @code
 * sizeof([1, [2, 3]], COUNT_RECURSIVE)
 * @endcode
 *
 * @return ($value is non-empty-array|non-empty-list ? int<1, max> : int<0, max>)
 *
 * @pure
 */
function sizeof(Countable|array $value, int $mode = COUNT_NORMAL): int {}

/**
 * Checks whether $key exists in $array, including when its value is null — unlike
 * isset($array[$key]), which would answer false in that case. This is an alias of
 * array_key_exists().
 *
 * @template K as array-key
 * @template V
 *
 * @param K $key
 * @param array<K, V> $array
 */
function key_exists($key, array $array): bool {}

/**
 * Evaluates an assertion and, when it is false, throws an AssertionError (or the given Throwable)
 * unless assertions are disabled. Whether assertions run at all is controlled by the
 * zend.assertions ini setting, so relying on it for validation logic in production is unsafe.
 *
 * @assert truthy $assertion
 *
 * @throws AssertionError
 */
function assert(mixed $assertion, Throwable|string|null $description = null): bool {}

class AssertionError extends Error {}

/**
 * Sets or retrieves options that control assert()'s runtime behaviour, such as whether assertions
 * execute or what happens when one fails.
 *
 * @deprecated
 */
function assert_options(int $option, mixed $value = UNKNOWN): mixed {}

/**
 * Compares two PHP-style version strings such as '1.2.3' or '2.0.0beta1'. Suffixes are ranked
 * rather than compared as plain text: dev sorts lowest, then alpha/a, beta/b, RC/rc, a plain
 * release, and finally pl/p (patch level) highest.
 *
 * @code
 * version_compare('1.0.0beta', '1.0.0', '<'); // true
 * @endcode
 *
 * @param null|'<'|'lt'|'<='|'le'|'>'|'gt'|'>='|'ge'|'=='|'='|'eq'|'!='|'<>'|'ne' $operator
 *
 * @return ($operator is null ? int : bool)
 *
 * @pure
 */
function version_compare(string $version1, string $version2, ?string $operator = null): int|bool {}

/**
 * Converts filename and the single-character project_id into a System V IPC key suitable for shared
 * memory, semaphore or message-queue functions, or -1 on failure. The key depends on the file's
 * device and inode number, so it changes if the file is deleted and recreated.
 */
function ftok(string $filename, string $project_id): int {}

/**
 * Applies the ROT13 substitution cipher to string, rotating each ASCII letter thirteen places
 * through the alphabet and leaving every other character unchanged. Applying it a second time to
 * the result recovers the original string.
 *
 * @pure
 */
function str_rot13(string $string): string {}

/**
 * Returns the names of the stream filters currently registered -- the same names usable with
 * stream_filter_append() and stream_filter_prepend().
 *
 * @return list<string>
 */
function stream_get_filters(): array {}

/**
 * Returns true if stream is attached to an interactive terminal, commonly used to decide whether to
 * emit colour or other terminal-only escape sequences.
 *
 * @param resource $stream
 */
function stream_isatty($stream): bool {}

/**
 * Registers a class as a stream filter implementation under a chosen filter name, so that name can
 * later be used with stream_filter_append or stream_filter_prepend. The class must extend the
 * built-in php_user_filter class. Returns false if the name is already registered.
 *
 * @param class-string $class
 */
function stream_filter_register(string $filter_name, string $class): bool {}

/**
 * Used inside a custom stream filter's filter() method: removes and returns the next bucket from
 * $brigade as a writable StreamBucket, or null if the brigade is empty. The filter typically reads
 * $bucket->data, transforms it, and passes the modified bucket on with stream_bucket_append().
 *
 * @param resource $brigade
 */
function stream_bucket_make_writeable($brigade): ?StreamBucket {}

/**
 * Inserts bucket at the front of brigade, the linked list of data chunks a custom stream filter's
 * filter() method reads from and writes to. Used inside a php_user_filter implementation to push
 * data back for processing on a later call.
 *
 * @param resource $brigade
 */
function stream_bucket_prepend($brigade, StreamBucket $bucket): void {}

/**
 * Appends $bucket to the end of $brigade, the resource passed to a stream filter's filter() method
 * that represents the queue of data chunks moving along a stream filter chain.
 *
 * @param resource $brigade
 */
function stream_bucket_append($brigade, StreamBucket $bucket): void {}

/**
 * Creates a new StreamBucket holding $buffer, attached to $stream, for a user-defined stream filter
 * to pass on to stream_bucket_append() from within its filter() method.
 *
 * @param resource $stream
 */
function stream_bucket_new($stream, string $buffer): StreamBucket {}

/**
 * Registers a name/value pair to be automatically appended to relative URLs and inserted as a
 * hidden form field in the page's output, the same mechanism PHP uses to propagate the session ID
 * when cookies are unavailable. Use output_reset_rewrite_vars() to clear every pair registered so
 * far.
 */
function output_add_rewrite_var(string $name, string $value): bool {}

/**
 * Clears every URL-rewriter variable previously registered with output_add_rewrite_var(), so they
 * stop being appended to local URLs and forms in the page's remaining output.
 */
function output_reset_rewrite_vars(): bool {}

/**
 * Returns the path to the directory PHP uses for temporary files.
 *
 * @return non-empty-string
 */
function sys_get_temp_dir(): string {}

/**
 * Returns the contents of PHP's realpath cache, which maps a looked-up path to the result of
 * resolving it. Each entry is keyed by the looked-up path and records the resolved absolute
 * realpath, whether it is a directory, an internal cache key, and the time at which the entry
 * expires.
 */
function realpath_cache_get(): array {}

/**
 * Returns how many bytes of memory the realpath cache, populated as realpath and file-existence
 * checks resolve paths, is currently using.
 */
function realpath_cache_size(): int {}

/**
 * Returns every property of $object, including private and protected ones, keyed by its internal
 * mangled name rather than its plain name: a private property is prefixed with a null byte, the
 * declaring class name and another null byte, and a protected one with a null byte, an asterisk and
 * another null byte. It exists mainly to support serialisation, not ordinary property access.
 *
 * @code
 * // private $x declared in class Foo becomes the key "\0Foo\0x"
 * get_mangled_object_vars($obj);
 * @endcode
 */
function get_mangled_object_vars(object $object): array {}

/**
 * Returns the type of $value as a string, using the actual class name for objects rather than the
 * generic "object" that gettype() returns.
 *
 * @return non-empty-string
 *
 * @pure
 */
#[Mago\AvailableSince(80000)]
function get_debug_type(mixed $value): string {}

/**
 * Returns the integer id PHP has assigned to resource, the same number shown when the resource is
 * cast to string or printed with var_dump().
 *
 * @param resource $resource
 *
 * @pure
 */
function get_resource_id($resource): int {}

/**
 * Returns the entries of array whose keys are not present, according to a user-supplied comparison
 * callback, in any of the following arrays. $rest holds one or more comparison arrays followed by
 * the callback itself as its final element.
 *
 * @no-named-arguments
 */
function array_diff_ukey(array $array, ...$rest): array {}

/**
 * Computes the values in array that are absent from every other array given, comparing values with
 * a user-supplied callback instead of loose equality. The last argument is always that comparison
 * callback; every array to diff against comes before it.
 *
 * @code
 * array_udiff($a, $b, $c, fn($x, $y) => $x <=> $y);
 * @endcode
 *
 * @no-named-arguments
 */
function array_udiff(array $array, ...$rest): array {}

/**
 * Like array_udiff(), but also requires the keys to match, compared by their string value, between
 * array and each of the arrays in rest -- so a value appearing under a different key still counts
 * as absent. Value comparison is delegated to a callback, the last element of rest, which works
 * like the comparator passed to usort(): values are treated as equal exactly when it returns zero.
 * The result keeps the entries of array that have no equal, same-keyed match in any of the other
 * arrays.
 *
 * @no-named-arguments
 */
function array_udiff_assoc(array $array, ...$rest): array {}

/**
 * Returns the entries of array whose keys are not matched, according to key_compare_func, by any
 * key in the following arrays, with values themselves compared for equality by their string
 * representation. The comparison callback is always the last element of rest, after every array
 * being compared against, and must return a negative, zero, or positive integer the way a sort
 * comparator does.
 *
 * @code
 * array_diff_uassoc($array1, $array2, fn($a, $b) => strcasecmp($a, $b));
 * @endcode
 *
 * @no-named-arguments
 */
function array_diff_uassoc(array $array, ...$rest): array {}

/**
 * Returns the entries of the first array whose value and key do not both match an entry in any of
 * the other arrays, comparing values and keys with user-supplied callbacks. The variadic argument
 * list is the arrays to compare against first, followed by the value-comparison callback and then
 * the key-comparison callback as the final two arguments.
 *
 * @code
 * array_udiff_uassoc($array1, $array2, $valueCompare, $keyCompare);
 * @endcode
 *
 * @no-named-arguments
 */
function array_udiff_uassoc(array $array, ...$rest): array {}

/**
 * Compares $array against each array in $rest by both key and value — unlike array_uintersect(),
 * which compares values only — using a user-supplied comparison callback for the values, and
 * returns the entries of $array whose key and value both match in every other array. The callback,
 * always the last argument, must behave like a sort comparator, returning a negative, zero or
 * positive integer rather than a boolean.
 *
 * @code
 * array_uintersect_assoc($prices, $discounted, fn($a, $b) => $a <=> $b);
 * @endcode
 *
 * @no-named-arguments
 */
function array_uintersect_assoc(array $array, ...$rest): array {}

/**
 * Like array_intersect_assoc(), but keys across the arrays are compared with a user-supplied
 * callback instead of the default comparison; values are still compared by casting to string.
 *
 * @no-named-arguments
 */
function array_intersect_uassoc(array $array, ...$rest): array {}

/**
 * Computes the intersection of the given arrays by key and value, using a user callback for each:
 * the callback comparing values comes first among the trailing arguments and the callback comparing
 * keys comes last, the reverse of what the two halves of the function's name might suggest.
 *
 * @code
 * array_uintersect_uassoc($a, $b, $valueCompare, $keyCompare);
 * @endcode
 *
 * @no-named-arguments
 */
function array_uintersect_uassoc(array $array, ...$rest): array {}

/**
 * Compares the keys of $array against the keys of one or more other arrays using a user-supplied
 * callback given as the last argument, and returns the entries of $array whose key the callback
 * reports as equal to a key in every other array.
 *
 * @code
 * array_intersect_ukey($a, $b, fn($k1, $k2) => $k1 <=> $k2);
 * @endcode
 *
 * @no-named-arguments
 */
function array_intersect_ukey(array $array, ...$rest): array {}

/**
 * Computes the intersection by value of two or more arrays, using a user-supplied comparison
 * callback instead of loose or strict equality. The callback must be the last argument, after every
 * array being intersected, and should return a negative, zero or positive integer the way a
 * strcmp()-style comparator does.
 *
 * @code
 * array_uintersect($a, $b, fn($x, $y) => $x <=> $y);
 * @endcode
 *
 * @no-named-arguments
 */
function array_uintersect(array $array, ...$rest): array {}

/**
 * Sorts $array in place by its keys, in ascending order.
 *
 * @template K as array-key
 * @template V
 *
 * @param array<K, V> $array
 *
 * @param-out ($array is non-empty-array|non-empty-list ? non-empty-array<K, V> : array<K, V>) $array
 */
function ksort(array &$array, int $flags = SORT_REGULAR): true {}

/**
 * Sorts an array in place by key, in descending order, preserving key-value associations.
 *
 * @template K as array-key
 * @template V
 *
 * @param array<K, V> $array
 *
 * @param-out ($array is non-empty-array|non-empty-list ? non-empty-array<K, V> : array<K, V>) $array
 */
function krsort(array &$array, int $flags = SORT_REGULAR): true {}

/**
 * Sorts an array in place using a natural-order string comparison, so that a name like 'img2' sorts
 * before 'img12' as a human would expect rather than after it as a plain byte-wise comparison would
 * order them. The original keys are preserved and follow their values.
 *
 * @code
 * natsort($files); // 'img2.png' ends up before 'img12.png'
 * @endcode
 *
 * @template K as array-key
 * @template V
 *
 * @param array<K, V> $array
 *
 * @param-out ($array is non-empty-array|non-empty-list ? non-empty-array<K, V> : array<K, V>) $array
 */
function natsort(array &$array): true {}

/**
 * Sorts $array in place using the same case-insensitive natural-order comparison as
 * strnatcasecmp(), so embedded numbers compare by numeric value rather than digit by digit. The
 * original keys are preserved rather than reindexed, and the function always returns true.
 *
 * @template K as array-key
 * @template V
 *
 * @param array<K, V> $array
 *
 * @param-out ($array is non-empty-array|non-empty-list ? non-empty-array<K, V> : array<K, V>) $array
 */
function natcasesort(array &$array): true {}

/**
 * Sorts an array in place by value, in ascending order, preserving key-value associations.
 *
 * @template K as array-key
 * @template V
 *
 * @param array<K, V> $array
 *
 * @param-out ($array is non-empty-array|non-empty-list ? non-empty-array<K, V> : array<K, V>) $array
 */
function asort(array &$array, int $flags = SORT_REGULAR): true {}

/**
 * Sorts $array in place in descending order by value, preserving the original keys. $flags selects
 * the comparison used, such as SORT_NUMERIC or SORT_STRING, in place of the default type-juggling
 * SORT_REGULAR comparison.
 *
 * @template K as array-key
 * @template V
 *
 * @param array<K, V> $array
 *
 * @param-out ($array is non-empty-array|non-empty-list ? non-empty-array<K, V> : array<K, V>) $array
 */
function arsort(array &$array, int $flags = SORT_REGULAR): true {}

/**
 * Sorts an array's values in place in ascending order. Discards the original keys and reindexes the
 * array from zero.
 *
 * @template T
 *
 * @param array<T> $array
 *
 * @param-out ($array is non-empty-array|non-empty-list ? non-empty-list<T> : list<T>) $array
 */
function sort(array &$array, int $flags = SORT_REGULAR): true {}

/**
 * Sorts an array in descending order, in place. Discards the original keys, reindexing the array
 * numerically from zero.
 *
 * @template T
 *
 * @param array<T> $array
 *
 * @param-out ($array is non-empty-array|non-empty-list ? non-empty-list<T> : list<T>) $array
 */
function rsort(array &$array, int $flags = SORT_REGULAR): true {}

/**
 * Sorts an array's values in place using a user-supplied comparison callback. Discards the original
 * keys and reindexes the array from zero.
 *
 * @template T
 *
 * @param array<T> $array
 * @param (callable(T, T): int) $callback
 *
 * @param-out ($array is non-empty-array|non-empty-list ? non-empty-list<T> : list<T>) $array
 */
function usort(array &$array, callable $callback): true {}

/**
 * Sorts an array in place by value, using a user-supplied comparison callback, preserving keys.
 *
 * @template K
 * @template V
 *
 * @param array<K, V> $array
 * @param (callable(V, V): int) $callback
 *
 * @param-out ($array is non-empty-array|non-empty-list ? non-empty-array<K, V> : array<K, V>) $array
 */
function uasort(array &$array, callable $callback): true {}

/**
 * Sorts $array in place by comparing its keys with $callback, which must return a negative, zero,
 * or positive int the way strcmp() does. Each key keeps the value it was paired with; only the
 * order changes. Always returns true.
 *
 * @template K as array-key
 * @template V
 *
 * @param array<K, V> $array
 * @param (callable(K, K): int) $callback
 *
 * @param-out ($array is non-empty-array|non-empty-list ? non-empty-array<K, V> : array<K, V>) $array
 */
function uksort(array &$array, callable $callback): true {}

/**
 * Randomly reorders the elements of array in place, using a non-cryptographic random source, and
 * discards the original keys so elements are renumbered from zero.
 *
 * @template T
 *
 * @param array<T> $array
 *
 * @param-out ($array is non-empty-array|non-empty-list ? non-empty-list<T> : list<T>) $array
 */
function shuffle(array &$array): true {}

/**
 * Counts the elements in an array, or calls a Countable object's own count() method. With $mode set
 * to COUNT_RECURSIVE, it also counts elements inside nested arrays.
 *
 * @return ($value is non-empty-array|non-empty-list ? int<1, max> : int<0, max>)
 *
 * @pure
 */
function count(Countable|array $value, int $mode = COUNT_NORMAL): int {}

/**
 * Moves an array's internal pointer to its last element and returns that element's value. Returns
 * false if the array is empty, which cannot be told apart from a genuine element whose value is
 * false.
 *
 * @template T
 *
 * @param object|array<T> $array
 * @param-out ($array is object ? object : ($array is list<T> ? list<T> : array<array-key, T>)) $array
 *
 * @return ($array is non-empty-array|non-empty-list ? T : false)
 */
function end(object|array &$array): mixed {}

/**
 * Moves array's internal pointer back one position and returns the element there, or false once the
 * pointer moves past the first element. Because the element itself might genuinely be false, use
 * key($array) !== null to tell a real false value apart from having run out of elements.
 *
 * @template T
 *
 * @param object|array<T> $array
 * @param-out ($array is object ? object : ($array is list<T> ? list<T> : array<array-key, T>)) $array
 *
 * @return T|false
 */
function prev(object|array &$array): mixed {}

/**
 * Advances the internal pointer of array by one position and returns the value now there, or false
 * once the pointer has moved past the last element. Because a stored value can itself be false, use
 * key() afterwards to tell a real element from the end of the array.
 *
 * @template K
 * @template V
 *
 * @param object|array<K, V> $array
 * @param-out ($array is object ? object : ($array is list<V> ? list<V> : array<K, V>)) $array
 *
 * @return V|false
 */
function next(object|array &$array): mixed {}

/**
 * Rewinds an array's internal pointer to its first element and returns that element's value.
 * Returns false if the array is empty, and mutates the array's internal pointer as a side effect.
 *
 * @template K
 * @template V
 *
 * @param object|array<K, V> $array
 * @param-out ($array is object ? object : ($array is list<V> ? ($array is non-empty-list<V> ? non-empty-list<V> : list<V>) : ($array is non-empty-array<K, V> ? non-empty-array<K,V> : array<K, V>))) $array
 *
 * @return ($array is non-empty-list ? V : ($array is non-empty-array ? V : V|false))
 */
function reset(object|array &$array): mixed {}

/**
 * Returns the value the array's internal pointer currently references, without moving the pointer.
 * Returns false once the pointer has advanced past the last element, or immediately for an empty
 * array -- though false is also a legitimate element value, so it cannot on its own signal that the
 * pointer is exhausted. Given an object rather than an array, it walks the object's properties the
 * way get_mangled_object_vars() would, rather than delegating to the object's own Iterator methods
 * as one might expect; this use on objects was deprecated in PHP 8.1.
 *
 * @template V
 *
 * @param object|array<array-key, V> $array
 *
 * @return ($array is object ? mixed : (V|false))
 *
 * @pure
 */
function current(object|array $array): mixed {}

/**
 * Returns the key of the array's current internal-pointer element, or of an iterator object's
 * current element, without advancing the pointer.
 *
 * @template K of array-key
 *
 * @param object|array<K, mixed> $array
 *
 * @return K|null
 *
 * @pure
 */
function key(object|array $array): string|int|null {}

/**
 * Returns the lowest of the given values, or the lowest value in an array when called with a single
 * array argument.
 *
 * @template T
 *
 * @param non-empty-array<T>|T $value
 * @param T ...$values
 *
 * @return T
 *
 * @pure
 */
function min(mixed $value, mixed ...$values): mixed {}

/**
 * Returns the highest of the given values, or the highest value in an array when called with a
 * single array argument.
 *
 * @template T
 *
 * @param non-empty-array<T>|T $value
 * @param T ...$values
 *
 * @return T
 *
 * @pure
 */
function max(mixed $value, mixed ...$values): mixed {}

/**
 * Restrict `$num` to the inclusive range `[$min, $max]`.
 *
 * @template T of int|float
 *
 * @param T $num
 * @param T $min
 * @param T $max
 *
 * @return T
 *
 * @pure
 */
#[Mago\AvailableSince(80600)]
function clamp(int|float $num, int|float $min, int|float $max): int|float {}

/**
 * Checks whether a value exists among the elements of an array. Comparison is loose by default;
 * pass strict to require the needle to match in both value and type.
 *
 * @template T
 * @template V
 *
 * @param T $needle
 * @param array<V> $haystack
 *
 * @pure
 */
function in_array(mixed $needle, array $haystack, bool $strict = false): bool {}

/**
 * Searches an array for a given value and returns the first key at which it is found. Returns false
 * if not found, and comparison is loose unless $strict is true.
 *
 * @template K of array-key
 * @template V
 *
 * @param V $needle
 * @param array<K, V> $haystack
 *
 * @return K|false
 *
 * @pure
 */
function array_search(mixed $needle, array $haystack, bool $strict = false): string|int|false {}

/**
 * Creates an array of count elements, all set to the same value, with keys starting at start_index.
 *
 * @template T
 *
 * @param int<0, max> $count
 * @param T $value
 *
 * @return (
 *   $start_index is 0 ?
 *   ($count is int<1, max> ? non-empty-list<T> : list<T>) :
 *   ($count is int<1, max> ? non-empty-array<int, T> : array<int, T>)
 * )
 *
 * @pure
 */
function array_fill(int $start_index, int $count, mixed $value): array {}

/**
 * Builds an array using every value from keys as a key, each mapped to the same value. A duplicate
 * key in keys collapses to a single entry, and if value is an object, every key shares that same
 * instance rather than a clone of it.
 *
 * @template K of array-key
 * @template V
 *
 * @param array<K> $keys
 * @param V $value
 *
 * @return ($keys is non-empty-array<K> ? non-empty-array<K, V> : array<K, V>)
 *
 * @pure
 */
function array_fill_keys(array $keys, mixed $value): array {}

/**
 * Appends one or more $values to the end of $array in place, each under a new sequential integer
 * key.
 *
 * @template K of array-key
 * @template V
 *
 * @param array<K, V> $array
 * @param V ...$values
 *
 * @param-out ($array is list<V> ? non-empty-list<V> : non-empty-array<K, V>) $array
 *
 * @return int<1, max>
 *
 * @pure
 */
function array_push(array &$array, mixed ...$values): int {}

/**
 * Removes and returns the last element of an array, shortening it by one. Returns null if the array
 * is empty, and mutates the array in place.
 *
 * @template K of array-key
 * @template V
 *
 * @param array<K, V> $array
 *
 * @param-out ($array is list<V> ? list<V> : array<K, V>) $array
 *
 * @return ($array is non-empty-array|non-empty-list ? V : V|null)
 */
function array_pop(array &$array): mixed {}

/**
 * Removes and returns the first element of $array in place, renumbering the remaining integer keys
 * while leaving string keys untouched.
 *
 * @template K of array-key
 * @template V
 *
 * @param array<K, V> $array
 * @param-out ($array is list<V> ? list<V> : array<K, V>) $array
 *
 * @return ($array is non-empty-array|non-empty-list ? V : null)
 *
 * @pure
 */
function array_shift(array &$array): mixed {}

/**
 * Extracts a slice of an array given an offset and optional length. Numeric keys are renumbered
 * from zero unless preserve_keys is true; string keys are always kept.
 *
 * @template K of array-key
 * @template V
 *
 * @param array<K, V> $array
 *
 * @return ($array is list<V> ? ($preserve_keys is true ? array<int, V> : list<V>) : array<K, V>) $array
 *
 * @pure
 */
function array_slice(array $array, int $offset, ?int $length = null, bool $preserve_keys = false): array {}

/**
 * Merges one or more arrays into a single array. String keys from later arrays overwrite earlier
 * ones with the same key, while integer keys are renumbered and all values are kept.
 *
 * @template K as array-key
 * @template V
 *
 * @param array<K, V> ...$arrays
 *
 * @return array<K, V>
 *
 * @pure
 */
function array_merge(array ...$arrays): array {}

/**
 * Turns on output buffering, so subsequent output is captured instead of being sent immediately. If
 * $callback is given, it is invoked with the buffered output whenever the buffer is flushed,
 * letting it transform the output before it is sent.
 *
 * @param (callable(string, null|int): string)|null $callback
 */
function ob_start(?callable $callback = null, int $chunk_size = 0, int $flags = PHP_OUTPUT_HANDLER_STDFLAGS): bool {}

/**
 * Sends the contents of the current output buffering level to the next level, or to the browser if
 * it is the outermost, and clears them, but leaves output buffering switched on, unlike
 * ob_end_flush. Returns false if there is no active output buffer.
 */
function ob_flush(): bool {}

/**
 * Discards the contents currently held by the active output buffer without sending them and without
 * turning buffering off, unlike ob_end_clean(). Returns false if there is no active buffer to
 * clean.
 */
function ob_clean(): bool {}

/**
 * Sends the current output buffer's contents onward to whatever is next in the buffer stack, or to
 * the browser, and then removes that buffer. Returns false if there was no active output buffer to
 * end.
 */
function ob_end_flush(): bool {}

/**
 * Discards the contents of the current output buffer without sending them, and turns that buffering
 * level off. Returns false if there is no active buffer, or if the buffer's callback refuses to let
 * it be deleted.
 */
function ob_end_clean(): bool {}

/**
 * Sends the contents of the current output buffer to the browser and turns off that buffering
 * level, also returning the flushed contents as a string. Returns false if no output buffer is
 * active.
 */
function ob_get_flush(): string|false {}

/**
 * Returns the current output buffer's contents and discards that buffer in one call. Returns false
 * if no output buffer is active.
 */
function ob_get_clean(): string|false {}

/**
 * Returns the length in bytes of the content currently held in the active output buffer, or false
 * if output buffering is not active.
 */
function ob_get_length(): int|false {}

/**
 * Returns the current nesting level of output buffering, or 0 if no output buffer is active.
 */
function ob_get_level(): int {}

/**
 * Returns information about the active output buffering handler. With $full_status false, the
 * default, only the outermost active buffer is described, as a single associative array; with it
 * true, every nested buffering level is described, as an array of such arrays from outermost to
 * innermost. Returns an empty array if output buffering is not active.
 *
 * @return array{
 *   level: int,
 *   type: int,
 *   flags: int,
 *   name: string,
 *   del: int,
 *   chunk_size: int,
 *   buffer_size: int,
 *   buffer_used: int
 * }
 */
function ob_get_status(bool $full_status = false): array {}

/**
 * Returns the contents currently held in the active output buffer without clearing it, or false if
 * output buffering is not active.
 */
function ob_get_contents(): string|false {}

/**
 * Enables or disables automatic flushing of output after every output-producing statement, so
 * output appears immediately rather than waiting for the buffer to fill or the script to end.
 * Enabling it defeats much of the point of output buffering.
 */
function ob_implicit_flush(bool $enable = true): void {}

/**
 * Returns the names of the output handlers currently active in the output buffering stack.
 *
 * @return list<non-empty-string>
 */
function ob_list_handlers(): array {}

/**
 * Sends $message to the system logger, tagged with the $priority level.
 */
function syslog(int $priority, string $message): true {}

/**
 * Closes the connection to the system logger that was opened by openlog. Always returns true.
 */
function closelog(): true {}

/**
 * Registers a callback to be invoked right before the response headers and body begin to be sent,
 * whether that is triggered by an explicit header or setcookie call, the first byte of output, or
 * the script ending. Only one such callback can be active at a time; registering another replaces
 * it.
 *
 * @param (callable(): void) $callback
 */
function header_register_callback(callable $callback): bool {}

/**
 * Like getimagesize(), but reads image data already held in string rather than opening a file --
 * useful for an image received over the network or built in memory. Same array shape, and the same
 * image_info behaviour for JPEG APP markers, apply.
 *
 * @param-out ?array $image_info
 *
 * @return false|array{
 *  0: int,
 *  1: int,
 *  2: int,
 *  3: string,
 *  bits: int,
 *  channels: int,
 *  mime: string
 * }
 */
function getimagesizefromstring(string $string, ?array &$image_info = null): array|false {}

/**
 * Sets the number of bytes stream tries to read from its underlying source in a single operation,
 * and returns whatever the chunk size was before this call.
 *
 * @param resource $stream
 */
function stream_set_chunk_size($stream, int $size): int {}

/**
 * Computes the metaphone key of a string, a phonetic algorithm similar to soundex but tuned to
 * match words that sound alike in English more accurately. max_phonemes limits the length of the
 * returned key; 0 means no limit.
 *
 * @pure
 */
function metaphone(string $string, int $max_phonemes = 0): string {}

/**
 * Applies $callback to every element of $array. The callback can modify an element in place by
 * declaring its first parameter as a reference.
 *
 * @template K
 * @template V
 * @template T
 * @template O
 *
 * @param object|array<K, V> $array
 * @param (callable(V, K, T): O) $callback
 * @param T|null $arg
 *
 * @param-out ($array is object ? object : (
 *    $array is list<V> ? (
 *      $array is non-empty-list<V> ? non-empty-list<O> : list<O>
 *    ) : (
 *      $array is non-empty-array<K, V> ? non-empty-array<K, O> : array<K, O>
 *    )
 * )) $array
 */
function array_walk(object|array &$array, callable $callback, mixed $arg = null): true {}

/**
 * Applies $callback to every scalar leaf value in $array, descending into nested arrays instead of
 * calling $callback on a sub-array itself the way array_walk() would. Declare the callback's first
 * parameter by reference to modify elements of $array in place; $arg, if given, is passed to the
 * callback as a third argument on every call.
 *
 * @code
 * array_walk_recursive($data, function (&$value) { $value = trim($value); });
 * @endcode
 */
function array_walk_recursive(object|array &$array, callable $callback, mixed $arg = null): true {}

/**
 * Imports variables from an array into the current symbol table, using the array's keys as variable
 * names. $flags controls what happens when a name collides with an existing variable; the default
 * overwrites it.
 *
 * @param array<string, mixed> $array
 *
 * @return int<0, max>
 */
function extract(array &$array, int $flags = EXTR_OVERWRITE, string $prefix = ''): int {}

/**
 * Builds an array from the current scope's variables, using the given names as keys. Names that are
 * not currently defined are silently skipped.
 *
 * @param string|array<string> $var_name
 * @param string|array<string> ...$var_names
 *
 * @return array<string, mixed>
 *
 * @pure
 */
function compact($var_name, ...$var_names): array {}

/**
 * Builds an array containing a sequence of values from start to end, inclusive, advancing by step
 * each time.
 *
 * @param positive-int|float $step
 *
 * @return (
 *   $start is string
 *   ? (
 *     $end is string
 *     ? ($step is int ? list<string> : list{float})
 *     : ($end is float ? list<float> : ($step is float ? list<float> : list<int>))
 *   )
 *   : (
 *     $start is float
 *     ? list<float>
 *     : ($end is float ? list<float> : ($step is float ? list<float> : list<int>))
 *   )
 * )
 *
 * @pure
 */
function range(string|int|float $start, string|int|float $end, int|float $step = 1): array {}

/**
 * Sorts array, and re-orders any further arrays given in rest to match, the way an SQL ORDER BY
 * with multiple columns would: the first array decides the order, and ties are broken by the arrays
 * that follow. Each array may be followed by its own SORT_ASC/SORT_DESC direction and a
 * SORT_REGULAR/SORT_NUMERIC/SORT_STRING-style comparison flag. Every array must have the same
 * length, and all of them are sorted in place through their references.
 *
 * @code
 * array_multisort($ages, SORT_ASC, $names); // $names re-ordered to match sorted $ages
 * @endcode
 *
 * @pure
 */
function array_multisort(array &$array, array|int &...$rest): bool {}

/**
 * Inserts one or more $values at the beginning of $array in place, renumbering existing integer
 * keys while leaving string keys untouched.
 *
 * @template K of array-key
 * @template V
 *
 * @param array<K, V> $array
 * @param V ...$values
 *
 * @param-out ($array is list<V> ? non-empty-list<V> : non-empty-array<K, V>) $array
 *
 * @return int<1, max>
 *
 * @pure
 */
function array_unshift(array &$array, mixed ...$values): int {}

/**
 * Removes a portion of an array and optionally replaces it with other elements, mutating the array
 * in place. Returns the removed elements as a new array; unlike array_slice(), the original array
 * argument is changed.
 *
 * @template InputKey of array-key
 * @template InputValue
 * @template ReplacementKey = never
 * @template ReplacementValue = never
 * @template ReplacementScalar
 *
 * @param list<InputValue>|array<InputKey, InputValue> $array
 * @param int $offset
 * @param null|int $length
 * @param array<ReplacementKey, ReplacementValue>|ReplacementScalar $replacement
 *
 * @param-out (
 *   $replacement is array<ReplacementKey, ReplacementValue> ? (
 *     $array is list<InputValue> ? (
 *       $replacement is list<ReplacementValue> ? (
 *         $replacement is non-empty-list<ReplacementValue> ? non-empty-list<InputValue|ReplacementValue> : list<InputValue|ReplacementValue>
 *       ) : (
 *         $replacement is non-empty-array<ReplacementKey, ReplacementValue> ? (
 *           non-empty-array<InputKey|ReplacementKey, InputValue|ReplacementValue>
 *         ) : (
 *           array<InputKey|ReplacementKey, InputValue|ReplacementValue>
 *         )
 *       )
 *     ) : (
 *       $replacement is non-empty-array<ReplacementKey, ReplacementValue> ? (
 *         non-empty-array<InputKey|ReplacementKey, InputValue|ReplacementValue>
 *       ) : (
 *         array<InputKey|ReplacementKey, InputValue|ReplacementValue>
 *       )
 *     )
 *   ) : (
 *     $replacement is list<never>|array{} ? (
 *       $array is list<InputValue> ? list<InputValue> : array<InputKey, InputValue>
 *     ) : (
 *       $array is list<InputValue> ? non-empty-list<InputValue|ReplacementScalar> : non-empty-array<InputKey|int, InputValue|ReplacementScalar>
 *     )
 *   )
 * ) $array
 *
 * @return ($array is list<InputValue> ? list<InputValue> : array<InputKey, InputValue>)
 *
 * @pure
 */
function array_splice(array &$array, int $offset, ?int $length = null, mixed $replacement = []): array {}
