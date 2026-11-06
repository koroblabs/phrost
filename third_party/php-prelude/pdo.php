<?php

namespace {
    class PDOException extends RuntimeException
    {
        public ?array $errorInfo;
        protected $code;
    }

    class PDO
    {
        public const PARAM_NULL = 0;

        public const PARAM_INT = 1;

        public const PARAM_STR = 2;

        public const PARAM_LOB = 3;

        public const PARAM_STMT = 4;

        public const PARAM_BOOL = 5;

        public const PARAM_STR_NATL = 1073741824;

        public const PARAM_STR_CHAR = 536870912;

        public const ATTR_DEFAULT_STR_PARAM = 21;

        public const SQLITE_DETERMINISTIC = 2048;

        public const SQLITE_OPEN_READONLY = 1;

        public const SQLITE_OPEN_READWRITE = 2;

        public const SQLITE_OPEN_CREATE = 4;

        public const SQLITE_ATTR_OPEN_FLAGS = 1000;

        public const PARAM_INPUT_OUTPUT = 2147483648;

        public const PARAM_EVT_ALLOC = 0;

        public const PARAM_EVT_FREE = 1;

        public const PARAM_EVT_EXEC_PRE = 2;

        public const PARAM_EVT_EXEC_POST = 3;

        public const PARAM_EVT_FETCH_PRE = 4;

        public const PARAM_EVT_FETCH_POST = 5;

        public const PARAM_EVT_NORMALIZE = 6;

        public const FETCH_LAZY = 1;

        public const FETCH_ASSOC = 2;

        public const FETCH_NUM = 3;

        public const FETCH_BOTH = 4;

        public const FETCH_OBJ = 5;

        public const FETCH_BOUND = 6;

        public const FETCH_COLUMN = 7;

        public const FETCH_CLASS = 8;

        public const FETCH_INTO = 9;

        public const FETCH_FUNC = 10;

        public const FETCH_GROUP = 65536;

        public const FETCH_UNIQUE = 196608;

        public const FETCH_KEY_PAIR = 12;

        public const FETCH_CLASSTYPE = 262144;

        public const FETCH_SERIALIZE = 524288;

        public const FETCH_PROPS_LATE = 1048576;

        public const FETCH_NAMED = 11;

        public const ATTR_AUTOCOMMIT = 0;

        public const ATTR_PREFETCH = 1;

        public const ATTR_TIMEOUT = 2;

        public const ATTR_ERRMODE = 3;

        public const ATTR_SERVER_VERSION = 4;

        public const ATTR_CLIENT_VERSION = 5;

        public const ATTR_SERVER_INFO = 6;

        public const ATTR_CONNECTION_STATUS = 7;

        public const ATTR_CASE = 8;

        public const ATTR_CURSOR_NAME = 9;

        public const ATTR_CURSOR = 10;

        public const ATTR_ORACLE_NULLS = 11;

        public const ATTR_PERSISTENT = 12;

        public const ATTR_STATEMENT_CLASS = 13;

        public const ATTR_FETCH_TABLE_NAMES = 14;

        public const ATTR_FETCH_CATALOG_NAMES = 15;

        public const ATTR_DRIVER_NAME = 16;

        public const ATTR_STRINGIFY_FETCHES = 17;

        public const ATTR_MAX_COLUMN_LEN = 18;

        public const ATTR_EMULATE_PREPARES = 20;

        public const ATTR_DEFAULT_FETCH_MODE = 19;

        public const ERRMODE_SILENT = 0;

        public const ERRMODE_WARNING = 1;

        public const ERRMODE_EXCEPTION = 2;

        public const CASE_NATURAL = 0;

        public const CASE_LOWER = 2;

        public const CASE_UPPER = 1;

        public const NULL_NATURAL = 0;

        public const NULL_EMPTY_STRING = 1;

        public const NULL_TO_STRING = 2;

        public const ERR_NONE = '00000';

        public const FETCH_ORI_NEXT = 0;

        public const FETCH_ORI_PRIOR = 1;

        public const FETCH_ORI_FIRST = 2;

        public const FETCH_ORI_LAST = 3;

        public const FETCH_ORI_ABS = 4;

        public const FETCH_ORI_REL = 5;

        public const FETCH_DEFAULT = 0;

        public const CURSOR_FWDONLY = 0;

        public const CURSOR_SCROLL = 1;

        public const MYSQL_ATTR_USE_BUFFERED_QUERY = 1000;

        public const MYSQL_ATTR_LOCAL_INFILE = 1001;

        public const MYSQL_ATTR_INIT_COMMAND = 1002;

        public const MYSQL_ATTR_MAX_BUFFER_SIZE = 1005;

        public const MYSQL_ATTR_READ_DEFAULT_FILE = 1003;

        public const MYSQL_ATTR_READ_DEFAULT_GROUP = 1004;

        public const MYSQL_ATTR_COMPRESS = 1003;

        public const MYSQL_ATTR_DIRECT_QUERY = 1004;

        public const MYSQL_ATTR_FOUND_ROWS = 1005;

        public const MYSQL_ATTR_IGNORE_SPACE = 1006;

        public const MYSQL_ATTR_SERVER_PUBLIC_KEY = 1012;

        public const MYSQL_ATTR_SSL_KEY = 1007;

        public const MYSQL_ATTR_SSL_CERT = 1008;

        public const MYSQL_ATTR_SSL_CA = 1009;

        public const MYSQL_ATTR_SSL_CAPATH = 1010;

        public const MYSQL_ATTR_SSL_CIPHER = 1011;

        public const MYSQL_ATTR_MULTI_STATEMENTS = 1013;

        public const MYSQL_ATTR_SSL_VERIFY_SERVER_CERT = 1014;

        public const MYSQL_ATTR_LOCAL_INFILE_DIRECTORY = 1015;

        public const PGSQL_ASSOC = 1;

        public const PGSQL_ATTR_DISABLE_PREPARES = 1000;

        public const PGSQL_BAD_RESPONSE = 5;

        public const PGSQL_BOTH = 3;

        public const PGSQL_TRANSACTION_IDLE = 0;

        public const PGSQL_TRANSACTION_ACTIVE = 1;

        public const PGSQL_TRANSACTION_INTRANS = 2;

        public const PGSQL_TRANSACTION_INERROR = 3;

        public const PGSQL_TRANSACTION_UNKNOWN = 4;

        public const PGSQL_CONNECT_ASYNC = 4;

        public const PGSQL_CONNECT_FORCE_NEW = 2;

        public const PGSQL_CONNECTION_AUTH_OK = 5;

        public const PGSQL_CONNECTION_AWAITING_RESPONSE = 4;

        public const PGSQL_CONNECTION_BAD = 1;

        public const PGSQL_CONNECTION_OK = 0;

        public const PGSQL_CONNECTION_MADE = 3;

        public const PGSQL_CONNECTION_SETENV = 6;

        public const PGSQL_CONNECTION_SSL_STARTUP = 7;

        public const PGSQL_CONNECTION_STARTED = 2;

        public const PGSQL_COMMAND_OK = 1;

        public const PGSQL_CONV_FORCE_NULL = 4;

        public const PGSQL_CONV_IGNORE_DEFAULT = 2;

        public const PGSQL_CONV_IGNORE_NOT_NULL = 8;

        public const PGSQL_COPY_IN = 4;

        public const PGSQL_COPY_OUT = 3;

        public const PGSQL_DIAG_CONTEXT = 87;

        public const PGSQL_DIAG_INTERNAL_POSITION = 112;

        public const PGSQL_DIAG_INTERNAL_QUERY = 113;

        public const PGSQL_DIAG_MESSAGE_DETAIL = 68;

        public const PGSQL_DIAG_MESSAGE_HINT = 72;

        public const PGSQL_DIAG_MESSAGE_PRIMARY = 77;

        public const PGSQL_DIAG_SEVERITY = 83;

        public const PGSQL_DIAG_SOURCE_FILE = 70;

        public const PGSQL_DIAG_SOURCE_FUNCTION = 82;

        public const PGSQL_DIAG_SOURCE_LINE = 76;

        public const PGSQL_DIAG_SQLSTATE = 67;

        public const PGSQL_DIAG_STATEMENT_POSITION = 80;

        public const PGSQL_DML_ASYNC = 1024;

        public const PGSQL_DML_EXEC = 512;

        public const PGSQL_DML_NO_CONV = 256;

        public const PGSQL_DML_STRING = 2048;

        public const PGSQL_DML_ESCAPE = 4096;

        public const PGSQL_EMPTY_QUERY = 0;

        public const PGSQL_ERRORS_DEFAULT = 1;

        public const PGSQL_ERRORS_TERSE = 0;

        public const PGSQL_ERRORS_VERBOSE = 2;

        public const PGSQL_FATAL_ERROR = 7;

        public const PGSQL_NONFATAL_ERROR = 6;

        public const PGSQL_NOTICE_ALL = 2;

        public const PGSQL_NOTICE_CLEAR = 3;

        public const PGSQL_NOTICE_LAST = 1;

        public const PGSQL_NUM = 2;

        public const PGSQL_POLLING_ACTIVE = 4;

        public const PGSQL_POLLING_FAILED = 0;

        public const PGSQL_POLLING_OK = 3;

        public const PGSQL_POLLING_READING = 1;

        public const PGSQL_POLLING_WRITING = 2;

        public const PGSQL_SEEK_CUR = 1;

        public const PGSQL_SEEK_END = 2;

        public const PGSQL_SEEK_SET = 0;

        public const PGSQL_STATUS_LONG = 1;

        public const PGSQL_STATUS_STRING = 2;

        public const PGSQL_TUPLES_OK = 2;

        public const SQLSRV_TXN_READ_UNCOMMITTED = 'READ_UNCOMMITTED';

        public const SQLSRV_TXN_READ_COMMITTED = 'READ_COMMITTED';

        public const SQLSRV_TXN_REPEATABLE_READ = 'REPEATABLE_READ';

        public const SQLSRV_TXN_SNAPSHOT = 'SNAPSHOT';

        public const SQLSRV_TXN_SERIALIZABLE = 'SERIALIZABLE';

        public const SQLSRV_ENCODING_BINARY = 2;

        public const SQLSRV_ENCODING_SYSTEM = 3;

        public const SQLSRV_ENCODING_UTF8 = 65001;

        public const SQLSRV_ENCODING_DEFAULT = 1;

        public const SQLSRV_ATTR_ENCODING = 1000;

        public const SQLSRV_ATTR_QUERY_TIMEOUT = 1001;

        public const SQLSRV_ATTR_DIRECT_QUERY = 1002;

        public const SQLSRV_ATTR_CURSOR_SCROLL_TYPE = 1003;

        public const SQLSRV_ATTR_CLIENT_BUFFER_MAX_KB_SIZE = 1004;

        public const SQLSRV_ATTR_FETCHES_NUMERIC_TYPE = 1005;

        public const SQLSRV_ATTR_FETCHES_DATETIME_TYPE = 1006;

        public const SQLSRV_ATTR_FORMAT_DECIMALS = 1007;

        public const SQLSRV_ATTR_DECIMAL_PLACES = 1008;

        public const SQLSRV_ATTR_DATA_CLASSIFICATION = 1009;

        public const SQLSRV_PARAM_OUT_DEFAULT_SIZE = -1;

        public const SQLSRV_CURSOR_KEYSET = 1;

        public const SQLSRV_CURSOR_DYNAMIC = 2;

        public const SQLSRV_CURSOR_STATIC = 3;

        public const SQLSRV_CURSOR_BUFFERED = 42;

        public const SQLITE_ATTR_READONLY_STATEMENT = 1001;

        public const SQLITE_ATTR_EXTENDED_RESULT_CODES = 1002;

        public const OCI_ATTR_ACTION = 1000;

        public const OCI_ATTR_CLIENT_INFO = 1001;

        public const OCI_ATTR_CLIENT_IDENTIFIER = 1002;

        public const OCI_ATTR_MODULE = 1003;

        public const OCI_ATTR_CALL_TIMEOUT = 1004;

        public const FB_ATTR_DATE_FORMAT = 1000;

        public const FB_ATTR_TIME_FORMAT = 1001;

        public const FB_ATTR_TIMESTAMP_FORMAT = 1002;

        /**
         * Opens a new database connection using the given data source name (DSN) and, for drivers
         * that need them, a username and password. Throws PDOException if the connection cannot be
         * established, and, since PHP 8.0, on any later error too, because PDO::ATTR_ERRMODE now
         * defaults to PDO::ERRMODE_EXCEPTION rather than the historic silent mode.
         *
         * @throws PDOException
         */
        public function __construct(
            string $dsn,
            ?string $username = null,
            ?string $password = null,
            ?array $options = null,
        ) {}

        /**
         * Prepares an SQL statement containing positional (?) or named (:name) placeholders for
         * later execution, without running it yet, and returns it as a PDOStatement. Returns false
         * on failure instead of throwing, unless the error mode is PDO::ERRMODE_EXCEPTION, which
         * has been the default since PHP 8.0.
         *
         * @throws PDOException
         */
        public function prepare(string $query, array $options = []): PDOStatement|false {}

        /**
         * Turns off autocommit and starts a transaction, so statements run after this are not
         * committed until commit() or rollBack() is called. Throws PDOException if a transaction is
         * already active.
         *
         * @throws PDOException
         */
        public function beginTransaction(): bool {}

        /**
         * Commits the current transaction and restores autocommit mode. Throws PDOException if no
         * transaction is currently active.
         *
         * @throws PDOException
         */
        public function commit(): bool {}

        /**
         * Rolls back the current transaction and restores autocommit mode. Throws PDOException if
         * no transaction is currently active.
         *
         * @throws PDOException
         */
        public function rollBack(): bool {}

        /**
         * Reports whether a transaction, started by beginTransaction(), is currently active on this
         * connection.
         */
        #[TentativeType]
        public function inTransaction(): bool {}

        /**
         * Sets a driver-specific or PDO-level attribute on the connection, such as
         * PDO::ATTR_ERRMODE to change the error-reporting mode or PDO::ATTR_DEFAULT_FETCH_MODE to
         * change the fetch mode new statements use by default. Returns false, rather than throwing,
         * if the driver does not support the given attribute.
         *
         * @throws PDOException
         */
        #[TentativeType]
        public function setAttribute(int $attribute, mixed $value): bool {}

        /**
         * Executes an SQL statement that produces no result set, such as INSERT, UPDATE or DELETE,
         * and returns the number of rows affected. A successful statement with no notion of
         * affected rows, such as CREATE TABLE, returns 0, which is distinct from the false returned
         * on failure. Returns false rather than throwing unless the error mode is
         * PDO::ERRMODE_EXCEPTION.
         *
         * @throws PDOException
         */
        public function exec(string $statement): int|false {}

        /**
         * Prepares and immediately executes an SQL statement in one call, for a statement that does
         * not need to bind parameters or run more than once, and returns the resulting
         * PDOStatement. When $fetchMode and $fetchModeArgs are given, they set the returned
         * statement's default fetch mode, the same as calling setFetchMode() on it afterwards.
         * Returns false rather than throwing unless the error mode is PDO::ERRMODE_EXCEPTION.
         * Because the statement text has no placeholders here, values from outside the code should
         * go through PDO::prepare() and bound parameters instead of being concatenated into the
         * query string.
         *
         * @return PDOStatement|false
         *
         * @throws PDOException
         */
        public function query(string $query, ?int $fetchMode = null, mixed ...$fetchModeArgs) {}

        /**
         * Returns the value generated for an auto-increment column by the last INSERT on this
         * connection, or, for drivers such as PostgreSQL that generate ids from a named sequence,
         * the last value from the sequence named by $name. Some drivers require $name to be given
         * explicitly to return anything meaningful. Returns false, rather than throwing, if the
         * driver does not support the operation.
         *
         * @throws PDOException
         */
        #[TentativeType]
        public function lastInsertId(?string $name = null): string|false {}

        /**
         * Returns the SQLSTATE code for the last operation attempted on this connection, or null if
         * no operation has been attempted yet. The same code is also carried by any PDOException
         * thrown, so this is mainly useful when the error mode is not set to throw.
         */
        #[TentativeType]
        public function errorCode(): ?string {}

        /**
         * Returns extended information about the last operation on this connection as a three-
         * element array: index 0 is the SQLSTATE code, index 1 is the driver-specific error code,
         * and index 2 is the driver-specific error message. When the last operation succeeded,
         * index 0 is the successful code '00000' and indices 1 and 2 are null.
         *
         * @return array{0: string, 1: int, 2: string}
         */
        #[TentativeType]
        public function errorInfo(): array {}

        /**
         * Returns the current value of a driver-specific or PDO-level attribute on this connection,
         * such as PDO::ATTR_SERVER_VERSION or PDO::ATTR_CLIENT_VERSION.
         *
         * @throws PDOException
         */
        #[TentativeType]
        public function getAttribute(int $attribute): mixed {}

        /**
         * Wraps $string in driver-appropriate quotes and escapes characters within it that would
         * otherwise terminate or corrupt an SQL string literal, producing a value safe to embed
         * directly in an SQL statement. $type hints at the value's SQL type, which some drivers use
         * to change how it is quoted. Returns false if the driver does not support quoting. Not for
         * quoting identifiers such as table or column names, and prepared statements with bound
         * parameters remain the safer way to include variable values in a query.
         */
        #[TentativeType]
        public function quote(string $string, int $type = PDO::PARAM_STR): string|false {}

        /**
         * Called automatically when a serialised PDO object is unserialised. PDO connections cannot
         * meaningfully be serialised, so this throws instead of restoring a working connection.
         */
        final public function __wakeup() {}

        /**
         * Called automatically by serialize() when it encounters a PDO object. PDO connections
         * cannot meaningfully be serialised, so this throws instead of returning the properties to
         * save.
         */
        final public function __sleep() {}

        /**
         * Returns the names of the PDO drivers compiled into the current PHP build, such as 'mysql'
         * or 'sqlite', regardless of whether a working server is actually reachable for any of
         * them. Returns an empty array if none are available.
         */
        #[TentativeType]
        public static function getAvailableDrivers(): array {}

        /**
         * Registers a user-defined aggregate SQL function, usable only when the underlying driver
         * is SQLite. $step_func is called once for every row contributing to the aggregate and
         * $finalize_func is called once at the end to compute the final result. $num_args restricts
         * how many arguments the SQL function accepts, or -1 to accept any number. Returns true on
         * success.
         *
         * @param string $function_name
         * @param callable $step_func
         * @param callable $finalize_func
         * @param int $num_args
         *
         * @return bool
         */
        public function sqliteCreateAggregate($function_name, $step_func, $finalize_func, $num_args = -1) {}

        /**
         * Registers a user-defined collation for use in SQL ORDER BY clauses and comparisons,
         * usable only when the underlying driver is SQLite. $callback receives two strings and must
         * return a negative, zero or positive integer, the same convention as strcmp().
         *
         * @param string $name
         * @param callable $callback
         *
         * @return bool
         */
        public function sqliteCreateCollation($name, $callback) {}

        /**
         * Registers a user-defined scalar SQL function, usable only when the underlying driver is
         * SQLite. $num_args restricts how many arguments the SQL function accepts, or -1 to accept
         * any number. $flags can mark the function as deterministic, meaning it always returns the
         * same output for the same input, which lets SQLite optimise queries that use it.
         *
         * @param string $function_name
         * @param callable $callback
         * @param int $num_args
         * @param int $flags
         *
         * @return bool
         */
        public function sqliteCreateFunction($function_name, $callback, $num_args = -1, $flags = 0) {}

        /**
         * Loads rows from $rows into $tableName using PostgreSQL's COPY FROM mechanism, the same
         * operation as \copy in psql, usable only when the underlying driver is PostgreSQL. Each
         * element of $rows is one row already formatted as a single string with its field values
         * joined by $separator, and $nullAs is the string used within a row to represent SQL NULL.
         * Returns true on success.
         *
         * @param string $tableName
         * @param array $rows
         * @param string $separator
         * @param string $nullAs
         * @param string|null $fields
         *
         * @return bool
         */
        public function pgsqlCopyFromArray(
            $tableName,
            array $rows,
            $separator = "\t",
            $nullAs = "\\\\N",
            $fields = null,
        ) {}

        /**
         * Loads rows from the file at $filename into $tableName using PostgreSQL's COPY FROM
         * mechanism, usable only when the underlying driver is PostgreSQL. The file is expected in
         * the same plain-text, $separator-delimited format that pgsqlCopyToFile() writes.
         *
         * @param string $tableName
         * @param string $filename
         * @param string $separator
         * @param string $nullAs
         * @param string|null $fields
         *
         * @return bool
         */
        public function pgsqlCopyFromFile(
            $tableName,
            $filename,
            $separator = "\t",
            $nullAs = "\\\\N",
            $fields = null,
        ) {}

        /**
         * Reads all rows from $tableName using PostgreSQL's COPY TO mechanism and returns them as
         * an array, one string per row with the row's field values already joined by $separator,
         * usable only when the underlying driver is PostgreSQL. Returns false on failure.
         *
         * @param string $tableName
         * @param string $separator
         * @param string $nullAs
         * @param string|null $fields
         *
         * @return array|false
         */
        public function pgsqlCopyToArray($tableName, $separator = "\t", $nullAs = "\\\\N", $fields = null) {}

        /**
         * Writes all rows from $tableName to the file at $filename using PostgreSQL's COPY TO
         * mechanism, in the same plain-text, $separator-delimited format pgsqlCopyToArray()
         * returns, usable only when the underlying driver is PostgreSQL.
         *
         * @param string $tableName
         * @param string $filename
         * @param string $separator
         * @param string $nullAs
         * @param string|null $fields
         *
         * @return bool
         */
        public function pgsqlCopyToFile($tableName, $filename, $separator = "\t", $nullAs = "\\\\N", $fields = null) {}

        /**
         * Creates a new, empty large object in a PostgreSQL database and returns its OID as a
         * string for later use with pgsqlLOBOpen(), usable only when the underlying driver is
         * PostgreSQL. Returns false on failure.
         *
         * @return string|false
         */
        public function pgsqlLOBCreate() {}

        /**
         * Opens the large object identified by $oid and returns a stream resource that can be read
         * from or written to according to $mode, usable only when the underlying driver is
         * PostgreSQL. Returns false on failure.
         *
         * @param string $oid
         * @param string $mode
         *
         * @return resource|false
         */
        public function pgsqlLOBOpen($oid, $mode = 'rb') {}

        /**
         * Deletes the large object identified by $oid from a PostgreSQL database, usable only when
         * the underlying driver is PostgreSQL.
         *
         * @param string $oid
         *
         * @return bool
         */
        public function pgsqlLOBUnlink($oid) {}

        /**
         * Retrieves a single asynchronous notification sent via PostgreSQL's LISTEN/NOTIFY
         * mechanism, waiting up to $timeoutMilliseconds for one to arrive, usable only when the
         * underlying driver is PostgreSQL. Returns false if none arrives in time, otherwise an
         * array with keys 'message' (the notified channel name), 'pid' (the sending backend's
         * process id) and 'payload'.
         *
         * @param int $fetchMode
         * @param int $timeoutMilliseconds
         *
         * @return array|false
         */
        public function pgsqlGetNotify($fetchMode = PDO::FETCH_DEFAULT, $timeoutMilliseconds = 0) {}

        /**
         * Returns the process id of the PostgreSQL backend process handling this connection, usable
         * only when the underlying driver is PostgreSQL.
         *
         * @return int
         */
        public function pgsqlGetPid() {}

        /**
         * Creates and returns a new connection to $dsn, equivalent to calling the constructor with
         * the same arguments. Declared to return static rather than PDO, so calling it on a
         * subclass yields an instance of that subclass without the subclass needing to redeclare
         * the constructor. Throws PDOException if the connection cannot be established.
         *
         * @throws PDOException
         */
        public static function connect(
            string $dsn,
            ?string $username = null,
            ?string $password = null,
            ?array $options = null,
        ): static {}
    }

    class PDOStatement implements IteratorAggregate
    {
        public string $queryString;

        /**
         * Executes the prepared statement. When $params is given, its values are bound to the
         * statement's placeholders, positionally or by name, for this call, taking precedence over
         * any values already bound with bindParam() or bindValue(). Returns true on success; throws
         * PDOException on failure under the default PDO::ERRMODE_EXCEPTION error mode, or returns
         * false otherwise.
         *
         * @throws PDOException
         */
        #[TentativeType]
        public function execute(?array $params = null): bool {}

        /**
         * Fetches the next row from the result set and advances the cursor, shaped according to
         * $mode: PDO::FETCH_ASSOC returns an array keyed by column name, PDO::FETCH_NUM returns an
         * array keyed by column position starting at 0, PDO::FETCH_BOTH returns both,
         * PDO::FETCH_OBJ returns a plain object with one property per column, and PDO::FETCH_LAZY
         * returns a PDORow that reads column values on demand. $mode defaults to
         * PDO::FETCH_DEFAULT, meaning whichever mode is currently configured via setFetchMode() or
         * the connection's PDO::ATTR_DEFAULT_FETCH_MODE attribute (PDO::FETCH_BOTH unless changed).
         * Returns false once there are no more rows. $cursorOrientation and $cursorOffset only take
         * effect for a scrollable cursor requested with PDO::ATTR_CURSOR before the statement was
         * executed; many drivers only support the default forward-only cursor, where they are
         * ignored.
         *
         * @throws PDOException
         */
        #[TentativeType]
        public function fetch(
            int $mode = PDO::FETCH_DEFAULT,
            int $cursorOrientation = PDO::FETCH_ORI_NEXT,
            int $cursorOffset = 0,
        ): mixed {}

        /**
         * Binds a PHP variable to a placeholder by reference, so it is the value the variable holds
         * at the time execute() runs, not at the time of this call, that gets used, and so that an
         * OUT or INOUT parameter of a stored procedure can write back into the variable after
         * execution. $type declares the SQL data type of the parameter, and $maxLength reserves
         * buffer space for it, which some drivers require for OUT parameters. Returns true on
         * success.
         *
         * @code
         * $stmt->bindParam(':name', $name);
         * $name = 'Alice';
         * $stmt->execute(); // sends 'Alice', the value at execute() time
         * @endcode
         *
         * @throws PDOException
         */
        #[TentativeType]
        public function bindParam(
            int|string $param,
            mixed &$var,
            int $type = PDO::PARAM_STR,
            int $maxLength = 0,
            mixed $driverOptions = null,
        ): bool {}

        /**
         * Binds a PHP variable to a column of the result set, identified by $column either as its
         * 1-indexed position or by its name, so the variable is updated with that column's value on
         * every subsequent fetch. $type and $maxLength give the driver the column's SQL type and,
         * for some drivers, the buffer size to allocate.
         *
         * @throws PDOException
         */
        #[TentativeType]
        public function bindColumn(
            int|string $column,
            mixed &$var,
            int $type = PDO::PARAM_STR,
            int $maxLength = 0,
            mixed $driverOptions = null,
        ): bool {}

        /**
         * Binds $value to a placeholder by its current value rather than by reference, so later
         * changes to the original variable have no effect on what gets sent. $type declares the SQL
         * data type, defaulting to PDO::PARAM_STR. Returns true on success.
         *
         * @throws PDOException
         */
        #[TentativeType]
        public function bindValue(int|string $param, mixed $value, int $type = PDO::PARAM_STR): bool {}

        /**
         * Returns the number of rows affected by the last DELETE, INSERT or UPDATE statement
         * executed by this PDOStatement. For a SELECT statement, most drivers do not return a
         * meaningful count here; use fetchAll() or count the rows fetched instead.
         *
         * @throws PDOException
         */
        #[TentativeType]
        public function rowCount(): int {}

        /**
         * Fetches a single column, selected by its 0-indexed position $column, from the next row of
         * the result set and advances the cursor. Returns false once there are no more rows, but
         * false is also what a column whose own value is false or null would return, so it cannot
         * be used alone to tell 'no more rows' apart from 'this row's value is falsy'.
         *
         * @throws PDOException
         */
        #[TentativeType]
        public function fetchColumn(int $column = 0): mixed {}

        /**
         * Fetches every remaining row from the result set into a single array, one element per row,
         * each shaped the way $mode would shape a single row returned by fetch(). Because the whole
         * result set is materialised at once, this can use significantly more memory than fetching
         * rows one at a time with fetch().
         *
         * @throws PDOException
         */
        #[TentativeType]
        public function fetchAll(int $mode = PDO::FETCH_DEFAULT, mixed ...$args): array {}

        /**
         * Fetches the next row from the result set and returns it as a new instance of $class,
         * defaulting to stdClass, with the row's column values assigned to properties of the same
         * name and $constructorArgs passed to the class's constructor. Returns false once there are
         * no more rows.
         *
         * @template T of object
         *
         * @param class-string<T>|null $class
         *
         * @return T|null|false
         *
         * @throws PDOException
         */
        #[TentativeType]
        public function fetchObject(?string $class = 'stdClass', array $constructorArgs = []): object|false {}

        /**
         * Returns the SQLSTATE code for the last operation attempted on this statement, or null if
         * no operation has been attempted yet.
         */
        #[TentativeType]
        public function errorCode(): ?string {}

        /**
         * Returns extended information about the last operation on this statement as a three-
         * element array: index 0 is the SQLSTATE code, index 1 is the driver-specific error code,
         * and index 2 is the driver-specific error message.
         *
         * @return array{0: string, 1: int, 2: string}
         */
        #[TentativeType]
        public function errorInfo(): array {}

        /**
         * Sets a statement-level attribute, such as PDO::ATTR_CURSOR set to PDO::CURSOR_SCROLL to
         * request a scrollable cursor before execute() is called, which fetch()'s
         * $cursorOrientation needs in order to have any effect, or the ODBC-specific
         * PDO::ATTR_CURSOR_NAME. Returns false if the driver does not support the given attribute.
         *
         * @throws PDOException
         */
        #[TentativeType]
        public function setAttribute(int $attribute, mixed $value): bool {}

        /**
         * Returns the current value of a statement-level attribute, such as one previously set with
         * setAttribute() or a driver-specific one like PDO::ATTR_CURSOR_NAME for ODBC cursors.
         */
        #[TentativeType]
        public function getAttribute(int $name): mixed {}

        /**
         * Returns the number of columns in the result set. Returns 0 if the statement has no result
         * set, for example after an INSERT or UPDATE.
         *
         * @throws PDOException
         */
        #[TentativeType]
        public function columnCount(): int {}

        /**
         * Returns metadata for the column at 0-indexed position $column in the result set, or false
         * if the column does not exist. Not every driver supports this, and support varies in which
         * of the returned keys are actually populated for a given driver.
         *
         * @return array{
         *   'name': string,
         *   'len': int,
         *   'precision': int,
         *   'oci:decl_type': int|string,
         *   'native_type': string,
         *   'scale': int,
         *   'flags': array,
         *   'pdo_type': int,
         * }|false
         */
        #[TentativeType]
        public function getColumnMeta(int $column): array|false {}

        /**
         * Sets the default fetch mode this statement's fetch() and fetchAll() calls use when they
         * are not given an explicit mode. $args carries mode-specific extra values: the class name
         * for PDO::FETCH_CLASS, the object to populate for PDO::FETCH_INTO, or the column index for
         * PDO::FETCH_COLUMN.
         *
         * @code
         * $stmt->setFetchMode(PDO::FETCH_CLASS, User::class);
         * $user = $stmt->fetch(); // a User instance, not an array
         * @endcode
         *
         * @param int $mode
         * @param mixed ...$args
         *
         * @return bool
         *
         * @throws PDOException
         */
        public function setFetchMode($mode, ...$args) {}

        /**
         * Discards the current result set and advances to the next one, for a statement such as a
         * stored procedure call that produces more than one result set. Returns false if there is
         * no further rowset, and not every driver supports multiple rowsets at all.
         *
         * @throws PDOException
         */
        public function nextRowset(): bool {}

        /**
         * Frees the result set so the connection can be used to run another statement, without
         * needing to fetch every remaining row first. Only closes the statement's own cursor, not
         * the underlying connection.
         *
         * @throws PDOException
         */
        public function closeCursor(): bool {}

        /**
         * Writes debugging information about the prepared statement, including its SQL text and its
         * bound parameters' names, types and values, directly to output rather than returning it as
         * a string.
         */
        public function debugDumpParams(): ?bool {}

        /**
         * Called automatically when a serialised PDOStatement is unserialised. A PDOStatement
         * cannot meaningfully be serialised, so this throws instead of restoring a working
         * statement.
         */
        final public function __wakeup() {}

        /**
         * Called automatically by serialize() when it encounters a PDOStatement. A PDOStatement
         * cannot meaningfully be serialised, so this throws instead of returning the properties to
         * save.
         */
        final public function __sleep() {}

        /**
         * Called automatically by PHP when the statement is used in a foreach loop, because
         * PDOStatement implements IteratorAggregate. Returns an Iterator that yields the
         * statement's remaining rows one at a time, each equivalent to a call to fetch().
         */
        public function getIterator(): Iterator {}

        public function connect() {}
    }

    final class PDORow
    {
        public string $queryString;
    }

    /**
     * @return array
     */
    function pdo_drivers(): array {}

    function confirm_pdo_ibm_compiled() {}
}

namespace Pdo {
    use Mago;
    use PDO;

    #[Mago\AvailableSince(80400)]
    class Sqlite extends PDO
    {
        public const int DETERMINISTIC = 0;
        public const int OPEN_READONLY = 1;
        public const int OPEN_READWRITE = 0;
        public const int OPEN_CREATE = 0;
        public const int ATTR_OPEN_FLAGS = 0;
        public const int ATTR_READONLY_STATEMENT = 0;
        public const int ATTR_EXTENDED_RESULT_CODES = 0;

        public function createAggregate(string $name, callable $step, callable $finalize, int $numArgs = -1): bool {}

        public function createCollation(string $name, callable $callback): bool {}

        public function createFunction(
            string $function_name,
            callable $callback,
            int $num_args = -1,
            int $flags = 0,
        ): bool {}

        public function loadExtension(string $name): void {}

        /** @return resource|false */
        public function openBlob(
            string $table,
            string $column,
            int $rowid,
            ?string $dbname = 'main',
            int $flags = \Pdo\Sqlite::OPEN_READONLY,
        ) {}
    }

    #[Mago\AvailableSince(80400)]
    class Mysql extends PDO
    {
        public const int ATTR_USE_BUFFERED_QUERY = 0;
        public const int ATTR_LOCAL_INFILE = 0;
        public const int ATTR_INIT_COMMAND = 0;
        public const int ATTR_MAX_BUFFER_SIZE = 0;
        public const int ATTR_READ_DEFAULT_FILE = 0;
        public const int ATTR_READ_DEFAULT_GROUP = 0;
        public const int ATTR_COMPRESS = 0;
        public const int ATTR_DIRECT_QUERY = 0;
        public const int ATTR_FOUND_ROWS = 0;
        public const int ATTR_IGNORE_SPACE = 0;
        public const int ATTR_SSL_KEY = 0;
        public const int ATTR_SSL_CERT = 0;
        public const int ATTR_SSL_CA = 0;
        public const int ATTR_SSL_CAPATH = 0;
        public const int ATTR_SSL_CIPHER = 0;
        public const int ATTR_SERVER_PUBLIC_KEY = 0;
        public const int ATTR_MULTI_STATEMENTS = 0;
        public const int ATTR_SSL_VERIFY_SERVER_CERT = 0;
        public const int ATTR_LOCAL_INFILE_DIRECTORY = 0;

        public function getWarningCount(): int {}
    }

    #[Mago\AvailableSince(80400)]
    class Pgsql extends PDO
    {
        public const int ATTR_DISABLE_PREPARES = 1000;
        public const int ATTR_RESULT_MEMORY_SIZE = 1001;
        public const int TRANSACTION_IDLE = 0;
        public const int TRANSACTION_ACTIVE = 1;
        public const int TRANSACTION_INTRANS = 2;
        public const int TRANSACTION_INERROR = 3;
        public const int TRANSACTION_UNKNOWN = 4;

        public function copyFromArray(
            string $tableName,
            array $rows,
            string $separator = "\t",
            string $nullAs = "\\\\N",
            ?string $fields = null,
        ): bool {}

        public function copyFromFile(
            string $tableName,
            string $filename,
            string $separator = "\t",
            string $nullAs = "\\\\N",
            ?string $fields = null,
        ): bool {}

        public function copyToArray(
            string $tableName,
            string $separator = "\t",
            string $nullAs = "\\\\N",
            ?string $fields = null,
        ): array|false {}

        public function copyToFile(
            string $tableName,
            string $filename,
            string $separator = "\t",
            string $nullAs = "\\\\N",
            ?string $fields = null,
        ): bool {}

        public function escapeIdentifier(string $input): string {}

        public function getNotify(int $fetchMode = \PDO::FETCH_DEFAULT, int $timeoutMilliseconds = 0): array|false {}

        public function getPid(): int {}

        public function lobCreate(): string|false {}

        /**
         * @return resource|false
         */
        public function lobOpen(string $oid, string $mode = 'rb') {}

        public function lobUnlink(string $oid): bool {}

        public function setNoticeCallback(?callable $callback): void {}
    }
}
