<?php

class LibXMLError
{
    public int $level;
    public int $code;
    public int $column;
    public string $message;
    public string $file;
    public int $line;
}

/**
 * Sets the stream context to use for the very next libxml-based document load or write, such as
 * simplexml_load_file() or DOMDocument::load(), letting that one operation use custom stream
 * options like HTTP headers. The context is consumed by that single call and does not persist.
 *
 * @param resource $context
 */
function libxml_set_streams_context($context): void {}

/**
 * Toggles whether libxml errors are suppressed and stored for later retrieval with
 * libxml_get_errors(), rather than raised immediately. Returns the previous value of this setting.
 */
function libxml_use_internal_errors(?bool $use_errors = null): bool {}

/**
 * Returns the most recently recorded libxml error or warning, or false if none has occurred. Errors
 * are only captured for retrieval this way once libxml_use_internal_errors() has been enabled;
 * otherwise they are reported through PHP's normal error handler instead, and this returns false.
 */
function libxml_get_last_error(): LibXMLError|false {}

/**
 * Clears the libxml error buffer accumulated by previous XML operations.
 */
function libxml_clear_errors(): void {}

/**
 * Returns the libxml errors accumulated since internal error buffering was turned on with
 * libxml_use_internal_errors(true), as an array of LibXMLError objects. Returns an empty array if
 * buffering was never enabled, and does not clear the buffer; libxml_clear_errors() does that.
 *
 * @return list<LibXMLError>
 */
function libxml_get_errors(): array {}

/**
 * Historically toggled whether the XML parser would resolve external entities and DTDs. As of PHP
 * 8.0, external entity loading is always disabled regardless of this setting, so the function has
 * no effect and returns true.
 *
 * @deprecated
 */
function libxml_disable_entity_loader(bool $disable = true): bool {}

/**
 * Registers a callback used to resolve external entities, such as external DTDs, referenced while
 * parsing an XML document. Passing null removes a previously registered callback and restores the
 * default resolution behaviour.
 */
function libxml_set_external_entity_loader(?callable $resolver_function): bool {}

/**
 * Returns the external entity loader callback currently registered with
 * libxml_set_external_entity_loader(), or null if none is set and the default loader is in effect.
 */
#[Mago\AvailableSince(80200)]
function libxml_get_external_entity_loader(): ?callable {}

const LIBXML_VERSION = 20901;

const LIBXML_DOTTED_VERSION = '2.9.1';

const LIBXML_LOADED_VERSION = 20901;

const LIBXML_NOENT = 2;

const LIBXML_DTDLOAD = 4;

const LIBXML_DTDATTR = 8;

const LIBXML_DTDVALID = 16;

const LIBXML_NOERROR = 32;

const LIBXML_NOWARNING = 64;

const LIBXML_NOBLANKS = 256;

const LIBXML_XINCLUDE = 1024;

const LIBXML_NSCLEAN = 8192;

const LIBXML_NOCDATA = 16384;

const LIBXML_NONET = 2048;

const LIBXML_PEDANTIC = 128;

const LIBXML_COMPACT = 65536;

const LIBXML_BIGLINES = 65535;

const LIBXML_NOXMLDECL = 2;

const LIBXML_PARSEHUGE = 524288;

const LIBXML_NOEMPTYTAG = 4;

const LIBXML_SCHEMA_CREATE = 1;

const LIBXML_HTML_NOIMPLIED = 8192;

const LIBXML_HTML_NODEFDTD = 4;

const LIBXML_ERR_NONE = 0;

const LIBXML_ERR_WARNING = 1;

const LIBXML_ERR_ERROR = 2;

const LIBXML_ERR_FATAL = 3;

const LIBXML_RECOVER = 1;

const LIBXML_NO_XXE = 8388608;
