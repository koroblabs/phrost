//! The types PHP's engine gives the variables no file declares.
//!
//! `$_SERVER`, `$_GET`, `$argv`: bound by the runtime before a line of the project runs, so
//! nothing in any file says what they hold and every read of one was untyped. `$_SERVER
//! ['REQUEST_TIME']` handed to an `int` parameter, `$_GET['id']` handed to a `string` -- the
//! commonest values in a PHP request, and the index had nothing to say about any of them.
//!
//! # Why a table here and not a stub
//!
//! A stub declares *symbols*, and these are variables. Writing `/** @var … */ $_SERVER = [];` in
//! a stub file puts the fact in that file's body, which is dropped the moment the file is closed
//! -- so nothing cross-file could read it without the index growing a global-variable table
//! anyway. And the fact is not PHP's source at all: it is the engine's, which is where
//! [`phrost_model::SUPERGLOBALS`] already lives. Built with the type table's own constructors
//! rather than parsed from a written type, so there is no PHP to parse and no arena to borrow.
//!
//! # Why a shape rather than `array<string, string>`
//!
//! Because it would be wrong on the key real code reads most. Measured over one checkout,
//! `$_SERVER`'s two most-read keys are `REQUEST_TIME` (28 reads) and `REQUEST_TIME_FLOAT` (10),
//! which are an `int` and a `float` -- so the convenient blanket type is a *wrong* answer at the
//! top of the list, and a wrong type is worse than none. A shape says the right thing per key,
//! says nothing at all for a key it does not hold (`keyed_of_held` returns `None`, which nothing
//! reports), and gives key completion for free because the shape reader already exists.
//!
//! The keys are the ones php.net documents, not the ones one project happens to read. Every one
//! is optional, because the manual's own note is that no server guarantees any of them.

use phrost_model::{BuiltinTy, Interner, Name};
use phrost_ty::{TypeId, TypeTable};
use rustc_hash::FxHashMap;

/// What the engine binds, and what each one holds.
pub(crate) fn install(types: &mut TypeTable, interner: &mut Interner) -> FxHashMap<Name, TypeId> {
    let string = types.builtin(BuiltinTy::String);
    let int = types.builtin(BuiltinTy::Int);
    let float = types.builtin(BuiltinTy::Float);
    let mixed = types.builtin(BuiltinTy::Mixed);
    let array = types.builtin(BuiltinTy::Array);
    let strings = types.generic(array, &[string, string]);
    let list_of_strings = types.generic(array, &[int, string]);
    let anything = types.generic(array, &[string, mixed]);
    // `$_GET['a']` is a string, and `?a[]=1` makes it an array -- both, and nothing narrower is
    // true. Nested deeper than one level it is an untyped array, which is what `array` says.
    let string_or_array = types.union(&[string, array]);
    let submitted = types.generic(array, &[string, string_or_array]);

    let held = [
        ("_SERVER", server_shape(types, interner, string, int, float)),
        ("_GET", submitted),
        ("_POST", submitted),
        ("_REQUEST", submitted),
        ("_COOKIE", strings),
        ("_ENV", strings),
        ("_FILES", uploads(types, interner, string, int)),
        // Anything at all, by construction: a session holds what the application put in it.
        ("_SESSION", anything),
        ("GLOBALS", anything),
        // The last four are not superglobals -- the engine writes them into one scope rather than
        // all of them -- but they are the same fact from the same source, and `$argv` is how every
        // CLI script starts.
        ("argv", list_of_strings),
        ("argc", int),
        ("http_response_header", list_of_strings),
        ("php_errormsg", string),
    ];

    held.into_iter()
        .map(|(name, ty)| (interner.name(name), ty))
        .collect()
}

/// The keys php.net documents for `$_SERVER`, with the four that are not strings.
fn server_shape(
    types: &mut TypeTable,
    interner: &mut Interner,
    string: TypeId,
    int: TypeId,
    float: TypeId,
) -> TypeId {
    const STRINGS: &[&str] = &[
        "PHP_SELF",
        "GATEWAY_INTERFACE",
        "SERVER_ADDR",
        "SERVER_NAME",
        "SERVER_SOFTWARE",
        "SERVER_PROTOCOL",
        "REQUEST_METHOD",
        "QUERY_STRING",
        "DOCUMENT_ROOT",
        "HTTP_ACCEPT",
        "HTTP_ACCEPT_CHARSET",
        "HTTP_ACCEPT_ENCODING",
        "HTTP_ACCEPT_LANGUAGE",
        "HTTP_CONNECTION",
        "HTTP_HOST",
        "HTTP_REFERER",
        "HTTP_USER_AGENT",
        "HTTPS",
        "REMOTE_ADDR",
        "REMOTE_HOST",
        "REMOTE_PORT",
        "REMOTE_USER",
        "REDIRECT_REMOTE_USER",
        "SCRIPT_FILENAME",
        "SERVER_ADMIN",
        "SERVER_PORT",
        "SERVER_SIGNATURE",
        "PATH_TRANSLATED",
        "SCRIPT_NAME",
        "REQUEST_URI",
        "PHP_AUTH_DIGEST",
        "PHP_AUTH_USER",
        "PHP_AUTH_PW",
        "AUTH_TYPE",
        "PATH_INFO",
        "ORIG_PATH_INFO",
        "CONTENT_TYPE",
        "CONTENT_LENGTH",
    ];

    let array = types.builtin(BuiltinTy::Array);
    let arguments = types.generic(array, &[int, string]);

    let mut fields = Vec::with_capacity(STRINGS.len() + 4);
    for key in STRINGS {
        fields.push(types.field(interner.name(key), true, string));
    }
    for (key, ty) in [
        ("REQUEST_TIME", int),
        ("REQUEST_TIME_FLOAT", float),
        ("argc", int),
        ("argv", arguments),
    ] {
        fields.push(types.field(interner.name(key), true, ty));
    }
    // Open: a server writes whatever variables it likes beside these.
    types.shape(&fields, true)
}

/// `$_FILES`, whose value is the one superglobal entry with a documented shape of its own.
///
/// Keyed by the form field's name, so the outer type is a map and the shape is what each holds.
/// `full_path` is PHP 8.1's and is the only optional one: the rest are written on every upload,
/// `error` included -- it is how a failed one is reported.
fn uploads(types: &mut TypeTable, interner: &mut Interner, string: TypeId, int: TypeId) -> TypeId {
    let fields = [
        ("name", string, false),
        ("full_path", string, true),
        ("type", string, false),
        ("tmp_name", string, false),
        ("error", int, false),
        ("size", int, false),
    ]
    .map(|(key, ty, optional)| types.field(interner.name(key), optional, ty));
    // Open, although PHP writes exactly these: a declaration naming the two keys a handler reads is
    // how upload code is written, and every other key read as an extra would be noise.
    let upload = types.shape(&fields, true);

    let array = types.builtin(BuiltinTy::Array);
    types.generic(array, &[string, upload])
}
