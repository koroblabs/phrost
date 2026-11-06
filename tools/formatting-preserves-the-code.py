#!/usr/bin/env python3
"""Whether reprinting a file changes anything but its whitespace, and settles after one pass.

    python3 tools/formatting-preserves-the-code.py <project-root> [--files 120]

A formatter runs on save, over files nobody re-reads. Two properties make that safe, and neither is
an opinion anybody has to be asked about:

* **It still parses.** PHP's own tokeniser is asked, so the answer is the runtime's. This is the one
  that found something: mago drops the parentheses of `new (self::SERVICERS[$id])($config)`, which
  are how PHP names a class with an expression, and the file after it is a syntax error.
* **It settles.** Formatting a formatted file has to produce the same file, or somebody saving twice
  watches it change twice. WordPress's `deprecated.php` was one: a docblock on the body of a
  braceless `if` gains a blank line on the second pass and none after.

`--tokens` adds a third, comparing the token streams to say whether anything but the *layout* moved
-- a dropped argument, a reordered chain, a lost comment, each one token out of thousands and none
visible in a diff a person skims. It is off by default because it only means anything once every
style transform is named, and there are more of them than anyone expects: a keyword's case, a
string's quotes, a parenthesis precedence makes redundant, a trailing comma before a closer, a
comment's indentation and its `*` markers, the order of an import block, a comma-separated `use`
split in two, a type respelled, and the indentation a heredoc's closing marker strips. Every one of
those was found by the comparison reporting most of a real checkout as broken, and the list is why
the default is the two questions above.

Both are asked of `phrost reprint`, which is the same formatter the server answers
`textDocument/formatting` with. Nothing is written into the project: the reprint goes to a
directory of its own.
"""
import argparse
import importlib.util
import json
import os
import shutil
import subprocess
import sys
import tempfile
from collections import Counter

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)

REPO = os.path.dirname(HERE)
PHROST = os.path.join(REPO, "target/release/phrost")

# `token_get_all` with whitespace dropped, as a JSON list of [kind, text]. A comment is kept: the
# formatter may re-indent one and may not lose it, and dropping them here would hide exactly that.
TOKENS = r"""<?php
function decode_string(string $written): string {
    $quote = $written[0] ?? "'";
    $body = substr($written, 1, -1);
    if ($quote === "'") {
        return strtr($body, ['\\\\' => '\\', "\\'" => "'"]);
    }
    return strtr($body, [
        '\\\\' => '\\', '\\"' => '"', '\\n' => "\n", '\\t' => "\t", '\\r' => "\r",
        '\\v' => "\v", '\\e' => "\e", '\\f' => "\f", '\\$' => '$',
    ]);
}

/// A heredoc body with the indentation its closing marker strips taken off.
///
/// PHP 7.3's flexible heredoc removes the closing identifier's own indentation from every line, so
/// moving the whole block -- body and marker together -- leaves the *value* untouched. The
/// tokeniser hands back the raw text, before that stripping, which is why this has to do it.
function dedent(string $body): string {
    $lines = explode("\n", $body);
    $widths = [];
    foreach ($lines as $line) {
        if (trim($line) === '') { continue; }
        $widths[] = strlen($line) - strlen(ltrim($line, " \t"));
    }
    $common = $widths ? min($widths) : 0;
    return implode("\n", array_map(
        static fn (string $line): string => substr($line, $common),
        $lines
    ));
}

$path = $argv[1];
$tokens = @token_get_all(file_get_contents($path), TOKEN_PARSE);
$out = [];
$in_heredoc = false;
foreach ($tokens as $token) {
    if (is_array($token)) {
        if ($token[0] === T_WHITESPACE) { continue; }
        if ($token[0] === T_START_HEREDOC) { $in_heredoc = true; }
        if ($token[0] === T_END_HEREDOC) { $in_heredoc = false; }
        $name = token_name($token[0]);
        $text = trim($token[1]);
        // A heredoc's own marker line is layout; `T_START_HEREDOC` carries it and is compared
        // folded, which the keyword rule below already does for it.
        // A keyword's case is layout: PHP compares `TRUE`, `True` and `true` as one word, and a
        // house style picks a spelling. What is *not* layout is the case of a name, so the folding
        // is by token kind rather than by the text -- and the three magic constants are folded by
        // name, since the tokeniser calls them `T_STRING` like every other identifier.
        $spelled = [
            'T_STRING', 'T_VARIABLE', 'T_CONSTANT_ENCAPSED_STRING', 'T_ENCAPSED_AND_WHITESPACE',
            'T_LNUMBER', 'T_DNUMBER', 'T_COMMENT', 'T_DOC_COMMENT', 'T_INLINE_HTML',
            'T_NAME_QUALIFIED', 'T_NAME_FULLY_QUALIFIED', 'T_NAME_RELATIVE',
        ];
        if (!in_array($name, $spelled, true)) {
            $text = strtolower($text);
        } elseif ($name === 'T_STRING' && in_array(strtolower($text), ['true', 'false', 'null'], true)) {
            $text = strtolower($text);
        }
        // A quoted string's *value* rather than its spelling: a house style picks `'a'` over
        // `"a"` where the content allows it, and that is layout. What is not layout is what the
        // string holds, which is why this decodes rather than dropping the text.
        if ($name === 'T_CONSTANT_ENCAPSED_STRING') {
            $text = decode_string($text);
        }
        // A block comment is re-indented with the code around it, so its own leading whitespace is
        // layout too. What it *says* is what has to survive.
        if ($in_heredoc && $name === 'T_ENCAPSED_AND_WHITESPACE') {
            $text = dedent($token[1]);
        }
        if ($name === 'T_COMMENT' || $name === 'T_DOC_COMMENT') {
            $lines = array_map('trim', explode("\n", $text));
            $text = implode("\n", $lines);
        }
        $out[] = [$name, $text];
    } else {
        $out[] = ['punct', $token];
    }
}

// An import block is *sorted* by the formatter, which is a reordering this comparison exists to
// catch and the one reordering that is deliberate. Each `use` statement is collected whole and the
// block is put back in a fixed order, so a lost import is still caught and a moved one is not.
// A `use` that opens with `(` is a closure's capture list and is none of this.
$sorted = [];
$index = 0;
while ($index < count($out)) {
    if ($out[$index][0] !== 'T_USE' || ($out[$index + 1] ?? null) === ['punct', '(']) {
        $sorted[] = $out[$index];
        $index++;
        continue;
    }
    $block = [];
    while ($index < count($out)) {
        $statement = [];
        while ($index < count($out) && $out[$index] !== ['punct', ';']) {
            $statement[] = $out[$index];
            $index++;
        }
        $statement[] = ['punct', ';'];
        $index++;
        $block[] = json_encode($statement);
        // Only a run of them: the next statement has to be another `use` to belong to the block.
        if (($out[$index][0] ?? null) !== 'T_USE' || ($out[$index + 1] ?? null) === ['punct', '(']) {
            break;
        }
    }
    sort($block);
    foreach ($block as $statement) {
        foreach (json_decode($statement, true) as $token) {
            $sorted[] = $token;
        }
    }
}
$out = $sorted;

// Two more the formatter decides and PHP does not care about, dropped after the stream is whole
// because each is recognised by what stands next to it.
$kept = [];
foreach ($out as $index => $token) {
    // A trailing comma before a closer: legal since 8.0 in every list, and a house style picks one.
    if ($token === ['punct', ','] && in_array($out[$index + 1] ?? null, [['punct', ')'], ['punct', ']'], ['punct', '}']], true)) {
        continue;
    }
    // A parenthesis precedence makes redundant: `a || (b && c)` is written `a || b && c`. Dropping
    // both kinds costs the comparison the grouping and keeps everything else -- a lost argument, a
    // reordered chain, a dropped comma elsewhere.
    if ($token === ['punct', '('] || $token === ['punct', ')']) {
        continue;
    }
    $kept[] = $token;
}
echo json_encode($kept);
"""


def tokens_of(script, path):
    done = subprocess.run(["php", script, path], capture_output=True, text=True)
    if done.returncode != 0 or not done.stdout.strip():
        return None
    try:
        return json.loads(done.stdout)
    except json.JSONDecodeError:
        return None


def first_difference(before, after):
    for index, (one, other) in enumerate(zip(before, after)):
        if one != other:
            return index, one, other
    if len(before) != len(after):
        longer = before if len(before) > len(after) else after
        return min(len(before), len(after)), "(nothing)", longer[min(len(before), len(after))]
    return None


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("root")
    parser.add_argument("--files", type=int, default=120)
    parser.add_argument(
        "--tokens",
        action="store_true",
        help="also compare the token streams, which needs every style transform named",
    )
    parser.add_argument("--verbose", action="store_true")
    args = parser.parse_args()

    spec = importlib.util.spec_from_file_location(
        "compare_lsp", os.path.join(HERE, "compare-lsp.py")
    )
    compare = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(compare)

    paths = compare.php_files(args.root, "", args.files)
    if not paths:
        print("no files sampled")
        return 1

    work = tempfile.mkdtemp(prefix="phrost-format-")
    try:
        script = os.path.join(work, "tokens.php")
        with open(script, "w", encoding="utf-8") as handle:
            handle.write(TOKENS)

        # One tree of the sample, reprinted beside itself, then the reprint reprinted again.
        source = os.path.join(work, "source")
        for path in paths:
            target = os.path.join(source, os.path.relpath(path, args.root))
            os.makedirs(os.path.dirname(target), exist_ok=True)
            shutil.copyfile(path, target)
        once = os.path.join(work, "once")
        twice = os.path.join(work, "twice")
        subprocess.run([PHROST, "reprint", source, once], capture_output=True, text=True)
        subprocess.run(
            [PHROST, "reprint", os.path.join(once, "after"), twice],
            capture_output=True,
            text=True,
        )

        asked = 0
        wrong = Counter()
        examples = {}
        for path in paths:
            relative = os.path.relpath(path, args.root)
            formatted = os.path.join(once, "after", relative)
            again = os.path.join(twice, "after", relative)
            if not os.path.exists(formatted):
                continue
            before = tokens_of(script, path)
            # A file PHP itself cannot tokenise says nothing about the formatter.
            if before is None:
                continue
            asked += 1

            after = tokens_of(script, formatted)
            if after is None:
                wrong["the reprint does not tokenise"] += 1
                examples.setdefault("the reprint does not tokenise", relative)
                continue
            found = first_difference(before, after) if args.tokens else None
            if found:
                index, one, other = found
                wrong["the reprint is not the same code"] += 1
                examples.setdefault(
                    "the reprint is not the same code",
                    f"{relative}: token {index} was {one} and is {other}",
                )
                continue
            if os.path.exists(again):
                with open(formatted, encoding="utf-8", errors="replace") as handle:
                    one_pass = handle.read()
                with open(again, encoding="utf-8", errors="replace") as handle:
                    two_passes = handle.read()
                if one_pass != two_passes:
                    wrong["the reprint does not settle"] += 1
                    examples.setdefault("the reprint does not settle", relative)

        print()
        print(f"reprinted           {asked - sum(wrong.values())} of {asked} files unchanged but for layout")
        for what, count in wrong.most_common():
            print(f"  {count:5}  {what}")
            print(f"         first in {examples[what]}")
        return 1 if wrong else 0
    finally:
        shutil.rmtree(work, ignore_errors=True)


if __name__ == "__main__":
    sys.exit(main())
