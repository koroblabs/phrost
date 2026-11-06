<?php

final class HashContext
{
    /**
     * Private, so a HashContext cannot be created with new; instances only come from hash_init().
     */
    private function __construct() {}

    /**
     * Always throws, because a hash context holds internal engine state with no serializable
     * representation; exists to give a clear failure instead of letting default object
     * serialization run against state that cannot be represented.
     */
    public function __serialize(): array {}

    /**
     * Always throws, for the same reason as __serialize(): a hash context cannot be rebuilt from a
     * plain array of data, so restoring one from serialized form is rejected outright.
     */
    public function __unserialize(array $data): void {}

    public function __debugInfo(): array {}
}

/**
 * Returns a hash of $data computed with the named $algo algorithm, as a lowercase hex string by
 * default or raw binary when $binary is true.
 *
 * @pure
 * @return non-falsy-string
 */
function hash(string $algo, string $data, bool $binary = false, array $options = []): string {}

/**
 * Compares two strings for equality in constant time, to avoid leaking information through timing.
 * Intended for comparing secrets such as hashes or tokens, not for general string comparison.
 *
 * @pure
 */
function hash_equals(string $known_string, string $user_string): bool {}

/**
 * Computes a hash of a file's contents using the given algorithm. Returns false if the file cannot
 * be read, and raw binary bytes instead of a hex string when binary is true.
 *
 * @pure
 * @return non-falsy-string|false
 */
function hash_file(string $algo, string $filename, bool $binary = false, array $options = []): string|false {}

/**
 * Computes a keyed-hash message authentication code (HMAC) of data using the given algorithm and
 * secret key. Returns the digest as a hex string, or as raw binary when the binary flag is true.
 *
 * @pure
 * @return non-falsy-string
 */
function hash_hmac(string $algo, string $data, string $key, bool $binary = false): string {}

/**
 * Computes a keyed HMAC digest of a file's contents using the named hashing algorithm and key,
 * without reading the whole file into memory first. Returns the digest as a lowercase hex string by
 * default, or as raw binary when $binary is true; returns false if the file cannot be read or the
 * algorithm is unknown.
 *
 * @pure
 * @return non-falsy-string|false
 */
function hash_hmac_file(string $algo, string $filename, string $key, bool $binary = false): string|false {}

/**
 * Creates an incremental hashing context for the given algorithm, to be fed with hash_update() and
 * finished with hash_final(). Setting $flags to HASH_HMAC turns it into a keyed HMAC context, in
 * which case $key is required and must not be empty.
 *
 * @pure
 */
function hash_init(string $algo, int $flags = 0, string $key = '', array $options = []): HashContext {}

/**
 * Feeds another chunk of $data into the incremental hashing operation represented by $context,
 * which was created by hash_init(). Mutates $context so that a later hash_final() reflects all the
 * chunks fed to it so far.
 */
function hash_update(HashContext $context, string $data): bool {}

/**
 * Reads up to $length bytes from $stream, or until end of file when $length is left at its default
 * -1, feeding them into the running hash held by $context. Returns the number of bytes actually
 * read and hashed.
 *
 * @param resource $stream
 */
function hash_update_stream(HashContext $context, $stream, int $length = -1): int {}

/**
 * Feeds the contents of a file into a hashing context previously created with hash_init(), reading
 * it incrementally rather than loading it into memory at once. $stream_context allows the file to
 * be opened through a stream wrapper, for instance when $filename is a URL. Returns false if the
 * file cannot be opened or read.
 *
 * @param ?resource $stream_context
 */
function hash_update_file(HashContext $context, string $filename, $stream_context = null): bool {}

/**
 * Returns the finalised digest of a hashing context previously fed with hash_update(), and consumes
 * the context so it cannot be updated further. $binary selects raw binary output when true, or
 * lowercase hexadecimal digits (the default) when false.
 */
function hash_final(HashContext $context, bool $binary = false): string {}

/**
 * Copies the internal state of a hashing context into a new, independent HashContext, so hashing
 * can be continued separately from that point in both the original and the copy.
 *
 * @pure
 */
function hash_copy(HashContext $context): HashContext {}

/**
 * Returns the names of all hashing algorithms supported by the hash functions, such as md5, sha256
 * or crc32b.
 *
 * @return non-empty-list<non-empty-string>
 * @pure
 */
function hash_algos(): array {}

/**
 * Derives a cryptographic key of length bytes from key using HKDF with the hashing algorithm named
 * by algo, mixed with an optional application-specific info string and salt. A length of zero uses
 * algo's own output size. Throws a ValueError if key is empty, algo names an unknown or non-
 * cryptographic hash, or length is negative or exceeds 255 times algo's output size.
 *
 * @param non-empty-string $key
 * @param int<0, max> $length
 *
 * @pure
 *
 * @throws ValueError
 */
function hash_hkdf(string $algo, string $key, int $length = 0, string $info = '', string $salt = ''): string {}

/**
 * Returns the names of the hashing algorithms that can be used with hash_hmac(), a subset of the
 * full list returned by hash_algos().
 *
 * @return non-empty-list<non-empty-string>
 *
 * @pure
 */
function hash_hmac_algos(): array {}

/**
 * Derives a key of the requested length from a password and salt using PBKDF2 with the given
 * hashing algorithm and number of iterations. A length of 0 uses the underlying algorithm's own
 * output size. Returns the raw binary key when binary is true, otherwise a lowercase hexadecimal
 * string twice as long.
 *
 * @param int<1, max> $iterations
 * @param int<0, max> $length
 *
 * @pure
 */
function hash_pbkdf2(
    string $algo,
    string $password,
    string $salt,
    int $iterations,
    int $length = 0,
    bool $binary = false,
    array $options = [],
): string {}

/**
 * Derives a key of exactly $length bytes from $password and $salt using the given hash algorithm,
 * following the salted S2K (string-to-key) construction historically used by OpenPGP-style tools.
 * It exists only for compatibility with the old, deprecated mhash extension; hash_pbkdf2() is the
 * modern replacement for deriving keys from a password. Returns false on failure, such as an
 * unknown $algo.
 *
 * @param int<0, max> $length
 *
 * @deprecated
 * @pure
 */
function mhash_keygen_s2k(int $algo, string $password, string $salt, int $length): string|false {}

/**
 * Returns the output size in bytes of the hashing algorithm named by an MHASH_* constant, or false
 * if the algorithm is not recognised. Deprecated in favour of hash() from the hash extension, which
 * the mhash functions now wrap.
 *
 * @deprecated
 * @pure
 */
function mhash_get_block_size(int $algo): int|false {}

/**
 * Returns the name of the hashing algorithm identified by the legacy MHASH_ constant $algo, or
 * false if $algo does not correspond to a known algorithm.
 *
 * @return string|false
 *
 * @deprecated
 * @pure
 */
function mhash_get_hash_name(int $algo): string|false {}

/**
 * Returns the highest hash algorithm ID accepted by the deprecated mhash API; valid IDs run from 0
 * up to and including this value and can be passed to mhash_get_hash_name() and related functions.
 *
 * @return int<0, max>
 *
 * @deprecated
 * @pure
 */
function mhash_count(): int {}

/**
 * Computes a hash of $data using one of the legacy MHASH_* algorithm constants, or a keyed HMAC
 * when $key is supplied. Superseded by hash() and hash_hmac(); returns false for an unrecognised
 * algorithm.
 *
 * @deprecated
 * @pure
 */
function mhash(int $algo, string $data, ?string $key = null): string|false {}

const HASH_HMAC = 1;

const MHASH_CRC32 = 0;

const MHASH_CRC32C = 34;

const MHASH_MD5 = 1;

const MHASH_SHA1 = 2;

const MHASH_HAVAL256 = 3;

const MHASH_RIPEMD160 = 5;

const MHASH_TIGER = 7;

const MHASH_GOST = 8;

const MHASH_CRC32B = 9;

const MHASH_HAVAL224 = 10;

const MHASH_HAVAL192 = 11;

const MHASH_HAVAL160 = 12;

const MHASH_HAVAL128 = 13;

const MHASH_TIGER128 = 14;

const MHASH_TIGER160 = 15;

const MHASH_MD4 = 16;

const MHASH_SHA256 = 17;

const MHASH_ADLER32 = 18;

const MHASH_SHA224 = 19;

const MHASH_SHA512 = 20;

const MHASH_SHA384 = 21;

const MHASH_WHIRLPOOL = 22;

const MHASH_RIPEMD128 = 23;

const MHASH_RIPEMD256 = 24;

const MHASH_RIPEMD320 = 25;

const MHASH_SNEFRU256 = 27;

const MHASH_MD2 = 28;

const MHASH_FNV132 = 29;

const MHASH_FNV1A32 = 30;

const MHASH_FNV164 = 31;

const MHASH_FNV1A64 = 32;

const MHASH_JOAAT = 33;

const MHASH_MURMUR3A = 35;

const MHASH_MURMUR3C = 36;

const MHASH_MURMUR3F = 37;

const MHASH_XXH32 = 38;

const MHASH_XXH64 = 39;

const MHASH_XXH3 = 40;

const MHASH_XXH128 = 41;
