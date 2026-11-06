<?php

class finfo
{
    /**
     * Opens a fileinfo instance, optionally against a specific magic database file
     * ($magic_database, or the default database when null), and records $flags as the default
     * detection flags used by later file() and buffer() calls unless they are given their own.
     */
    public function __construct(int $flags = 0, ?string $magic_database = null) {}

    /**
     * Changes the default detection flags used by later file() and buffer() calls on this instance.
     * Returns true on success, false on failure.
     */
    public function set_flags(int $flags): bool {}

    /**
     * Determines the requested information about $filename -- by default a human-readable
     * description, or the MIME type and/or encoding when $flags includes FILEINFO_MIME_TYPE,
     * FILEINFO_MIME_ENCODING, or both combined via FILEINFO_MIME -- reading only as much of the
     * file as detection needs rather than the whole content. $context is a stream context, relevant
     * only when $filename is accessed through a stream wrapper. Returns false on failure, for
     * example if the file cannot be opened.
     *
     * @param null|resource $context
     *
     * @pure
     */
    public function file(string $filename, int $flags = FILEINFO_NONE, mixed $context = null): string|false {}

    /**
     * Behaves like file(), but examines the in-memory string $string directly instead of a path on
     * disk, useful for identifying content that has not been written to a file. Returns false on
     * failure.
     *
     * @param null|resource $context
     *
     * @pure
     */
    public function buffer(string $string, int $flags = FILEINFO_NONE, mixed $context = null): string|false {}
}

function finfo_open(int $flags = 0, ?string $magic_database = null): finfo|false {}

#[Deprecated(since: '8.5', message: 'as finfo objects are freed automatically')]
function finfo_close(finfo $finfo): bool {}

function finfo_set_flags(finfo $finfo, int $flags): bool {}

/**
 * Returns fileinfo's (libmagic's) description of the given file's contents, or false on failure.
 * The $flags argument changes what is returned: FILEINFO_MIME_TYPE returns only the MIME type,
 * FILEINFO_MIME_ENCODING returns only the character encoding, and FILEINFO_MIME returns both
 * combined.
 *
 * @code
 * finfo_file($finfo, $path, FILEINFO_MIME_TYPE);
 * @endcode
 *
 * @param finfo $finfo
 * @param null|resource $context
 */
function finfo_file(finfo $finfo, string $filename, int $flags = 0, mixed $context = null): string|false {}

/**
 * Examines the given string buffer's content and returns a description of its type, using the rules
 * loaded into the given finfo instance, in the style of the Unix file command. flags accepts the
 * same FILEINFO_* constants as finfo::file, such as FILEINFO_MIME to return the MIME type together
 * with the encoding, or FILEINFO_MIME_TYPE to return the MIME type alone. Returns false on failure.
 *
 * @param finfo $finfo
 * @param null|resource $context
 */
function finfo_buffer(finfo $finfo, string $string, int $flags = FILEINFO_NONE, mixed $context = null): string|false {}

/**
 * Detects a file's MIME type by inspecting its actual content rather than its name or extension.
 * filename may be a path, a stream resource, or an object exposing one. Returns false if the type
 * cannot be determined.
 *
 * @param resource|object|string $filename
 */
function mime_content_type($filename): string|false {}

const FILEINFO_NONE = 0;

const FILEINFO_SYMLINK = 2;

const FILEINFO_MIME = 1040;

const FILEINFO_MIME_TYPE = 16;

const FILEINFO_MIME_ENCODING = 1024;

const FILEINFO_DEVICES = 8;

const FILEINFO_CONTINUE = 32;

const FILEINFO_PRESERVE_ATIME = 128;

const FILEINFO_RAW = 256;

const FILEINFO_EXTENSION = 2097152;

const FILEINFO_APPLE = 2048;
