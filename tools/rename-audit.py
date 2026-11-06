#!/usr/bin/env python3
"""What rename changes, what it refuses, and what it silently leaves behind.

    python3 tools/rename-audit.py <project-root>

Rename is the one request where a wrong answer destroys somebody's code instead of merely
disappointing them, so a sample of "did it answer" is not enough. Three questions are asked of every
case, and the third is the one that matters:

1. Does `prepareRename` allow it?
2. What does `rename` actually edit -- how many sites, in how many files?
3. **What is left behind?** Every remaining occurrence of the old name in the project's own code that
   the edit did not touch, split into code, comment, and string. A rename that misses a call site in
   *code* leaves the project broken; one that misses a docblock leaves it merely stale; one that
   misses a string may be either, and this cannot tell which -- so it says where they are.

Nothing is written to disk. The edits are computed, checked against the text they claim to replace,
and thrown away.

The cases are discovered from the project rather than listed, so this can be pointed at another
checkout. Each prints the file and line it chose.
"""
import argparse
import json
import os
import re
import subprocess
import sys
import time
from collections import Counter

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from lspclient import Closed, Server, Timeout, path_of, uri_of  # noqa: E402

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
STILL_INDEXING = -32801

CAPABILITIES = {
    "window": {"workDoneProgress": True},
    "workspace": {"workspaceFolders": True},
    "textDocument": {
        "synchronization": {"didSave": True},
        "rename": {"prepareSupport": True, "dynamicRegistration": False},
        "hover": {"contentFormat": ["markdown"]},
    },
}


def php_text(path):
    with open(path, encoding="utf-8", errors="replace") as handle:
        return handle.read()


class Lines:
    def __init__(self, text):
        self.text = text
        self.starts = [0] + [index + 1 for index, ch in enumerate(text) if ch == "\n"]

    def position(self, offset):
        line = max(index for index, start in enumerate(self.starts) if start <= offset)
        prefix = self.text[self.starts[line] : offset]
        return {"line": line, "character": len(prefix.encode("utf-16-le")) // 2}

    def offset(self, position):
        line = min(position["line"], len(self.starts) - 1)
        at, used = self.starts[line], 0
        while at < len(self.text) and used < position["character"] and self.text[at] != "\n":
            used += len(self.text[at].encode("utf-16-le")) // 2
            at += 1
        return at


class Case:
    """One rename to try: where the cursor goes, what to call it, and what should happen."""

    def __init__(self, shape, path, offset, old, new, expect, why="", scope="project"):
        self.shape = shape
        self.path = path
        self.offset = offset
        self.old = old
        self.new = new
        self.expect = expect  # "edits", "refuse", or "either"
        self.why = why
        # Where a leftover would count. A local cannot leave its file, so scanning the project for
        # its spelling would report every unrelated `$id` in the codebase as a miss.
        self.scope = scope


class Finder:
    """Finds one example of each rename shape in a real checkout.

    Discovery walks the files rather than shelling out per shape: the patterns want backslashes and
    anchors that are painful to pass through two levels of escaping, and one walk of the project's
    own code is cheaper than eight greps of the whole checkout.
    """

    def __init__(self, root, user_glob):
        self.root = root
        self.user_glob = user_glob
        self.counted = {}

    def files(self, pattern, extra=()):
        out = subprocess.run(
            ["rg", "--no-ignore", "--glob", self.user_glob, "--glob", "!vendor/**", "-l", *extra,
             "--", pattern, self.root],
            capture_output=True,
            text=True,
        )
        return sorted(line for line in out.stdout.split("\n") if line)

    def holding(self, word):
        """How many files of the project's own code write `word`."""
        if word not in self.counted:
            self.counted[word] = len(self.files(re.escape(word), ("-w",)))
        return self.counted[word]

    def walk(self, custom):
        for directory, subdirs, names in os.walk(self.root):
            subdirs[:] = sorted(name for name in subdirs if name not in ("vendor", ".git", "node_modules"))
            if custom not in directory:
                continue
            for name in sorted(names):
                if name.endswith(".php"):
                    yield os.path.join(directory, name)

    def cases(self, custom):
        """One case per shape, taken from the first file that has that shape."""
        wanted = [
            self.a_class,
            self.an_interface_method,
            self.a_parameter,
            self.a_shadowed_local,
            self.in_a_string,
            self.in_a_docblock,
            self.a_framework_callback,
            self.under_an_alias,
            self.from_vendor,
            self.from_the_framework,
        ]
        found = {}
        for path in self.walk(custom):
            if len(found) == len(wanted):
                break
            text = php_text(path)
            for want in wanted:
                if want.__name__ in found:
                    continue
                made = want(text, path)
                if made:
                    found[want.__name__] = made if isinstance(made, list) else [made]
        return [case for want in wanted for case in found.get(want.__name__, [])]

    # -- one shape each ---------------------------------------------------------------------

    def a_class(self, text, path):
        match = re.search(r"^(?:final )?class (\w+)", text, re.MULTILINE)
        if not match or self.holding(match.group(1)) < 4:
            return None
        name = match.group(1)
        return Case(
            "a class used across files", path, match.start(1) + 1, name, name + "Renamed", "edits"
        )

    def an_interface_method(self, text, path):
        declared = re.search(r"^interface (\w+)", text, re.MULTILINE)
        if not declared:
            return None
        for method in re.finditer(r"public function (\w+)\(", text):
            name = method.group(1)
            if name.startswith("__"):
                continue
            implementers = self.files(rf"function {re.escape(name)}\(")
            if len(implementers) >= 4:
                return Case(
                    f"a method of interface {declared.group(1)}, declared in "
                    f"{len(implementers) - 1} other files too",
                    path,
                    method.start(1) + 1,
                    name,
                    name + "Renamed",
                    "edits",
                    "an implementation left with the old name no longer satisfies the interface",
                )
        return None

    def a_parameter(self, text, path):
        # Uses are read off the masked text, so a `@param $id` in the docblock above is not mistaken
        # for a use of the parameter -- the two are different questions and the first pass of this
        # tool asked the wrong one.
        code = mask(text)
        # A signature with a body after it, so the pair of positions is a declaration and a use of
        # the same parameter. An interface's parameters have no uses anywhere, and a pair taken from
        # one compares a silence with a silence.
        for match in re.finditer(
            r"function\s+\w+\s*\(([^)]*?[\w\\|?]\s+\$(\w+)\s*[,)][^)]*)\)[^{;]*\{", code, re.S
        ):
            name = match.group(2)
            body = match.end()
            uses = [
                use.start()
                for use in re.finditer(rf"(?<![\w>]){re.escape('$' + name)}\b", code[body:])
            ]
            if uses:
                uses = [body + uses[0]]
                return [
                    Case(
                        "a parameter, cursor on its own declaration",
                        path,
                        match.start(2),
                        name,
                        name + "Renamed",
                        "edits",
                        "the declaration is where a person renames from",
                        scope="file",
                    ),
                    Case(
                        "the same parameter, cursor on a use of it",
                        path,
                        uses[0] + 1,
                        name,
                        name + "Renamed",
                        "edits",
                        scope="file",
                    ),
                ]
        return None

    def a_shadowed_local(self, text, path):
        """A variable *assigned* in two different function bodies of the same file."""
        code = mask(text)
        bodies = [match.start() for match in re.finditer(r"function\s+\w+\s*\(", code)]
        if len(bodies) < 2:
            return None
        for match in re.finditer(r"^\s+\$(\w+)\s*=[^=]", code, re.MULTILINE):
            name = match.group(1)
            assigned = [
                use.start()
                for use in re.finditer(rf"^\s+\${re.escape(name)}\s*=[^=]", code, re.MULTILINE)
            ]
            written = [use.start() for use in re.finditer(rf"\${re.escape(name)}\b", code)]
            if len(assigned) < 2 or len(written) < 3:
                continue
            # Assigned in two different function bodies, so a sweep by name would cross a boundary.
            owners = {max([-1] + [start for start in bodies if start < at]) for at in assigned}
            if len(owners) > 1:
                return Case(
                    f"${name}, a local assigned in {len(owners)} functions of one file",
                    path,
                    match.start(1),
                    name,
                    name + "Renamed",
                    "edits",
                    "renaming one must not touch the same spelling in the other function",
                    scope="file",
                )
        return None

    def in_a_string(self, text, path):
        match = re.search(r"'(\\?\w+(?:\\\w+)+)'", text)
        if not match:
            return None
        return Case(
            "a class path written inside a string literal",
            path,
            match.end(1) - 2,
            match.group(1).rsplit("\\", 1)[-1],
            "Renamed",
            "either",
            "a string that resolves to a class is renamed with every other spelling of it; one "
            "that resolves to nothing is data and must be left alone -- and whichever this is, "
            "prepareRename has to agree with what rename does",
        )

    def in_a_docblock(self, text, path):
        match = re.search(r"\*\s+@param\s+[\w\\|\[\]<>, ]*\$(\w+)", text)
        if not match:
            return None
        return Case(
            "a $name in a @param docblock line",
            path,
            match.start(1),
            match.group(1),
            match.group(1) + "Renamed",
            "either",
            "allowing it and editing nothing is the outcome that is not acceptable",
            scope="file",
        )

    def a_framework_callback(self, text, path):
        """A method a framework names in a string: Drupal's `'#submit' => ['::saveNote']`.

        The rename starts at the declaration, which is where a person renaming a method starts, and
        the string is what it has to carry along: left behind, the form calls a method that no
        longer exists -- which is what renaming `saveNote` did on the reference project until
        4e153354.
        """
        for match in re.finditer(r"'::(\w+)'", text):
            name = match.group(1)
            declared = re.search(rf"function\s+({re.escape(name)})\s*\(", text)
            if declared:
                return Case(
                    "a method a framework string names ('::method')",
                    path,
                    declared.start(1) + 1,
                    name,
                    name + "Renamed",
                    "edits",
                    "the string is a call the framework makes; left behind, it names a method "
                    "that no longer exists",
                )
        return None

    def under_an_alias(self, text, path):
        match = re.search(r"^use ([\w\\]+)\s+as\s+(\w+);", text, re.MULTILINE)
        if not match:
            return None
        alias, real = match.group(2), match.group(1).rsplit("\\", 1)[-1]
        body = re.search(rf"(?<![\w\\]){re.escape(alias)}\b(?!\s*;)", text[match.end() :])
        if not body:
            return None
        return Case(
            f"a use of `{alias}`, which is `{real}` imported under another name",
            path,
            match.end() + body.start() + 1,
            alias,
            alias + "Renamed",
            "either",
            "whatever it does, the editor's rename box must not say one name and edit another",
        )

    def from_vendor(self, text, path):
        match = re.search(r"^use (Symfony|Psr|GuzzleHttp)\\[\w\\]*?(\w+);", text, re.MULTILINE)
        if not match:
            return None
        return Case(
            f"a vendor class ({match.group(1)}\\...\\{match.group(2)})",
            path,
            match.start(2) + 1,
            match.group(2),
            match.group(2) + "Renamed",
            "refuse",
            "an edit under vendor/ is undone by the next composer install",
        )

    def from_the_framework(self, text, path):
        match = re.search(r"^use Drupal\\Core\\[\w\\]*?(\w+);", text, re.MULTILINE)
        if not match:
            return None
        return Case(
            f"a Drupal core class (Drupal\\Core\\...\\{match.group(1)})",
            path,
            match.start(1) + 1,
            match.group(1),
            match.group(1) + "Renamed",
            "refuse",
            "core is checked out outside vendor/, and ownership is decided by that path alone",
        )


class Audit:
    def __init__(self, root, finder):
        self.root = root
        self.finder = finder
        binary = os.path.join(REPO, "target/release/phrost")
        if not os.path.exists(binary):
            sys.exit(f"not built: {binary}")
        self.server = Server("phrost", [binary, "serve"], root, log="/dev/null")
        self.opened = set()

    def start(self, probe, warmup):
        self.server.initialize(CAPABILITIES)
        deadline = time.monotonic() + warmup
        while time.monotonic() < deadline:
            self.server.poll(1.0)
            if self.ask("workspace/symbol", {"query": probe}).get("result"):
                return True
        return False

    def ask(self, method, params, timeout=90.0):
        for _ in range(60):
            try:
                answer = self.server.request(method, params, timeout=timeout)
            except (Timeout, Closed) as failed:
                return {"error": {"message": str(failed)}}
            if (answer.get("error") or {}).get("code") == STILL_INDEXING:
                time.sleep(0.5)
                continue
            return answer
        return {"error": {"message": "still indexing"}}

    def open(self, path):
        if path not in self.opened:
            self.server.open(path, php_text(path))
            self.opened.add(path)

    def run(self, case):
        self.open(case.path)
        text = php_text(case.path)
        lines = Lines(text)
        here = lines.position(case.offset)
        where = {"textDocument": {"uri": uri_of(case.path)}, "position": here}

        prepared = self.ask("textDocument/prepareRename", where)
        renamed = self.ask("textDocument/rename", dict(where, newName=case.new))
        return here, prepared, renamed


def mask(text):
    """compare-lsp's masker, reached the same way capability-audit reaches its scanner."""
    import importlib.util

    where = os.path.join(os.path.dirname(os.path.abspath(__file__)), "compare-lsp.py")
    spec = importlib.util.spec_from_file_location("compare_lsp_mask", where)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module.mask(text)


def leftovers(finder, case, edits):
    """Occurrences of the old name in the project's own code that the edit did not touch.

    Classified by whether they are code, comment, or string, because those three are three
    different severities: code left behind is a broken project, a comment left behind is a stale
    one, and a string is whichever the string was for.
    """
    edited = {}
    for uri, changes in edits.items():
        path = path_of(uri)
        text = php_text(path)
        lines = Lines(text)
        edited[path] = {lines.offset(edit["range"]["start"]) for edit in changes}

    wrong = []
    remaining = Counter()
    examples = []
    word = re.compile(r"(?<![\w$])" + re.escape(case.old) + r"(?!\w)")
    scanned = (
        [case.path] if case.scope == "file" else finder.files(re.escape(case.old), ("-w",))
    )
    for path in scanned:
        text = php_text(path)
        masked = mask(text)
        lines = Lines(text)
        for match in word.finditer(text):
            if match.start() in edited.get(path, ()):
                continue
            # `mask` keeps every offset, blanking comment and string bodies -- so what survives in
            # the masked text at the same place is what is really code.
            if masked[match.start() : match.end()] == case.old:
                kind = "code"
            elif re.match(r"\s*\*", text[text.rfind("\n", 0, match.start()) + 1 :]):
                kind = "docblock"
            else:
                kind = "string"
            remaining[kind] += 1
            if kind == "code" and len(examples) < 6:
                examples.append(
                    f"{os.path.relpath(path, finder.root)}:"
                    f"{lines.position(match.start())['line'] + 1}  "
                    f"{text[text.rfind(chr(10), 0, match.start()) + 1 :].split(chr(10))[0].strip()[:90]}"
                )

    # Every edit must replace exactly the old name; anything else is an edit in the wrong place.
    for path, changes in edits.items():
        text = php_text(path_of(path))
        lines = Lines(text)
        for edit in changes:
            start = lines.offset(edit["range"]["start"])
            end = lines.offset(edit["range"]["end"])
            if text[start:end] != case.old:
                wrong.append(f"{os.path.basename(path_of(path))}: replaces {text[start:end]!r}")
    return remaining, examples, wrong


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("root", nargs="?", default=".")
    parser.add_argument("--custom", default="modules/custom", help="path fragment for user code")
    parser.add_argument("--warmup", type=float, default=600.0)
    parser.add_argument("--json", dest="out")
    args = parser.parse_args()
    sys.stdout.reconfigure(line_buffering=True)

    root = os.path.abspath(args.root)
    finder = Finder(root, "*.php")
    print(f"rename-audit  {root}\nfinding cases...", flush=True)
    cases = finder.cases(args.custom)
    print(f"{len(cases)} cases", flush=True)

    probe = None
    for case in cases:
        declared = re.search(r"(?:class|interface|trait|enum)\s+(\w+)", php_text(case.path))
        if declared:
            probe = declared.group(1)
            break

    audit = Audit(root, finder)
    if not audit.start(probe, args.warmup):
        print("NOT READY -- treat these answers as suspect")

    # A new name PHP would not accept, and a name in the middle of nothing: two refusals that need
    # no discovery.
    extra = [
        Case("an invalid new name", cases[0].path, cases[0].offset, cases[0].old, "1nope",
             "refuse", "a name PHP cannot parse leaves every file it touched unparseable"),
        Case("a reserved word as the new name", cases[0].path, cases[0].offset, cases[0].old,
             "class", "refuse", "same"),
    ]

    report = []
    for case in cases + extra:
        here, prepared, renamed = audit.run(case)
        edits = ((renamed.get("result") or {}).get("changes")) or {}
        error = (renamed.get("error") or {}).get("message")
        prepare_error = (prepared.get("error") or {}).get("message")
        allowed = prepared.get("result") is not None

        print(f"\n=== {case.shape}")
        print(f"    {os.path.relpath(case.path, root)}:{here['line'] + 1}  "
              f"{case.old!r} -> {case.new!r}   (expected: {case.expect})")
        if case.why:
            print(f"    why it matters: {case.why}")
        promised = (prepared.get("result") or {}).get("placeholder")
        print(
            "    prepareRename: "
            + (
                f"allowed, offering to edit {promised!r}"
                if allowed
                else f"refused: {prepare_error or 'null'}"
            )
        )
        if error:
            print(f"    rename: refused: {error}")
        else:
            print(f"    rename: {sum(len(v) for v in edits.values())} edits in {len(edits)} files")
            for uri, changes in sorted(edits.items(), key=lambda pair: path_of(pair[0])):
                where = path_of(uri)
                text = php_text(where)
                lines = Lines(text)
                replaced = Counter(
                    text[lines.offset(edit["range"]["start"]) : lines.offset(edit["range"]["end"])]
                    for edit in changes
                )
                print(
                    f"        {os.path.relpath(where, root)}: {len(changes)} "
                    f"replacing {dict(replaced)}"
                )

        row = {
            "shape": case.shape,
            "file": os.path.relpath(case.path, root),
            "line": here["line"] + 1,
            "old": case.old,
            "new": case.new,
            "expected": case.expect,
            "prepare_allowed": allowed,
            "prepare_error": prepare_error,
            "rename_error": error,
            "edits": sum(len(v) for v in edits.values()),
            "files": len(edits),
        }

        if edits or (allowed and not error):
            remaining, examples, wrong = leftovers(finder, case, edits)
            row["left_behind"] = dict(remaining)
            row["misplaced_edits"] = wrong
            print(f"    left behind: {dict(remaining) or 'nothing'}")
            for line in examples:
                print(f"        in code: {line}")
            for line in wrong:
                print(f"        MISPLACED EDIT: {line}")
            # The failure the whole tool exists for.
            if allowed and not error and not edits:
                print("    >>> ALLOWED AND EDITED NOTHING <<<")
                row["silent_noop"] = True
        report.append(row)

    audit.server.stop()

    print("\n| shape | prepare | edits | files | left in code |")
    print("| --- | --- | ---: | ---: | ---: |")
    for row in report:
        print(
            f"| {row['shape']} | "
            + ("allowed" if row["prepare_allowed"] else "refused")
            + f" | {row['edits']} | {row['files']} "
            f"| {(row.get('left_behind') or {}).get('code', '-')} |"
        )

    if args.out:
        with open(args.out, "w", encoding="utf-8") as handle:
            json.dump(report, handle, indent=2)
        print(f"\nwritten to {args.out}")


if __name__ == "__main__":
    main()
