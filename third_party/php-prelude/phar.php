<?php

class PharException extends Exception {}

class Phar extends RecursiveDirectoryIterator implements RecursiveIterator, SeekableIterator, Countable, ArrayAccess
{
    public const BZ2 = 8192;
    public const GZ = 4096;
    public const NONE = 0;
    public const PHAR = 1;
    public const TAR = 2;
    public const ZIP = 3;
    public const COMPRESSED = 61440;
    public const PHP = 0;
    public const PHPS = 1;
    public const MD5 = 1;
    public const OPENSSL = 16;
    public const SHA1 = 2;
    public const SHA256 = 3;
    public const SHA512 = 4;
    public const OPENSSL_SHA256 = 5;
    public const OPENSSL_SHA512 = 6;

    /**
     * Opens an existing phar, tar or zip archive at the given filesystem path, or creates a new
     * empty archive object if the file does not yet exist, registering it under alias or the alias
     * stored inside the file. The flags parameter is the standard
     * FilesystemIterator/DirectoryIterator iteration flags inherited from the parent class, not an
     * option that changes how the archive itself is read or written. Throws
     * UnexpectedValueException if the file cannot be opened, is corrupted, or fails signature
     * verification, and BadMethodCallException for invalid arguments such as an empty filename.
     *
     * @throws BadMethodCallException
     * @throws UnexpectedValueException
     */
    public function __construct(
        string $filename,
        int $flags = FilesystemIterator::SKIP_DOTS | FilesystemIterator::UNIX_PATHS,
        ?string $alias = null,
    ) {}

    /**
     * Destroys the Phar object and releases its resources; PHP invokes it automatically once the
     * object is no longer referenced.
     */
    public function __destruct() {}

    /**
     * Adds an empty directory entry at the given path inside the archive. Writes immediately unless
     * buffering is active (see startBuffering()), and throws a PharException instead of making any
     * change when the phar.readonly ini directive is enabled.
     *
     * @throws PharException
     */
    public function addEmptyDir(string $directory): void {}

    /**
     * Adds a file from the filesystem to the archive, stored under localName if given or under
     * filename itself otherwise. Writes immediately unless buffering is active (see
     * startBuffering()), and throws a PharException when phar.readonly is enabled.
     *
     * @throws PharException
     */
    public function addFile(string $filename, ?string $localName = null): void {}

    /**
     * Adds a file to the archive whose contents are taken directly from the given string rather
     * than read from disk, stored at localName inside the archive. Writes immediately unless
     * buffering is active (see startBuffering()), and throws a PharException when phar.readonly is
     * enabled.
     *
     * @throws PharException
     */
    public function addFromString(string $localName, string $contents): void {}

    /**
     * Recursively adds every file under directory to the archive, optionally restricted to names
     * matching the regular expression pattern, and returns an array reporting the outcome for each
     * file processed. Writes immediately unless buffering is active (see startBuffering()), and
     * throws a PharException when phar.readonly is enabled.
     *
     * @throws PharException
     */
    public function buildFromDirectory(string $directory, string $pattern = ''): array {}

    /**
     * Adds files to the archive from a Traversable that yields either SplFileInfo objects, in which
     * case baseDirectory is stripped from each path to form the in-archive name, or localName =>
     * filesystem-path pairs directly. Returns an array reporting the outcome for each file
     * processed. Writes immediately unless buffering is active (see startBuffering()), and throws a
     * PharException when phar.readonly is enabled.
     *
     * @throws PharException
     */
    public function buildFromIterator(Traversable $iterator, ?string $baseDirectory = null): array {}

    /**
     * Recompresses every file already stored in the archive individually, using compression of
     * Phar::GZ or Phar::BZ2, changing the existing archive in place rather than producing a new
     * file. Writes immediately unless buffering is active (see startBuffering()), and throws a
     * PharException when phar.readonly is enabled.
     *
     * @throws PharException
     */
    public function compressFiles(int $compression): void {}

    /**
     * Decompresses every file stored in the archive that was compressed individually, changing the
     * existing archive in place. Writes immediately unless buffering is active (see
     * startBuffering()), and throws a PharException when phar.readonly is enabled.
     *
     * @throws PharException
     */
    public function decompressFiles(): bool {}

    /**
     * Writes a new file containing the whole archive compressed with compression, Phar::GZ or
     * Phar::BZ2, alongside the original without modifying it, and returns a Phar object for the new
     * file or null on failure. Disabled, throwing a PharException, when phar.readonly is enabled.
     */
    public function compress(int $compression, ?string $extension = null): ?Phar {}

    /**
     * Writes a new, uncompressed copy of the whole archive alongside the original without modifying
     * it, and returns a Phar object for the new file or null on failure. Disabled, throwing a
     * PharException, when phar.readonly is enabled.
     */
    public function decompress(?string $extension = null): ?Phar {}

    /**
     * Writes a new executable archive in a different container format and/or compression alongside
     * the original without modifying it, and returns a Phar object for the new file or null on
     * failure. format is Phar::PHAR, Phar::TAR or Phar::ZIP, or Phar::SAME to keep the current
     * format; compression is Phar::NONE, Phar::GZ or Phar::BZ2, or Phar::SAME to keep the current
     * compression. Disabled, throwing a PharException, when phar.readonly is enabled.
     */
    public function convertToExecutable(
        ?int $format = null,
        ?int $compression = null,
        ?string $extension = null,
    ): ?Phar {}

    /**
     * Writes a new non-executable data archive, opened as PharData rather than Phar, in a different
     * container format and/or compression alongside the original without modifying it, and returns
     * a PharData object for the new file or null on failure. format and compression take the same
     * Phar::PHAR/TAR/ZIP, Phar::NONE/GZ/BZ2 and Phar::SAME constants as convertToExecutable().
     * Disabled, throwing a PharException, when phar.readonly is enabled.
     */
    public function convertToData(
        ?int $format = null,
        ?int $compression = null,
        ?string $extension = null,
    ): ?PharData {}

    /**
     * Copies a file already stored in the archive from one internal path to another; the
     * destination comes first and the source second, the reverse order of the built-in copy()
     * function. Writes immediately unless buffering is active (see startBuffering()), and throws a
     * PharException when phar.readonly is enabled.
     *
     * @code
     * $phar->copy('backup/app.php', 'app.php'); // destination, then source
     * @endcode
     *
     * @throws PharException
     */
    public function copy(string $to, string $from): bool {}

    /**
     * Counts the files at the current directory level of the archive, or every file at every level
     * beneath it when mode is COUNT_RECURSIVE.
     *
     * @return int<0, max>
     */
    public function count(int $mode = COUNT_NORMAL): int {}

    /**
     * Removes the named file from the archive. Writes immediately unless buffering is active (see
     * startBuffering()), and throws a PharException when phar.readonly is enabled.
     *
     * @throws PharException
     */
    public function delete(string $localName): bool {}

    /**
     * Removes the archive's global metadata. Writes immediately unless buffering is active (see
     * startBuffering()), and throws a PharException when phar.readonly is enabled.
     *
     * @throws PharException
     */
    public function delMetadata(): bool {}

    /**
     * Extracts files from the archive onto the real filesystem under directory: every file when
     * files is null, a single file when it is a string, or a list of internal paths when it is an
     * array. Existing files at the destination are left untouched unless overwrite is true. Throws
     * a PharException if the extraction cannot be completed.
     *
     * @throws PharException
     */
    public function extractTo(string $directory, array|string|null $files = null, bool $overwrite = false): bool {}

    /**
     * Returns the alias registered for the archive, used to address it as phar://alias/..., or null
     * if none is set.
     */
    public function getAlias(): ?string {}

    /**
     * Returns the archive's global metadata, unserialised, or null if none was set.
     * unserializeOptions is passed straight through to the underlying unserialize() call, so
     * options such as allowed_classes apply here too.
     */
    public function getMetadata(array $unserializeOptions = []): mixed {}

    public function getModified(): bool {}

    /**
     * Returns the archive's embedded signature as an array with hash and hash_type keys, or false
     * if the archive is unsigned.
     *
     * @return false|array{
     *   hash: string,
     *   hash_type: string
     * }
     */
    public function getSignature(): array|false {}

    /**
     * Returns the archive's stub, the block of PHP code that runs automatically when the phar file
     * is executed directly rather than opened for its contents.
     */
    public function getStub(): string {}

    /**
     * Returns the phar file-format version recorded in this archive's own manifest, such as
     * "1.1.1", not the version of any application packaged inside it. Compare with the static
     * Phar::apiVersion(), which reports what the running extension itself supports.
     */
    public function getVersion(): string {}

    /**
     * Returns whether the archive has global metadata set.
     */
    public function hasMetadata(): bool {}

    /**
     * Returns whether write operations are currently being buffered because of an open
     * startBuffering() call.
     */
    public function isBuffering(): bool {}

    /**
     * Returns Phar::GZ or Phar::BZ2 if the whole archive is compressed that way, or false if it is
     * not compressed as a whole; individual files inside it may still be compressed separately via
     * compressFiles().
     */
    public function isCompressed(): int|false {}

    /**
     * Returns whether the archive's container format matches format: Phar::PHAR for the native phar
     * format, Phar::TAR for a tar-based archive, or Phar::ZIP for a zip-based archive.
     */
    public function isFileFormat(int $format): bool {}

    /**
     * Returns whether the archive can currently be modified, taking into account both the
     * phar.readonly ini directive and the underlying file's own filesystem permissions.
     */
    public function isWritable(): bool {}

    /**
     * Returns whether a file exists in the archive at localName, backing isset($phar['name']) and
     * empty($phar['name']).
     *
     * @param string $localName
     */
    public function offsetExists($localName): bool {}

    /**
     * Returns a file-info object describing the file stored at localName inside the archive,
     * backing $phar['name'].
     *
     * @param string $localName
     */
    public function offsetGet($localName): SplFileInfo {}

    /**
     * Adds or replaces a file in the archive at localName with value as its contents, equivalent to
     * addFromString(), backing $phar['name'] = $value. Writes immediately unless buffering is
     * active (see startBuffering()), and throws a PharException when phar.readonly is enabled.
     *
     * @param string $localName
     * @param string $value
     *
     * @throws PharException
     */
    public function offsetSet($localName, $value): void {}

    /**
     * Removes the file at localName from the archive, equivalent to delete(), backing
     * unset($phar['name']). Writes immediately unless buffering is active (see startBuffering()),
     * and throws a PharException when phar.readonly is enabled.
     *
     * @param string $localName
     *
     * @throws PharException
     */
    public function offsetUnset($localName): void {}

    /**
     * Sets the alias stored inside the archive itself, used to address it as phar://alias/...
     * afterwards. Writes immediately unless buffering is active (see startBuffering()), and throws
     * a PharException when phar.readonly is enabled.
     *
     * @throws PharException
     */
    public function setAlias(string $alias): bool {}

    /**
     * Replaces the archive's stub with the standard loader stub, which includes index when run from
     * the command line or webIndex when served over the web, falling back to built-in defaults when
     * either is omitted. Writes immediately unless buffering is active (see startBuffering()), and
     * throws a PharException when phar.readonly is enabled.
     *
     * @throws PharException
     */
    public function setDefaultStub(?string $index = null, ?string $webIndex = null): bool {}

    /**
     * Sets the archive's global metadata, serialising metadata for storage. Writes immediately
     * unless buffering is active (see startBuffering()), and throws a PharException when
     * phar.readonly is enabled.
     *
     * @throws PharException
     */
    public function setMetadata(mixed $metadata): void {}

    /**
     * Sets the algorithm used to sign the archive; the OpenSSL-based algorithms require privateKey,
     * a PEM-encoded private key, and cannot be used without one. Writes immediately unless
     * buffering is active (see startBuffering()), and throws a PharException when phar.readonly is
     * enabled.
     *
     * @throws PharException
     */
    public function setSignatureAlgorithm(int $algo, ?string $privateKey = null): void {}

    /**
     * Replaces the archive's stub, the block of PHP code that runs automatically when the phar file
     * is executed directly. Writes immediately unless buffering is active (see startBuffering()),
     * and throws a PharException when phar.readonly is enabled.
     *
     * @param string $stub
     *
     * @throws PharException
     */
    public function setStub($stub, int $length = UNKNOWN): bool {}

    /**
     * Begins buffering every subsequent write to the archive, such as addFile(), addFromString(),
     * delete() and setMetadata(), instead of flushing each one to disk immediately. Without it,
     * each write is its own disk write, which is far slower when making many small changes.
     *
     * @code
     * $phar->startBuffering();
     * $phar->addFile('a.php');
     * $phar->addFile('b.php');
     * $phar->stopBuffering(); // one disk write instead of two
     * @endcode
     */
    public function startBuffering(): void {}

    /**
     * Ends buffering started by startBuffering() and flushes every change made since then to disk
     * in a single pass.
     */
    public function stopBuffering(): void {}

    /**
     * Returns the phar file-format API version implemented by the running extension, such as
     * "1.1.1", not the version of PHP or of any packaged application.
     */
    final public static function apiVersion(): string {}

    /**
     * Returns whether compression is supported: with no argument, whether any compression is
     * available at all; with Phar::GZ or Phar::BZ2, whether that specific algorithm's extension,
     * zlib or bz2, is loaded.
     */
    final public static function canCompress(int $compression = 0): bool {}

    /**
     * Returns whether writing phar archives is permitted at all, i.e. whether the phar.readonly ini
     * directive is disabled.
     */
    final public static function canWrite(): bool {}

    /**
     * Returns the standard loader stub as a string without needing an existing Phar object,
     * including index when run from the command line or webIndex when served over the web.
     */
    final public static function createDefaultStub(?string $index = null, ?string $webIndex = null): string {}

    /**
     * Returns the compression algorithm names supported by the running build, such as "GZ" or
     * "BZ2", depending on which of zlib and bz2 are loaded.
     *
     * @return list<non-empty-string>
     */
    final public static function getSupportedCompression(): array {}

    /**
     * Returns the signature and hash type names supported by the running build, such as "MD5",
     * "SHA-256" or "OpenSSL", depending on which extensions are loaded.
     *
     * @return list<non-empty-string>
     */
    final public static function getSupportedSignatures(): array {}

    /**
     * Makes fopen(), file_get_contents(), file_exists() and the rest of the filesystem functions
     * resolve relative paths against the currently running phar archive's own contents instead of
     * the real filesystem, for code inside the archive that was not written to be phar-aware.
     */
    final public static function interceptFileFuncs(): void {}

    /**
     * Returns whether filename is an acceptable name for a phar archive: when executable is true it
     * must carry an extension valid for an executable archive, such as .phar; when false,
     * extensions used for non-executable data archives are also accepted.
     */
    final public static function isValidPharFilename(string $filename, bool $executable = true): bool {}

    /**
     * Loads a phar archive file and maps it into memory under alias, or the alias stored inside it
     * if none is given, without running its stub, so its contents become reachable via phar://.
     * Returns whether it loaded successfully.
     */
    final public static function loadPhar(string $filename, ?string $alias = null): bool {}

    /**
     * Intended for use inside a running archive's own stub: maps the currently executing phar file
     * into memory under alias, or the alias stored inside it if none is given, treating offset as
     * the byte position where the manifest begins so a custom stub longer than the default is
     * skipped correctly.
     *
     * @code
     * Phar::mapPhar('myapp.phar');
     * include 'phar://myapp.phar/index.php';
     * @endcode
     */
    final public static function mapPhar(?string $alias = null, int $offset = 0): bool {}

    /**
     * Returns the currently executing phar archive's identity, or an empty string if the running
     * script is not inside one. When returnPhar is true this is the full phar://... path to the
     * running stub; when false it is just the archive's alias, or its path if it has none.
     */
    final public static function running(bool $returnPhar = true): string {}

    /**
     * Makes an external file or directory on the real filesystem reachable at externalPath under
     * the archive's own phar:// namespace, without copying it into the archive. Throws a
     * PharException if the mount cannot be created.
     *
     * @throws PharException
     */
    final public static function mount(string $pharPath, string $externalPath): void {}

    /**
     * Rewrites the listed $_SERVER variable names so that scripts inside a phar served over the web
     * see paths pointing inside the phar:// namespace instead of the real filesystem, letting path-
     * based routers keep working unmodified.
     */
    final public static function mungServer(array $variables): void {}

    /**
     * Deletes a phar archive file from disk entirely. Throws a PharException if the archive is
     * currently open or otherwise still in use.
     *
     * @throws PharException
     */
    final public static function unlinkArchive(string $filename): bool {}

    /**
     * Registers alias, or the archive's own alias if omitted, as a web application: requests are
     * served from index inside the archive by default, mimeTypes maps file extensions to content
     * types beyond the built-in defaults, fileNotFoundScript names a script to run inside the
     * archive when a requested path does not exist, and rewrite can remap the requested path before
     * it is resolved. Typically called at the top of the archive's own stub.
     */
    final public static function webPhar(
        ?string $alias = null,
        ?string $index = null,
        ?string $fileNotFoundScript = null,
        array $mimeTypes = [],
        ?callable $rewrite = null,
    ): void {}

    /**
     * Returns whether the current entry is a directory that can be recursed into with
     * getChildren(); allow_links controls whether a symbolic link to a directory counts.
     *
     * @param bool $allow_links
     *
     * @return bool
     */
    public function hasChildren($allow_links = false) {}

    /**
     * Returns an iterator over the current directory entry's contents, for recursing into
     * subdirectories of the archive.
     */
    public function getChildren() {}

    /**
     * Rewinds the iteration back to the first entry at the current directory level.
     *
     * @return void
     */
    public function rewind() {}

    /**
     * Advances the iteration to the next entry at the current directory level.
     *
     * @return void
     */
    public function next() {}

    /**
     * Returns the pathname of the current entry in the iteration.
     *
     * @return string
     */
    public function key() {}

    /**
     * Returns the current entry in the iteration.
     */
    public function current() {}

    /**
     * Returns whether the iteration's current position still points at an entry.
     *
     * @return bool
     */
    public function valid() {}

    /**
     * Moves the iteration directly to position within the current directory level.
     *
     * @param int $position
     *
     * @return void
     */
    public function seek($position) {}

    public function _bad_state_ex() {}
}

class PharData extends Phar
{
    /**
     * Opens the tar, zip or Phar archive at $filename, creating a new empty archive there if it
     * does not already exist. The format is guessed from the file extension unless $format
     * specifies Phar::TAR, Phar::ZIP or Phar::PHAR explicitly, and $flags controls how iteration
     * over the archive's entries behaves, as with FilesystemIterator's SKIP_DOTS and UNIX_PATHS
     * flags. Throws UnexpectedValueException if the file cannot be opened or created, or
     * PharException if its contents are corrupt.
     *
     * @throws UnexpectedValueException
     */
    public function __construct(
        string $filename,
        int $flags = FilesystemIterator::SKIP_DOTS | FilesystemIterator::UNIX_PATHS,
        ?string $alias = null,
        int $format = 0,
    ) {}

    /**
     * Reports whether a file exists at $localName within the archive.
     *
     * @param string $localName
     */
    public function offsetExists($localName): bool {}

    /**
     * Returns a PharFileInfo object (a subtype of SplFileInfo) representing the file at $localName
     * within the archive, giving access to its contents and metadata without extracting it.
     *
     * @param string $localName
     */
    public function offsetGet($localName): SplFileInfo {}

    /**
     * Adds a file to the archive at $localName, or overwrites it if one already exists, storing
     * $value as its literal contents. Fails if the archive cannot be modified.
     *
     * @code
     * $archive['data/readme.txt'] = 'contents';
     * @endcode
     *
     * @param string $localName
     * @param string $value
     */
    public function offsetSet($localName, $value): void {}

    /**
     * Removes the file at $localName from the archive. Fails if the archive cannot be modified.
     *
     * @param string $localName
     */
    public function offsetUnset($localName): void {}

    /**
     * Reports whether the entry at the current iterator position is itself a directory, so a
     * recursive iterator can descend into it. When $allow_links is true, a symbolic link to a
     * directory also counts.
     *
     * @param bool $allow_links
     *
     * @return bool
     */
    public function hasChildren($allow_links = false) {}

    /**
     * Returns a new iterator over the contents of the directory at the current iterator position,
     * for use by RecursiveIteratorIterator.
     */
    public function getChildren() {}

    /**
     * Resets iteration to the first entry in the current directory listing.
     */
    public function rewind() {}

    /**
     * Advances the iterator to the next entry in the current directory listing.
     */
    public function next() {}

    /**
     * Returns the filename of the entry at the current iterator position.
     *
     * @return string
     */
    public function key() {}

    /**
     * Returns a PharFileInfo object for the entry at the current iterator position.
     */
    public function current() {}

    /**
     * Reports whether the current iterator position refers to an existing entry.
     *
     * @return bool
     */
    public function valid() {}

    /**
     * Moves the iterator directly to the entry at the given zero-based $position within the current
     * directory listing, without visiting the entries in between.
     *
     * @param int $position
     *
     * @return void
     */
    public function seek($position) {}
}

class PharFileInfo extends SplFileInfo
{
    /**
     * Constructs a PharFileInfo for the archive entry identified by $filename, given as a phar://
     * stream path into an already-open Phar or PharData archive.
     */
    public function __construct(string $filename) {}

    /**
     * Releases the PharFileInfo object.
     */
    public function __destruct() {}

    /**
     * Sets the Unix permission bits recorded for this entry within the archive.
     */
    public function chmod(int $perms): void {}

    /**
     * Compresses this individual file entry within the archive using the given algorithm, Phar::GZ
     * or Phar::BZ2, returning true on success. This does not apply to entries within a tar-format
     * archive, where compression can only be applied to the whole archive via PharData::compress().
     * Fails if the archive cannot be modified.
     */
    public function compress(int $compression): bool {}

    /**
     * Removes Phar::GZ or Phar::BZ2 compression from this individual file entry, returning true on
     * success. As with compress(), this does not apply to entries within a tar-format archive.
     */
    public function decompress(): bool {}

    /**
     * Removes any metadata stored for this file entry, returning true whether or not metadata was
     * present. Fails if the archive cannot be modified.
     */
    public function delMetadata(): bool {}

    /**
     * Returns the size, in bytes, this entry occupies within the archive after compression, which
     * equals its uncompressed size if the entry is stored uncompressed.
     */
    public function getCompressedSize(): int {}

    /**
     * Returns the CRC32 checksum recorded for this file entry in the archive.
     */
    public function getCRC32(): int {}

    /**
     * Returns the entire uncompressed contents of this file as a string, without extracting it to
     * disk.
     */
    public function getContent(): string {}

    /**
     * Returns the metadata previously stored for this file entry with setMetadata(), or null if
     * none was set. $unserializeOptions is passed through to unserialize() when the stored metadata
     * needs deserialising, so it can restrict which classes may be constructed.
     */
    public function getMetadata(array $unserializeOptions = []): mixed {}

    public function getPharFlags(): int {}

    /**
     * Reports whether this file entry has metadata set.
     */
    public function hasMetadata(): bool {}

    /**
     * Reports whether this entry is stored compressed within the archive. When $compression is
     * given, as Phar::GZ or Phar::BZ2, reports whether it is compressed with that specific
     * algorithm rather than compression in general.
     */
    public function isCompressed(?int $compression = null): bool {}

    /**
     * Reports whether this entry's stored CRC32 checksum has already been verified against its
     * contents.
     */
    public function isCRCChecked(): bool {}

    /**
     * Stores arbitrary $metadata alongside this file entry within the archive, replacing any
     * previous value, so it can later be retrieved with getMetadata(). Fails if the archive cannot
     * be modified.
     */
    public function setMetadata(mixed $metadata): void {}
}
