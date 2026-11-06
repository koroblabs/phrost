<?php

/**
 * @var non-empty-string
 */
const DATE_ATOM = "Y-m-d\\TH:i:sP";

/**
 * @var non-empty-string
 */
const DATE_COOKIE = 'l, d-M-Y H:i:s T';

/**
 * @var non-empty-string
 */
const DATE_ISO8601 = "Y-m-d\\TH:i:sO";

/**
 * @var non-empty-string
 */
const DATE_ISO8601_EXPANDED = "X-m-d\\TH:i:sP";

/**
 * @var non-empty-string
 */
const DATE_RFC822 = 'D, d M y H:i:s O';

/**
 * @var non-empty-string
 */
const DATE_RFC850 = 'l, d-M-y H:i:s T';

/**
 * @var non-empty-string
 */
const DATE_RFC1036 = 'D, d M y H:i:s O';

/**
 * @var non-empty-string
 */
const DATE_RFC1123 = 'D, d M Y H:i:s O';

/**
 * @var non-empty-string
 */
#[Deprecated(since: '8.5', message: 'as this format ignores the associated timezone and always uses GMT')]
const DATE_RFC7231 = "D, d M Y H:i:s \\G\\M\\T";

/**
 * @var non-empty-string
 */
const DATE_RFC2822 = 'D, d M Y H:i:s O';

/**
 * @var non-empty-string
 */
const DATE_RFC3339 = "Y-m-d\\TH:i:sP";

/**
 * @var non-empty-string
 */
const DATE_RFC3339_EXTENDED = "Y-m-d\\TH:i:s.vP";

/** @var non-empty-string */
const DATE_RSS = DATE_RFC1123;

/** @var non-empty-string */
const DATE_W3C = DATE_RFC3339;

/**
 * @var int
 * @deprecated
 */
const SUNFUNCS_RET_TIMESTAMP = 0;

/**
 * @var int
 * @deprecated
 */
const SUNFUNCS_RET_STRING = 1;

/**
 * @var int
 * @deprecated
 */
const SUNFUNCS_RET_DOUBLE = 2;

function strtotime(string $datetime, ?int $baseTimestamp = null): int|false {}

/**
 * @return ($format is non-empty-string ? non-empty-string : string)
 */
function date(string $format, ?int $timestamp = null): string {}

function idate(string $format, ?int $timestamp = null): int|false {}

/**
 * @return ($format is non-empty-string ? non-empty-string : string)
 */
function gmdate(string $format, ?int $timestamp = null): string {}

function mktime(
    int $hour,
    ?int $minute = null,
    ?int $second = null,
    ?int $month = null,
    ?int $day = null,
    ?int $year = null,
): int|false {}

function gmmktime(
    int $hour,
    ?int $minute = null,
    ?int $second = null,
    ?int $month = null,
    ?int $day = null,
    ?int $year = null,
): int|false {}

function checkdate(int $month, int $day, int $year): bool {}

/**
 * Formats a Unix timestamp (or the current time) as a local date/time string according to the
 * current locale, using C strftime() conversion specifiers rather than date()'s. Deprecated since
 * PHP 8.1, because which specifiers are supported and how the locale is applied both vary by
 * platform and C library; date() combined with setlocale(), or the intl extension, is the portable
 * replacement. Returns false on failure, such as an unsupported specifier.
 *
 * @deprecated
 */
function strftime(string $format, ?int $timestamp = null): string|false {}

/**
 * Formats a Unix timestamp as a locale-dependent string using strftime()-style format codes,
 * interpreting the timestamp in GMT/UTC rather than the local time zone. It is deprecated because
 * it depends on the C library's locale handling, which behaves inconsistently across platforms; it
 * returns false on failure.
 *
 * @deprecated
 */
function gmstrftime(string $format, ?int $timestamp = null): string|false {}

/**
 * Returns the current Unix timestamp, the number of seconds since 1 January 1970 00:00:00 UTC.
 */
function time(): int {}

/**
 * Returns the local date and time broken into components, indexed 0-9 by default or, when
 * $associative is true, keyed tm_sec, tm_min, tm_hour, tm_mday, tm_mon, tm_year, tm_wday, tm_yday
 * and tm_isdst. Following C's struct tm convention, tm_mon is 0-based (0 for January) and tm_year
 * counts years since 1900, unlike PHP's usual date functions.
 *
 * @code
 * $year = 1900 + localtime(time(), true)['tm_year'];
 * @endcode
 *
 * @return array<int|string, int>
 */
function localtime(?int $timestamp = null, bool $associative = false): array {}

/**
 * Returns an associative array breaking a Unix timestamp down into its date and time components, or
 * the current time if $timestamp is null. Keys are seconds, minutes, hours, mday (day of the
 * month), wday (day of the week, 0 for Sunday), mon (month number), year, yday (day of the year,
 * starting at 0), weekday (the full day name), month (the full month name), and 0 (the timestamp
 * itself).
 *
 * @return array{
 *   seconds: int,
 *   minutes: int,
 *   hours: int,
 *   mday: int,
 *   wday: int,
 *   mon: int,
 *   year: int,
 *   yday: int,
 *   weekday: int,
 *   month: string,
 *   0: int
 * }
 */
function getdate(?int $timestamp = null): array {}

/**
 * Creates a new DateTime instance from a textual date and time description, the procedural
 * equivalent of new DateTime(). Returns false if $datetime cannot be parsed.
 */
function date_create(string $datetime = 'now', ?DateTimeZone $timezone = null): DateTime|false {}

/**
 * Parses a date/time string into a new DateTimeImmutable object, optionally in the given timezone.
 * Unlike constructing DateTimeImmutable directly, it returns false on an unparseable string instead
 * of throwing.
 */
function date_create_immutable(string $datetime = 'now', ?DateTimeZone $timezone = null): DateTimeImmutable|false {}

/**
 * Parses $datetime according to $format and returns a new DateTime set to the result, or false if
 * the string does not match the format. Any date or time component the format does not mention is
 * filled in from the current date and time rather than defaulting to zero.
 *
 * @code
 * date_create_from_format('H:i:s', '14:30:00'); // date part defaults to today
 * @endcode
 */
function date_create_from_format(string $format, string $datetime, ?DateTimeZone $timezone = null): DateTime|false {}

/**
 * Parses $datetime according to $format, the same format accepted by DateTime::createFromFormat,
 * and returns a new DateTimeImmutable, or false if the string does not match the format.
 */
function date_create_immutable_from_format(
    string $format,
    string $datetime,
    ?DateTimeZone $timezone = null,
): DateTimeImmutable|false {}

/**
 * Parses a date and time string into its components without constructing a DateTime object. The
 * returned array holds year, month, day, hour, minute, second and fraction for what was parsed,
 * warning_count and warnings and error_count and errors for anything the parser could not make
 * sense of, and is_localtime for whether the string carried time zone or offset information.
 *
 * @return array<string, mixed>
 */
function date_parse(string $datetime): array {}

/**
 * Parses datetime according to the literal format string format, using the same specifiers as
 * date(), rather than guessing the format the way strtotime() does. Returns an associative array
 * with year, month, day, hour, minute, second and fraction for whichever components the format
 * captured, reporting false for any it did not, plus warning_count and warnings, and error_count
 * and errors, describing anything in datetime that did not match format.
 *
 * @return array<string, mixed>
 */
function date_parse_from_format(string $format, string $datetime): array {}

/**
 * Returns the warnings and errors produced by the most recent date-and-time string parse (for
 * example strtotime or DateTime::createFromFormat), or false if the last parse produced neither.
 * The array carries warning_count and error_count integer totals alongside warnings and errors
 * arrays, each mapping the character offset in the parsed string to the message reported there.
 *
 * @return array<string, int|array>|false
 */
function date_get_last_errors(): array|false {}

/**
 * Formats the given date and time object into a string, using the same format characters accepted
 * by the date function. Equivalent to calling $object->format($format).
 */
function date_format(DateTimeInterface $object, string $format): string {}

/**
 * Adjusts a DateTime object's timestamp by the relative or absolute expression in modifier, such as
 * '+1 day' or 'next Monday', mutating the object in place and returning it. Returns false, leaving
 * the object unchanged, if modifier cannot be parsed.
 */
function date_modify(DateTime $object, string $modifier): DateTime|false {}

/**
 * Adds interval to object's date and time, modifying object in place and also returning it,
 * mirroring DateTime::add().
 */
function date_add(DateTime $object, DateInterval $interval): DateTime {}

/**
 * Subtracts the given interval from the DateTime object's date and time. Mutates the object in
 * place and returns the same instance, so the return value is only useful for chaining further
 * calls.
 */
function date_sub(DateTime $object, DateInterval $interval): DateTime {}

/**
 * Returns the DateTimeZone associated with a DateTime or DateTimeImmutable object — the procedural
 * equivalent of DateTimeInterface::getTimezone(). Returns false on failure.
 */
function date_timezone_get(DateTimeInterface $object): DateTimeZone|false {}

/**
 * Sets the time zone used by a DateTime object, mutating it in place, and returns that same object.
 * This is the procedural form of DateTime::setTimezone().
 */
function date_timezone_set(DateTime $object, DateTimeZone $timezone): DateTime {}

/**
 * Returns the timezone offset in effect for the given date and time, in seconds from UTC; the
 * procedural equivalent of DateTimeInterface::getOffset(). Because the offset accounts for
 * daylight-saving rules, it can differ between two DateTime objects in the same timezone.
 */
function date_offset_get(DateTimeInterface $object): int {}

/**
 * Returns a DateInterval representing the difference between two DateTimeInterface instances. When
 * $absolute is true the interval is always reported as positive (its invert property forced to 0),
 * regardless of which date is later.
 */
function date_diff(
    DateTimeInterface $baseObject,
    DateTimeInterface $targetObject,
    bool $absolute = false,
): DateInterval {}

/**
 * Sets the hour, minute, second and microsecond of a DateTime object, mutating it in place, the
 * procedural equivalent of DateTime::setTime(). Returns the same instance, so calls can be chained.
 */
function date_time_set(DateTime $object, int $hour, int $minute, int $second = 0, int $microsecond = 0): DateTime {}

/**
 * Sets the year, month and day on a DateTime object in place and returns the same object, so calls
 * can be chained. A month or day value outside its normal range rolls over into the following or
 * preceding month or year rather than being rejected.
 */
function date_date_set(DateTime $object, int $year, int $month, int $day): DateTime {}

/**
 * Sets $object's date using the ISO-8601 week-numbering calendar: an ISO year, a week number within
 * it, and a day of the week from 1 (Monday) to 7 (Sunday), defaulting to Monday. Mutates $object in
 * place and returns the same instance.
 */
function date_isodate_set(DateTime $object, int $year, int $week, int $dayOfWeek = 1): DateTime {}

/**
 * Sets $object's date and time from the Unix timestamp $timestamp, keeping its existing timezone.
 * Mutates $object in place and returns the same object.
 */
function date_timestamp_set(DateTime $object, int $timestamp): DateTime {}

/**
 * Returns the Unix timestamp represented by the given date and time object, equivalent to calling
 * its getTimestamp() method.
 */
function date_timestamp_get(DateTimeInterface $object): int {}

/**
 * Creates a DateTimeZone for the given timezone identifier, equivalent to calling the DateTimeZone
 * constructor directly. Returns false instead of throwing when the identifier is not a valid
 * timezone.
 */
function timezone_open(string $timezone): DateTimeZone|false {}

/**
 * Returns the name of the given time zone, such as "Europe/London" or "+02:00". Procedural
 * equivalent of DateTimeZone::getName().
 *
 * @return non-empty-string
 */
function timezone_name_get(DateTimeZone $object): string {}

/**
 * Returns the name of a timezone matching the given abbreviation, such as EST or CEST. utcOffset,
 * in seconds, and isDST narrow the search to a timezone with that GMT offset and daylight-saving
 * state; leave either at -1 to ignore that criterion. Because several timezones can share an
 * abbreviation, an arbitrary matching one is returned. Returns false if none matches.
 *
 * @param non-empty-string $abbr
 * @return non-empty-string|false
 */
function timezone_name_from_abbr(string $abbr, int $utcOffset = -1, int $isDST = -1): string|false {}

/**
 * Returns the timezone's offset from UTC, in seconds, at the moment given by datetime, taking into
 * account whichever daylight-saving rule is in effect at that instant.
 */
function timezone_offset_get(DateTimeZone $object, DateTimeInterface $datetime): int {}

/**
 * Returns the DST and offset transitions for the timezone between timestampBegin and timestampEnd.
 * Each entry is keyed by ts, the transition's Unix timestamp; time, the same instant as an ISO 8601
 * string; offset, the resulting offset from UTC in seconds; isdst, whether daylight saving is in
 * effect after the transition; and abbr, the timezone abbreviation then in use. Returns false if
 * the transitions cannot be determined.
 *
 * @return array<int, array>|false
 */
function timezone_transitions_get(
    DateTimeZone $object,
    int $timestampBegin = PHP_INT_MIN,
    int $timestampEnd = 2147483647,
): array|false {}

/**
 * Returns geographic location information for a timezone: country_code, latitude, longitude, and a
 * comments string, as recorded in the IANA timezone database. Returns false if location data is not
 * available for the timezone.
 *
 * @return array<string, float|string>|false
 */
function timezone_location_get(DateTimeZone $object): array|false {}

/**
 * Returns the timezone identifiers PHP recognises (e.g. "Europe/Kyiv") — the procedural equivalent
 * of DateTimeZone::listIdentifiers(). $timezoneGroup restricts the list to one of DateTimeZone's
 * continent constants, or DateTimeZone::ALL for every zone; passing DateTimeZone::PER_COUNTRY
 * additionally requires $countryCode, a two-letter ISO 3166-1 country code, to filter by country.
 *
 * @code
 * timezone_identifiers_list(DateTimeZone::PER_COUNTRY, 'UA');
 * @endcode
 *
 * @return array<int, string>
 */
function timezone_identifiers_list(int $timezoneGroup = DateTimeZone::ALL, ?string $countryCode = null): array {}

/**
 * Returns every recognised time zone abbreviation together with the offsets it can represent, keyed
 * by the lowercase abbreviation. Each entry is a list of arrays with keys dst (whether that offset
 * observes daylight saving), offset (the UTC offset in seconds) and timezone_id (the associated
 * time zone identifier, or null if none).
 *
 * @return array<string, array>
 */
function timezone_abbreviations_list(): array {}

/**
 * Returns the version identifier of the timezone database that PHP's date and time functions are
 * using.
 */
function timezone_version_get(): string {}

/**
 * Creates a DateInterval by parsing a relative time phrase such as '1 day' or '2 weeks 3 days',
 * using the same relative-format rules as strtotime(). Returns false if $datetime cannot be parsed
 * as a relative format.
 */
function date_interval_create_from_date_string(string $datetime): DateInterval|false {}

/**
 * Formats a DateInterval according to a format string of percent-prefixed specifiers, the
 * procedural equivalent of DateInterval::format(). Specifiers such as %y, %m, %d, %h, %i and %s
 * refer to the interval's own fields, while %a gives the total number of whole days when the
 * interval was produced by taking the difference between two dates.
 *
 * @code
 * date_interval_format($interval, '%a total days, %h hours');
 * @endcode
 */
function date_interval_format(DateInterval $object, string $format): string {}

/**
 * Sets the timezone used by date/time functions and by objects created without an explicit timezone
 * for the rest of the request. Returns false, leaving the previous timezone in effect, if the given
 * identifier is not a valid timezone.
 */
function date_default_timezone_set(string $timezoneId): bool {}

/**
 * Returns the timezone that date and time functions use when none is given explicitly, as
 * configured by date_default_timezone_set() or the date.timezone ini setting.
 */
function date_default_timezone_get(): string {}

/**
 * Returns the time of sunrise for a given day and location, deprecated in favour of date_sun_info.
 * $returnFormat selects the shape of the result: SUNFUNCS_RET_STRING, the default, gives a
 * formatted time string; SUNFUNCS_RET_TIMESTAMP gives a Unix timestamp; SUNFUNCS_RET_DOUBLE gives
 * the number of hours since midnight as a float. Returns false where the sun does not rise on that
 * day at that location, such as inside the polar circles.
 *
 * @deprecated
 */
function date_sunrise(
    int $timestamp,
    int $returnFormat = SUNFUNCS_RET_STRING,
    ?float $latitude = null,
    ?float $longitude = null,
    ?float $zenith = null,
    ?float $utcOffset = null,
): string|int|float|false {}

/**
 * Calculates the time of sunset for the day given by $timestamp at a given location, as a formatted
 * string, a floating-point number of hours, or a Unix timestamp depending on $returnFormat
 * (SUNFUNCS_RET_STRING, SUNFUNCS_RET_DOUBLE or SUNFUNCS_RET_TIMESTAMP). Latitude and longitude fall
 * back to the date.default_latitude and date.default_longitude ini settings when omitted, and the
 * function returns false if the calculation cannot be made for that date and location.
 *
 * @deprecated
 */
function date_sunset(
    int $timestamp,
    int $returnFormat = SUNFUNCS_RET_STRING,
    ?float $latitude = null,
    ?float $longitude = null,
    ?float $zenith = null,
    ?float $utcOffset = null,
): string|int|float|false {}

/**
 * Calculates the times of sunrise, sunset, and the start and end of civil, nautical and
 * astronomical twilight for the given Unix timestamp and geographic location. At latitudes where
 * the sun does not rise or set on that day, the corresponding value is bool true (permanent
 * daylight) or false (permanent darkness) instead of a timestamp.
 *
 * @return array{
 *   sunrise: int|bool,
 *   sunset: int|bool,
 *   transit: int|bool,
 *   civil_twilight_begin: int|bool,
 *   civil_twilight_end: int|bool,
 *   nautical_twilight_begin: int|bool,
 *   nautical_twilight_end: int|bool,
 *   astronomical_twilight_begin: int|bool,
 *   astronomical_twilight_end: int|bool,
 * }
 */
function date_sun_info(int $timestamp, float $latitude, float $longitude): array {}

interface DateTimeInterface
{
    /** @var non-empty-string */
    public const string ATOM = DATE_ATOM;

    /** @var non-empty-string */
    public const string COOKIE = DATE_COOKIE;

    /** @var non-empty-string */
    public const string ISO8601 = DATE_ISO8601;

    /** @var non-empty-string */
    public const string ISO8601_EXPANDED = DATE_ISO8601_EXPANDED;

    /** @var non-empty-string */
    public const string RFC822 = DATE_RFC822;

    /** @var non-empty-string */
    public const string RFC850 = DATE_RFC850;

    /** @var non-empty-string */
    public const string RFC1036 = DATE_RFC1036;

    /** @var non-empty-string */
    public const string RFC1123 = DATE_RFC1123;

    /** @var non-empty-string */
    #[Deprecated(since: '8.5', message: 'as this format ignores the associated timezone and always uses GMT')]
    public const string RFC7231 = DATE_RFC7231;

    /** @var non-empty-string */
    public const string RFC2822 = DATE_RFC2822;

    /** @var non-empty-string */
    public const string RFC3339 = DATE_RFC3339;

    /** @var non-empty-string */
    public const string RFC3339_EXTENDED = DATE_RFC3339_EXTENDED;

    /** @var non-empty-string */
    public const string RSS = DATE_RSS;

    /** @var non-empty-string */
    public const string W3C = DATE_W3C;

    /**
     * @psalm-mutation-free
     *
     * @param string $format
     *
     * @return ($format is non-empty-string ? non-empty-string : string)
     */
    public function format(string $format): string;

    /**
     * Returns the timezone associated with the date and time.
     */
    public function getTimezone(): DateTimeZone;

    /**
     * Returns the offset from UTC, in seconds; positive when the timezone is ahead of UTC and
     * negative when it is behind.
     */
    public function getOffset(): int;

    /**
     * Returns the point in time as a Unix timestamp, the number of whole seconds since the epoch.
     */
    public function getTimestamp(): int;

    /**
     * Returns the microseconds component of the time, in the range 0 to 999999.
     */
    public function getMicrosecond(): int;

    /**
     * Returns a DateInterval describing the elapsed time between the receiver and $targetObject.
     * Unless $absolute is true, the interval's invert property is 1 when $targetObject is earlier
     * than the receiver and 0 otherwise.
     */
    public function diff(DateTimeInterface $targetObject, bool $absolute = false): DateInterval;

    /**
     * Restores the object's internal state when it is unserialized; called automatically by PHP and
     * not meant to be invoked directly.
     */
    public function __wakeup(): void;

    /**
     * Returns the data PHP uses to represent the object when it is passed to serialize(); called
     * automatically and not meant to be invoked directly.
     */
    public function __serialize(): array;

    /**
     * Restores the object's state from the array produced by __serialize(); called automatically by
     * PHP during unserialize() and not meant to be invoked directly.
     */
    public function __unserialize(array $data): void;
}

class DateTime implements DateTimeInterface
{
    /**
     * Creates a new DateTime for $datetime, a string in any of the formats understood by PHP's date
     * and time parser and defaulting to the current moment, set to $timezone if given or the
     * default timezone otherwise. Throws DateMalformedStringException if $datetime cannot be
     * parsed.
     *
     * @throws DateMalformedStringException
     */
    public function __construct(string $datetime = 'now', ?DateTimeZone $timezone = null) {}

    /**
     * Returns the data PHP uses to represent the object when it is passed to serialize(); called
     * automatically and not meant to be invoked directly.
     */
    public function __serialize(): array {}

    /**
     * Restores the object's state from the array produced by __serialize(); called automatically by
     * PHP during unserialize() and not meant to be invoked directly.
     */
    public function __unserialize(array $data): void {}

    /**
     * Restores the object's internal state when it is unserialized; called automatically by PHP and
     * not meant to be invoked directly.
     */
    public function __wakeup(): void {}

    /**
     * Reconstructs a DateTime from the array representation produced by var_export(); called
     * automatically by the code var_export() generates and not meant to be invoked directly.
     */
    public static function __set_state(array $array): DateTime {}

    /**
     * Returns a new mutable DateTime with the same date, time and timezone as $object, leaving the
     * DateTimeImmutable unchanged.
     */
    public static function createFromImmutable(DateTimeImmutable $object): static {}

    /**
     * Returns a new mutable DateTime with the same date, time and timezone as $object, leaving
     * $object unchanged.
     *
     * @return static
     */
    #[Mago\AvailableSince(80000)]
    public static function createFromInterface(DateTimeInterface $object): DateTime {} // TODO return type should be static

    /**
     * Parses $datetime according to $format and returns a new DateTime, or false if $datetime does
     * not match $format. Use DateTime::getLastErrors() afterwards to inspect the warnings and
     * errors from the parse.
     */
    public static function createFromFormat(
        string $format,
        string $datetime,
        ?DateTimeZone $timezone = null,
    ): DateTime|false {}

    /**
     * Returns a new DateTime set to the Unix timestamp $timestamp, with the fractional part of a
     * float timestamp preserved as microseconds; the returned object's timezone is always UTC.
     *
     * @code
     * $d = DateTime::createFromTimestamp(1700000000);
     * // $d's timezone is always UTC, regardless of the default timezone
     * @endcode
     */
    #[Mago\AvailableSince(80400)]
    public static function createFromTimestamp(int|float $timestamp): static {}

    /**
     * Returns the warnings and errors recorded during the most recent date and time string parse
     * anywhere in the script, such as DateTime::createFromFormat(), or false when there is nothing
     * to report.
     *
     * @return array{
     *     warning_count: non-negative-int,
     *     warnings: array<non-negative-int, non-empty-string>,
     *     error_count: non-negative-int,
     *     errors: array<non-negative-int, non-empty-string>,
     * }|false
     */
    public static function getLastErrors(): array|false {}

    /**
     * Returns the date and time formatted according to $format, using the same format characters as
     * the date() function.
     *
     * @psalm-mutation-free
     *
     * @param string $format
     *
     * @return ($format is non-empty-string ? non-empty-string : string)
     */
    public function format(string $format): string {}

    /**
     * Mutates the receiver by applying $modifier, a relative or absolute date and time description
     * in the same form strtotime() accepts, and returns $this for chaining. This is unlike
     * DateTimeImmutable::modify(), which leaves the receiver untouched and returns a new object.
     *
     * @code
     * $a = new DateTime('2024-01-01');
     * $b = $a->modify('+1 day');
     * // $a === $b, and both now represent 2024-01-02
     * @endcode
     */
    public function modify(string $modifier): DateTime {}

    /**
     * Mutates the receiver by adding $interval to it and returns $this for chaining. If $interval's
     * invert property is set, for instance because it came from DateTime::diff() against an earlier
     * date, this has the effect of subtracting it instead.
     */
    public function add(DateInterval $interval): DateTime {}

    /**
     * Mutates the receiver by subtracting $interval from it and returns $this for chaining. If
     * $interval's invert property is set, this has the effect of adding it instead.
     */
    public function sub(DateInterval $interval): DateTime {}

    /**
     * Returns the timezone associated with the date and time.
     */
    public function getTimezone(): DateTimeZone {}

    /**
     * Mutates the receiver to represent the same point in time in $timezone, changing what its
     * local getters such as format() report while leaving getTimestamp() unchanged, and returns
     * $this for chaining.
     */
    public function setTimezone(DateTimeZone $timezone): DateTime {}

    /**
     * Returns the offset from UTC, in seconds; positive when the timezone is ahead of UTC and
     * negative when it is behind.
     */
    public function getOffset(): int {}

    /**
     * Returns the microseconds component of the time, in the range 0 to 999999.
     */
    #[Mago\AvailableSince(80400)]
    public function getMicrosecond(): int {}

    /**
     * Mutates the receiver, replacing its hour, minute, second and microsecond while leaving its
     * date unchanged, and returns $this for chaining.
     */
    public function setTime(int $hour, int $minute, int $second = 0, int $microsecond = 0): DateTime {}

    /**
     * Mutates the receiver, replacing its year, month and day while leaving its time of day
     * unchanged, and returns $this for chaining. A day or month outside its normal range overflows
     * into the following ones rather than being rejected.
     *
     * @code
     * $d = new DateTime('2023-01-15');
     * $d->setDate(2023, 2, 30);
     * // February has 28 days in 2023, so this becomes 2023-03-02
     * @endcode
     */
    public function setDate(int $year, int $month, int $day): DateTime {}

    /**
     * Mutates the receiver, setting its date from an ISO-8601 year and week number, with $dayOfWeek
     * running from 1 for Monday to 7 for Sunday, and leaves the time of day unchanged; returns
     * $this for chaining.
     */
    public function setISODate(int $year, int $week, int $dayOfWeek = 1): DateTime {}

    /**
     * Mutates the receiver to the point in time given by the Unix timestamp $timestamp, keeping its
     * existing timezone, and returns $this for chaining.
     */
    public function setTimestamp(int $timestamp): DateTime {}

    /**
     * Mutates the receiver, replacing its microseconds component (0 to 999999) while leaving the
     * rest of the date and time unchanged, and returns $this for chaining.
     */
    #[Mago\AvailableSince(80400)]
    public function setMicrosecond(int $microsecond): static {}

    /**
     * Returns the point in time as a Unix timestamp, the number of whole seconds since the epoch.
     */
    public function getTimestamp(): int {}

    /**
     * Returns a DateInterval describing the elapsed time between the receiver and $targetObject.
     * Unless $absolute is true, the interval's invert property is 1 when $targetObject is earlier
     * than the receiver and 0 otherwise.
     */
    public function diff(DateTimeInterface $targetObject, bool $absolute = false): DateInterval {}
}

class DateTimeImmutable implements DateTimeInterface
{
    /**
     * Creates an immutable date-time object, parsing $datetime (default 'now', meaning the current
     * moment) in the context of $timezone, or the default timezone when none is given. Throws
     * DateMalformedStringException if $datetime cannot be parsed.
     *
     * @throws DateMalformedStringException
     */
    public function __construct(string $datetime = 'now', ?DateTimeZone $timezone = null) {}

    /**
     * Returns the object's internal state as an array, which PHP passes to __unserialize() to
     * reconstruct an equivalent object. Called automatically when the object is serialized, for
     * example by serialize(); not normally called directly.
     */
    public function __serialize(): array {}

    /**
     * Restores the object's internal state from the array previously produced by __serialize().
     * Called automatically by PHP when unserializing a serialized DateTimeImmutable; not normally
     * called directly.
     */
    public function __unserialize(array $data): void {}

    /**
     * Called by unserialize() to let an object repair any state that does not survive
     * serialization. Because DateTimeImmutable defines __serialize() and __unserialize(), this
     * method is not actually invoked; those two take precedence.
     */
    public function __wakeup(): void {}

    /**
     * Reconstructs a DateTimeImmutable from the array representation produced by var_export().
     * Called from the code var_export() generates, not intended to be called directly.
     */
    public static function __set_state(array $array): DateTimeImmutable {}

    /**
     * Parses $datetime according to $format and returns a new instance, or false if the string does
     * not match the format; call getLastErrors() afterwards to see why a parse failed.
     *
     * @psalm-mutation-free
     *
     * @return static|false
     */
    public static function createFromFormat(
        string $format,
        string $datetime,
        ?DateTimeZone $timezone = null,
    ): DateTimeImmutable|false {}

    /**
     * Creates a new instance from a Unix timestamp. Pass a float rather than an int to also set the
     * microsecond component, taken from its fractional part.
     */
    #[Mago\AvailableSince(80400)]
    public static function createFromTimestamp(int|float $timestamp): static {}

    /**
     * Returns the warnings and errors produced by the most recent createFromFormat() call, or false
     * if that call produced none. warning_count and error_count give the totals, while warnings and
     * errors give the corresponding messages indexed by the character offset in the parsed string
     * where each occurred.
     *
     * @return array{
     *     warning_count: non-negative-int,
     *     warnings: array<non-negative-int, non-empty-string>,
     *     error_count: non-negative-int,
     *     errors: array<non-negative-int, non-empty-string>,
     * }|false
     */
    public static function getLastErrors(): array|false {}

    /**
     * Formats the date and time as a string, using the same format characters as PHP's date()
     * function.
     *
     * @psalm-mutation-free
     *
     * @param string $format
     *
     * @return ($format is non-empty-string ? non-empty-string : string)
     */
    public function format(string $format): string {}

    /**
     * Returns the timezone associated with this date-time.
     *
     * @psalm-mutation-free
     *
     * @psalm-ignore-falsable-return
     */
    public function getTimezone(): DateTimeZone {}

    /**
     * Returns the timezone's offset from UTC in seconds at this point in time, positive east of UTC
     * and negative west of it.
     *
     * @psalm-mutation-free
     */
    public function getOffset(): int {}

    /**
     * Returns the Unix timestamp (seconds since 1970-01-01 00:00:00 UTC), discarding any
     * microsecond component.
     *
     * @psalm-mutation-free
     */
    public function getTimestamp(): int {}

    /**
     * Returns the microsecond component of the time, from 0 to 999999.
     */
    #[Mago\AvailableSince(80400)]
    public function getMicrosecond(): int {}

    /**
     * Returns the difference between this date-time and $targetObject as a DateInterval. With
     * $absolute set to true the sign is discarded and invert is always 0; otherwise invert reflects
     * whether $targetObject is earlier than the receiver.
     *
     * @psalm-mutation-free
     *
     * @param bool $absolute
     *
     * @return DateInterval
     */
    public function diff(DateTimeInterface $targetObject, bool $absolute = false): DateInterval {}

    /**
     * Returns a new instance with $modifier (a relative format such as '+1 day' or 'next monday')
     * applied; the receiver itself is left unchanged.
     *
     * @code
     * $next = $date->modify('+1 day');
     * // $date is unchanged; $next holds the new value
     * @endcode
     *
     * @psalm-mutation-free
     *
     * @return static
     */
    #[NoDiscard(message: 'as DateTimeImmutable::modify() does not modify the object itself')]
    public function modify(string $modifier): DateTimeImmutable {}

    /**
     * Returns a new instance with $interval added; the receiver itself is left unchanged.
     *
     * @psalm-mutation-free
     *
     * @return static
     */
    #[NoDiscard(message: 'as DateTimeImmutable::add() does not modify the object itself')]
    public function add(DateInterval $interval): DateTimeImmutable {}

    /**
     * Returns a new instance with $interval subtracted; the receiver itself is left unchanged.
     *
     * @psalm-mutation-free
     *
     * @return static
     */
    #[NoDiscard(message: 'as DateTimeImmutable::sub() does not modify the object itself')]
    public function sub(DateInterval $interval): DateTimeImmutable {}

    /**
     * Returns a new instance attached to $timezone; the underlying point in time is unchanged, only
     * how it is displayed. The receiver itself is left unchanged.
     *
     * @psalm-mutation-free
     *
     * @return static
     */
    #[NoDiscard(message: 'as DateTimeImmutable::setTimezone() does not modify the object itself')]
    public function setTimezone(DateTimeZone $timezone): DateTimeImmutable {}

    /**
     * Returns a new instance with the time of day set to $hour, $minute, $second and $microsecond,
     * leaving the date unchanged; the receiver itself is left unchanged.
     *
     * @psalm-mutation-free
     *
     * @return static
     */
    #[NoDiscard(message: 'as DateTimeImmutable::setTime() does not modify the object itself')]
    public function setTime(int $hour, int $minute, int $second = 0, int $microsecond = 0): DateTimeImmutable {}

    /**
     * Returns a new instance with the date set to $year, $month and $day, leaving the time of day
     * unchanged; the receiver itself is left unchanged. Values outside the normal range overflow
     * into the neighbouring unit, as with mktime().
     *
     * @psalm-mutation-free
     *
     * @return static
     */
    #[NoDiscard(message: 'as DateTimeImmutable::setDate() does not modify the object itself')]
    public function setDate(int $year, int $month, int $day): DateTimeImmutable {}

    /**
     * Returns a new instance set to the given ISO-8601 week-numbering year and week, on $dayOfWeek
     * (1 for Monday through 7 for Sunday); the receiver itself is left unchanged.
     *
     * @psalm-mutation-free
     *
     * @return static
     */
    #[NoDiscard(message: 'as DateTimeImmutable::setISODate() does not modify the object itself')]
    public function setISODate(int $year, int $week, int $dayOfWeek = 1): DateTimeImmutable {}

    /**
     * Returns a new instance set to the Unix timestamp $timestamp, keeping the receiver's timezone
     * for display; the receiver itself is left unchanged.
     *
     * @psalm-mutation-free
     *
     * @return static
     */
    #[NoDiscard(message: 'as DateTimeImmutable::setTimestamp() does not modify the object itself')]
    public function setTimestamp(int $timestamp): DateTimeImmutable {}

    /**
     * Returns a new instance with the microsecond component set to $microsecond, leaving the rest
     * of the date and time unchanged; the receiver itself is left unchanged.
     *
     * @psalm-mutation-free
     *
     * @return static
     */
    #[Mago\AvailableSince(80400)]
    #[NoDiscard(message: 'as DateTimeImmutable::setMicrosecond() does not modify the object itself')]
    public function setMicrosecond(int $microsecond): static {}

    /**
     * Creates a new DateTimeImmutable copying the date, time and timezone of the mutable $object;
     * $object itself is left unchanged.
     *
     * @psalm-mutation-free
     *
     * @return static
     */
    public static function createFromMutable(DateTime $object): static {}

    /**
     * Creates a new DateTimeImmutable copying the date, time and timezone of $object, whether it is
     * a DateTime or a DateTimeImmutable.
     *
     * @psalm-mutation-free
     *
     * @return static
     */
    #[Mago\AvailableSince(80000)]
    public static function createFromInterface(DateTimeInterface $object): DateTimeImmutable {}
}

class DateTimeZone
{
    public const int AFRICA = 1;
    public const int AMERICA = 2;
    public const int ANTARCTICA = 4;
    public const int ARCTIC = 8;
    public const int ASIA = 16;
    public const int ATLANTIC = 32;
    public const int AUSTRALIA = 64;
    public const int EUROPE = 128;
    public const int INDIAN = 256;
    public const int PACIFIC = 512;
    public const int UTC = 1024;
    public const int ALL = 2047;
    public const int ALL_WITH_BC = 4095;
    public const int PER_COUNTRY = 4096;

    /**
     * Creates a timezone object for $timezone, either a named identifier such as 'Europe/London' or
     * 'UTC', or a UTC offset string such as '+02:00'. Throws DateInvalidTimeZoneException if
     * $timezone is not a valid identifier or offset.
     *
     * @psalm-external-mutation-free
     *
     * @throws DateInvalidTimeZoneException
     */
    public function __construct(string $timezone) {}

    /**
     * Returns the timezone's identifier, such as 'Europe/London', 'UTC', or a UTC offset string.
     *
     * @return non-empty-string
     */
    public function getName(): string {}

    /**
     * Returns this timezone's offset from UTC in seconds at $datetime, accounting for any daylight
     * saving time in effect at that moment.
     *
     * @psalm-mutation-free
     */
    public function getOffset(DateTimeInterface $datetime): int {}

    /**
     * Returns the timezone's daylight-saving transitions between $timestampBegin and $timestampEnd,
     * or false if the timezone has no transition data (as for UTC). Each entry gives the
     * transition's Unix timestamp (ts), local time (time), UTC offset in seconds (offset), whether
     * daylight saving is in effect from that point (isdst), and the abbreviation in use (abbr).
     *
     * @return list<array{
     *     ts: int,
     *     time: non-empty-string,
     *     offset: int,
     *     isdst: bool,
     *     abbr: string,
     * }>|false
     */
    public function getTransitions(int $timestampBegin = PHP_INT_MIN, int $timestampEnd = 2147483647): array|false {}

    /**
     * Returns geographic location data for the timezone, or false if none is available. Gives the
     * two-letter country code (country_code), latitude and longitude, and any free-text comments.
     *
     * @return array{
     *     country_code: non-empty-string,
     *     latitude: float,
     *     longitude: float,
     *     comments: string,
     * }|false
     */
    public function getLocation(): array|false {}

    /**
     * Returns every known timezone abbreviation, indexed by abbreviation, each mapping to a list of
     * the zones that use it. Each entry gives whether daylight saving applies there (dst), the UTC
     * offset in seconds (offset), and the full timezone identifier (timezone_id).
     *
     * @return array<string, list<array{
     *     dst: bool,
     *     offset: int,
     *     timezone_id: non-empty-string,
     * }>>
     */
    public static function listAbbreviations(): array {}

    /**
     * Returns the list of valid timezone identifiers, optionally restricted to one continental
     * group such as DateTimeZone::EUROPE or DateTimeZone::ASIA, or to the identifiers used by a
     * single country by passing DateTimeZone::PER_COUNTRY together with its two-letter
     * $countryCode.
     *
     * @param non-empty-string|null $countryCode
     *
     * @return list<non-empty-string>
     */
    public static function listIdentifiers(
        int $timezoneGroup = DateTimeZone::ALL,
        ?string $countryCode = null,
    ): array {}

    /**
     * Returns the object's internal state as an array, which PHP passes to __unserialize() to
     * reconstruct an equivalent object. Called automatically when the object is serialized, for
     * example by serialize(); not normally called directly.
     */
    public function __serialize(): array {}

    /**
     * Restores the object's internal state from the array previously produced by __serialize().
     * Called automatically by PHP when unserializing a serialized DateTimeZone; not normally called
     * directly.
     */
    public function __unserialize(array $data): void {}

    /**
     * Called by unserialize() to let an object repair any state that does not survive
     * serialization. Because DateTimeZone defines __serialize() and __unserialize(), this method is
     * not actually invoked; those two take precedence.
     */
    public function __wakeup(): void {}

    /**
     * Reconstructs a DateTimeZone from the array representation produced by var_export(). Called
     * from the code var_export() generates, not intended to be called directly.
     */
    public static function __set_state(array $array): DateTimeZone {}
}

class DateInterval
{
    public int $y;
    public int $m;
    public int $d;
    public int $h;
    public int $i;
    public int $s;
    public float $f;
    /** @var int<0, 1> */
    public int $invert;
    public int|false $days;
    public bool $from_string;
    public string $date_string;

    public function __construct(string $duration) {}

    public static function createFromDateString(string $datetime): DateInterval {}

    public function format(string $format): string {}

    public function __serialize(): array {}

    public function __unserialize(array $data): void {}

    public function __wakeup(): void {}

    public static function __set_state(array $array): DateInterval {}
}

/**
 * @implements IteratorAggregate<int, DateTimeInterface>
 */
class DatePeriod implements IteratorAggregate
{
    public const int EXCLUDE_START_DATE = UNKNOWN;
    public const int INCLUDE_END_DATE = UNKNOWN;

    /**
     * @readonly
     */
    public ?DateTimeInterface $start;

    /**
     * @readonly
     */
    public ?DateTimeInterface $current;

    /**
     * @readonly
     */
    public ?DateTimeInterface $end;

    /**
     * @readonly
     */
    public ?DateInterval $interval;

    /**
     * @readonly
     */
    public int $recurrences;

    /**
     * @readonly
     */
    public bool $include_start_date;

    /**
     * @readonly
     */
    public bool $include_end_date;

    public static function createFromISO8601String(string $specification, int $options = 0): static {}

    /**
     * @param DateTimeInterface|string $start
     * @param DateInterval|int $interval
     * @param DateTimeInterface|int $end
     * @param int $options
     */
    public function __construct($start, $interval = UNKNOWN, $end = UNKNOWN, $options = UNKNOWN) {}

    /**
     * Returns the start date passed to the constructor.
     */
    public function getStartDate(): DateTimeInterface {}

    /**
     * Returns the end date passed to the constructor, or null if the period was constructed with a
     * number of recurrences instead of an end date.
     */
    public function getEndDate(): ?DateTimeInterface {}

    /**
     * Returns the interval between each occurrence in the period.
     */
    public function getDateInterval(): DateInterval {}

    /**
     * Returns the number of recurrences passed to the constructor, or null if the period was
     * constructed with an end date instead.
     */
    public function getRecurrences(): ?int {}

    /**
     * Returns the object's internal state as an array, which PHP passes to __unserialize() to
     * reconstruct an equivalent object. Called automatically when the object is serialized, for
     * example by serialize(); not normally called directly.
     */
    public function __serialize(): array {}

    /**
     * Restores the object's internal state from the array previously produced by __serialize().
     * Called automatically by PHP when unserializing a serialized DatePeriod; not normally called
     * directly.
     */
    public function __unserialize(array $data): void {}

    /**
     * Called by unserialize() to let an object repair any state that does not survive
     * serialization. Because DatePeriod defines __serialize() and __unserialize(), this method is
     * not actually invoked; those two take precedence.
     */
    public function __wakeup(): void {}

    /**
     * Reconstructs a DatePeriod from the array representation produced by var_export(). Called from
     * the code var_export() generates, not intended to be called directly.
     */
    public static function __set_state(array $array): DatePeriod {}

    /**
     * Returns an iterator yielding each DateTimeInterface occurrence in the period, in order. PHP
     * calls this automatically when the DatePeriod is used in a foreach loop, as required by
     * IteratorAggregate.
     */
    public function getIterator(): Iterator {}
}

#[Mago\AvailableSince(80300)]
class DateError extends Error {}

class DateObjectError extends DateError {}

class DateRangeError extends DateError {}

class DateException extends Exception {}

class DateInvalidTimeZoneException extends DateException {}

#[Mago\AvailableSince(80300)]
class DateInvalidOperationException extends DateException {}

#[Mago\AvailableSince(80300)]
class DateMalformedStringException extends DateException {}

class DateMalformedIntervalStringException extends DateException {}

class DateMalformedPeriodStringException extends DateException {}
