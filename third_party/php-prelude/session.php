<?php

interface SessionHandlerInterface
{
    /**
     * Closes the session, releasing whatever open() acquired. PHP calls it last in the save-handler
     * contract, after write() has stored the session data, either via session_write_close() or
     * automatically at the end of the request. Returns true on success, false on failure.
     */
    public function close(): bool;

    /**
     * Deletes the stored data for session $id. PHP calls it from session_destroy() and from
     * session_regenerate_id() when discarding the old session id. Returns true on success, false on
     * failure.
     */
    public function destroy(string $id): bool;

    /**
     * Deletes session data older than $max_lifetime seconds. PHP calls it only probabilistically,
     * according to session.gc_probability and session.gc_divisor, rather than on every request.
     * Returns the number of sessions deleted, or false on failure.
     */
    public function gc(int $max_lifetime): int|false;

    /**
     * Prepares the save handler for a session, given the save path $path and the session (cookie)
     * name $name. PHP calls it first in the save-handler contract, before read(). Returns true on
     * success, false on failure.
     */
    public function open(string $path, string $name): bool;

    /**
     * Reads the stored data for session $id. PHP calls it right after open() and feeds the returned
     * string through the configured session serializer to populate $_SESSION. Returns an empty
     * string if the session has no stored data yet, or false on failure.
     */
    public function read(string $id): string|false;

    /**
     * Stores $data, already encoded by the configured session serializer, for session $id. PHP
     * calls it near the end of the request, once $_SESSION's final state is known, typically just
     * before close(). Returns true on success, false on failure.
     */
    public function write(string $id, string $data): bool;
}

interface SessionIdInterface
{
    /**
     * Generates a new session id. PHP calls it whenever a fresh id is needed: at session start with
     * no existing id, or from session_regenerate_id(). The returned string must be a valid id under
     * the configured session.sid_length and session.sid_bits_per_character.
     */
    public function create_sid(): string;
}

interface SessionUpdateTimestampHandlerInterface
{
    /**
     * Reports whether $id refers to a session that already exists, without creating one. PHP calls
     * it before read(), and only when session.use_strict_mode is enabled, to decide whether to
     * accept a client-supplied id or generate a new one instead.
     */
    public function validateId(string $id): bool;

    /**
     * Updates the last-modified time recorded for session $id without necessarily rewriting $data.
     * PHP calls it in place of write() when session.lazy_write is enabled and the session's data
     * has not changed since it was read, avoiding an unnecessary rewrite. Returns true on success,
     * false on failure.
     */
    public function updateTimestamp(string $id, string $data): bool;
}

class SessionHandler implements SessionHandlerInterface, SessionIdInterface
{
    /**
     * Closes the session by delegating to PHP's built-in save handler (files, by default, or
     * whatever session.save_handler configures). Returns true on success, false on failure.
     */
    public function close(): bool {}

    /**
     * Generates a new session id using PHP's built-in id-generation algorithm, governed by
     * session.sid_length and session.sid_bits_per_character.
     */
    public function create_sid(): string {}

    /**
     * Deletes the stored data for session $id by delegating to PHP's built-in save handler. Returns
     * true on success, false on failure.
     */
    public function destroy(string $id): bool {}

    /**
     * Deletes session data older than $max_lifetime seconds by delegating to PHP's built-in save
     * handler. Returns the number of sessions deleted, or false on failure.
     */
    public function gc(int $max_lifetime): int|false {}

    /**
     * Opens the session, given the save path $path and session name $name, by delegating to PHP's
     * built-in save handler. Intended to be subclassed: override individual methods and call
     * parent:: for the ones left unchanged. Returns true on success, false on failure.
     */
    public function open(string $path, string $name): bool {}

    /**
     * Reads the stored data for session $id by delegating to PHP's built-in save handler. Returns
     * an empty string if there is nothing stored yet, or false on failure.
     */
    public function read(string $id): string|false {}

    /**
     * Stores $data for session $id by delegating to PHP's built-in save handler. Returns true on
     * success, false on failure.
     */
    public function write(string $id, string $data): bool {}
}

/**
 * Returns the name used for the session id (the cookie or query parameter name), and, if $name is
 * given, changes it and returns the previous name. It only has an effect if called before
 * session_start().
 */
function session_name(?string $name = null): string|false {}

/**
 * Gets or sets the session save handler module used by session_start(). Passing $module changes the
 * module for subsequent session starts and returns the name of the module that was previously in
 * effect; omitting it just returns the current module name. Returns false on failure.
 */
function session_module_name(?string $module = null): string|false {}

/**
 * Gets, and optionally sets, the directory PHP uses to store session data. A newly set path only
 * takes effect if this is called before the session has started.
 */
function session_save_path(?string $path = null): string|false {}

/**
 * Without an argument, returns the id of the current session, or an empty string if no session is
 * active. Passing $id sets the id to be used the next time a session is started or resumed; it has
 * no effect on a session that is already active.
 */
function session_id(?string $id = null): string|false {}

/**
 * Replaces the current session id with a newly generated one while keeping the session's data,
 * sending the corresponding session cookie. When $delete_old_session is true, the data stored under
 * the old id is destroyed as well; otherwise it is left behind. Commonly fails if headers have
 * already been sent.
 */
function session_regenerate_id(bool $delete_old_session = false): bool {}

/**
 * Registers session_write_close to run automatically when the script ends. This is what
 * session_start uses internally, so there is normally no need to call it directly.
 */
function session_register_shutdown(): void {}

/**
 * Decodes $data — session data in the format produced by session_encode(), following the
 * session.serialize_handler currently configured — and populates $_SESSION with its contents. Must
 * be called after session_start(). Returns false if $data cannot be decoded.
 */
function session_decode(string $data): bool {}

/**
 * Encodes the current contents of $_SESSION into the session save handler's storage format, as a
 * string. Returns false on failure.
 */
function session_encode(): string|false {}

/**
 * Starts a new session, or resumes the current one if a session cookie or session ID is already
 * present. The $options array can override session INI directives — such as cache_limiter or
 * read_and_close — for this call only, without changing the global ini settings. Returns false if
 * the session could not be started, for example because headers have already been sent.
 */
function session_start(array $options = []): bool {}

/**
 * Generates a new, valid session ID without altering the currently active session, optionally
 * prefixed with the given string. The prefix, if given, must consist of alphanumeric characters
 * only. Returns false on failure.
 */
function session_create_id(string $prefix = ''): string|false {}

/**
 * Immediately runs the session garbage collector, deleting session data older than the configured
 * lifetime, bypassing the usual session.gc_probability sampling. Returns the number of sessions
 * deleted, or false on failure.
 */
function session_gc(): int|false {}

/**
 * Destroys all data associated with the current session on the server, but leaves the $_SESSION
 * superglobal and the session cookie untouched, so both must be cleared separately to fully end the
 * session.
 */
function session_destroy(): bool {}

/**
 * Clears every variable currently held in the session data for the active session, without ending
 * the session itself or deleting its stored data, unlike session_destroy.
 */
function session_unset(): bool {}

/**
 * Registers the functions PHP calls to open, close, read, write, destroy and garbage-collect
 * session data, replacing the default file-based handler — for example, to store sessions in a
 * database. $open may instead be an object implementing SessionHandlerInterface (and optionally
 * SessionUpdateTimestampHandlerInterface, for $create_sid/$validate_sid/$update_timestamp), in
 * which case the remaining callables are taken from its methods and left unset. Returns false if
 * the handler could not be registered.
 */
function session_set_save_handler(
    object|callable $open,
    callable|bool $close = UNKNOWN,
    callable $read = UNKNOWN,
    callable $write = UNKNOWN,
    callable $destroy = UNKNOWN,
    callable $gc = UNKNOWN,
    ?callable $create_sid = null,
    ?callable $validate_sid = null,
    ?callable $update_timestamp = null,
): bool {}

/**
 * Gets, or when value is given, sets the cache limiter that controls the caching headers PHP sends
 * for pages using sessions, and returns the name now in effect. Recognised values are "nocache"
 * (the default), "public", "private" and "private_no_expire".
 */
function session_cache_limiter(?string $value = null): string|false {}

/**
 * Returns the session.cache_expire setting, in minutes, which controls how long a client may cache
 * a page containing session data. Passing $value updates the setting, but the change only affects
 * cache headers sent by subsequent requests, not the one already in progress.
 */
function session_cache_expire(?int $value = null): int|false {}

/**
 * Sets the parameters of the cookie that session_start() will send when it creates the session
 * cookie; it has no effect on a session that has already started. Accepts either a lifetime in
 * seconds as the first argument with the remaining parameters filled in individually, or a single
 * associative array of options (keys such as lifetime, path, domain, secure, httponly and samesite)
 * as the sole argument.
 *
 * @code
 * session_set_cookie_params(['lifetime' => 0, 'path' => '/', 'secure' => true, 'httponly' => true, 'samesite' => 'Strict']);
 * @endcode
 */
function session_set_cookie_params(
    array|int $lifetime_or_options,
    ?string $path = null,
    ?string $domain = null,
    ?bool $secure = null,
    ?bool $httponly = null,
): bool {}

/**
 * Returns the cookie parameters that session_start() will use for the session cookie, taken from
 * the session.cookie_* configuration directives.
 *
 * @return array{
 *  lifetime: int,
 *  path: string,
 *  domain: string,
 *  secure: bool,
 *  httponly: bool,
 *  samesite: string
 * }
 */
function session_get_cookie_params(): array {}

/**
 * Writes the current session data and closes the session, releasing its lock, while leaving
 * $_SESSION accessible for the rest of the request. Alias of session_commit(). Further changes to
 * $_SESSION are not persisted unless the session is reopened.
 */
function session_write_close(): bool {}

/**
 * Alias of session_write_close(): writes the current session data and ends the session, releasing
 * its lock so another request can open it.
 */
function session_commit(): bool {}

/**
 * Returns the current session state: PHP_SESSION_DISABLED if sessions are disabled entirely,
 * PHP_SESSION_NONE if sessions are enabled but none is currently active, or PHP_SESSION_ACTIVE if a
 * session has been started.
 */
function session_status(): int {}

/**
 * Ends the current session without saving the changes made to $_SESSION during this request,
 * discarding them instead of writing them to the session store as session_write_close() would.
 */
function session_abort(): bool {}

/**
 * Reloads $_SESSION from the session storage handler, discarding any changes made to it during the
 * current request.
 */
function session_reset(): bool {}
