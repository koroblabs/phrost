//! Reprinting a file in a house style.
//!
//! mago's formatter, behind an API that says nothing about mago: what comes in is text and a style,
//! what comes out is text. The rule this crate exists to keep is that a parse tree never leaves it,
//! and a formatter is the clearest case for it -- the whole job is text in, text out.
//!
//! Nothing is invented here. Writing a PHP formatter means re-deciding several hundred questions
//! that mago has already answered and that a house style has already settled, and the presets below
//! are those answers.

mod drupal;
mod hunks;
mod output;
mod parens;

use std::borrow::Cow;

use mago_allocator::LocalArena;
use mago_formatter::{Formatter, presets::FormatterPreset};
use phrost_model::{PhpVersion, TextRange, TextSize};

/// Which house style to reprint in.
///
/// The presets mago ships, named rather than described: each is several dozen settled decisions, and
/// restating them here would be a copy that drifts.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum FormatStyle {
    /// PER-CS, which is what a project that has not said otherwise gets.
    #[default]
    Default,
    Psr12,
    /// Laravel Pint.
    Pint,
    Tempest,
    /// `hackfmt`.
    Hack,
    /// Two-space indent, single quotes, no trailing close tag -- Drupal's coding standard.
    Drupal,
}

/// What a reprint is for: the style it is laid out in, and the oldest PHP it has to run on.
///
/// The version is not cosmetic. The parser reads every version alike, but what the formatter *writes*
/// depends on it: a trailing comma after the last parameter needs 8.0, and since mago 1.53 the
/// parentheses of `(new Foo())()` come off for 8.4, where `new Foo()()` is legal -- and on 8.3 a
/// syntax error. Formatting for the newest PHP there is wrote that into every project on an older
/// one, on save.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct FormatTarget {
    pub style: FormatStyle,
    /// The lowest PHP the project supports -- composer's `require.php` -- or `None` where nothing
    /// says.
    pub php: Option<PhpVersion>,
}

impl From<FormatStyle> for FormatTarget {
    fn from(style: FormatStyle) -> Self {
        Self { style, php: None }
    }
}

impl FormatTarget {
    /// The version handed to the formatter.
    ///
    /// Where nothing says, 8.3: the newest that writes nothing an older 8.x cannot run except the
    /// trailing commas 8.0 brought, which is what an unstated project got before -- so only the
    /// one rewrite that breaks working code is held back, and a project that states its floor gets
    /// exactly what that floor allows.
    fn php_for_mago(self) -> mago_php_version::PHPVersion {
        match self.php {
            Some(php) => {
                mago_php_version::PHPVersion::new(u32::from(php.major()), u32::from(php.minor()), 0)
            }
            None => mago_php_version::PHPVersion::PHP83,
        }
    }
}

impl FormatStyle {
    /// The style a name asks for, or nothing when nothing goes by that name.
    ///
    /// By name because the name is what travels: a framework provider says `"drupal"` and a
    /// configuration file will say the same, and neither should have to know this type.
    pub fn named(name: &str) -> Option<Self> {
        Some(match name {
            "default" => Self::Default,
            "psr-12" => Self::Psr12,
            "pint" => Self::Pint,
            "tempest" => Self::Tempest,
            "hack" => Self::Hack,
            "drupal" => Self::Drupal,
            _ => return None,
        })
    }

    pub fn name(self) -> &'static str {
        self.preset().name()
    }

    fn preset(self) -> FormatterPreset {
        match self {
            Self::Default => FormatterPreset::Default,
            Self::Psr12 => FormatterPreset::Psr12,
            Self::Pint => FormatterPreset::Pint,
            Self::Tempest => FormatterPreset::Tempest,
            Self::Hack => FormatterPreset::Hack,
            Self::Drupal => FormatterPreset::Drupal,
        }
    }
}

/// One replacement that turns `text` into its formatted form, or nothing when there is none to make.
///
/// Nothing also covers the case that matters most: a file that does not parse is left alone.
/// Reformatting a half-written statement produces a file the author did not write and cannot undo in
/// one step, and an editor asks to format on save while the buffer is whatever it is.
pub fn format(text: &str, target: impl Into<FormatTarget>) -> Option<(TextRange, String)> {
    narrowed(text, &settled(text, target.into())?)
}

/// The edits a *selection* asks for: the same reprint, cut to lines, and only where it overlaps.
///
/// One reprint rather than formatting the selected text on its own, which is not a thing that can
/// be done -- a fragment of PHP does not parse, and a formatter's decisions about a line depend on
/// the construct it sits in. So the file is formatted whole and the answer is filtered, which is
/// what makes it a subset of what a whole-document format would do rather than a second opinion.
///
/// Empty when the selection covers nothing that moved, which is the common answer on a file that
/// is already formatted and is not a failure.
pub fn format_range(
    text: &str,
    target: impl Into<FormatTarget>,
    range: TextRange,
) -> Option<Vec<(TextRange, String)>> {
    let printed = settled(text, target.into())?;
    let asked = whole_lines(text, range);
    Some(
        hunks::between(text, &printed)
            .into_iter()
            // Contained, not merely touching. A range format may only edit inside the range it was
            // given, and a hunk that runs past the end cannot be clipped -- half of a reprinted
            // statement is text neither the author nor the formatter wrote. So a change spanning
            // the boundary is not offered, which on a file that is already formatted is no loss
            // (the hunks are the lines somebody just edited, and they are inside the selection) and
            // on a file where every line moves means the honest answer: this is a whole-document
            // format, ask for one.
            .filter(|(at, _)| at.start() >= asked.start() && at.end() <= asked.end())
            .collect(),
    )
}

/// `range` grown to the line boundaries around it.
///
/// A client sends whatever the cursor selected, which is usually part of a line at each end, and
/// every hunk is whole lines. Comparing the two without this refuses a hunk for the very line the
/// caller was pointing at.
fn whole_lines(text: &str, range: TextRange) -> TextRange {
    let start = text[..usize::from(range.start()).min(text.len())]
        .rfind('\n')
        .map_or(0, |at| at + 1);
    let end = text[usize::from(range.end()).min(text.len())..]
        .find('\n')
        .map_or(text.len(), |at| usize::from(range.end()) + at + 1);
    TextRange::new(
        TextSize::try_from(start).unwrap_or_default(),
        TextSize::try_from(end).unwrap_or_default(),
    )
}

/// `text` reprinted and settled, or nothing when it does not parse.
fn settled(text: &str, target: FormatTarget) -> Option<String> {
    let style = target.style;
    let mut printed = reprinted(text, target)?;

    // One respelling cannot be made on the output, and it is the one that needs the formatter most:
    // giving an array literal a line break of its own means re-laying out everything inside it. So
    // the break is written into the *input* and the formatter is asked again -- which keeps it,
    // because `preserve_breaking_array_like` reads a break an author wrote as intent. See
    // [`drupal::hinted_array_break`] for what the sniff asks for and why the output cannot answer.
    // Hint, reprint, and stop when the text stops moving. Both halves are needed and for one
    // reason: a hint changes the *breaks* the formatter reads, and several of its settings read a
    // break as the author's intent -- so the pass after a hint sees a different input and can lay
    // the same code out differently. Running until it settles is what makes the first save land on
    // the answer the second one would have produced, which is the property somebody watching their
    // file rewrite itself actually wants.
    //
    // The loop runs for every style and not only Drupal, because the settling and the hint are two
    // different things: the hint is Drupal's, and a pass that does not reach a fixpoint is the
    // formatter's. WordPress's `deprecated.php` is one -- a docblock on the body of a braceless
    // `if` gains a blank line on the *second* pass and none after -- so a person saving it twice
    // watched it change twice.
    // Drupal's hint runs whatever the first pass did, because it changes the *input* and so can
    // move a text the formatter already calls settled. Plain settling runs only when the first
    // pass changed something, which is what a save over an already-formatted file skips.
    if style == FormatStyle::Drupal || printed != text {
        // Whether `printed` is known to reprint to itself. True of a first pass that changed
        // nothing, which is a save over an already-formatted file -- most saves. The formatter is a
        // pure function, so reprinting such a text again can only give it back, and with no hint to
        // try that second reprint is a whole pass for nothing: measured on the reference project's
        // largest files, 14-21 ms a save of which it was nearly half.
        let mut settled = printed == text;
        for _ in 0..MOST_HINTS {
            let candidate = match style {
                FormatStyle::Drupal => drupal::hinted_array_break(&printed),
                _ => None,
            };
            if candidate.is_none() && settled {
                break;
            }
            let Some(again) = reprinted(candidate.as_deref().unwrap_or(&printed), target) else {
                break;
            };
            if again == printed {
                break;
            }
            // A new text, not yet known to reprint to itself -- the next pass is what finds out.
            settled = false;
            printed = again;
        }
    }

    let printed = output::trailing_output_kept(text, printed);

    // What the formatter hands back has to parse, and once it did not: mago before 1.58 dropped the
    // parentheses of `new (self::SERVICERS[$id])($config)`, which are not redundant -- they are how
    // PHP instantiates a class an *expression* names -- and the file after it was a syntax error. A
    // formatter runs on save over files nobody re-reads, so handing back text PHP refuses is the
    // one failure that costs somebody their work rather than their patience.
    //
    // Refusing leaves the file as it was written, which is what `format` already answers for a
    // buffer mid-edit. One parse per request, against a formatter that has already done several.
    // And it has to compute what it did -- see `parens::grouping_kept`.
    let session = phrost_syntax::ParseSession::new();
    let parsed = session.parse(&printed);
    match parsed.has_errors() || !parens::grouping_kept(text, parsed.program()) {
        true => None,
        false => Some(printed),
    }
}

/// A guard against a pathological input rather than a limit real code reaches: each pass settles
/// one array, and the worst file on the reference checkout has one.
const MOST_HINTS: usize = 32;

/// `text` reprinted in `style`, respellings included, or nothing when it does not parse.
fn reprinted(text: &str, target: FormatTarget) -> Option<String> {
    let style = target.style;
    let arena = LocalArena::new();
    let mut settings = style.preset().settings();
    if style == FormatStyle::Drupal {
        // `$a || ($b && $c)` keeps its parentheses. They change nothing -- `&&` binds tighter -- and
        // the preset drops them for that reason, but a reader who wrote them wrote them so the
        // grouping would not have to be worked out, and a formatter taking them away reads as having
        // changed the condition. Drupal core writes them.
        settings.preserve_redundant_logical_binary_expression_parentheses = true;
        // Wider than the 120 every other preset inherits, because of which sniffs *error*.
        //
        // Drupal has no error-level line limit -- `Drupal.Files.LineLength` warns above 80 and its
        // own standard exempts a line that cannot be split -- while
        // `Drupal.Arrays.Array.ArrayIndentation` and its neighbours error on nearly any break
        // *inside* an array: every later line of one has to start two columns in from the line its
        // `[` is on, so a value broken after its `=>`, an operator continuation and a hugged
        // closing bracket are each a finding. At 120 the formatter chose the error to avoid the
        // warning.
        //
        // 200 is measured rather than chosen. Applying the sniff's own rule to the formatter's
        // output over the reference project's 1 673 files: **36** violations introduced at 120,
        // 6 at 160, **1** at 200 and 1 at every width past it -- and that one is the instrument's
        // own artifact, with no file made worse than it was written. It also settles the
        // instability §8 records: 25 of those files were not a fixpoint, because a break the first
        // pass introduced was read as author intent by the second, and at 200 the breaks are not
        // introduced -- 1 file left. The lines it lengthens are the ones nothing can break anyway,
        // a long single-quoted string beside its key; 95 fewer files are rewritten at all.
        settings.print_width = drupal::WIDTH;
    }
    let formatter = Formatter::new(&arena, target.php_for_mago(), settings);

    let formatted = formatter
        .format_code(
            Cow::Borrowed(b"buffer"),
            Cow::Owned(text.as_bytes().to_vec()),
        )
        .ok()?;
    let formatted = std::str::from_utf8(formatted).ok()?;
    let respelled = match style {
        FormatStyle::Drupal => drupal::respelled(formatted),
        _ => formatted.to_owned(),
    };
    // Each pass, so every text the settling loop compares is one PHP would accept -- see `parens`.
    Some(parens::restored(&respelled).unwrap_or(respelled))
}

/// The smallest replacement that turns `text` into `formatted`.
///
/// A whole-document edit is what the protocol allows and what a formatter naturally produces, and it
/// is also what makes a client scroll to the top and forget which regions were folded. Trimming the
/// common ends costs one pass and usually reduces a format-on-save to the lines that actually moved.
fn narrowed(text: &str, formatted: &str) -> Option<(TextRange, String)> {
    if text == formatted {
        return None;
    }

    // On character boundaries, so the ranges stay valid UTF-8 offsets.
    let mut prefix = text
        .bytes()
        .zip(formatted.bytes())
        .take_while(|(left, right)| left == right)
        .count();
    while !text.is_char_boundary(prefix) || !formatted.is_char_boundary(prefix) {
        prefix -= 1;
    }

    let mut suffix = text[prefix..]
        .bytes()
        .rev()
        .zip(formatted[prefix..].bytes().rev())
        .take_while(|(left, right)| left == right)
        .count();
    while !text.is_char_boundary(text.len() - suffix)
        || !formatted.is_char_boundary(formatted.len() - suffix)
    {
        suffix -= 1;
    }

    let range = TextRange::new(
        phrost_model::TextSize::try_from(prefix).ok()?,
        phrost_model::TextSize::try_from(text.len() - suffix).ok()?,
    );
    Some((
        range,
        formatted[prefix..formatted.len() - suffix].to_owned(),
    ))
}

/// The whole of what a caller gets: the text after the one edit is applied.
///
/// Beside the code rather than in the test module, because the respellings' own tests are in their
/// own file and both halves of the crate's formatting need the same three lines.
#[cfg(test)]
pub(crate) fn formatted(source: &str, style: FormatStyle) -> String {
    match format(source, style) {
        None => source.to_owned(),
        Some((range, replacement)) => {
            let mut out = source[..usize::from(range.start())].to_owned();
            out.push_str(&replacement);
            out.push_str(&source[usize::from(range.end())..]);
            out
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `(new Foo())()` keeps its parentheses on a project that has to run below 8.4, and on one that
    /// says nothing; only a floor of 8.4 or later lets the formatter write `new Foo()()`, which PHP
    /// 8.3 refuses to parse.
    #[test]
    fn a_new_is_unwrapped_only_where_the_project_runs_on_8_4() {
        let source = "<?php\n\nfunction run(): void\n{\n    (new Handler())();\n}\n";
        let printed = |php: Option<PhpVersion>| {
            let target = FormatTarget {
                style: FormatStyle::Default,
                php,
            };
            match format(source, target) {
                None => source.to_owned(),
                Some((range, replacement)) => {
                    let mut out = source.to_owned();
                    out.replace_range(
                        usize::from(range.start())..usize::from(range.end()),
                        &replacement,
                    );
                    out
                }
            }
        };
        assert!(printed(Some(PhpVersion::PHP_8_3)).contains("(new Handler())();"));
        assert!(printed(None).contains("(new Handler())();"));
        assert!(printed(Some(PhpVersion::PHP_8_4)).contains("new Handler()();"));
    }

    /// `new (self::SERVICERS[$id])($config)` keeps its parentheses, which are how PHP instantiates
    /// a class an *expression* names.
    ///
    /// mago before 1.58 dropped them, and the file after it was a syntax error -- found by
    /// `tools/formatting-preserves-the-code.py`, one file in 120 on the reference project. The
    /// guard in [`settled`] that refused such a reprint stays: a formatter runs on save over files
    /// nobody re-reads, so text PHP refuses is the one failure that costs somebody their work.
    #[test]
    fn an_instantiated_expression_keeps_its_parentheses() {
        let source = "<?php\n\nclass R\n{\n    public const array S = [];\n\n    public function m(string $id): object\n    {\n        return new (self::S[$id])([]);\n    }\n}\n";
        assert_eq!(formatted(source, FormatStyle::Default), source);
        assert!(formatted(source, FormatStyle::Drupal).contains("return new (self::S[$id])([]);"));
    }

    /// The right-hand side of `instanceof` keeps its parentheses wherever PHP needs them.
    ///
    /// PHP takes a class name or a variable there and anything else only in parentheses, and mago
    /// keeps them for a class constant alone: `$job instanceof ((string) $jobToFake)` in Laravel's
    /// `QueueFake` came back as `$job instanceof (string) $jobToFake`, which PHP refuses -- and the
    /// guard against a reprint that does not parse let it through, because mago's parser accepts
    /// it. Found by comparing what a reprinted checkout *means* with what it meant.
    #[test]
    fn an_instanceof_keeps_the_parentheses_php_needs() {
        let source = "<?php\n\nfunction f(object $job, mixed $class, array $classes): bool\n{\n    return $job instanceof ((string) $class)\n        || $job instanceof (get_class($job))\n        || $job instanceof ('Foo')\n        || $job instanceof ($classes[0])\n        || $job instanceof (Foo::class);\n}\n";
        expect_test::expect![[r#"
            <?php

            function f(object $job, mixed $class, array $classes): bool
            {
                return (
                    $job instanceof ((string) $class)
                    || $job instanceof (get_class($job))
                    || $job instanceof ('Foo')
                    || $job instanceof $classes[0]
                    || $job instanceof (Foo::class)
                );
            }
        "#]]
        .assert_eq(&formatted(source, FormatStyle::Default));
    }

    /// Text outside the PHP tags is output, so a reprint leaves every byte of it -- a trailing blank
    /// line included. Laravel's mail templates are `.blade.php` files with no `<?php` in them at
    /// all, and one of them lost its last newline.
    #[test]
    fn text_outside_the_tags_is_left_as_written() {
        for source in [
            "<table>\n<tr><td>x</td></tr>\n</table>\n\n",
            "<?php $a = 1; ?>\n<p>after</p>\n\n",
        ] {
            assert_eq!(
                formatted(source, FormatStyle::Default),
                source,
                "{source:?}"
            );
        }
    }

    /// A file whose arithmetic the formatter would regroup is left as written: `$a + ($b - $c) + $d`
    /// came back `$a + $b - $c + $d`, a different float. Parentheses PHP's own grouping already
    /// implies are still the formatter's to drop, and the file beside it still formats.
    #[test]
    fn a_regrouped_sum_is_refused() {
        let source = "<?php\n\n$q = $a + ($b - $c) + $d;\n";
        assert_eq!(formatted(source, FormatStyle::Default), source);
        assert_eq!(formatted(source, FormatStyle::Drupal), source);

        let kept = "<?php\n\n$q = $a - ($b - $c) + $d;\n$r = ($a + $b) - $c;\n";
        expect_test::expect![[r#"
            <?php

            $q = $a - ($b - $c) + $d;
            $r = $a + $b - $c;
        "#]]
        .assert_eq(&formatted(kept, FormatStyle::Default));
    }

    /// A heredoc the formatter would indent with tabs and spaces together is refused, since PHP
    /// refuses the file.
    ///
    /// WordPress's `network.php` writes an unindented heredoc inside an `else :` that follows a
    /// `?>`, and mago takes the tab the template text ended on as part of the indentation it then
    /// adds spaces to -- so the closing marker came back as eight spaces and a tab, which PHP
    /// reports as "tabs and spaces cannot be mixed". mago's parser records the mix and does not
    /// object to it, so the guard has to.
    #[test]
    fn a_heredoc_indented_with_tabs_and_spaces_is_refused() {
        let source = "<ol>\n\t<?php if ($nginx) : ?>\n\t<li>x</li>\n\t<?php else :\n\t\t$rules = <<<EOF\nRewriteEngine On\nEOF;\n\t\techo $rules;\n\tendif; ?>\n</ol>\n";
        let printed = formatted(source, FormatStyle::Default);
        let mixed = printed.lines().any(|line| {
            line.trim_start_matches([' ', '\t']) == "EOF;"
                && line.contains(' ')
                && line.contains('\t')
        });
        assert!(!mixed, "{printed}");
    }

    /// The closing tag at the end of a file goes when nothing but the newline PHP swallows follows
    /// it, and stays when output does: removing it there turns that output into whitespace.
    /// Symfony's translation fixtures end `... null); ?>` and a blank line.
    #[test]
    fn a_closing_tag_before_output_stays() {
        expect_test::expect![[r#"
            "<?php\n\necho 1;\n"
        "#]]
        .assert_debug_eq(&formatted("<?php\necho 1;\n?>\n", FormatStyle::Default));
        expect_test::expect![[r#"
            "<?php\n\necho 1; ?>\n\n"
        "#]]
        .assert_debug_eq(&formatted("<?php echo 1; ?>\n\n", FormatStyle::Default));
    }

    /// An empty file stays empty: a newline outside the tags is output.
    #[test]
    fn an_empty_file_stays_empty() {
        assert_eq!(formatted("", FormatStyle::Default), "");
    }

    #[test]
    fn drupal_is_two_spaces_and_single_quotes() {
        let out = formatted(
            "<?php\nclass Repo\n{\n    public function run(): void\n    {\n        echo \"x\";\n    }\n}\n",
            FormatStyle::Drupal,
        );
        assert_eq!(
            out,
            "<?php\n\nclass Repo {\n\n  public function run(): void {\n    echo 'x';\n  }\n\n}\n"
        );
    }

    /// The file the range tests select inside: tidy but for one method.
    const ONE_SLOPPY_METHOD: &str = "<?php\n\nnamespace R;\n\nclass B\n{\n    public function alpha(): int\n    {\n        return 1;\n    }\n\n    public function beta(  ):int{return   2;}\n\n    public function gamma(): int\n    {\n        return 3;\n    }\n}\n";

    /// Where `needle` sits, as a range covering part of the line it is on.
    fn part_of_the_line_holding(text: &str, needle: &str) -> TextRange {
        let at = text.find(needle).expect("the fixture holds it");
        TextRange::new(
            TextSize::try_from(at).unwrap(),
            TextSize::try_from(at + needle.len()).unwrap(),
        )
    }

    fn with_range_formatted(text: &str, needle: &str) -> String {
        let hunks = format_range(
            text,
            FormatStyle::Default,
            part_of_the_line_holding(text, needle),
        )
        .expect("the fixture parses");
        let mut out = String::new();
        let mut at = 0;
        for (range, replacement) in &hunks {
            out.push_str(&text[at..usize::from(range.start())]);
            out.push_str(replacement);
            at = usize::from(range.end());
        }
        out.push_str(&text[at..]);
        out
    }

    #[test]
    fn a_selection_reformats_what_it_covers_and_leaves_the_file_alone() {
        assert_eq!(
            with_range_formatted(ONE_SLOPPY_METHOD, "beta("),
            ONE_SLOPPY_METHOD.replace(
                "    public function beta(  ):int{return   2;}\n",
                "    public function beta(): int\n    {\n        return 2;\n    }\n"
            )
        );
    }

    #[test]
    fn a_selection_over_lines_that_are_already_right_asks_for_nothing() {
        // Not a failure and the commonest answer: a file being formatted on save is mostly
        // already formatted, and every line of it would otherwise offer an edit that changes
        // nothing.
        assert_eq!(
            format_range(
                ONE_SLOPPY_METHOD,
                FormatStyle::Default,
                part_of_the_line_holding(ONE_SLOPPY_METHOD, "return 1;")
            ),
            Some(Vec::new())
        );
    }

    #[test]
    fn a_change_running_past_the_selection_is_not_offered() {
        // Every line of this moves, so there is one hunk over the whole file and no honest way to
        // give the caller a part of it. The answer is that a range format has nothing to say --
        // the file wants a document format, and offering to rewrite lines nobody selected is what
        // the protocol forbids.
        let sloppy =
            "<?php\nnamespace R;\nclass A {\n    public function one(  ):int{return   1;}\n}\n";
        assert_eq!(
            format_range(
                sloppy,
                FormatStyle::Default,
                part_of_the_line_holding(sloppy, "one(")
            ),
            Some(Vec::new())
        );
    }

    #[test]
    fn a_file_that_does_not_parse_is_left_alone_here_too() {
        let broken = "<?php\nclass A { public function f(): int { return }\n";
        assert_eq!(
            format_range(
                broken,
                FormatStyle::Default,
                TextRange::new(0.into(), 10.into())
            ),
            None
        );
    }

    #[test]
    fn the_default_style_is_four_spaces_and_a_brace_on_its_own_line() {
        let out = formatted(
            "<?php\nclass Repo {\n  public function run(): void {\n    echo 'x';\n  }\n}\n",
            FormatStyle::Default,
        );
        assert!(
            out.contains("\n    public function run"),
            "four spaces: {out:?}"
        );
        assert!(
            out.contains("class Repo\n{"),
            "brace on its own line: {out:?}"
        );
    }

    /// The case that decides whether this is safe to run on save.
    #[test]
    fn a_file_that_does_not_parse_is_left_alone() {
        for broken in [
            "<?php\nclass Repo {\n    public function run(",
            "<?php\n$x = ;\n",
            "<?php\nfunction f() { return 1",
        ] {
            assert_eq!(format(broken, FormatStyle::Drupal), None, "{broken:?}");
        }
    }

    /// The parentheses an author put around `&&` inside `||` stay, in Drupal's style: they change
    /// nothing and say what the author meant.
    #[test]
    fn drupal_keeps_the_parentheses_around_a_conjunction_in_a_disjunction() {
        let source = "<?php\n\nfunction f(string $url, int $key): bool {\n  return $url === '/admin' || ($key === 1 && str_starts_with($url, '/application/'));\n}\n";
        assert_eq!(
            format(source, FormatStyle::Drupal),
            None,
            "already in style"
        );
    }

    #[test]
    fn a_file_already_in_the_style_needs_no_edit() {
        let already =
            "<?php\n\nclass Repo {\n\n  public function run(): void {\n    echo 'x';\n  }\n\n}\n";
        assert_eq!(format(already, FormatStyle::Drupal), None);
    }

    /// The edit covers what moved and not the file, so a client keeps the cursor and the folds.
    #[test]
    fn the_edit_is_narrowed_to_what_changed() {
        let source =
            "<?php\n\nclass Repo {\n\n  public function run(): void {\n    echo \"x\";\n  }\n\n}\n";
        let (range, replacement) = format(source, FormatStyle::Drupal).expect("one quote changes");

        assert_eq!(
            &source[usize::from(range.start())..usize::from(range.end())],
            "\"x\""
        );
        assert_eq!(replacement, "'x'");
    }

    /// A multi-byte character on either side of the change must not be cut in half.
    #[test]
    fn a_narrowed_edit_lands_on_character_boundaries() {
        let source = "<?php\n\nclass Repo {\n\n  public function run(): void {\n    echo \"— ліл —\";\n  }\n\n}\n";
        let (range, replacement) = format(source, FormatStyle::Drupal).expect("the quotes change");

        assert!(source.is_char_boundary(usize::from(range.start())));
        assert!(source.is_char_boundary(usize::from(range.end())));
        assert!(
            replacement.contains("ліл"),
            "the text survives: {replacement:?}"
        );
    }

    #[test]
    fn a_style_is_found_by_the_name_a_provider_would_give() {
        assert_eq!(FormatStyle::named("drupal"), Some(FormatStyle::Drupal));
        assert_eq!(FormatStyle::named("psr-12"), Some(FormatStyle::Psr12));
        assert_eq!(FormatStyle::named("nothing-like-it"), None);
        // Round trip, so a name that mago renames is caught here rather than in an editor.
        for style in [
            FormatStyle::Default,
            FormatStyle::Psr12,
            FormatStyle::Pint,
            FormatStyle::Tempest,
            FormatStyle::Hack,
            FormatStyle::Drupal,
        ] {
            assert_eq!(FormatStyle::named(style.name()), Some(style), "{style:?}");
        }
    }
}
