//! What must hold of every token, whatever arm produced it.
//!
//! Two of these are protocol requirements rather than taste. A token that crosses a line cannot be
//! encoded at all, and two tokens over the same characters have no defined meaning -- the client
//! reads a flat sequence of deltas, so an overlap makes everything after it land in the wrong place.
//! Neither is visible in a snapshot of one construct; both are certain to happen eventually, because
//! thirty arms fire independently and two of them fire on everything.

use super::all;
use crate::tokens::TokenKind;

/// Something of every shape the arms handle, in one file.
const DENSE: &str = r#"<?php

namespace App\Nested\Deep;

use App\First;
use App\Second as Alias;
use function App\helper;
use const App\LIMIT;
use App\Group\{Thing, function act, const CAP};

const LOCAL = 1;

#[Route('/x')]
abstract class Repo extends Base implements Reads, Writes
{
    use Counts;

    public const int MAX = 10;

    protected static array $seen = [];

    public function __construct(
        private readonly Base $base,
        public ?First $first = null,
    ) {
    }

    /**
     * The docblock arm is here because it is the one that broke both invariants: a type written
     * across lines is the only token any arm can produce that spans more than one, and carving a
     * shape's keys out of it is the only place two tokens can be built over the same characters.
     *
     * @param array{
     *   name: string,
     *   nested: array{deep?: int}
     * } $shape
     * @param \App\Filter|null $filter
     * @return ?Second
     */
    abstract public function find(int $id, array $shape, $filter): ?Second;

    final public static function make(string|Reads $from): static
    {
        $local = self::MAX;
        $local = static::make('x');
        $local = parent::find(1);
        $local = new \App\Other\Thing();
        $local = Alias::LIMIT;
        $local = $from->read()?->value;
        $local = Repo::$seen;
        $local = helper($local);
        $local = LIMIT;
        $local = fn (int $n) => $n + $id;
        $local = function (int $n) use ($local) {
            return $n;
        };
        return new static();
    }
}

interface Reads extends Countable
{
    public function read(): string;
}

trait Counts
{
    public int $total = 0 {
        get => $this->total;
    }
}

enum Suit: string implements Reads
{
    case Hearts = 'h';

    public function read(): string
    {
        return Suit::Hearts->value;
    }
}
"#;

#[test]
fn no_token_crosses_a_line() {
    // The protocol encodes a token as a line, a start column and a length. There is no way to say
    // "and it continues on the next line".
    for token in all(DENSE) {
        let text = &DENSE[usize::from(token.range.start())..usize::from(token.range.end())];
        assert!(
            !text.contains('\n'),
            "multi-line token: {token:?} = {text:?}"
        );
    }
}

#[test]
fn no_two_tokens_cover_the_same_character() {
    // An overlap does not merely colour one name twice: the client reads deltas in order, so
    // everything after the overlap lands somewhere it does not belong.
    let tokens = all(DENSE);
    for pair in tokens.windows(2) {
        let (left, right) = (pair[0], pair[1]);
        assert!(
            left.range.end() <= right.range.start(),
            "overlapping tokens: {left:?} then {right:?}"
        );
    }
}

#[test]
fn no_token_is_empty_or_outside_the_text() {
    for token in all(DENSE) {
        let (start, end) = (
            usize::from(token.range.start()),
            usize::from(token.range.end()),
        );
        assert!(start < end, "empty or inside-out token: {token:?}");
        assert!(
            end <= DENSE.len(),
            "token past the end of the text: {token:?}"
        );
    }
}

#[test]
fn every_name_token_covers_something_that_could_be_a_name() {
    // The check that catches an arm reaching for the wrong field: a token over `, ` or over `->`
    // still has a plausible kind, and only its text shows that it is wrong.
    //
    // Asked of the kinds the *tree* decided, and of no others. A keyword, a comment or a string is a
    // lexical answer over whatever the lexer said it was, and `<?php`, `(int)` and `/** ... */` are
    // all correct and none of them is a name.
    for token in all(DENSE) {
        if !matches!(
            token.kind,
            TokenKind::Namespace
                | TokenKind::Class
                | TokenKind::Interface
                | TokenKind::Enum
                | TokenKind::EnumMember
                | TokenKind::Property
                | TokenKind::Method
                | TokenKind::Function
                | TokenKind::Parameter
                | TokenKind::Variable
        ) {
            continue;
        }
        let text = &DENSE[usize::from(token.range.start())..usize::from(token.range.end())];
        assert!(
            text.chars()
                .all(|character| character.is_alphanumeric()
                    || matches!(character, '_' | '$' | '\\')),
            "token over something that is not a name: {token:?} = {text:?}"
        );
        // A namespace token is a *path* and the separators bounding it are part of it -- that is
        // what makes `Drupal\\app_core\\Supplier\\Product\\ProductType` one colour up to the
        // name rather than three separators coloured and the fourth not. Every other kind here
        // names one thing, and a separator at either end of one of those means the token was
        // measured off the wrong offset.
        if token.kind != TokenKind::Namespace {
            assert!(
                !text.starts_with('\\') && !text.ends_with('\\'),
                "token includes a separator it should not: {token:?} = {text:?}"
            );
        }
    }
}

/// Nothing a person can see is left uncoloured, which is the whole claim `full` mode rests on.
///
/// Whitespace and the characters a theme gives the plain foreground -- brackets, commas, semicolons,
/// operators -- are deliberately unclaimed. Everything else is either a name or a lexical token, and
/// a gap anywhere else means a construct nobody thought about draws as plain text in an editor that
/// has been told to trust this list.
#[test]
fn every_word_of_the_file_is_claimed_by_something() {
    let tokens = all(DENSE);
    let mut claimed = vec![false; DENSE.len()];
    for token in &tokens {
        let (start, end) = (
            usize::from(token.range.start()),
            usize::from(token.range.end()),
        );
        claimed[start..end].fill(true);
    }

    // Word characters only: those are what carry meaning a reader looks for, and what a missing arm
    // would leave grey.
    const PUNCTUATION: &str = "(){}[];,:?!<>=+-*/%&|^~.@#";
    for (at, character) in DENSE.char_indices() {
        if claimed[at]
            || character.is_whitespace()
            || PUNCTUATION.contains(character)
            || matches!(character, '\'' | '"' | '\\' | '$')
        {
            continue;
        }
        let line = DENSE[..at].lines().count();
        panic!("nothing claims {character:?} at offset {at} (line {line})");
    }
}

#[test]
fn the_dense_fixture_actually_exercises_every_kind() {
    // Without this the four tests above pass on an empty list, which is how a property test comes
    // to guard nothing.
    use crate::tokens::TokenKind;

    let kinds = all(DENSE)
        .into_iter()
        .map(|token| token.kind)
        .collect::<Vec<_>>();

    for kind in [
        TokenKind::Namespace,
        TokenKind::Class,
        TokenKind::Interface,
        TokenKind::Enum,
        TokenKind::EnumMember,
        TokenKind::Property,
        TokenKind::Method,
        TokenKind::Function,
        TokenKind::Parameter,
        TokenKind::Variable,
        TokenKind::Decorator,
    ] {
        assert!(kinds.contains(&kind), "the fixture produces no {kind:?}");
    }
}
