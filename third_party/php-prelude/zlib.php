<?php

/**
 * @var int
 */
const FORCE_GZIP = UNKNOWN;

/**
 * @var int
 */
const FORCE_DEFLATE = UNKNOWN;

/**
 * @var int
 */
const ZLIB_ENCODING_RAW = UNKNOWN;

/**
 * @var int
 */
const ZLIB_ENCODING_GZIP = UNKNOWN;

/**
 * @var int
 */
const ZLIB_ENCODING_DEFLATE = UNKNOWN;

/**
 * @var int
 */
const ZLIB_NO_FLUSH = UNKNOWN;

/**
 * @var int
 */
const ZLIB_PARTIAL_FLUSH = UNKNOWN;

/**
 * @var int
 */
const ZLIB_SYNC_FLUSH = UNKNOWN;

/**
 * @var int
 */
const ZLIB_FULL_FLUSH = UNKNOWN;

/**
 * @var int
 */
const ZLIB_BLOCK = UNKNOWN;

/**
 * @var int
 */
const ZLIB_FINISH = UNKNOWN;

/**
 * @var int
 */
const ZLIB_FILTERED = UNKNOWN;

/**
 * @var int
 */
const ZLIB_HUFFMAN_ONLY = UNKNOWN;

/**
 * @var int
 */
const ZLIB_RLE = UNKNOWN;

/**
 * @var int
 */
const ZLIB_FIXED = UNKNOWN;

/**
 * @var int
 */
const ZLIB_DEFAULT_STRATEGY = UNKNOWN;

/**
 * @var string
 */
const ZLIB_VERSION = UNKNOWN;

/**
 * @var int
 */
const ZLIB_VERNUM = UNKNOWN;

/**
 * @var int
 */
const ZLIB_OK = UNKNOWN;

/**
 * @var int
 */
const ZLIB_STREAM_END = UNKNOWN;

/**
 * @var int
 */
const ZLIB_NEED_DICT = UNKNOWN;

/**
 * @var int
 */
const ZLIB_ERRNO = UNKNOWN;

/**
 * @var int
 */
const ZLIB_STREAM_ERROR = UNKNOWN;

/**
 * @var int
 */
const ZLIB_DATA_ERROR = UNKNOWN;

/**
 * @var int
 */
const ZLIB_MEM_ERROR = UNKNOWN;

/**
 * @var int
 */
const ZLIB_BUF_ERROR = UNKNOWN;

/**
 * @var int
 */
const ZLIB_VERSION_ERROR = UNKNOWN;

final class InflateContext {}

final class DeflateContext {}

function ob_gzhandler(string $data, int $flags): string|false {}

function zlib_get_coding_type(): string|false {}

/**
 * Reads an entire gzip-compressed file into an array, one array element per line, decompressing as
 * it goes. Returns false on failure. $use_include_path, when true, searches for $filename along
 * include_path.
 *
 * @return array<int, string>|false
 */
function gzfile(string $filename, bool $use_include_path = false): array|false {}

/**
 * Opens a gzip-compressed file for reading or writing and returns a file-pointer-like resource
 * usable with the gz* functions, analogous to fopen(). For writing, the mode string may have a
 * compression level from 0 (none) to 9 (best) appended, for example 'wb9'. Returns false on
 * failure.
 *
 * @code
 * gzopen('data.txt.gz', 'wb9');
 * @endcode
 *
 * @return resource|false
 */
function gzopen(string $filename, string $mode, bool $use_include_path = false) {}

/**
 * Reads a gzip-compressed file, decompresses it, and writes the result directly to the current
 * output buffer, the same way readfile() does for an uncompressed file. Returns the number of
 * uncompressed bytes written, or false on failure. $use_include_path, when true, searches
 * include_path for the file.
 */
function readgzfile(string $filename, bool $use_include_path = false): int|false {}

/**
 * Compresses $data and returns the compressed string, or false on failure. $encoding selects the
 * container format: ZLIB_ENCODING_RAW for a raw deflate stream, ZLIB_ENCODING_DEFLATE for a zlib-
 * wrapped deflate stream, or ZLIB_ENCODING_GZIP for a gzip-wrapped stream. $level ranges from 0, no
 * compression, to 9, maximum, or -1 to use zlib's default level.
 */
function zlib_encode(string $data, int $encoding, int $level = -1): string|false {}

/**
 * Decompresses $data produced by any of zlib's raw-deflate, zlib or gzip formats, detecting
 * automatically which one was used, and returns false if $data is not valid compressed data.
 * $max_length caps how much output is produced, with the default 0 meaning no limit.
 */
function zlib_decode(string $data, int $max_length = 0): string|false {}

/**
 * Compresses $data with DEFLATE and returns the compressed bytes, or false on failure. By default
 * ($encoding = ZLIB_ENCODING_RAW) the result carries no header or checksum, unlike gzcompress()
 * (zlib-wrapped) or gzencode() (gzip-wrapped); passing ZLIB_ENCODING_DEFLATE or ZLIB_ENCODING_GZIP
 * as $encoding produces those formats instead. $level ranges from 0 (no compression) to 9
 * (maximum), or -1 for the zlib library's own default.
 */
function gzdeflate(string $data, int $level = -1, int $encoding = ZLIB_ENCODING_RAW): string|false {}

/**
 * Compresses data into the gzip container format, complete with header, deflate-compressed body and
 * trailer, so the result can be decompressed by gzdecode() or by external tools such as gunzip.
 * $level sets the compression level from 0 (none) to 9 (maximum); the default, -1, uses zlib's own
 * default trade-off. Returns false on failure.
 */
function gzencode(string $data, int $level = -1, int $encoding = ZLIB_ENCODING_GZIP): string|false {}

/**
 * Compresses data with the DEFLATE algorithm at the given compression $level (0 = none, 9 =
 * maximum, -1 = zlib's own default), returning false on failure. $encoding selects the container
 * format: ZLIB_ENCODING_DEFLATE (the default, a zlib header), ZLIB_ENCODING_RAW (no header at all)
 * or ZLIB_ENCODING_GZIP (a gzip header).
 */
function gzcompress(string $data, int $level = -1, int $encoding = ZLIB_ENCODING_DEFLATE): string|false {}

/**
 * Decompresses data previously compressed with gzdeflate, that is, raw DEFLATE data with no zlib or
 * gzip header or checksum. max_length caps the size of the decompressed output in bytes, with 0
 * meaning no limit. Returns false if the data cannot be decompressed within that limit.
 */
function gzinflate(string $data, int $max_length = 0): string|false {}

/**
 * Decodes a string previously compressed with gzencode() -- gzip-format data complete with its
 * header and checksum -- returning the original data, or false on failure. max_length caps how many
 * bytes of decoded output are returned; zero means no limit.
 */
function gzdecode(string $data, int $max_length = 0): string|false {}

/**
 * Decompresses data that was compressed with gzcompress(), which uses the zlib format rather than
 * the raw deflate format read by gzinflate() or the gzip file format read by gzdecode(). Passing
 * max_length caps how many bytes of decompressed output are returned, with the default of 0 meaning
 * no limit. Returns false if data is not validly compressed in that format.
 */
function gzuncompress(string $data, int $max_length = 0): string|false {}

/**
 * Writes data to a gzip-compressed file stream opened with gzopen. When length is given, writing
 * stops after that many uncompressed bytes even if data is longer. Returns the number of bytes
 * written, or false on failure.
 *
 * @param resource $stream
 */
function gzwrite($stream, string $data, ?int $length = null): int|false {}

/**
 * Alias of gzwrite(): writes $data — or only its first $length bytes, if given — to the gzip-
 * compressed file stream $stream. Returns the number of bytes written, or false on failure.
 *
 * @param resource $stream
 */
function gzputs($stream, string $data, ?int $length = null): int|false {}

/**
 * Rewinds stream, a pointer opened with gzopen(), back to the beginning of the file. Returns false
 * on failure.
 *
 * @param resource $stream
 */
function gzrewind($stream): bool {}

/**
 * Closes the gzip file pointer $stream that was opened with gzopen(), the gzip-aware equivalent of
 * fclose().
 *
 * @param resource $stream
 */
function gzclose($stream): bool {}

/**
 * Reports whether the read/write position of a gzip file pointer opened with gzopen() has reached
 * end of file, the gzip-aware counterpart of feof().
 *
 * @param resource $stream
 */
function gzeof($stream): bool {}

/**
 * Reads and returns a single decompressed character from a gzip file pointer opened with gzopen(),
 * or false at end of file.
 *
 * @param resource $stream
 */
function gzgetc($stream): string|false {}

/**
 * Reads the given gzip stream to its end, writes the decompressed data directly to the current
 * output buffer, and closes the stream, returning the number of bytes written.
 *
 * @param resource $stream
 */
function gzpassthru($stream): int {}

/**
 * Moves the file pointer of a gzip stream opened by gzopen(), analogous to fseek(). SEEK_END is not
 * supported. On a stream opened for writing, only forward seeks are possible, emulated by writing
 * null bytes to pad the gap; on a stream opened for reading, backward seeks work but are
 * implemented by rewinding and re-reading from the start, which is slow for large offsets. Returns
 * 0 on success and -1 on failure.
 *
 * @param resource $stream
 */
function gzseek($stream, int $offset, int $whence = SEEK_SET): int {}

/**
 * Returns the current byte position of the gzip file pointer $stream, or false on error, the gzip-
 * aware equivalent of ftell.
 *
 * @param resource $stream
 */
function gztell($stream): int|false {}

/**
 * Reads up to $length bytes of uncompressed data from a gzip file pointer opened with gzopen(),
 * stopping earlier if the end of the file is reached. Returns false on error.
 *
 * @param resource $stream
 */
function gzread($stream, int $length): string|false {}

/**
 * Reads a single line, up to and including the newline or the end of the compressed data, from the
 * gzip-compressed file pointer stream, like fgets() but for gz streams. $length, if given, stops
 * after at most length - 1 bytes. Returns false at end of file or on error.
 *
 * @param resource $stream
 */
function gzgets($stream, ?int $length = null): string|false {}

/**
 * Initialises an incremental DEFLATE compression context for use with deflate_add, in the container
 * format given by $encoding (ZLIB_ENCODING_RAW, ZLIB_ENCODING_DEFLATE or ZLIB_ENCODING_GZIP).
 * $options can set a compression level, under the key 'level', and other zlib tuning parameters.
 * Returns false on failure.
 *
 * @code
 * deflate_init(ZLIB_ENCODING_GZIP, ['level' => 9]);
 * @endcode
 */
function deflate_init(int $encoding, array|object $options = []): DeflateContext|false {}

/**
 * Feeds another chunk of data through an incremental compression context created by deflate_init
 * and returns the compressed bytes produced for it. flush_mode controls whether the output is
 * immediately decodable, with a sync or full flush, or is the final block of the stream, with
 * ZLIB_FINISH.
 */
function deflate_add(DeflateContext $context, string $data, int $flush_mode = ZLIB_SYNC_FLUSH): string|false {}

/**
 * Initialises an incremental streaming decompression context for the given encoding, one of the
 * ZLIB_ENCODING_* constants, to be fed compressed data through inflate_add(). Returns false, with a
 * warning, if encoding or an entry in options is invalid.
 */
function inflate_init(int $encoding, array|object $options = []): InflateContext|false {}

/**
 * Feeds data into an incremental inflate context created by inflate_init(), returning whatever
 * additional decompressed output has become available. flush_mode controls whether output is forced
 * immediately, ZLIB_SYNC_FLUSH being the default, withheld until enough data has accumulated with
 * ZLIB_NO_FLUSH, or treated as the final chunk of the compressed stream with ZLIB_FINISH.
 */
function inflate_add(InflateContext $context, string $data, int $flush_mode = ZLIB_SYNC_FLUSH): string|false {}

/**
 * Returns the current status of an incremental inflate (decompression) context, most commonly
 * indicating that more input is expected or that the compressed stream has been fully decoded.
 */
function inflate_get_status(InflateContext $context): int {}

/**
 * Returns the number of compressed bytes consumed so far from the input fed to $context via
 * inflate_add().
 */
function inflate_get_read_len(InflateContext $context): int {}
