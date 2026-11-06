//! A format string and its map of values disagreeing about a placeholder.
//!
//! Every fixture here is written with neutral names on purpose. The check recognises the *shape* --
//! a string literal and, beside it, an array whose keys are spelled like placeholders -- and knows
//! nothing about which library invented the convention, so a fixture naming one would be testing
//! something the code does not do.
//!
//! As everywhere in this directory, the silences matter more than the findings: a wrong report on a
//! call that is correct is what makes a person stop reading the list.

use expect_test::expect;

use super::diagnostics::check;

/// The plain case, both directions at once: a typo in the key leaves the message asking for a value
/// nothing supplies and the map holding one nothing reads.
#[test]
fn a_mistyped_key_is_reported_from_both_sides() {
    check(
        r#"
//- /app.php
<?php
function translate(string $message, array $values): string
{
    return $message;
}

function run(string $who): string
{
    return translate('Hello @nmae', ['@name' => $who]);
}
"#,
        expect![[r#"
            9:29-9:34 warning [missing-placeholder] Nothing supplies `@nmae`, so it is printed as written
            9:38-9:45 warning [unused-placeholder] The message has no `@name` to replace
        "#]],
    );
}

/// A typo that is a *prefix* of the placeholder is silence, and correctly so. `strtr` replaces
/// whatever it matches, so `['@nam' => 'Bob']` against `Hello @name` really does substitute -- and
/// produces `Hello Bobe`. The value is used and the placeholder is supplied; what is wrong with the
/// result is the leftover `e`, which is a different claim from either of the two made here and one
/// this check does not make.
#[test]
fn a_key_that_is_a_prefix_of_the_placeholder_does_replace_it() {
    check(
        r#"
//- /app.php
<?php
function translate(string $message, array $values): string
{
    return $message;
}

function run(string $who): string
{
    return translate('Hello @name', ['@nam' => $who]);
}
"#,
        expect![""],
    );
}

/// Three sigils, three keys. `@` escapes, `%` escapes and emphasises, `:` is for a URL -- so the
/// sigil is part of the key and a message asking for one is not answered by another.
#[test]
fn each_sigil_is_a_key_of_its_own() {
    check(
        r#"
//- /app.php
<?php
function translate(string $message, array $values): string
{
    return $message;
}

function run(string $who, string $url): string
{
    return translate('@a %b :c', ['@a' => $who, '%b' => $who, ':c' => $url]);
}
"#,
        expect![""],
    );
}

/// All three are read when the map writes all three, and each is judged against its own sigil.
#[test]
fn every_sigil_the_map_writes_is_read() {
    check(
        r#"
//- /app.php
<?php
function translate(string $message, array $values): string
{
    return $message;
}

function run(string $who): string
{
    return translate('@a %b :c', ['@z' => $who, '%z' => $who, ':z' => $who]);
}
"#,
        expect![[r#"
            9:23-9:25 warning [missing-placeholder] Nothing supplies `@a`, so it is printed as written
            9:26-9:28 warning [missing-placeholder] Nothing supplies `%b`, so it is printed as written
            9:29-9:31 warning [missing-placeholder] Nothing supplies `:c`, so it is printed as written
            9:35-9:39 warning [unused-placeholder] The message has no `@z` to replace
            9:49-9:53 warning [unused-placeholder] The message has no `%z` to replace
            9:63-9:67 warning [unused-placeholder] The message has no `:z` to replace
        "#]],
    );
}

/// The same placeholder written with the wrong sigil still points at the call, through the half of
/// the report that does not depend on the alphabet: `@name` supplies nothing. The other half is the
/// documented cost of the alphabet rule -- `%` is a sigil this call never writes a key with, so
/// `%name` is not read as a slot. See `phrost_view::placeholders` for what that rule buys.
#[test]
fn the_wrong_sigil_is_reported_through_the_unused_key() {
    check(
        r#"
//- /app.php
<?php
function translate(string $message, array $values): string
{
    return $message;
}

function run(string $who): string
{
    return translate('Hello %name', ['@name' => $who]);
}
"#,
        expect![[r#"
            9:38-9:45 warning [unused-placeholder] The message has no `@name` to replace
        "#]],
    );
}

/// A message written in a language of its own is not a message with placeholders. An XPath
/// predicate names attributes with `@` and binds values with `:`, and the map -- keyed entirely on
/// `:` -- is what says which of the two marks a slot here. Reading both sigils produced 167 wrong
/// reports of exactly this shape on a framework's own source.
#[test]
fn a_sigil_the_map_never_uses_is_that_language_s_own_syntax() {
    check(
        r#"
//- /app.php
<?php
class Page
{
    public function xpath(string $expression, array $bindings): array
    {
        return [];
    }

    public function run(string $path, string $url): void
    {
        $this->xpath('//a[@href=:path]/img[@src=:url and @width=:w]', [':path' => $path, ':url' => $url, ':w' => 1]);
    }
}
"#,
        expect![""],
    );
}

/// `::` is somebody else's operator, and both halves of it have to be passed over: an XPath axis and
/// a PostgreSQL cast were thirteen of the fourteen reports left after every other rule here.
#[test]
fn a_sigil_after_another_sigil_is_an_operator() {
    check(
        r#"
//- /app.php
<?php
class Page
{
    public function xpath(string $expression, array $bindings): array
    {
        return [];
    }

    public function query(string $sql, array $bindings): array
    {
        return [];
    }

    public function run(string $label, int $key): void
    {
        $this->xpath('//td[text()=:label]/following::a', [':label' => $label]);
        $this->query('SELECT 1 FROM t WHERE id = :key::regclass', [':key' => $key]);
    }
}
"#,
        expect![""],
    );
}

/// A method, a plain function and a constructor are the same call to this check: it reads the two
/// arguments, not the callee.
#[test]
fn a_method_a_function_and_a_constructor_read_the_same() {
    check(
        r#"
//- /app.php
<?php
class Message
{
    public function __construct(string $text, array $values) {}
}

function translate(string $message, array $values): string
{
    return $message;
}

class Page
{
    public function say(string $message, array $values): string
    {
        return $message;
    }

    public function run(string $who): void
    {
        $this->say('a @one', []);
        translate('b @two', ['@x' => $who]);
        new Message('c @three', ['@y' => $who]);
    }
}
"#,
        expect![[r#"
            22:22-22:26 warning [missing-placeholder] Nothing supplies `@two`, so it is printed as written
            22:30-22:34 warning [unused-placeholder] The message has no `@x` to replace
            23:24-23:30 warning [missing-placeholder] Nothing supplies `@three`, so it is printed as written
            23:34-23:38 warning [unused-placeholder] The message has no `@y` to replace
        "#]],
    );
}

/// An empty map says nothing about what it is. `[]` is the shape of every second argument in PHP, so
/// treating it as a placeholder map that supplies nothing would report on calls this check has no
/// business reading at all -- which is why `$this->say('a @one', [])` above is silent.
#[test]
fn an_empty_map_is_not_evidence_of_anything() {
    check(
        r#"
//- /app.php
<?php
function configure(string $name, array $options): void {}

function run(): void
{
    configure('cache@2x', []);
}
"#,
        expect![""],
    );
}

/// A map holding no key spelled like a placeholder is not a placeholder map, however the first
/// argument reads.
#[test]
fn a_map_of_ordinary_keys_is_not_a_placeholder_map() {
    check(
        r#"
//- /app.php
<?php
function configure(string $name, array $options): void {}

function run(): void
{
    configure('Save %files', ['label' => 'x', 'weight' => 1]);
}
"#,
        expect![""],
    );
}

/// A map may hold both: a logger's context array carries `exception` and `channel` beside its
/// placeholders, and the formatter drops those rather than looking for them in the message. One
/// sigil key is still evidence enough to read the rest.
#[test]
fn a_key_that_is_not_a_placeholder_is_passed_over() {
    check(
        r#"
//- /app.php
<?php
class Log
{
    public function error(string $message, array $context): void {}

    public function run(string $id): void
    {
        $this->error('failed for @id', ['@id' => $id, 'channel' => 'app', 'link' => '']);
    }
}
"#,
        expect![""],
    );
}

/// A message that is not one literal says nothing about which placeholders it holds. Every one of
/// these is silence rather than a guess.
#[test]
fn a_message_that_is_not_a_literal_says_nothing() {
    check(
        r#"
//- /app.php
<?php
const PREFIX = 'Hello ';

function translate(string $message, array $values): string
{
    return $message;
}

function run(string $message, string $who): void
{
    translate($message, ['@name' => $who]);
    translate('Hello ' . '@other', ['@name' => $who]);
    translate(PREFIX, ['@name' => $who]);
    translate("Hello $who", ['@name' => $who]);
}
"#,
        expect![""],
    );
}

/// A literal holding a backslash is declined: what `strtr` matches against is the decoded string, so
/// every offset after the escape would point at the wrong character in the file and a report would
/// underline the wrong span.
#[test]
fn an_escaped_literal_is_declined_because_its_offsets_would_be_wrong() {
    check(
        r#"
//- /app.php
<?php
function translate(string $message, array $values): string
{
    return $message;
}

function run(string $who): void
{
    translate('It\'s @name', ['@nope' => $who]);
}
"#,
        expect![""],
    );
}

/// A second argument that is not an array literal is not a map this can read, and neither is one
/// written by name.
#[test]
fn a_map_that_is_not_a_literal_says_nothing() {
    check(
        r#"
//- /app.php
<?php
function translate(string $message, array $values): string
{
    return $message;
}

function run(array $values, string $who): void
{
    translate('Hello @name', $values);
    translate('Hello @name', values: ['@nope' => $who]);
}
"#,
        expect![""],
    );
}

/// A spread into the map could carry the very key that otherwise reads as missing, so the
/// unsupplied direction declines. The unused direction survives it: nothing another element brings
/// can make a literal key the message never writes into one it does.
#[test]
fn a_spread_stops_the_unsupplied_direction_and_not_the_other() {
    check(
        r#"
//- /app.php
<?php
function translate(string $message, array $values): string
{
    return $message;
}

function run(array $rest, string $who): void
{
    translate('Hello @name', ['@nope' => $who, ...$rest]);
}
"#,
        expect![[r#"
            9:31-9:38 warning [unused-placeholder] The message has no `@nope` to replace
        "#]],
    );
}

/// A key this layer cannot name could be the missing one, for the same reason a spread could.
#[test]
fn a_computed_key_stops_the_unsupplied_direction() {
    check(
        r#"
//- /app.php
<?php
function translate(string $message, array $values): string
{
    return $message;
}

function run(string $key, string $who): void
{
    translate('Hello @name', ['@known' => $who, $key => $who]);
}
"#,
        expect![[r#"
            9:31-9:39 warning [unused-placeholder] The message has no `@known` to replace
        "#]],
    );
}

/// The third argument is options -- a langcode, a context -- and never a second map. A call carrying
/// one is still read, and the options are not read as placeholders.
#[test]
fn a_third_argument_is_options_and_is_not_read_as_a_map() {
    check(
        r#"
//- /app.php
<?php
function translate(string $message, array $values, array $options = []): string
{
    return $message;
}

function run(string $who): void
{
    translate('Hello @name', ['@name' => $who], ['langcode' => 'de', 'context' => '@count']);
}
"#,
        expect![""],
    );
}

/// A message whose placeholders sit at another position is a different contract, and this reads
/// none of it. A plural form takes `(count, singular, plural, values)` and supplies `@count`
/// itself; a condition builder takes `(table, alias, condition, values)` and its condition is SQL,
/// where `'%Y-%m-%d %H:%i'` is a date format rather than four placeholders.
#[test]
fn only_the_first_two_arguments_are_a_message_and_a_map() {
    check(
        r#"
//- /app.php
<?php
class Query
{
    public function plural(int $count, string $one, string $many, array $values): string
    {
        return $many;
    }

    public function join(string $table, string $alias, string $on, array $values): void {}

    public function run(int $count, string $type): void
    {
        $this->plural($count, '1 @type', '@count @type items', ['@type' => $type]);
        $this->join('t', 'a', "a.at > :from AND DATE_FORMAT(a.at, '%Y-%m-%d %H:%i') = :day", [':from' => 1, ':day' => 2]);
    }
}
"#,
        expect![""],
    );
}

/// An address is not a placeholder. The sigil has to start a word: `support@example.com` has `@`
/// after a word character, and so does the `:30` of a clock time.
#[test]
fn a_sigil_in_the_middle_of_a_word_is_part_of_the_word() {
    check(
        r#"
//- /app.php
<?php
function translate(string $message, array $values): string
{
    return $message;
}

function run(string $who): string
{
    return translate('Mail support@example.com before 12:30 about @name', ['@name' => $who]);
}
"#,
        expect![""],
    );
}

/// A sigil followed by punctuation or nothing is not a placeholder either: `100%`, `Note:`, an `@`
/// on its own.
#[test]
fn a_sigil_needs_a_word_character_after_it() {
    check(
        r#"
//- /app.php
<?php
function translate(string $message, array $values): string
{
    return $message;
}

function run(string $who): string
{
    return translate('Note: 100% done, @ last -- @name', ['@name' => $who]);
}
"#,
        expect![""],
    );
}

/// Replacement is a substring match, not a comparison of two token sets. A key holding a character
/// the scanner stops at still supplies the placeholder the scanner found at that offset -- the same
/// answer `strtr` gives.
#[test]
fn a_key_reaching_past_the_scanned_word_still_supplies_it() {
    check(
        r#"
//- /app.php
<?php
function translate(string $message, array $values): string
{
    return $message;
}

function run(string $who): string
{
    return translate('Owner @node.title here', ['@node.title' => $who]);
}
"#,
        expect![""],
    );
}

/// A `-` or `.` between two word characters is part of the name, so a report quotes the whole of it.
/// At the end of a token it is punctuation: a sentence closing on `@date.` names `@date`.
#[test]
fn a_hyphen_between_word_characters_stays_in_the_name() {
    check(
        r#"
//- /app.php
<?php
function translate(string $message, array $values): string
{
    return $message;
}

function run(string $url, string $when): string
{
    return translate('See <a href=":views-ui">Views</a> by @date.', [':views_ui' => $url, '@date' => $when]);
}
"#,
        expect![[r#"
            9:36-9:45 warning [missing-placeholder] Nothing supplies `:views-ui`, so it is printed as written
            9:70-9:81 warning [unused-placeholder] The message has no `:views_ui` to replace
        "#]],
    );
}

/// Every occurrence is its own place in the output, so every occurrence is its own finding: a reader
/// fixing the message needs to see both `@name`s.
#[test]
fn each_occurrence_of_an_unsupplied_placeholder_is_reported() {
    check(
        r#"
//- /app.php
<?php
function translate(string $message, array $values): string
{
    return $message;
}

function run(string $who): string
{
    return translate('@name and @name again, @ok', ['@ok' => $who]);
}
"#,
        expect![[r#"
            9:23-9:28 warning [missing-placeholder] Nothing supplies `@name`, so it is printed as written
            9:33-9:38 warning [missing-placeholder] Nothing supplies `@name`, so it is printed as written
        "#]],
    );
}

/// A double-quoted message with no escapes is the same string as a single-quoted one, and reads the
/// same.
#[test]
fn a_double_quoted_message_reads_the_same() {
    check(
        r#"
//- /app.php
<?php
function translate(string $message, array $values): string
{
    return $message;
}

function run(string $who): string
{
    return translate("Hello @name", ['@other' => $who]);
}
"#,
        expect![[r#"
            9:29-9:34 warning [missing-placeholder] Nothing supplies `@name`, so it is printed as written
            9:38-9:46 warning [unused-placeholder] The message has no `@other` to replace
        "#]],
    );
}
