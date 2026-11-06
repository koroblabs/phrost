<?php

class ZipArchive implements Countable
{
    public const LIBZIP_VERSION = '1.7.3';

    public const CREATE = 1;

    public const EXCL = 2;

    public const CHECKCONS = 4;

    public const OVERWRITE = 8;

    public const FL_NOCASE = 1;

    public const FL_NODIR = 2;

    public const FL_COMPRESSED = 4;

    public const FL_UNCHANGED = 8;

    public const FL_RECOMPRESS = 16;

    public const FL_ENCRYPTED = 32;

    public const FL_OVERWRITE = 8192;

    public const FL_LOCAL = 256;

    public const FL_CENTRAL = 512;

    public const EM_TRAD_PKWARE = 1;

    public const EM_UNKNOWN = 65535;

    public const CM_DEFAULT = -1;

    public const CM_STORE = 0;

    public const CM_SHRINK = 1;

    public const CM_REDUCE_1 = 2;

    public const CM_REDUCE_2 = 3;

    public const CM_REDUCE_3 = 4;

    public const CM_REDUCE_4 = 5;

    public const CM_IMPLODE = 6;

    public const CM_DEFLATE = 8;

    public const CM_DEFLATE64 = 9;

    public const CM_PKWARE_IMPLODE = 10;

    public const CM_BZIP2 = 12;

    public const CM_LZMA = 14;

    public const CM_TERSE = 18;

    public const CM_LZ77 = 19;

    public const CM_WAVPACK = 97;

    public const CM_PPMD = 98;

    public const ER_OK = 0;

    public const ER_MULTIDISK = 1;

    public const ER_RENAME = 2;

    public const ER_CLOSE = 3;

    public const ER_SEEK = 4;

    public const ER_READ = 5;

    public const ER_WRITE = 6;

    public const ER_CRC = 7;

    public const ER_ZIPCLOSED = 8;

    public const ER_NOENT = 9;

    public const ER_EXISTS = 10;

    public const ER_OPEN = 11;

    public const ER_TMPOPEN = 12;

    public const ER_ZLIB = 13;

    public const ER_MEMORY = 14;

    public const ER_CHANGED = 15;

    public const ER_COMPNOTSUPP = 16;

    public const ER_EOF = 17;

    public const ER_INVAL = 18;

    public const ER_NOZIP = 19;

    public const ER_INTERNAL = 20;

    public const ER_INCONS = 21;

    public const ER_REMOVE = 22;

    public const ER_DELETED = 23;

    public const EM_NONE = 0;

    public const EM_AES_128 = 257;

    public const EM_AES_192 = 258;

    public const EM_AES_256 = 259;

    public const RDONLY = 16;

    public const FL_ENC_GUESS = 0;

    public const FL_ENC_RAW = 64;

    public const FL_ENC_STRICT = 128;

    public const FL_ENC_UTF_8 = 2048;

    public const FL_ENC_CP437 = 4096;

    public const CM_LZMA2 = 33;

    public const CM_XZ = 95;

    public const ER_ENCRNOTSUPP = 24;

    public const ER_RDONLY = 25;

    public const ER_NOPASSWD = 26;

    public const ER_WRONGPASSWD = 27;

    public const ER_OPNOTSUPP = 28;

    public const ER_INUSE = 29;

    public const ER_TELL = 30;

    public const ER_COMPRESSED_DATA = 31;

    public const ER_CANCELLED = 32;

    public const OPSYS_DOS = 0;

    public const OPSYS_AMIGA = 1;

    public const OPSYS_OPENVMS = 2;

    public const OPSYS_UNIX = 3;

    public const OPSYS_VM_CMS = 4;

    public const OPSYS_ATARI_ST = 5;

    public const OPSYS_OS_2 = 6;

    public const OPSYS_MACINTOSH = 7;

    public const OPSYS_Z_SYSTEM = 8;

    public const OPSYS_WINDOWS_NTFS = 10;

    public const OPSYS_MVS = 11;

    public const OPSYS_VSE = 12;

    public const OPSYS_ACORN_RISC = 13;

    public const OPSYS_VFAT = 14;

    public const OPSYS_ALTERNATE_MVS = 15;

    public const OPSYS_BEOS = 16;

    public const OPSYS_TANDEM = 17;

    public const OPSYS_OS_400 = 18;

    public const OPSYS_OS_X = 19;

    public const OPSYS_CPM = 9;

    public const OPSYS_DEFAULT = 3;

    public const FL_OPEN_FILE_NOW = 1073741824;

    public const CM_ZSTD = 93;

    public const ER_DATA_LENGTH = 33;

    public const ER_NOT_ALLOWED = 34;

    public const AFL_RDONLY = 2;

    public const AFL_IS_TORRENTZIP = 4;

    public const AFL_WANT_TORRENTZIP = 8;

    public const AFL_CREATE_OR_KEEP_FILE_FOR_EMPTY_ARCHIVE = 16;

    public const LENGTH_TO_END = 0;

    public const LENGTH_UNCHECKED = -2;

    public int $status;

    public int $statusSys;

    public int $numFiles;

    public string $filename;

    public string $comment;

    public int $lastId;

    /**
     * Opens an existing zip file, or creates a new one, and binds it to this object for subsequent
     * reads and writes. Returns true on success, or one of the ER_* status codes as an int on
     * failure rather than false, so a failure check needs strict comparison or a look at
     * getStatusString() rather than a plain boolean test. flags is a bitmask of CREATE (create the
     * file if it does not already exist), EXCL (fail if it does), CHECKCONS (run extra consistency
     * checks on the archive and fail if they do not pass), OVERWRITE (always start from an empty
     * archive, discarding any existing content) and RDONLY (open without permitting any
     * modification).
     */
    public function open(string $filename, int $flags = null): int|bool {}

    /**
     * Writes every pending addition, deletion, rename and other change to the underlying file and
     * closes the archive. Nothing done by any other method on this class reaches disk until close()
     * is called and succeeds; calling it is what actually commits the session's work. Returns false
     * on failure, in which case getStatusString() describes what went wrong and none of the pending
     * changes can be assumed to have been applied.
     */
    public function close(): bool {}

    /**
     * Returns the number of entries currently in the archive, including any added or deleted in
     * this session that have not yet reached disk. Implements the Countable interface, so
     * count($archive) does the same thing.
     */
    public function count(): int {}

    /**
     * Returns a human-readable description of the result of the last operation performed on the
     * archive, for use after a method has returned false or an integer failure code.
     */
    public function getStatusString(): string {}

    /**
     * Adds a directory entry to the archive. Like every add* method, this only records the intent
     * in memory; the entry does not exist in the archive file until close() is called and succeeds.
     * Returns false if the directory could not be added, for instance because an entry with that
     * name already exists.
     */
    public function addEmptyDir(string $dirname, int $flags = 0): bool {}

    /**
     * Adds a new entry to the archive whose content is the given string, rather than the contents
     * of a file on disk. content is copied immediately, so changing the variable afterwards has no
     * effect, but the entry itself is not written into the archive file until close() succeeds.
     * flags defaults to FL_OVERWRITE, so calling this again with a name already present in the
     * archive silently replaces that entry instead of failing.
     */
    public function addFromString(string $name, string $content, int $flags = 8192): bool {}

    /**
     * Adds a file from the filesystem to the archive under entryname, or under filepath itself if
     * entryname is omitted — so a path passed without an explicit entryname is stored inside the
     * archive complete with its own directory components. The file's bytes are not actually read
     * until close() runs, so deleting, truncating or overwriting filepath at any point before
     * close() succeeds silently produces an empty or wrong entry, with nothing to indicate why.
     * flags defaults to FL_OVERWRITE, replacing any existing entry of the same name rather than
     * failing.
     *
     * @code
     * $zip->addFile('/tmp/upload.csv', 'data.csv');
     * unlink('/tmp/upload.csv');
     * $zip->close(); // data.csv is now empty -- the file was gone before close() read it
     * @endcode
     */
    public function addFile(
        string $filepath,
        string $entryname = null,
        int $start = 0,
        int $length = 0,
        int $flags = 8192,
    ): bool {}

    /**
     * Adds every file matching a shell glob pattern to the archive in one call. options recognises
     * add_path (a prefix prepended to each match's name inside the archive), remove_path (a prefix
     * stripped from each matched filesystem path before it becomes the entry name) and
     * remove_all_path (a boolean that strips all directory components, storing every match flat).
     * Returns an array of the entry names that were added, or false if the pattern matched nothing
     * or a match could not be added.
     */
    public function addGlob(string $pattern, int $flags = 0, array $options = []): array|false {}

    /**
     * Adds every file under path whose name matches a PCRE regular expression, walking the
     * directory tree rooted at path. Recognises the same add_path, remove_path and remove_all_path
     * options as addGlob. Returns an array of the entry names added, or false on failure.
     */
    public function addPattern(string $pattern, string $path = '.', array $options = []): array|false {}

    /**
     * Renames the entry at the given zero-based index. Like every other change, the rename is only
     * recorded in memory and takes effect once close() succeeds.
     */
    public function renameIndex(int $index, string $new_name): bool {}

    /**
     * Renames an existing entry identified by its current name. Only takes effect once close()
     * succeeds.
     */
    public function renameName(string $name, string $new_name): bool {}

    /**
     * Sets the comment stored for the archive as a whole, distinct from any comment attached to an
     * individual entry. Only takes effect once close() succeeds.
     */
    public function setArchiveComment(string $comment): bool {}

    /**
     * Returns the archive-level comment, or false if none is set. FL_UNCHANGED returns the comment
     * as it stood before any change made in this session, ignoring a comment set earlier in the
     * same session by setArchiveComment().
     */
    public function getArchiveComment(int $flags = null): string|false {}

    /**
     * Sets the comment for the entry at the given zero-based index. Only takes effect once close()
     * succeeds.
     */
    public function setCommentIndex(int $index, string $comment): bool {}

    /**
     * Sets the comment for the entry with the given name. Only takes effect once close() succeeds.
     */
    public function setCommentName(string $name, string $comment): bool {}

    /**
     * Changes the compression method used for the entry at the given zero-based index, the next
     * time the archive is written. method is one of the CM_* constants: CM_STORE stores the data
     * with no compression at all, CM_DEFLATE is the ordinary method every zip reader supports, and
     * the rest, such as CM_BZIP2, trade narrower support for a better ratio. Only takes effect once
     * close() succeeds.
     */
    public function setCompressionIndex(int $index, int $method, int $compflags = 0): bool {}

    /**
     * Same as setCompressionIndex, but identifies the entry by name rather than by index.
     */
    public function setCompressionName(string $name, int $method, int $compflags = 0): bool {}

    /**
     * Encrypts the entry at the given zero-based index with the given method, the next time the
     * archive is written. method is one of the EM_* constants: EM_NONE removes encryption,
     * EM_AES_128, EM_AES_192 and EM_AES_256 are the methods worth using, and EM_TRAD_PKWARE is the
     * zip format's original, cryptographically weak scheme, kept only for compatibility with old
     * tools. password, if given, applies to this entry alone; otherwise the password most recently
     * passed to setPassword() is used. Only takes effect once close() succeeds.
     */
    public function setEncryptionIndex(int $index, int $method, ?string $password = null): bool {}

    /**
     * Same as setEncryptionIndex, but identifies the entry by name rather than by index.
     */
    public function setEncryptionName(string $name, int $method, ?string $password = null): bool {}

    /**
     * Sets the password used to decrypt entries when reading from an archive that is already open —
     * getFromName(), getFromIndex(), getStream() and extractTo() all rely on it for an encrypted
     * entry. It has no effect on writing: encrypting an entry when the archive is next closed is
     * controlled separately, by setEncryptionName() or setEncryptionIndex().
     */
    public function setPassword(string $password): bool {}

    /**
     * Returns the comment for the entry at the given zero-based index, or false if it has none.
     * FL_UNCHANGED reads the comment as it stood before any change made in this session.
     */
    public function getCommentIndex(int $index, int $flags = null): string|false {}

    /**
     * Returns the comment for the entry with the given name, or false if it has none. FL_UNCHANGED
     * reads the comment as it stood before any change made in this session.
     */
    public function getCommentName(string $name, int $flags = null): string|false {}

    /**
     * Marks the entry at the given zero-based index for deletion. The entry is only actually
     * removed from the archive file once close() succeeds.
     */
    public function deleteIndex(int $index): bool {}

    /**
     * Marks the entry with the given name for deletion. The entry is only actually removed from the
     * archive file once close() succeeds.
     */
    public function deleteName(string $name): bool {}

    /**
     * Looks up an entry by name and returns its metadata, or false if no entry has that name. index
     * is the entry's zero-based position within the archive; size and comp_size are the
     * uncompressed and stored sizes in bytes; crc is the CRC-32 checksum of the uncompressed data;
     * mtime is a Unix timestamp; comp_method and encryption_method are the CM_* and EM_* constants
     * describing how the entry is stored, with encryption_method equal to EM_NONE for an entry that
     * is not encrypted. FL_NOCASE matches the name case-insensitively and FL_NODIR ignores the
     * directory part of the stored name when comparing; FL_UNCHANGED reports the entry as it stood
     * before any change made in this session, rather than reflecting a pending rename or delete.
     *
     * @return array{name: string, index: int, crc: int, size: int, mtime: int, comp_size: int, comp_method: int, encryption_method: int}|false
     */
    public function statName(string $name, int $flags = null): array|false {}

    /**
     * Looks up the entry at the given zero-based index and returns the same metadata shape as
     * statName -- name, index, crc, size, mtime, comp_size, comp_method and encryption_method -- or
     * false if the index does not exist. FL_UNCHANGED reports the entry as it stood before any
     * change made in this session.
     */
    public function statIndex(int $index, int $flags = null): array|false {}

    /**
     * Returns the zero-based index of the entry with the given name, or false if no entry matches.
     * FL_NOCASE matches case-insensitively and FL_NODIR ignores the directory part of the stored
     * name when comparing.
     */
    public function locateName(string $name, int $flags = null): int|false {}

    /**
     * Returns the name of the entry at the given zero-based index, or false if the index does not
     * exist. FL_UNCHANGED returns the name as it stood before any pending rename made earlier in
     * this session, rather than a new name that has not yet been written.
     */
    public function getNameIndex(int $index, int $flags = null): string|false {}

    /**
     * Reverts any pending change to the archive-level comment, restoring it to what it was when the
     * archive was opened. It does not affect a pending entry-level change such as an addition,
     * deletion or rename.
     */
    public function unchangeArchive(): bool {}

    /**
     * Reverts every pending change made in this session -- added, deleted, renamed and modified
     * entries, and the archive comment -- restoring the archive to the state it was in when opened.
     * Anything already committed by an earlier close() is unaffected.
     */
    public function unchangeAll(): bool {}

    /**
     * Reverts any pending change to the entry at the given zero-based index -- a rename, a comment,
     * compression or encryption change, or a deletion -- restoring it to the state it was in when
     * the archive was opened.
     */
    public function unchangeIndex(int $index): bool {}

    /**
     * Reverts any pending change to the entry with the given name, restoring it to the state it was
     * in when the archive was opened.
     */
    public function unchangeName(string $name): bool {}

    /**
     * Extracts one or more entries onto disk under pathto, recreating whatever directory structure
     * their names describe. pathto, and any directories the extracted entries need, are created if
     * they do not already exist. Returns false if any part of the extraction fails.
     *
     * @param array<string>|string|null $files
     */
    public function extractTo(string $pathto, array|string|null $files = null): bool {}

    /**
     * Returns the uncompressed content of the entry with the given name as a string, or false if no
     * such entry exists or it could not be read. len caps the number of bytes read, with 0 meaning
     * read the whole entry. FL_COMPRESSED returns the entry's raw, still-compressed bytes instead
     * of decompressing it; FL_UNCHANGED reads the entry as it stood before any change made in this
     * session.
     */
    public function getFromName(string $name, int $len = 0, int $flags = null): string|false {}

    /**
     * Returns the uncompressed content of the entry at the given zero-based index as a string, or
     * false if the index does not exist or it could not be read. len caps the number of bytes read,
     * with 0 meaning read the whole entry. FL_COMPRESSED returns the raw, still-compressed bytes;
     * FL_UNCHANGED reads the entry as it stood before any change made in this session.
     */
    public function getFromIndex(int $index, int $len = 0, int $flags = null): string|false {}

    /**
     * Opens the entry with the given name for reading and returns a stream resource, or false on
     * failure, so its content can be read incrementally with fread() and the rest of the stream API
     * instead of being loaded into memory all at once. The returned stream is read-only.
     *
     * @return resource|false
     */
    public function getStream(string $name) {}

    /**
     * Same as getStream, but identifies the entry by its zero-based index rather than by name.
     *
     * @return resource|false
     */
    public function getStreamIndex(int $index, int $flags = 0) {}

    /**
     * Sets the platform-specific external file attributes stored for the entry with the given name.
     * For opsys equal to OPSYS_UNIX this is where Unix permission bits live, occupying attr shifted
     * sixteen bits to the left, so a mode of 0644 is passed as 0644 shifted left by 16. Only takes
     * effect once close() succeeds.
     *
     * @code
     * $zip->setExternalAttributesName('run.sh', ZipArchive::OPSYS_UNIX, 0100755 << 16);
     * @endcode
     */
    public function setExternalAttributesName(string $name, int $opsys, int $attr, int $flags = null): bool {}

    /**
     * Reads the platform-specific external file attributes stored for the entry with the given
     * name, writing the operating system identifier into opsys and the raw attribute value into
     * attr. Returns false if the entry does not exist.
     */
    public function getExternalAttributesName(string $name, int &$opsys, int &$attr, int $flags = null): bool {}

    /**
     * Same as setExternalAttributesName, but identifies the entry by its zero-based index rather
     * than by name.
     */
    public function setExternalAttributesIndex(int $index, int $opsys, int $attr, int $flags = null): bool {}

    /**
     * Same as getExternalAttributesName, but identifies the entry by its zero-based index rather
     * than by name.
     */
    public function getExternalAttributesIndex(int $index, int &$opsys, int &$attr, int $flags = null): bool {}

    /**
     * Reports whether the underlying zip library can handle the given EM_* encryption method at all
     * on this build. enc selects whether the check is for encoding, that is writing, which is the
     * default, or decoding, that is reading -- a method this build can read is not guaranteed to be
     * one it can also write.
     */
    public static function isEncryptionMethodSupported(int $method, bool $enc = true): bool {}

    /**
     * Reports whether the underlying zip library can handle the given CM_* compression method at
     * all on this build. enc selects whether the check is for encoding, that is writing, which is
     * the default, or decoding, that is reading.
     */
    public static function isCompressionMethodSupported(int $method, bool $enc = true): bool {}

    /**
     * Registers a callback that close() invokes repeatedly while it writes the archive; returning a
     * truthy value from the callback aborts the write in progress. Returns false if the callback
     * could not be registered.
     */
    public function registerCancelCallback(callable $callback): bool {}

    /**
     * Registers a callback that close() invokes repeatedly while it writes the archive, passing the
     * current progress as a float between 0 and 1. rate is the minimum change in progress, also
     * between 0 and 1, that must accumulate before the callback fires again, so a smaller rate
     * means more frequent, finer-grained calls.
     */
    public function registerProgressCallback(float $rate, callable $callback): bool {}

    /**
     * Sets the modification time stored for the entry with the given name, as a Unix timestamp.
     * Only takes effect once close() succeeds.
     */
    public function setMtimeName(string $name, int $timestamp, int $flags = null): bool {}

    /**
     * Sets the modification time stored for the entry at the given zero-based index, as a Unix
     * timestamp. Only takes effect once close() succeeds.
     */
    public function setMtimeIndex(int $index, int $timestamp, int $flags = null): bool {}

    /**
     * Replaces the content of the existing entry at the given zero-based index with the content of
     * a file from the filesystem, the same way addFile adds a new one. As with addFile, the
     * replacement file's bytes are not actually read until close() succeeds, so removing or
     * changing filepath beforehand leaves the entry empty or wrong with nothing to indicate why.
     */
    public function replaceFile(
        string $filepath,
        int $index,
        int $start = null,
        int $length = null,
        int $flags = null,
    ): bool {}

    /**
     * Resets the status and statusSys properties of the archive back to no error, without otherwise
     * changing the archive or any of its entries.
     */
    public function clearError(): void {}

    public function setArchiveFlag(int $flag, int $value): bool {}

    public function getArchiveFlag(int $flag, int $flags = 0): int {}

    /**
     * Behaves like getStream, returning a read-only stream resource for the entry with the given
     * name, but additionally accepts flags -- FL_COMPRESSED returns a stream over the entry's raw,
     * still-compressed bytes rather than decompressing it.
     */
    public function getStreamName(string $name, int $flags = 0) {}
}

/**
 * Opens a zip archive through the legacy procedural zip extension, returning a resource for use
 * with zip_read() and zip_close(). On failure it can return a negative int error code instead of
 * false, so checking against false alone misses that case.
 *
 * @code
 * $zip = zip_open($file);
 * if (is_int($zip)) { // an error code, not a resource
 * }
 * @endcode
 *
 * @return resource|int|false
 */
function zip_open(string $filename) {}

/**
 * Closes the zip archive resource opened by zip_open(). Part of the legacy read-only procedural zip
 * API; ZipArchive supersedes it and also supports writing.
 *
 * @param resource $zip
 */
function zip_close($zip): void {}

/**
 * Reads the next entry from a zip archive opened with zip_open, returning a resource representing
 * that entry for use with the zip_entry_* functions, or false once there are no more entries or on
 * error. Deprecated in favour of the ZipArchive class.
 *
 * @param resource $zip
 *
 * @return resource|false
 *
 * @deprecated
 */
function zip_read($zip) {}

/**
 * Opens a zip entry, previously obtained from zip_read, so its contents can be read with
 * zip_entry_read. Part of the older procedural zip extension API, superseded by ZipArchive.
 *
 * @param resource $zip_dp
 * @param resource $zip_entry
 *
 * @deprecated
 */
function zip_entry_open($zip_dp, $zip_entry, string $mode = 'rb'): bool {}

/**
 * Closes a zip entry resource previously opened with zip_entry_open(), releasing it. Part of the
 * older procedural zip functions, superseded by the ZipArchive class.
 *
 * @param resource $zip_entry
 */
function zip_entry_close($zip_entry): bool {}

/**
 * Reads up to len bytes of decompressed data from zip_entry, a resource previously opened with
 * zip_entry_open(), continuing from wherever the last read on that entry left off. Returns false on
 * error.
 *
 * @param resource $zip_entry
 */
function zip_entry_read($zip_entry, int $len = 1024): string|false {}

/**
 * Returns the uncompressed size of a zip entry opened with the legacy procedural zip_open/zip_read
 * API, as distinct from the ZipArchive class. Returns false on failure.
 *
 * @param resource $zip_entry
 */
function zip_entry_filesize($zip_entry): int|false {}

/**
 * Returns the path, within the archive, of the zip entry resource $zip_entry, from the older
 * procedural zip_open()/zip_read() API rather than ZipArchive. Returns false on failure.
 *
 * @param resource $zip_entry
 */
function zip_entry_name($zip_entry): string|false {}

/**
 * Returns the compressed size, in bytes, of zip_entry, a resource obtained from zip_read() in the
 * legacy procedural zip API. Returns false on error.
 *
 * @param resource $zip_entry
 */
function zip_entry_compressedsize($zip_entry): int|false {}

/**
 * Returns the name of the compression method used for the zip entry represented by $zip_entry, a
 * resource obtained from zip_read(), or false on failure.
 *
 * @param resource $zip_entry
 */
function zip_entry_compressionmethod($zip_entry): string|false {}
