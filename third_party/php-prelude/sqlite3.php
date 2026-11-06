<?php

/**
 * @var int
 */
const SQLITE3_ASSOC = UNKNOWN;

/**
 * @var int
 */
const SQLITE3_NUM = UNKNOWN;

/**
 * @var int
 */
const SQLITE3_BOTH = UNKNOWN;

/**
 * @var int
 */
const SQLITE3_INTEGER = UNKNOWN;

/**
 * @var int
 */
const SQLITE3_FLOAT = UNKNOWN;

/**
 * @var int
 */
const SQLITE3_TEXT = UNKNOWN;

/**
 * @var int
 */
const SQLITE3_BLOB = UNKNOWN;

/**
 * @var int
 */
const SQLITE3_NULL = UNKNOWN;

/**
 * @var int
 */
const SQLITE3_OPEN_READONLY = UNKNOWN;

/**
 * @var int
 */
const SQLITE3_OPEN_READWRITE = UNKNOWN;

/**
 * @var int
 */
const SQLITE3_OPEN_CREATE = UNKNOWN;

/**
 * @var int
 */
const SQLITE3_DETERMINISTIC = UNKNOWN;

#[Mago\AvailableSince(80300)]
class SQLite3Exception extends \Exception {}

class SQLite3
{
    public const int OK = UNKNOWN;
    public const int DENY = UNKNOWN;
    public const int IGNORE = UNKNOWN;
    public const int CREATE_INDEX = UNKNOWN;
    public const int CREATE_TABLE = UNKNOWN;
    public const int CREATE_TEMP_INDEX = UNKNOWN;
    public const int CREATE_TEMP_TABLE = UNKNOWN;
    public const int CREATE_TEMP_TRIGGER = UNKNOWN;
    public const int CREATE_TEMP_VIEW = UNKNOWN;
    public const int CREATE_TRIGGER = UNKNOWN;
    public const int CREATE_VIEW = UNKNOWN;
    public const int DELETE = UNKNOWN;
    public const int DROP_INDEX = UNKNOWN;
    public const int DROP_TABLE = UNKNOWN;
    public const int DROP_TEMP_INDEX = UNKNOWN;
    public const int DROP_TEMP_TABLE = UNKNOWN;
    public const int DROP_TEMP_TRIGGER = UNKNOWN;
    public const int DROP_TEMP_VIEW = UNKNOWN;
    public const int DROP_TRIGGER = UNKNOWN;
    public const int DROP_VIEW = UNKNOWN;
    public const int INSERT = UNKNOWN;
    public const int PRAGMA = UNKNOWN;
    public const int READ = UNKNOWN;
    public const int SELECT = UNKNOWN;
    public const int TRANSACTION = UNKNOWN;
    public const int UPDATE = UNKNOWN;
    public const int ATTACH = UNKNOWN;
    public const int DETACH = UNKNOWN;
    public const int ALTER_TABLE = UNKNOWN;
    public const int REINDEX = UNKNOWN;
    public const int ANALYZE = UNKNOWN;
    public const int CREATE_VTABLE = UNKNOWN;
    public const int DROP_VTABLE = UNKNOWN;
    public const int FUNCTION = UNKNOWN;
    public const int SAVEPOINT = UNKNOWN;
    public const int COPY = UNKNOWN;
    public const int RECURSIVE = UNKNOWN;

    public function __construct(
        string $filename,
        int $flags = SQLITE3_OPEN_READWRITE | SQLITE3_OPEN_CREATE,
        string $encryptionKey = '',
    ) {}

    public function open(
        string $filename,
        int $flags = SQLITE3_OPEN_READWRITE | SQLITE3_OPEN_CREATE,
        string $encryptionKey = '',
    ): void {}

    public function close(): bool {}

    public static function version(): array {}

    public function lastInsertRowID(): int {}

    public function lastErrorCode(): int {}

    public function lastExtendedErrorCode(): int {}

    public function lastErrorMsg(): string {}

    public function changes(): int {}

    public function busyTimeout(int $milliseconds): bool {}

    public function loadExtension(string $name): bool {}

    public function backup(
        SQLite3 $destination,
        string $sourceDatabase = 'main',
        string $destinationDatabase = 'main',
    ): bool {}

    public static function escapeString(string $string): string {}

    public function prepare(string $query): SQLite3Stmt|false {}

    public function exec(string $query): bool {}

    public function query(string $query): SQLite3Result|false {}

    public function querySingle(string $query, bool $entireRow = false): mixed {}

    public function createFunction(string $name, callable $callback, int $argCount = -1, int $flags = 0): bool {}

    public function createAggregate(
        string $name,
        callable $stepCallback,
        callable $finalCallback,
        int $argCount = -1,
    ): bool {}

    public function createCollation(string $name, callable $callback): bool {}

    /** @return resource|false */
    public function openBlob(
        string $table,
        string $column,
        int $rowid,
        string $database = 'main',
        int $flags = SQLITE3_OPEN_READONLY,
    ) {}

    /**
     * Chooses whether SQLite3 method calls report an error by throwing an SQLite3Exception, instead
     * of the default of returning false and leaving the error to be read with lastErrorCode and
     * lastErrorMsg.
     */
    public function enableExceptions(bool $enable = false): bool {}

    /**
     * Chooses whether errors are reported using SQLite's extended result codes, which distinguish
     * specific causes such as SQLITE_IOERR_READ, rather than only a primary code such as
     * SQLITE_IOERR. Affects what lastErrorCode subsequently returns.
     */
    public function enableExtendedResultCodes(bool $enable = true): bool {}

    /**
     * Registers a callback that is asked to approve, deny or silently ignore each action, such as
     * reading a given table or column, that a statement takes while being prepared, by returning
     * the corresponding SQLITE_OK, SQLITE_DENY or SQLITE_IGNORE constant. Passing null removes any
     * authorizer previously set.
     */
    public function setAuthorizer(?callable $callback): bool {}
}

class SQLite3Stmt
{
    /**
     * Builds a prepared statement for the given query against the given connection. The constructor
     * is private, so a statement is obtained by calling SQLite3::prepare rather than constructed
     * directly.
     */
    private function __construct(SQLite3 $sqlite3, string $query) {}

    /**
     * Binds a variable to a named or 1-based positional parameter by reference, so execute reads
     * whatever the variable holds at the time it runs rather than at the time of this call. Type is
     * one of SQLITE3_INTEGER, SQLITE3_FLOAT, SQLITE3_TEXT, SQLITE3_BLOB or SQLITE3_NULL and
     * controls how the value is stored.
     *
     * @code
     * $id = 1; $stmt->bindParam(':id', $id, SQLITE3_INTEGER); $id = 2; $stmt->execute(); // runs with 2
     * @endcode
     */
    public function bindParam(string|int $param, mixed &$var, int $type = SQLITE3_TEXT): bool {}

    /**
     * Binds a value to a named or 1-based positional parameter for the next execute call, copying
     * it immediately rather than watching a variable, so later changes to the original variable
     * have no effect. Type is the same SQLITE3_INTEGER, SQLITE3_FLOAT, SQLITE3_TEXT, SQLITE3_BLOB
     * or SQLITE3_NULL set used by bindParam.
     */
    public function bindValue(string|int $param, mixed $value, int $type = SQLITE3_TEXT): bool {}

    /**
     * Clears all values currently bound to the statement's parameters, leaving the statement itself
     * still prepared and ready for new bindings.
     */
    public function clear(): bool {}

    /**
     * Closes the statement and releases it. Always succeeds, and the statement cannot be executed
     * again afterwards.
     */
    public function close(): true {}

    /**
     * Runs the prepared statement with its currently bound parameter values, implicitly resetting
     * it first if it had already been executed, and returns an SQLite3Result cursor over the result
     * rows on success. Returns false on failure, or throws an SQLite3Exception instead if the
     * owning connection has enableExceptions(true) set.
     *
     * @throws SQLite3Exception
     */
    public function execute(): SQLite3Result|false {}

    /**
     * Returns the statement's SQL text, or false if it is not available. With expand true, bound
     * parameter placeholders in the returned string are substituted by their current values instead
     * of being left as placeholders.
     */
    public function getSQL(bool $expand = false): string|false {}

    /**
     * Returns the number of parameters the statement's SQL declares.
     */
    public function paramCount(): int {}

    /**
     * Returns true if the statement does not itself modify the database, such as a SELECT, and
     * false for statements such as INSERT, UPDATE or DELETE.
     */
    public function readOnly(): bool {}

    /**
     * Rewinds the statement so it can be executed again from the start, without discarding its
     * currently bound parameter values. Use clear to remove the bindings themselves.
     */
    public function reset(): bool {}

    /**
     * Returns true if execute has been called and the statement has not yet either run to
     * completion or been reset, false otherwise.
     */
    public function busy(): bool {}

    public const int EXPLAIN_MODE_PREPARED = 0;
    public const int EXPLAIN_MODE_EXPLAIN = 1;
    public const int EXPLAIN_MODE_EXPLAIN_QUERY_PLAN = 2;

    /**
     * Returns the statement's current explain mode: 0 for ordinary execution, 1 for EXPLAIN, which
     * makes execute return the underlying bytecode instructions instead of query results, or 2 for
     * EXPLAIN QUERY PLAN, which makes it return the query planner's chosen strategy instead.
     */
    public function explain(): int {}

    /**
     * Sets the statement's explain mode, changing what execute subsequently returns: 0 for ordinary
     * execution, 1 for EXPLAIN to return the underlying bytecode instructions instead of query
     * results, or 2 for EXPLAIN QUERY PLAN to return the query planner's chosen strategy instead.
     */
    public function setExplain(int $mode): bool {}
}

class SQLite3Result
{
    /**
     * Cannot be called directly; a SQLite3Result is instead handed back by SQLite3::query and
     * SQLite3Stmt::execute once a query has produced a result set.
     */
    private function __construct() {}

    /**
     * Returns the number of columns in the result set, which bounds the valid indices accepted by
     * columnName and columnType.
     */
    public function numColumns(): int {}

    /**
     * Returns the name of the column at the given zero-based index. Returns false if the index does
     * not correspond to a column in the result set.
     */
    public function columnName(int $column): string|false {}

    /**
     * Returns the storage type of the value held in the given zero-based column for the current
     * row, as one of SQLITE3_INTEGER, SQLITE3_FLOAT, SQLITE3_TEXT, SQLITE3_BLOB or SQLITE3_NULL.
     * SQLite columns are dynamically typed, so the same column can report a different type on a
     * different row. Returns false if the index does not correspond to a column in the result set.
     */
    public function columnType(int $column): int|false {}

    /**
     * Fetches the next row from the result set and advances the cursor. Returns false once every
     * row has been consumed, and also on failure, so the two cases cannot be told apart from the
     * return value alone. mode selects the shape of the returned row: SQLITE3_ASSOC keys it by
     * column name, SQLITE3_NUM keys it by zero-based column position, and SQLITE3_BOTH, the
     * default, includes both sets of keys in the one array.
     *
     * @code
     * while ($row = $result->fetchArray(SQLITE3_ASSOC)) {
     *     echo $row['name'];
     * }
     * @endcode
     */
    public function fetchArray(int $mode = SQLITE3_BOTH): array|false {}

    public function fetchAll(int $mode = SQLITE3_BOTH): array|false {}

    /**
     * Rewinds the result set's cursor back to the first row, so the next call to fetchArray returns
     * it again. Returns true on success and false on failure.
     */
    public function reset(): bool {}

    /**
     * Closes the result set and releases the resources associated with it. Always returns true.
     */
    public function finalize(): true {}
}
