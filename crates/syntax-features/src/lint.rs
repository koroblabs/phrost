//! What a file has wrong with it that one file can tell.
//!
//! mago's linter, behind an API that says nothing about mago. 192 rules over syntax and resolved
//! names -- no type engine, no project index -- so the whole of it is one file's own business and
//! costs nothing that grows with a project.
//!
//! Nothing is invented here either. A PHP linter is several hundred settled arguments about style
//! and safety, and re-deciding them is not work this project should be doing.
//!
//! # Which rules
//!
//! The integrations are the point. A rule that is right for one framework is wrong for another:
//! Drupal's own standard writes `TRUE` and `NULL` in upper case, and a linter that does not know
//! which framework it is looking at reports every one of them. So the framework decides, through the
//! same seam that already decides everything else framework-shaped.

use mago_allocator::LocalArena;
use mago_database::file::File;
use mago_linter::{Linter, integration::Integration, settings::Settings};
use mago_names::resolver::NameResolver;
use mago_reporting::{AnnotationKind, Issue, Level};
use mago_span::{HasSpan, Span};
use mago_syntax::cst::{Binary, Expression, ForeachTarget, Literal, Node, Program, Variable};
use phrost_model::{PhpVersion, TextRange};
use std::borrow::Cow;

use phrost_syntax::text_range;

mod deprecation_notice;
mod descent;
mod format_values;
mod php_floor;

use deprecation_notice::DeprecationNotice;
use format_values::FormatValues;
use php_floor::PhpFloor;

/// One thing a rule found.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct LintIssue {
    /// Where to underline. The first annotation's span, which is the one a rule points at.
    pub range: TextRange,
    /// The rule's name, which is what a configuration file switches off.
    pub code: String,
    pub message: String,
    pub severity: LintSeverity,
    /// What the rule suggests doing, when it suggests anything.
    pub help: Option<String>,
    pub category: LintCategory,
}

/// What kind of claim a rule makes.
///
/// The split that decides what a person is shown without asking. Four of these state facts about the
/// code -- it is wrong, it is unsafe, it is a hole, it is removed -- and five state preferences about
/// how it should read. Measured on a real project, all nine at once produce 8241 findings against 309
/// from the index; the four are a different order of magnitude and none of them is an opinion.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LintCategory {
    Correctness,
    Safety,
    Security,
    Deprecation,
    BestPractices,
    Clarity,
    Consistency,
    Maintainability,
    Redundancy,
}

impl LintCategory {
    /// The categories that state a fact rather than a preference.
    ///
    /// What a reader gets unasked. The rest are worth having and are somebody's house style, which
    /// is a different thing from a defect and belongs behind a decision.
    pub const FACTUAL: &'static [Self] = &[
        Self::Correctness,
        Self::Safety,
        Self::Security,
        Self::Deprecation,
    ];

    /// The category a name asks for, or nothing when nothing goes by that name.
    pub fn named(name: &str) -> Option<Self> {
        Some(match name {
            "correctness" => Self::Correctness,
            "safety" => Self::Safety,
            "security" => Self::Security,
            "deprecation" => Self::Deprecation,
            "best-practices" => Self::BestPractices,
            "clarity" => Self::Clarity,
            "consistency" => Self::Consistency,
            "maintainability" => Self::Maintainability,
            "redundancy" => Self::Redundancy,
            _ => return None,
        })
    }

    pub const ALL: &'static [Self] = &[
        Self::Correctness,
        Self::Safety,
        Self::Security,
        Self::Deprecation,
        Self::BestPractices,
        Self::Clarity,
        Self::Consistency,
        Self::Maintainability,
        Self::Redundancy,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Correctness => "correctness",
            Self::Safety => "safety",
            Self::Security => "security",
            Self::Deprecation => "deprecation",
            Self::BestPractices => "best-practices",
            Self::Clarity => "clarity",
            Self::Consistency => "consistency",
            Self::Maintainability => "maintainability",
            Self::Redundancy => "redundancy",
        }
    }

    fn of(category: mago_linter::category::Category) -> Self {
        use mago_linter::category::Category;

        match category {
            Category::Correctness => Self::Correctness,
            Category::Safety => Self::Safety,
            Category::Security => Self::Security,
            Category::Deprecation => Self::Deprecation,
            Category::BestPractices => Self::BestPractices,
            Category::Clarity => Self::Clarity,
            Category::Consistency => Self::Consistency,
            Category::Maintainability => Self::Maintainability,
            Category::Redundancy => Self::Redundancy,
        }
    }
}

/// How much a finding matters.
///
/// mago's five levels folded to three, because that is what the protocol has. `Note` and `Help` both
/// become a hint: they are the levels a rule uses when it is offering rather than objecting.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LintSeverity {
    Error,
    Warning,
    Hint,
}

/// Every issue the rules find in `text`, with the framework integrations named by `integrations`.
///
/// A name that matches no integration is ignored rather than refused: the list comes from framework
/// detection, and a framework the linter has never heard of should cost nothing rather than break
/// linting for the file.
///
/// What is reported comes from `selection`, which is built once -- see
/// [`ProjectConfig`](phrost_project::ProjectConfig) for why there is only one place that builds it.
/// `name` is the file's path as the project sees it, and it is not decoration: rules judge a file by
/// what it is called -- whether the class inside matches the name it is in -- and a placeholder makes
/// every one of them fire. Measured: with a stand-in name, `file-name` alone reported 1586 findings on
/// a 1662-file project, one per file.
///
/// `php` is the oldest PHP the project says it runs on -- composer's `require.php` -- or `None`
/// where nothing says. It decides which rules gated on a version speak; see [`php_floor`] for why
/// a deprecation is not one of them.
/// Which rules a run reports, as one value.
///
/// Category alone is not a good enough filter, and that is measured: with every category on, a real
/// Drupal project reports 6 855 findings, and the largest single rule is `strict-types` at 987 -- a
/// rule that says to write `declare(strict_types=1)`, which Drupal core deliberately does not.
/// `strict-types` is a *correctness* rule, and correctness is the category a person most wants. So a
/// project has to be able to name one rule, in both directions.
#[derive(Clone, Copy, Default, Debug)]
pub struct LintSelection<'a> {
    pub categories: &'a [LintCategory],
    /// Never reported, whatever the category says.
    pub off: &'a [String],
    /// Always reported, whatever the category says.
    pub on: &'a [String],
}

/// Rules whose category says fact and whose content says preference.
///
/// Not the same list as a project's `off`, and the difference is the point: a project naming a rule
/// is the last word, while this is only a default and `on` overrides it. A shipped list that could
/// not be overridden would be this crate deciding a settled argument on somebody else's behalf,
/// which its own docs say is not work this project should do.
///
/// Each entry needs a measured reason.
const PREFERENCE_IN_A_FACTUAL_CATEGORY: &[&str] = &[
    // `if ($offer = $application->getSelectedQuote())` is *the* PHP idiom for "get it and use it if
    // it is there", and `phrost-body`'s narrowing has an arm for it by name -- the engine reads it,
    // narrows through it, and then this told the author not to write it. 151 findings on one real
    // project, all of them that shape. A C-era typo hazard filed under correctness.
    "no-assign-in-condition",
    // "Parameters that may contain sensitive information should be marked with the
    // `#[SensitiveParameter]` attribute", at error level, decided by the parameter's *name*: any
    // that ends in `token`, `password` or `secret`. 450 findings on four frameworks' own source,
    // and of fourteen read, one was a password. The rest were `TokenInterface $token` six times
    // over, `OidcLogoutToken $logoutToken`, a console lexer's `parseToken(string $token)`, a MIME
    // header's `tokenNeedsEncoding(string $token)`, a profiler's `setToken(string $token)` and a
    // workflow's `int $nbToken` -- how many tokens a Petri net's place holds. The attribute keeps a
    // value out of a stack trace, which prints an object as its class name and has nothing to say
    // about a count. It needs PHP 8.2, so until the linter was told which PHP it reads it never
    // ran; this keeps that true of the editor's default and leaves the rule one name away.
    "sensitive-parameter",
];

impl LintSelection<'_> {
    /// Whether a rule with this code and category is reported.
    ///
    /// A named rule wins over its category, and `off` wins over `on`: the two are a project's own
    /// words and the only way to be wrong about them is to guess which it meant, so the safer of the
    /// two answers is the one that reports less.
    fn wants(&self, code: &str, category: LintCategory) -> bool {
        if self.off.iter().any(|name| name == code) {
            return false;
        }
        if self.on.iter().any(|name| name == code) {
            return true;
        }
        // Checked after `on` and before the category, which is what makes it a default rather than
        // a decision: a project that wants the rule says so and gets it.
        if PREFERENCE_IN_A_FACTUAL_CATEGORY.contains(&code) {
            return false;
        }
        self.categories.contains(&category)
    }
}

/// `no-literal-password`'s heuristic is a suffix match on a *name* -- `password`, `token`,
/// `secret`, `api_key` -- with almost nothing asked of the *value*: mago reports any literal longer
/// than two characters (quotes included) next to one of those names. Hand-audited against every
/// finding on a real Drupal project: 21 fired, and 6 of them held a value that was never a
/// credential --
///
///   - `['token' => 'Missing required parameter.']`, an error-response key, not a stored token;
///   - `'access_token' => 'string'` and `'refresh_token' => 'string'` twice over, a schema mapping
///     field names to their PHP type, where `'string'` is the type's own name;
///   - `'resetPassword' => '/securecontrol/reset/passwordreset'`, an API route, not a password;
///   - `'password' => 'password'`, a config-key lookup table mapping a display name to the
///     Drupal config key that holds the real value, which happens to repeat its own name;
///   - `$data['clientSecret'] = 'warehouse/sanctions/credentials'`, a pointer to where a secret
///     store keeps the secret, not the secret.
///
/// The other 15 -- ten real-looking passwords and API keys hardcoded into a settings file, five
/// obviously-fake values in test fixtures -- are exactly what the rule exists to catch, so the fix
/// is not to drop the rule from the factual set: that would hide the ten findings that matter most.
/// It is also not enough on its own to explain the bug report that started this audit --
/// `private string $secret = 'x';` -- because `'x'` shares none of the six shapes above; it fails a
/// simpler test, that no real password/token/secret is one character long.
///
/// So the value is judged, narrowly, on shapes a credential cannot have: too short to be one, a
/// sentence (contains a space), a path (see [`looks_like_a_path`]), a PHP type's own name, or
/// identical to the name that supposedly holds it. Every one of the six measured false positives
/// fails exactly one of these, and none of the fifteen real findings does.
///
/// The direction of the trade is deliberate and is the opposite of the usual one. This is a
/// security rule, so a miss is silent and a false positive is merely visible, and every test here
/// is written to be a shape a generated credential *cannot* take rather than one it usually does
/// not. The one cost knowingly accepted is a hardcoded passphrase with spaces in it, which is
/// rarer than an error message sitting next to a key called `token`.
fn is_credential_shaped(name: &str, value: &str) -> bool {
    let name = unwrapped(name);
    let value = unwrapped(value);

    if value.len() <= 3 || value.contains(' ') || looks_like_a_path(value) {
        return false;
    }
    if value.eq_ignore_ascii_case(name) {
        return false;
    }

    const PHP_TYPE_KEYWORDS: &[&str] = &[
        "string", "int", "integer", "float", "double", "bool", "boolean", "array", "object",
        "mixed", "void", "null", "iterable", "self", "static", "parent", "callable", "true",
        "false", "never",
    ];
    // The other vocabulary a value names a type in: an attribute-casting map, where
    // `'password' => 'hashed'` says how the field is stored, not what it holds. It is the first
    // thing a new Laravel application declares, so leaving it out reported every fresh install.
    const CAST_TYPES: &[&str] = &[
        "hashed",
        "encrypted",
        "date",
        "datetime",
        "immutable_date",
        "immutable_datetime",
        "timestamp",
        "collection",
        "real",
    ];
    const CAST_TYPE_PREFIXES: &[&str] = &["encrypted:", "decimal:", "datetime:", "date:"];
    !PHP_TYPE_KEYWORDS
        .iter()
        .chain(CAST_TYPES)
        .any(|keyword| value.eq_ignore_ascii_case(keyword))
        && !CAST_TYPE_PREFIXES
            .iter()
            .any(|prefix| value.to_ascii_lowercase().starts_with(prefix))
}

/// A slash-separated run of lowercase words: a route, a config path, a key into a secret store.
///
/// Not simply "contains a slash", which is what this started as. base64 and bcrypt both use `/` in
/// their alphabets, so rejecting every value with one in it would drop exactly the encoded secrets
/// this rule is most worth having for -- silently, which is the expensive direction for a security
/// finding. What actually separates a path from a token is character classes: a generated
/// credential carries capitals or digits and a path written by a person does not.
fn looks_like_a_path(value: &str) -> bool {
    value.contains('/')
        && !value
            .bytes()
            .any(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit())
}

/// Strips a leading `$` or one matching pair of quotes, so a variable, a property, a constant and
/// an array key -- the shapes `no-literal-password` reports on -- compare on the same footing.
fn unwrapped(text: &str) -> &str {
    if let Some(stripped) = text.strip_prefix('$') {
        return stripped;
    }
    let bytes = text.as_bytes();
    if bytes.len() >= 2 && matches!(bytes[0], b'\'' | b'"') && bytes[bytes.len() - 1] == bytes[0] {
        return &text[1..text.len() - 1];
    }
    text
}

/// The source text an annotation points at.
fn annotation_text(text: &str, span: Span) -> &str {
    text.get(span.start.offset as usize..span.end.offset as usize)
        .unwrap_or_default()
}

/// mago's own advice for silencing a finding, and phrost's -- found by running every rule over a
/// real project's `--lint all` and grepping every `message` and `help` for mago's vocabulary
/// (`@mago-expect`, `mago.toml`, "mago" itself), and cross-checked against mago-linter 1.47.4's own
/// source for the same words. Of 192 rules, exactly one names itself: `no-empty-catch-clause`'s
/// `help` tells the reader to add a `@mago-expect` comment. phrost's users have never heard of
/// mago, and the way to silence a rule in *phrost* is naming its `code` in `phrost.toml`'s `[lint]`
/// table (see [`phrost_project::config::LintConfig`]), not a comment mago's own linter reads.
///
/// A substring table rather than a branch on `code`: the replacement is keyed to mago's own
/// wording, not to which rule happened to say it, so a future mago version reusing the same phrase
/// on a different rule is still caught. If mago ever changes the wording, the substring stops
/// matching and mago's own text starts appearing again in `lint/mago_vocabulary.rs`'s snapshot --
/// loud, not a silent leak.
const MAGO_SUPPRESSION_ADVICE: &[(&str, &str)] = &[(
    "If ignoring it is intentional, add a `@mago-expect` comment explaining why.",
    "If ignoring it is intentional, turn the rule off for the project instead of the finding: add \
     its code to `phrost.toml`'s `[lint]` table, under `off`.",
)];

fn speaking_only_of_phrost(text: std::borrow::Cow<'_, str>) -> String {
    let mut text = text.into_owned();
    for (mago_wording, phrost_wording) in MAGO_SUPPRESSION_ADVICE {
        if text.contains(mago_wording) {
            text = text.replace(mago_wording, phrost_wording);
        }
    }
    text
}

/// What one walk of the tree tells the `no-insecure-comparison` filter.
///
/// The rule matches on a *name* the way `no-literal-password` does -- anything ending in `token`,
/// `password`, `secret` or `apikey` -- so it reports a value being routed on as a credential being
/// checked. What it hands over is the operator's span and the operands it judged sensitive, and
/// both filters below need the operand it *dismissed*: the finding has to be turned back into the
/// expression it is about, and only the tree can do that.
struct Comparisons<'arena> {
    /// Every equality comparison in the file. `no-insecure-comparison` points its primary
    /// annotation at exactly `binary.operator.span()`, which is how a finding is matched to one.
    equalities: Vec<&'arena Binary<'arena>>,
    /// The names a `foreach` binds, `$` included, as written.
    iterated: Vec<&'arena [u8]>,
}

impl<'arena> Comparisons<'arena> {
    fn of(program: &'arena Program<'arena>) -> Self {
        let mut held = Self {
            equalities: Vec::new(),
            iterated: Vec::new(),
        };

        // An explicit worklist rather than recursion: a generated file can hold a thousand-deep
        // expression, and a lint runs on every keystroke of an open buffer.
        let mut pending = vec![Node::Program(program)];
        while let Some(node) = pending.pop() {
            match node {
                Node::Binary(binary) if binary.operator.is_equality() => {
                    held.equalities.push(binary);
                }
                Node::Foreach(r#foreach) => {
                    let (key, value) = match &foreach.target {
                        ForeachTarget::Value(target) => (None, target.value),
                        ForeachTarget::KeyValue(target) => (Some(target.key), target.value),
                    };
                    held.iterated.extend(
                        [key, Some(value)]
                            .into_iter()
                            .flatten()
                            .filter_map(direct_variable),
                    );
                }
                _ => {}
            }
            node.visit_children(|child| pending.push(child));
        }

        held
    }

    /// Whether the finding whose operator is at `operator` is a value being routed on.
    ///
    /// Two shapes, and neither is a guess about what the code means -- each is a statement about
    /// what the *name* the rule matched can be evidence of.
    ///
    /// **Nobody authenticates against a string written into the source.** A secret compared to a
    /// literal is not a timing problem, it is a hardcoded credential, and that is
    /// `no-literal-password`'s finding rather than this one's -- so dropping it here loses no
    /// report, it removes a second one about the same line. mago has the rule too and stops one
    /// step short of it: `is_simple_literal` treats a string as simple only when its raw form is
    /// two bytes long, so a comparison against `''` is dismissed and one against `')'` is reported.
    /// Measured over four corpora, that one step is 24 of 56 findings -- `'{' === $token` in
    /// Symfony's JSON lexer, `'--' == $token` in its argv parser, `$token === ')'` in Laravel's
    /// Blade compiler.
    ///
    /// **A name a `foreach` binds is the name of one element, and the collection is what decides
    /// what the elements are.** A name heuristic cannot ask a collection -- a collection's name is
    /// plural, and the two plurals a real project offered were `$this->tokens` (an argv parser's
    /// arguments) and `self::streetTokens()` (the words of an address) -- so a loop variable is not
    /// evidence about the value. Only where the other operand is a bare local too: a stored
    /// credential is fetched from somewhere, and every genuine finding in the corpora has a call, a
    /// property or a subscript on one side (`$token === $child->getToken()`,
    /// `$this->locks[$key] === $token`, `$this->getPassword() !== $user->getPassword()`).
    ///
    /// What that second one knowingly costs is `foreach ($tokens as $token) { if ($token ===
    /// $given) }`, a candidate checked against a list. It is the shape the rule exists for and it
    /// went unmeasured on 35 000 files of five projects, while the loop over lexical tokens is what
    /// both real reports were.
    fn is_dispatch(&self, operator: Span) -> bool {
        let Some(binary) = self
            .equalities
            .iter()
            .find(|binary| binary.operator.span() == operator)
        else {
            // A filter that cannot find what it is judging must not act as though it judged it.
            return false;
        };

        if is_a_written_string(binary.lhs) || is_a_written_string(binary.rhs) {
            return true;
        }

        match (direct_variable(binary.lhs), direct_variable(binary.rhs)) {
            (Some(left), Some(right)) => {
                self.iterated.contains(&left) || self.iterated.contains(&right)
            }
            _ => false,
        }
    }
}

/// The name of a plain `$variable`, through any parentheses around it.
fn direct_variable<'arena>(expression: &'arena Expression<'arena>) -> Option<&'arena [u8]> {
    match expression {
        Expression::Parenthesized(parenthesized) => direct_variable(parenthesized.expression),
        Expression::Variable(Variable::Direct(variable)) => Some(variable.name),
        _ => None,
    }
}

/// Whether the expression is a string written out in the source, through any parentheses around it.
fn is_a_written_string(expression: &Expression<'_>) -> bool {
    match expression {
        Expression::Parenthesized(parenthesized) => is_a_written_string(parenthesized.expression),
        Expression::Literal(Literal::String(_)) => true,
        _ => false,
    }
}

/// Whether a `no-literal-password` finding's value looks like it could be a credential at all.
///
/// `name_span` is the primary annotation -- the name the rule matched -- and the value is the
/// secondary one, found by kind rather than by position: `no-literal-password` always adds exactly
/// these two, but nothing here should assume an order it does not need. A finding with no secondary
/// annotation is reported rather than dropped -- a filter that cannot judge a value must not act as
/// though it judged it and found nothing wrong.
fn password_finding_is_credential_shaped(text: &str, issue: &Issue, name_span: Span) -> bool {
    let Some(value_span) = issue
        .annotations
        .iter()
        .find(|annotation| annotation.kind == AnnotationKind::Secondary)
        .map(|annotation| annotation.span)
    else {
        return true;
    };

    is_credential_shaped(
        annotation_text(text, name_span),
        annotation_text(text, value_span),
    )
}

pub fn lint(
    text: &str,
    name: &str,
    integrations: &[&str],
    php: Option<PhpVersion>,
    selection: LintSelection<'_>,
) -> Vec<LintIssue> {
    let arena = LocalArena::new();
    let file = File::ephemeral(
        Cow::Owned(name.as_bytes().to_vec()),
        Cow::Owned(text.as_bytes().to_vec()),
    );
    let program = mago_syntax::parser::parse_file(&arena, &file);
    let resolved = NameResolver::new(&arena).resolve(program);

    // Built for the newest PHP, so that every rule gated on a version exists to be asked; which of
    // them speak to this project is the floor's to say, below.
    let mut settings = Settings {
        php_version: PhpFloor::NEWEST,
        ..Settings::default()
    };
    for name in integrations {
        if let Ok(integration) = name.parse::<Integration>() {
            settings.integrations.insert(integration);
        }
    }
    // Named rules are switched at the *settings*, not filtered out of the answer.
    //
    // Filtering can only ever hide a rule that ran. Some rules are off by default -- `missing-docs`
    // is one -- and no filter can turn one of those on: the linter never produces the issue for a
    // filter to keep. Naming it did nothing at all and looked exactly like a setting that does not
    // work, which is the failure this whole file is meant to avoid.
    if let Some(rules) = named_rules(selection) {
        settings.rules = rules;
    }

    let linter = Linter::new(&arena, &settings, None, false);
    let floor = PhpFloor::of(php);
    // Which category each rule speaks for, asked of the linter rather than listed here: a list would
    // go stale silently, and a rule whose category this did not know would be filtered out of every
    // answer with nothing to say so.
    let categories_by_code = linter
        .rules()
        .iter()
        // By `code` and not by `name`: the code is what an issue carries and what a configuration
        // file switches off, while the name is the human one. Keying on the name silently matched
        // nothing, so every issue was filtered out and the project looked clean.
        //
        // A rule the project's oldest PHP does not admit is left out, which is what drops its
        // findings below: decided here, once per rule, rather than once per finding.
        .filter_map(|rule| {
            let meta = rule.meta();
            let category = LintCategory::of(meta.category);
            floor
                .admits(category, meta.requirements)
                .then_some((meta.code, category))
        })
        .collect::<std::collections::HashMap<_, _>>();

    // Built on the first `no-insecure-comparison` finding and not before: it is one walk of the
    // tree, and a lint runs on every keystroke of an open buffer while that rule fires on almost no
    // file at all -- one on 1 669, four on WordPress's 1 900.
    let mut comparisons: Option<Comparisons<'_>> = None;

    linter
        .lint(&file, program, &resolved)
        .into_iter()
        .filter_map(|issue| {
            // The first annotation is what the rule points at; an issue with none has nothing to
            // underline and would land at the top of the file.
            let span = issue.annotations.first()?.span;
            let code = issue.code.as_deref()?;
            let category = *categories_by_code.get(code)?;
            if !selection.wants(code, category) {
                return None;
            }
            if code == "no-literal-password"
                && !password_finding_is_credential_shaped(text, &issue, span)
            {
                return None;
            }
            if code == "no-insecure-comparison"
                && comparisons
                    .get_or_insert_with(|| Comparisons::of(program))
                    .is_dispatch(span)
            {
                return None;
            }
            if code == "no-error-control-operator"
                && DeprecationNotice::is_silenced_at(program, span)
            {
                return None;
            }
            // Asked of the tree only for the one rule that miscounts, which fires on almost no
            // file; see `format_values` for the two things it gets wrong.
            let values = (code == "no-missing-format-argument")
                .then(|| FormatValues::of(program, &issue, span))
                .flatten();
            if values == Some(FormatValues::Unpacked) {
                return None;
            }
            let message = speaking_only_of_phrost(issue.message);
            Some(LintIssue {
                range: text_range(span),
                code: code.to_owned(),
                message: match values {
                    Some(FormatValues::Written(given)) => FormatValues::stated(message, given),
                    _ => message,
                },
                severity: match issue.level {
                    // A deprecation is a warning whatever level its rule gives it. The line runs,
                    // on the PHP that deprecates it and on every one before, and PHP's own word
                    // for it is `E_DEPRECATED` -- while three of mago's rules (a long cast, a
                    // semicolon after a `case`, the backtick operator) call it an error, on a
                    // project that may not be on the PHP 8.5 that minds.
                    Level::Error if category == LintCategory::Deprecation => LintSeverity::Warning,
                    Level::Error => LintSeverity::Error,
                    Level::Warning => LintSeverity::Warning,
                    Level::Note | Level::Help => LintSeverity::Hint,
                },
                help: issue.help.map(speaking_only_of_phrost),
                category,
            })
        })
        .collect()
}

/// The rule settings with the project's named rules switched on and off.
///
/// Through serde rather than a match over two hundred fields: the settings type already names every
/// rule in kebab-case, which is the same spelling a report shows and a project writes. `None` when
/// the project named nothing, so the defaults are used untouched.
///
/// A name that matches no rule is ignored rather than fatal, and deliberately: the list comes from a
/// project's own file, mago's rules move between versions, and refusing to lint a file over one stale
/// name would lose every other rule with it.
fn named_rules(selection: LintSelection<'_>) -> Option<mago_linter::settings::RulesSettings> {
    if selection.off.is_empty() && selection.on.is_empty() {
        return None;
    }

    let mut table = serde_json::Map::new();
    for (names, enabled) in [(selection.on, true), (selection.off, false)] {
        for name in names {
            table.insert(name.clone(), serde_json::json!({ "enabled": enabled }));
        }
    }

    // Each name is tried alone first, because the settings refuse an unknown field: one stale name
    // in a project's file would otherwise throw away every rule beside it. What survives is applied
    // together, in one go, so the result is the settings type's own doing rather than a merge this
    // would have to keep in step with two hundred fields.
    let known = table
        .into_iter()
        .filter(|(name, value)| {
            let one = serde_json::Value::Object(serde_json::Map::from_iter([(
                name.clone(),
                value.clone(),
            )]));
            serde_json::from_value::<mago_linter::settings::RulesSettings>(one).is_ok()
        })
        .collect::<serde_json::Map<_, _>>();

    serde_json::from_value::<mago_linter::settings::RulesSettings>(serde_json::Value::Object(known))
        .ok()
}

#[cfg(test)]
#[path = "lint/no_literal_password.rs"]
mod no_literal_password_tests;

#[cfg(test)]
#[path = "lint/mago_vocabulary.rs"]
mod mago_vocabulary_tests;

#[cfg(test)]
#[path = "lint/no_insecure_comparison.rs"]
mod no_insecure_comparison_tests;

#[cfg(test)]
#[path = "lint/no_missing_format_argument.rs"]
mod no_missing_format_argument_tests;

#[cfg(test)]
#[path = "lint/no_error_control_operator.rs"]
mod no_error_control_operator_tests;

#[cfg(test)]
#[path = "lint/version_gated_rules.rs"]
mod version_gated_rules_tests;

#[cfg(test)]
mod tests {
    use super::*;

    /// A file whose only fault is one the security rules are for.
    const EVAL: &str = "<?php\n\nclass Runner\n{\n    public function run(string $code): void\n    {\n        eval($code);\n    }\n}\n";

    /// The idiom PHP uses instead of a separate declaration, which the type layer narrows through.
    const ASSIGN_IN_CONDITION: &str = r#"<?php
declare(strict_types=1);

final class Service
{
    public function run(Application $application): void
    {
        if ($offer = $application->getSelectedQuote()) {
            $offer->accept();
        }
    }
}
"#;

    #[test]
    fn a_preference_filed_under_correctness_is_off_until_it_is_asked_for() {
        // Its category says fact, its content says house style, and `phrost-body` narrows through
        // the very shape it objects to. So the category alone does not turn it on.
        let unasked = codes(ASSIGN_IN_CONDITION, &[], LintCategory::FACTUAL);
        assert!(
            !unasked.iter().any(|code| code == "no-assign-in-condition"),
            "reported unasked: {unasked:?}"
        );

        // Named, it is the project's own word and wins.
        let asked = selected(
            ASSIGN_IN_CONDITION,
            &[],
            LintSelection {
                categories: LintCategory::FACTUAL,
                on: &["no-assign-in-condition".to_owned()],
                ..LintSelection::default()
            },
        );
        assert!(
            asked.iter().any(|code| code == "no-assign-in-condition"),
            "asked for and still silent: {asked:?}"
        );
    }

    fn codes(text: &str, integrations: &[&str], categories: &[LintCategory]) -> Vec<String> {
        selected(
            text,
            integrations,
            LintSelection {
                categories,
                ..LintSelection::default()
            },
        )
    }

    fn selected(text: &str, integrations: &[&str], selection: LintSelection<'_>) -> Vec<String> {
        // A name the file's own contents justify, so a rule about naming has nothing to say and the
        // test is about the rule it names.
        let mut found = lint(text, "src/Runner.php", integrations, None, selection)
            .into_iter()
            .map(|issue| issue.code)
            .collect::<Vec<_>>();
        found.sort();
        found.dedup();
        found
    }

    #[test]
    fn a_rule_reports_what_it_is_for() {
        assert_eq!(codes(EVAL, &[], &[LintCategory::Safety],), ["no-eval"]);
    }

    /// The filter that decides what a reader is shown, and the reason it exists: `eval` is a safety
    /// finding and nothing else asked about it.
    #[test]
    fn a_category_nobody_asked_for_says_nothing() {
        assert_eq!(
            codes(EVAL, &[], &[LintCategory::Consistency],),
            Vec::<String>::new()
        );
        assert!(
            codes("<?php\neval($x);\n", &[], &[]).is_empty(),
            "no category, no answer"
        );
    }

    /// The whole reason integrations are plumbed through: Drupal's own standard writes `TRUE` and
    /// `NULL` in upper case, so a linter that does not know which framework it is reading reports
    /// every one of them.
    #[test]
    fn the_framework_decides_whether_a_rule_applies() {
        let source = "<?php\n$flag = TRUE;\n$nothing = NULL;\n";

        assert!(
            codes(source, &[], &[LintCategory::Consistency])
                .contains(&"lowercase-keyword".to_owned()),
            "without the integration it is a finding"
        );
        assert!(
            !codes(source, &["drupal"], &[LintCategory::Consistency])
                .contains(&"lowercase-keyword".to_owned()),
            "with it, upper case is the standard"
        );
    }

    /// A name the linter has never heard of costs nothing rather than breaking the file's linting.
    #[test]
    fn an_unknown_framework_is_ignored() {
        assert_eq!(
            codes(EVAL, &["not-a-framework"], &[LintCategory::Safety],),
            ["no-eval"]
        );
    }

    /// A rule that is off by default is really switched on, and not merely unfiltered.
    ///
    /// The distinction the first version got wrong: filtering can only hide a rule that ran, and
    /// `missing-docs` does not run unless the settings say so. Naming it did nothing at all -- which
    /// looks exactly like a setting that does not work, on a real project by 438 findings.
    #[test]
    fn a_rule_that_is_off_by_default_can_be_named_on() {
        let source = "<?php\nclass Runner\n{\n    public function run(): void {}\n}\n";
        let on = ["missing-docs".to_owned()];

        assert!(
            selected(
                source,
                &[],
                LintSelection {
                    categories: &[LintCategory::Clarity],
                    off: &[],
                    on: &[],
                }
            )
            .is_empty(),
            "off by default, and asking for its category is not asking for it"
        );
        assert!(
            selected(
                source,
                &[],
                LintSelection {
                    categories: &[LintCategory::Clarity],
                    off: &[],
                    on: &on,
                }
            )
            .contains(&"missing-docs".to_owned()),
            "named on, so it runs"
        );
    }

    /// A rule a project names is reported whatever its category says, and one it switches off is not
    /// reported whatever the category says. The measured need: `strict-types` is a *correctness* rule
    /// and this project deliberately does not follow it, so wanting correctness cannot mean wanting
    /// that.
    #[test]
    fn a_named_rule_overrides_its_category() {
        let off = ["no-eval".to_owned()];
        assert!(
            selected(
                EVAL,
                &[],
                LintSelection {
                    categories: &[LintCategory::Safety],
                    off: &off,
                    on: &[],
                }
            )
            .is_empty(),
            "named off, so the category does not bring it back"
        );

        let on = ["no-eval".to_owned()];
        assert_eq!(
            selected(
                EVAL,
                &[],
                LintSelection {
                    categories: &[],
                    off: &[],
                    on: &on,
                }
            ),
            ["no-eval"],
            "named on, with no category asked for at all"
        );

        // Both at once is a project contradicting itself, and the answer that reports less is the
        // one that cannot be wrong in the direction that costs attention.
        assert!(
            selected(
                EVAL,
                &[],
                LintSelection {
                    categories: &[LintCategory::Safety],
                    off: &off,
                    on: &on,
                }
            )
            .is_empty()
        );
    }

    /// Every rule's category has to be knowable, or the filter drops its findings silently -- which
    /// it did, keyed on the human name instead of the code, and the project looked clean.
    #[test]
    fn every_category_name_round_trips() {
        for category in LintCategory::ALL {
            assert_eq!(
                LintCategory::named(category.as_str()),
                Some(*category),
                "{category:?}"
            );
        }
        assert_eq!(LintCategory::named("nothing-like-it"), None);
    }
}
