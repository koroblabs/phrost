#!/usr/bin/env python3
"""Asks whether every capability `initialize` advertises actually answers on real code.

    python3 tools/capability-audit.py <project-root> [--filter modules/custom]

`compare-lsp.py` already scores definition, hover, completion and outline against another server.
This asks a narrower and blunter question of the *rest* of the advertised list: at twenty real
positions, does it answer at all, and is the answer worth having?

The distinction matters because a technically-non-empty answer is the easiest thing in a language
server to ship by accident. A `selectionRange` that only ever returns the token under the cursor is
non-empty at every position and useless at all of them; so is a `documentHighlight` that highlights
the occurrence you are already looking at. So every capability here is scored twice -- `answered`
and `useful` -- with `useful` defined per capability and stated in `USEFUL`, below.

Positions are chosen by scanning the file *text*, reusing `compare-lsp.py`'s scanner, so the sample
is not drawn from phrost's own index.
"""
import argparse
import difflib
import importlib.util
import json
import os
import re
import subprocess
import sys
import time
from collections import Counter, defaultdict

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from lspclient import Closed, Server, Timeout, is_dependency_dir, path_of, uri_of  # noqa: E402

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


def _compare_module():
    """`compare-lsp.py`'s scanner, imported by path because its name is not an identifier.

    Reused rather than copied so the two tools cannot drift into asking about different positions,
    which would make their two reports incomparable without saying so.
    """
    where = os.path.join(os.path.dirname(os.path.abspath(__file__)), "compare-lsp.py")
    spec = importlib.util.spec_from_file_location("compare_lsp", where)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


C = _compare_module()

STILL_INDEXING = -32801

# What each capability is measured against. Stated here rather than buried in the checks so the
# verdicts in the report can be argued with.
USEFUL = {
    "declaration": "lands somewhere, and agrees with definition (PHP has no separate declaration)",
    "typeDefinition": "lands on a type, and on a variable that is not where definition lands",
    "implementation": "returns at least one place that is not the declaration asked about",
    "references": "returns more than the declaration itself",
    "documentHighlight": "marks every occurrence the text has, not just the one under the cursor",
    "selectionRange": "the parent chain is deeper than the token under the cursor",
    "foldingRange": "ranges cover more than the outermost brace, and span real regions",
    "semanticTokens": "colours most of the identifiers in the file, in more than one type",
    "inlayHint": "hints appear, and say something the source text does not already",
    "signatureHelp": "a signature with parameters, and an activeParameter that tracks the comma",
    "codeAction": "an action with an edit or a command, not a title with nothing behind it",
    "documentFormatting": "reformats a mangled buffer back, not just requoting",
    "workspaceSymbol": "the query finds the symbol it names",
    "typeHierarchy": "prepare succeeds and at least one direction has an entry",
}

CAPABILITIES = {
    "window": {"workDoneProgress": True},
    "workspace": {"workspaceFolders": True, "symbol": {"dynamicRegistration": False}},
    "textDocument": {
        "synchronization": {"didSave": True, "willSave": False},
        "definition": {"linkSupport": False},
        "declaration": {"linkSupport": False},
        "typeDefinition": {"linkSupport": False},
        "implementation": {"linkSupport": False},
        "hover": {"contentFormat": ["markdown", "plaintext"]},
        "completion": {
            "completionItem": {"snippetSupport": True, "documentationFormat": ["markdown"]},
            "contextSupport": True,
        },
        "publishDiagnostics": {"relatedInformation": True},
        "documentSymbol": {"hierarchicalDocumentSymbolSupport": True},
        "foldingRange": {"lineFoldingOnly": False},
        "selectionRange": {"dynamicRegistration": False},
        "documentHighlight": {"dynamicRegistration": False},
        "rename": {"prepareSupport": True, "dynamicRegistration": False},
        "signatureHelp": {
            "signatureInformation": {"parameterInformation": {"labelOffsetSupport": True}},
            "contextSupport": True,
        },
        "codeAction": {
            "codeActionLiteralSupport": {
                "codeActionKind": {
                    "valueSet": ["", "quickfix", "refactor", "source", "source.organizeImports"]
                }
            },
            "resolveSupport": {"properties": ["edit"]},
        },
        # Without dynamic registration the server never registers type hierarchy, and a client that
        # does not claim it measures an absence it caused itself.
        "typeHierarchy": {"dynamicRegistration": True},
        # The legend a real client declares, and it has to be declared: a server is entitled to send
        # only the kinds the client says it can render, and phrost's legend is the *intersection* --
        # `answers/tokens.rs` says so. Declaring `tokenTypes: []` therefore asks for nothing and gets
        # nothing, which this tool then reported as **0/20 for a working capability**. A direct probe
        # of the same file returns 659 tokens. An instrument that understates is as wrong as one that
        # flatters, and this one understated for as long as the empty list was there.
        "semanticTokens": {
            "requests": {"full": True, "range": True},
            "tokenTypes": [
                "namespace",
                "type",
                "class",
                "enum",
                "interface",
                "struct",
                "typeParameter",
                "parameter",
                "variable",
                "property",
                "enumMember",
                "event",
                "function",
                "method",
                "macro",
                "keyword",
                "modifier",
                "comment",
                "string",
                "number",
                "regexp",
                "operator",
                "decorator",
            ],
            "tokenModifiers": [
                "declaration",
                "definition",
                "readonly",
                "static",
                "deprecated",
                "abstract",
                "async",
                "modification",
                "documentation",
                "defaultLibrary",
            ],
            "formats": ["relative"],
        },
        "inlayHint": {"resolveSupport": {"properties": ["tooltip"]}},
        "formatting": {"dynamicRegistration": False},
    },
}


# -- extra site scans this tool needs and compare-lsp does not -----------------------------------

DECLARED = re.compile(
    r"^(?:\s*)(?:final\s+|abstract\s+|readonly\s+)*(class|interface|trait|enum)\s+([A-Za-z_]\w*)",
    re.MULTILINE,
)
METHOD = re.compile(
    r"^\s*(?:final\s+|abstract\s+|public\s+|protected\s+|private\s+|static\s+)*"
    r"function\s+&?([A-Za-z_]\w*)\s*\(",
    re.MULTILINE,
)
# A call whose argument list has something in it: `foo(1, $x)`. Nested parens are excluded on
# purpose -- the inner call would be the real question and this scan cannot tell which is which.
CALL_ARGS = re.compile(r"([A-Za-z_]\w*)\s*\(([^()\n]*[^\s()][^()\n]*)\)")
# Written like a call and not one. `declare(strict_types=1)` accounted for thirteen of twenty
# signature-help questions in the first run, and a server is right to say nothing about it.
NOT_A_CALL = {
    "declare", "if", "elseif", "while", "for", "foreach", "switch", "match", "catch", "fn",
    "function", "list", "isset", "unset", "empty", "array", "echo", "print", "exit", "die",
    "return", "use", "and", "or", "xor", "clone", "yield", "require", "include",
    "require_once", "include_once", "int", "string", "float", "bool", "void", "static",
}
SCALARS = {
    "string", "int", "integer", "float", "bool", "boolean", "array", "mixed", "null", "void",
    "iterable", "callable", "object", "resource", "never", "false", "true", "self", "static",
    "this", "list", "non-empty-string", "positive-int",
}


class Extra:
    """The positions the shared scanner does not produce, read off the same masked text."""

    @staticmethod
    def declarations(masked):
        """The name token of every class-like declaration in the file."""
        return [
            (C.inside(match.start(2), match.group(2)), match.group(2), match.group(1))
            for match in DECLARED.finditer(masked)
        ]

    @staticmethod
    def methods(masked):
        """The name token of every method or function declaration in the file."""
        return [
            (C.inside(match.start(1), match.group(1)), match.group(1))
            for match in METHOD.finditer(masked)
        ]

    @staticmethod
    def arguments(masked):
        """A position inside a non-empty argument list, and one after its first comma if there is one."""
        found = []
        for match in CALL_ARGS.finditer(masked):
            if match.group(1).lower() in NOT_A_CALL:
                continue
            # A declaration's parameter list is not a call either, and asking inside one is asking
            # about the signature being written rather than a signature being used.
            before = masked[max(0, match.start() - 20) : match.start()]
            if re.search(r"function\s+$", before):
                continue
            inner = match.start(2)
            found.append((inner + 1, match.group(1), 0))
            comma = match.group(2).find(",")
            if comma >= 0:
                found.append((inner + comma + 2, match.group(1), 1))
        return found

    @staticmethod
    def signatures(masked):
        """The parenthesised spans of every function or method *declaration* in the file.

        A variable written inside one of these is a parameter being declared, which is a different
        position from a use of it -- and, as it turns out, one several capabilities answer nothing
        at. Kept as spans so a site can be classified by containment.
        """
        spans = []
        for match in re.finditer(r"function\s+&?\w*\s*\(", masked):
            depth = 0
            for at in range(match.end() - 1, len(masked)):
                if masked[at] == "(":
                    depth += 1
                elif masked[at] == ")":
                    depth -= 1
                    if depth == 0:
                        spans.append((match.end() - 1, at))
                        break
        return spans

    @staticmethod
    def scope_of(spans, offset, end_of_file):
        """The span a variable at `offset` lives in: its own function, up to the next one.

        PHP has no block scope and a variable never crosses a function boundary, so the declaration
        a site belongs to and the next declaration bracket everything that can be the same variable.
        A closure's `use (...)` sits inside that window and so travels with it.
        """
        starts = sorted(start for start, _ in spans)
        before = [start for start in starts if start <= offset]
        after = [start for start in starts if start > offset]
        return (before[-1] if before else 0, after[0] if after else end_of_file)


# -- the session ----------------------------------------------------------------------------------


class Audit:
    def __init__(self, root, argv, verbose):
        self.root = root
        self.verbose = verbose
        self.diagnostics = {}
        self.registered = set()
        self.timings = defaultdict(list)
        log = os.path.join(
            os.path.expanduser("~/.cache/phrost-compare"), "capability-audit.stderr.log"
        )
        os.makedirs(os.path.dirname(log), exist_ok=True)
        self.server = Server("phrost", argv, root, log=log)
        self.server.on_notification(self._heard)
        self.working = set()
        self.ready_signal = None
        # One counter per document, because the server refuses a version it has already passed --
        # correctly. Two checks each re-opening a buffer with their own fixed numbers meant the
        # second one silently measured the first one's text.
        self.versions = Counter()

    def send(self, path, text):
        """Replaces an open buffer's text, at a version the server will accept."""
        self.versions[path] += 1
        self.server.open(path, text, version=self.versions[path])

    def _heard(self, method, params):
        if method == "textDocument/publishDiagnostics":
            self.diagnostics[path_of(params.get("uri", ""))] = params.get("diagnostics") or []
        elif method == "$/progress":
            value = params.get("value") or {}
            token = json.dumps(params.get("token"))
            if value.get("kind") == "begin":
                self.working.add(token)
            elif value.get("kind") == "end":
                self.working.discard(token)
                self.ready_signal = self.ready_signal or "progress end"

    def start(self, probe, warmup):
        started = time.monotonic()
        answer = self.server.initialize(CAPABILITIES)
        caps = answer.get("capabilities", {})
        deadline = time.monotonic() + warmup
        how = "no probe"
        while time.monotonic() < deadline:
            self.server.poll(1.0)
            if probe is None:
                break
            path, name = probe
            found = self.server.request("workspace/symbol", {"query": name}, timeout=60.0)
            if any(
                os.path.basename(path_of((hit.get("location") or {}).get("uri", "")))
                == os.path.basename(path)
                for hit in found.get("result") or []
            ):
                how = f"index has {name}"
                break
        else:
            how = "NOT READY -- treat these answers as suspect"
        self.server.pump(0.5)
        print(f"  ready in {time.monotonic() - started:.1f}s ({how})", flush=True)
        return caps

    def ask(self, method, params, timeout=30.0):
        for _ in range(40):
            started = time.monotonic()
            try:
                answer = self.server.request(method, params, timeout=timeout)
            except (Timeout, Closed) as failed:
                return {"error": {"message": str(failed)}}
            if (answer.get("error") or {}).get("code") == STILL_INDEXING:
                time.sleep(0.5)
                continue
            self.timings[method].append((time.monotonic() - started) * 1000)
            return answer
        return {"error": {"message": "still indexing"}}

    def result(self, method, params, timeout=30.0):
        return self.ask(method, params, timeout).get("result")

    def stop(self):
        self.server.stop()


# -- scoring --------------------------------------------------------------------------------------


class Score:
    """One capability's tally, plus the examples that make the tally arguable."""

    def __init__(self, name, wanted):
        self.name = name
        self.wanted = wanted
        self.asked = 0
        self.answered = 0
        self.useful = 0
        self.good = []  # (where, what) for answers worth having
        self.bad = []  # (where, why) for answers that should have been better
        self.notes = Counter()

    def record(self, where, answered, useful, detail="", note=None):
        self.asked += 1
        self.answered += bool(answered)
        self.useful += bool(useful)
        if note:
            self.notes[note] += 1
        if useful and len(self.good) < 4:
            self.good.append((where, detail))
        elif answered and not useful and len(self.bad) < 6:
            self.bad.append((where, detail or "answered but not useful"))
        elif not answered and len(self.bad) < 6:
            self.bad.append((where, detail or "empty"))

    def row(self):
        return {
            "capability": self.name,
            "asked": self.asked,
            "answered": self.answered,
            "useful": self.useful,
            "useful_means": self.wanted,
            "notes": dict(self.notes.most_common(8)),
            "good": self.good,
            "bad": self.bad,
        }


def locations_of(result):
    """A goto answer as (basename, line) pairs, whichever of the three shapes it came in."""
    if not result:
        return []
    if isinstance(result, dict):
        result = [result]
    found = []
    for item in result:
        uri = item.get("targetUri") or item.get("uri")
        span = item.get("targetSelectionRange") or item.get("targetRange") or item.get("range")
        if uri and span:
            found.append((os.path.basename(path_of(uri)), span["start"]["line"]))
    return found


def depth_of(selection):
    """How many nested ranges a selectionRange answer offers, counting the innermost as one."""
    depth = 0
    node = selection
    seen = set()
    while isinstance(node, dict):
        span = node.get("range") or {}
        key = json.dumps(span, sort_keys=True)
        if key in seen:  # a self-parenting chain would otherwise spin
            break
        seen.add(key)
        depth += 1
        node = node.get("parent")
    return depth


def apply_edits(text, edits):
    """Applies a set of LSP TextEdits to a string, last position first."""
    lines = C.Lines(text)
    offsets = lines.starts

    def offset(position):
        line = min(position["line"], len(offsets) - 1)
        start = offsets[line]
        # Characters are UTF-16 code units; walk them rather than assuming one byte each.
        want = position["character"]
        at = start
        used = 0
        while at < len(text) and used < want and text[at] != "\n":
            used += len(text[at].encode("utf-16-le")) // 2
            at += 1
        return at

    ordered = sorted(edits, key=lambda edit: offset(edit["range"]["start"]), reverse=True)
    out = text
    for edit in ordered:
        start = offset(edit["range"]["start"])
        end = offset(edit["range"]["end"])
        out = out[:start] + edit["newText"] + out[end:]
    return out


HEREDOC_OPENS = re.compile(r"<<<\s*[\'\"]?([A-Za-z_]\w*)")


def blank_comments(text):
    """Blanks every comment, leaving the code's own offsets alone.

    Strings must already be gone, or `$sql = "https://x/y";` loses its tail to a `//` that is data
    -- the same ordering the server's own reader of this question takes, and the reason its tests
    say a comment opener inside a string is part of the string. `#[` is an attribute and not a hash
    comment, which is the one place PHP 8 made this harder than it looks.
    """
    out = []
    i = 0
    while i < len(text):
        two = text[i : i + 2]
        if two == "/*":
            end = text.find("*/", i + 2)
            end = len(text) if end < 0 else end + 2
            out.append("".join(" " if c != "\n" else "\n" for c in text[i:end]))
            i = end
        elif two == "//" or (text[i] == "#" and two != "#["):
            end = text.find("\n", i)
            end = len(text) if end < 0 else end
            out.append(" " * (end - i))
            i = end
        else:
            out.append(text[i])
            i += 1
    return "".join(out)


def writes_outside_a_string(path, name):
    """Whether a file writes `name` anywhere a string literal or a comment does not cover.

    The last shape a lookaround cannot reach: `require_once __DIR__ . '/DatabaseRepository.php';`
    holds the word inside a quoted path, with a `/` before it and a `.` after, so neither a
    lookbehind on the quote nor one on the backslash sees it. Reading the file settles it -- there
    are a handful of candidates by this point, and the word is the thing being asked about.

    And **prose is not a reference**. Three files of one real project write
    `Constructs a RegionDistrictDefaultFormatter object.` in a copy-pasted docblock and name the class
    nowhere else, so `references` answering two sites in one file was correct and scored 1/4. The
    server has exactly this rule -- `a_word_in_prose_is_not_a_construct_that_reaches_anything` --
    and an instrument grading it needs the same one, which is §9's note about these tools said once
    more.
    """
    try:
        text = open(path, encoding="utf-8", errors="replace").read()
    except OSError:
        return True
    blanked = blank_comments(STRING_LITERAL.sub(lambda match: " " * len(match.group(0)), text))
    return re.search(r"(?<![\w$])" + re.escape(name) + r"(?!\w)", blanked) is not None


def names_its_own_parameter(masked, site):
    """Whether the argument at this site is a bare variable, which a parameter hint would repeat.

    An approximation on purpose: the tool cannot know the parameter's name without asking the server
    it is grading. What it can see is the shape a hint is never drawn over -- `$sql` alone, where the
    name is already on the line -- against a literal or an expression, where it is not.
    """
    # One character back, because `Extra.arguments` gives a position *inside* the list rather than
    # the argument's start: for `($sql, ...)` it points at the `s`, and a test written from there
    # never sees the `$` it is looking for.
    at = max(0, site[0] - 1)
    rest = masked[at : at + 60]
    return re.match(r"\s*\$\w+\s*[,)]", rest) is not None


def mangle(text):
    """Strips the formatting out of a buffer without changing what it means.

    A formatter that only requotes strings leaves this looking exactly as it arrived, which is the
    claim this whole tool exists to test.

    A **heredoc body is data**, and this used to mangle it like code: leading whitespace stripped and
    the spaces around `,;=+.` squeezed, inside a string whose contents the formatter has no business
    restoring and no way to. It scored one file in twenty as a formatter that loses 5 % of the
    buffer, and what it had really done was rewrite a Twig template and ask for it back.
    """
    out = []
    terminator = None
    quote = None
    for line in text.split("\n"):
        stripped = line.strip()
        # A string literal that runs past the end of its line is data on every line it covers. PHP
        # lets a single- or double-quoted string hold newlines, and this project writes its SQL that
        # way: `'INSERT INTO users (...) VALUES` opens on one line and closes four below. Mangled as
        # code, every comma inside it moved.
        if quote is not None:
            out.append(line)
            if unquoted_after(line, quote) is None:
                quote = None
            continue
        if terminator is not None:
            out.append(line)
            # PHP 7.3 and up allow the closing label to be indented, and anything may follow it.
            if stripped == terminator or stripped.startswith(terminator + ";"):
                terminator = None
            continue
        if stripped.startswith("*") or stripped.startswith("/*") or stripped.startswith("//"):
            out.append(line)  # comment bodies are not the formatter's to normalise
            continue
        opened = HEREDOC_OPENS.search(line)
        if opened:
            terminator = opened.group(1)
            out.append(line)
            continue
        # Where the line leaves a string open, only the code *before* the quote is squeezed: the
        # rest of the line is already inside the literal. Asked before squeezing, because the
        # opening line is the one that carries the data -- `$this->execute($db, 'INSERT INTO users
        # (user_id, username, ...) VALUES` is code up to the quote and a payload after it.
        quote, opened_at = still_open(stripped)
        if quote is None:
            out.append(squeeze(stripped))
        else:
            out.append(squeeze(stripped[:opened_at]) + stripped[opened_at:])
    return "\n".join(out)


def still_open(line):
    """The quote a line leaves unclosed and where it opens, or `(None, None)` when it closes them all.

    Complete literals are taken out first, so what is counted is the quotes left over. A line with an
    odd one has opened a string that runs on to the next line -- which PHP allows for both kinds, and
    which is how one project writes its SQL.
    """
    blanked = STRING_LITERAL.sub(lambda match: " " * len(match.group(0)), line)
    for mark in ("'", '"'):
        if blanked.count(mark) % 2:
            return mark, blanked.index(mark)
    return None, None


def unquoted_after(line, mark):
    """`None` once an unescaped `mark` closes the string, else `mark` because it is still open.

    Escapes are the whole of the work. A continuation line is *inside* the literal, so nothing on it
    is a token -- `("sq3", "What was your first pet\\'s name?")` inside a single-quoted SQL string
    holds an escaped apostrophe, and reading it as the closing quote handed the rest of the payload
    back to the squeezer.
    """
    at = 0
    while at < len(line):
        if line[at] == "\\":
            at += 2
            continue
        if line[at] == mark:
            return None
        at += 1
    return mark


STRING_LITERAL = re.compile(r"'(?:\\.|[^'\\])*'|\"(?:\\.|[^\"\\])*\"")


def squeeze(line):
    """The spaces around `,;=+.` taken out, everywhere except inside a string.

    A string is data. `'Content-Type, Authorization, X-Requested-With'` holds three of the
    characters this squeezes and a formatter cannot put the spaces back -- so squeezing them made a
    correct reprint score 0.985 of the original, three times on a thirteen-file project. The same
    mistake as mangling a heredoc, one spelling smaller.
    """
    held = []

    def keep(match):
        held.append(match.group(0))
        return f"\x00{len(held) - 1}\x00"

    masked = STRING_LITERAL.sub(keep, line)
    masked = re.sub(r"\s*([,;])\s*", r"\1", masked)
    masked = re.sub(r"\s*([=+.])\s*", r"\1", masked)
    return re.sub(r"\x00(\d+)\x00", lambda m: held[int(m.group(1))], masked)


# -- the checks -------------------------------------------------------------------------------


class Checks:
    """Every capability's probe. One method each, so adding one adds a method."""

    def __init__(self, audit, plan, limit, root):
        self.audit = audit
        self.plan = plan
        self.limit = limit
        self.root = root
        self.grepped = {}
        self.scores = {}
        # A quota per file as well as a total, so twenty positions are twenty positions from across
        # the sample rather than every position in the first two files. A capability whose sample is
        # one directory deep is measuring that directory.
        self.per_file = max(1, -(-limit // max(1, len(plan))))
        self.spent = Counter()

    def score(self, name):
        if name not in self.scores:
            self.scores[name] = Score(name, USEFUL.get(name, ""))
        return self.scores[name]

    def next_file(self):
        self.spent.clear()

    def take(self, name):
        """Whether this capability may ask one more question, here and overall."""
        score = self.score(name)
        if score.asked >= self.limit or self.spent[name] >= self.per_file:
            return False
        self.spent[name] += 1
        return True

    # -- goto-shaped ------------------------------------------------------------------------

    @staticmethod
    def where_written(spans, offset):
        """`parameter` when the position is inside a declaration's own parameter list, else `use`."""
        return "parameter" if any(start < offset < end for start, end in spans) else "use"

    def hovered_type(self, uri, position):
        """What hover says the thing under the cursor is, as a bare type string.

        Used as the oracle for typeDefinition: a variable typed `string` has no type definition to
        go to, and counting that silence as a failure would score the sample rather than the server.
        """
        card = self.audit.result(
            "textDocument/hover", {"textDocument": {"uri": uri}, "position": position}
        )
        contents = (card or {}).get("contents") or {}
        body = contents.get("value") if isinstance(contents, dict) else str(contents)
        if not body:
            return None
        # The card holds a fenced signature -- `string $errorKey`, or a bare `$x` where nothing is
        # known. The type is whatever precedes the variable.
        #
        # The *line* holding the variable, not the whole fence. A promoted constructor property
        # renders as a full class card -- `<?php`, a namespace, the class, then
        # `private array $values` -- and taking everything before the first `$` made the type
        # `<?php\nnamespace ...`, which `class_like` then read as a name worth going to. One
        # `typeDefinition` in twenty scored as an empty answer where an `array` has nothing to go to
        # and silence was right.
        fence = re.search(r"```(?:php)?\n(.*?)\n```", body, re.S)
        block = (fence.group(1) if fence else body.split("\n")[0]).strip()
        lines = [line.strip() for line in block.split("\n") if line.strip()]
        head = next((line for line in reversed(lines) if "$" in line), lines[0] if lines else "")
        if "$" in head:
            head = head[: head.index("$")]
        # Without the modifiers a property carries. `protected string $name` is a `string`, and
        # reading the whole of what precedes the variable made the type `protected string` -- which
        # `class_like` reads as a name worth going to, so a correct silence scored as an empty
        # answer. The declaration keyword is not part of the type.
        words = [
            word
            for word in head.split()
            if word not in ("public", "protected", "private", "static", "readonly", "var")
        ]
        return " ".join(words).strip() or None

    @staticmethod
    def class_like(written):
        """Whether a type string names something that could have a declaration to go to."""
        if not written:
            return False
        # Without the arguments a generic carries. `array<string, array<string, mixed>>` is an
        # `array` and has nothing to go to; keeping the arguments made it a name no scalar list
        # holds, so a correct silence scored as an empty answer. `Collection<Item>` reduces to
        # `Collection`, which is still worth going to.
        written = re.sub(r"<.*", "", written)
        parts = [
            part.strip().lstrip("?\\").rstrip("[]")
            for part in re.split(r"[|&]", written)
            if part.strip()
        ]
        return any(part and part.lower() not in SCALARS and part[:1].isalpha() for part in parts)

    def navigation(self, path, text, lines, sites):
        """declaration and typeDefinition, at the same positions as definition."""
        uri = uri_of(path)
        # A variable first where the file has one: typeDefinition and definition only differ there,
        # and a sample of class names cannot tell the two questions apart at all.
        ordered = [site for site in sites if site.kind == "var"] + [
            site for site in sites if site.kind != "var"
        ]
        for site in ordered:
            where = {"textDocument": {"uri": uri}, "position": lines.position(site.offset)}
            label = f"{os.path.basename(path)} {site.shape}"

            if self.take("declaration"):
                mine = locations_of(self.audit.result("textDocument/declaration", where))
                theirs = locations_of(self.audit.result("textDocument/definition", where))
                agrees = bool(mine) and set(mine) == set(theirs)
                self.score("declaration").record(
                    label,
                    bool(mine),
                    agrees,
                    f"-> {mine[:2]}" if mine else "empty",
                    note="agrees with definition"
                    if agrees
                    else ("differs from definition" if mine else "empty"),
                )

            if self.take("typeDefinition"):
                mine = locations_of(self.audit.result("textDocument/typeDefinition", where))
                plain = locations_of(self.audit.result("textDocument/definition", where))
                same = bool(mine) and set(mine) == set(plain)
                if site.kind == "var":
                    # Only a class-typed variable has a type definition to go to; `string` has
                    # none, and silence there is the right answer rather than a miss. Where there
                    # is one, it must not be where definition already went -- that is definition
                    # wearing a second name.
                    written = self.hovered_type(uri, lines.position(site.offset))
                    wanted = self.class_like(written)
                    useful = (bool(mine) and not same) if wanted else not mine
                    note = (
                        f"var typed {written or 'nothing'}: "
                        + (
                            "correctly silent"
                            if not wanted and not mine
                            else "answered a scalar"
                            if not wanted
                            else "its own answer"
                            if useful
                            else "same as definition"
                            if mine
                            else "EMPTY"
                        )
                    )
                else:
                    # For a written class name the type *is* the class, so agreeing with definition
                    # is the correct answer and not a demerit.
                    written, useful = None, bool(mine)
                    note = f"{site.kind}: " + ("answers" if mine else "empty")
                self.score("typeDefinition").record(
                    f"{label} [{site.kind}]",
                    bool(mine) or (site.kind == "var" and not self.class_like(written)),
                    useful,
                    f"-> {mine[:2]}, definition -> {plain[:2]}" if mine else "empty",
                    note=note,
                )

    def implementation(self, path, text, lines, masked, sites):
        """At an interface or abstract class -- the only place the question has an answer.

        Asking on a `final class` and reporting the silence would be measuring the sample, not the
        server, so the positions are the abstractions: what the file declares when it declares an
        interface or an abstract class, and every `*Interface` name it mentions.
        """
        uri = uri_of(path)
        spots = [
            (offset, name, kind)
            for offset, name, kind in Extra.declarations(masked)
            if kind in ("interface", "trait")
            or re.search(rf"abstract\s+class\s+{name}\b", masked)
        ]
        spots += [
            (site.offset, site.name, "use of")
            for site in sites
            if site.kind in ("name", "new") and site.name.endswith(("Interface", "Base"))
        ]
        for offset, name, kind in spots:
            if not self.take("implementation"):
                return
            where = {"textDocument": {"uri": uri}, "position": lines.position(offset)}
            mine = locations_of(self.audit.result("textDocument/implementation", where))
            here = (os.path.basename(path), lines.position(offset)["line"])
            beyond = [hit for hit in mine if hit != here]
            self.score("implementation").record(
                f"{os.path.basename(path)} {kind} {name}",
                bool(mine),
                bool(beyond),
                f"{len(mine)} places, e.g. {mine[:3]}" if mine else "empty",
                note=f"{kind}: {'answers' if beyond else 'empty'}",
            )

    def references(self, path, text, lines, masked):
        """At declaration sites, where a reference search is what a person actually runs."""
        uri = uri_of(path)
        spots = [
            (offset, name, kind) for offset, name, kind in Extra.declarations(masked)
        ] + [(offset, name, "method") for offset, name in Extra.methods(masked)]
        for offset, name, kind in C.stride(spots, 4):
            if not self.take("references"):
                return
            where = {"textDocument": {"uri": uri}, "position": lines.position(offset)}
            found = (
                self.audit.result(
                    "textDocument/references",
                    dict(where, context={"includeDeclaration": True}),
                    timeout=90.0,
                )
                or []
            )
            files = {path_of(hit["uri"]) for hit in found}
            # An upper bound nobody can argue with: how many files of the project's own code write
            # the identifier at all. It over-counts -- a comment, a string, a same-named member of
            # another class -- so a shortfall is a question and not yet a verdict. What it settles
            # is the other direction: a name written in twenty files and answered in one is a gap,
            # and a class genuinely used nowhere is not.
            # Only for a class. A **method** name is not a name the file text can bound: every class
            # writes `__construct`, and `response_notes_list` in another class is another member.
            # Scored against the word count, a correct answer of one file read as 1/12 -- four of
            # them on a thirteen-file project. What is left for a method is the direction the whole
            # check is about: it answered, and the declaration is among what it answered with.
            mentions = self.mentions(name) if kind != "method" else set()
            enough = (
                len(files) >= len(mentions)
                if kind != "method"
                else bool(found) and os.path.abspath(path) in {os.path.abspath(f) for f in files}
            )
            self.score("references").record(
                f"{os.path.basename(path)} {kind} {name}",
                bool(found),
                enough,
                f"{len(found)} refs across {len(files)} files; "
                + (
                    f"{len(mentions)} files of user code write the word"
                    if kind != "method"
                    else "a method name bounds nothing"
                ),
                note=(
                    "empty"
                    if not found
                    else "the declaration and its callers"
                    if kind == "method"
                    else "every file that writes it"
                    if len(files) >= len(mentions)
                    else f"{len(files)}/{len(mentions)} files that write it"
                ),
            )

    def mentions(self, name):
        """Files of the project's own code whose text contains `name` as a word.

        Not merely `rg -w`. Two shapes share a class's spelling and are not it, and both are
        identifiable without a parser:

        - a **namespace segment**. `Drupal\\my_module\\AttachmentType\\...` and
          `namespace ...\\AttachmentType;` hold the word between backslashes, and every file that
          imports anything from that namespace was counted as referencing the class.
        - a **quoted word**. `self::stringValue($row, 'AttachmentType')` is an array key in somebody
          else's payload.

        Measured on the reference project: `AttachmentType` went from 27 files to 6, which is the
        declaration plus its five importers -- and is exactly what phrost answered. Scoring it
        against 27 made a correct answer read as 6/27.
        """
        if name in self.grepped:
            return self.grepped[name]
        out = subprocess.run(
            [
                "rg", "-l", "--no-ignore", "--pcre2", "--glob", "*.php", "--glob", "!vendor/**",
                "--", rf"(?<![\\'\"])\b{re.escape(name)}\b(?!['\"\\])", self.root,
            ],
            capture_output=True,
            text=True,
        )
        # Through the one rule about whose code a path holds, which `rg`'s globs cannot express:
        # Drupal installs its core at `<docroot>/core` and its contributed modules at
        # `modules/contrib`, composer-installed and nowhere near `vendor/`. Counting those as the
        # project's own is the blind spot §9 records for these tools -- and the server does not have
        # it, so an upper bound built without the rule is measuring a different project.
        found = {
            line
            for line in out.stdout.split("\n")
            if line and not self.somebody_elses(line) and writes_outside_a_string(line, name)
        }
        self.grepped[name] = found
        return found

    def somebody_elses(self, path):
        """Whether any directory on the way to `path` is code the project did not write."""
        at = os.path.dirname(os.path.abspath(path))
        root = os.path.abspath(self.root)
        while at.startswith(root) and at != root:
            if is_dependency_dir(at, root):
                return True
            at = os.path.dirname(at)
        return False

    def highlight(self, path, text, lines, masked, sites):
        """Marked against what the *text* says: a name written five times wants five marks.

        The failure this is looking for is a highlight that marks the occurrence under the cursor and
        stops, which is non-empty everywhere and worth nothing anywhere. So the question is recall,
        not presence -- and a name genuinely written once is a correct answer of one.
        """
        uri = uri_of(path)
        signatures = Extra.signatures(masked)
        spots = [
            (site.offset, site.name, "$" + site.name, self.where_written(signatures, site.offset))
            for site in sites
            if site.kind == "var"
        ]
        spots += [
            (site.offset, site.name, site.name, site.kind)
            for site in sites
            if site.kind in ("member", "static")
        ]
        for offset, name, written, kind in C.stride(spots, 3):
            if not self.take("documentHighlight"):
                return
            where = {"textDocument": {"uri": uri}, "position": lines.position(offset)}
            marks = self.audit.result("textDocument/documentHighlight", where) or []
            kinds = Counter(
                {1: "text", 2: "read", 3: "write"}.get(mark.get("kind"), "?") for mark in marks
            )
            # A variable does not cross a function boundary, so counting its name across the whole
            # file compares the answer against other functions' variables. It scored three correct
            # answers as failures: `FreezeAction::isEnabled(Application $app)` whose body is
            # `return TRUE;` -- one mark is the only mark there is, and the five others belong to
            # two sibling methods -- and `AuditLoggerInterface`, three signatures declaring
            # `$clientType` and no bodies at all. The window is between the signature this site
            # belongs to and the next one, which is where PHP says the variable lives.
            window = (
                Extra.scope_of(signatures, offset, len(masked))
                if kind in ("parameter", "use")
                else (0, len(masked))
            )
            occurrences = len(
                re.findall(
                    r"(?<![\w$])" + re.escape(written) + r"(?!\w)",
                    masked[window[0] : window[1]],
                )
            )
            # `written` for a variable includes the `$`, so a lookbehind on `$` would never fire.
            enough = len(marks) >= min(2, occurrences) and len(marks) > 0
            self.score("documentHighlight").record(
                f"{os.path.basename(path)} {written} [{kind}]",
                bool(marks),
                enough,
                f"{len(marks)} marks {dict(kinds)}, written {occurrences}x in the file",
                note=(
                    f"{kind}: empty"
                    if not marks
                    else (
                        f"{kind}: all occurrences"
                        if len(marks) >= occurrences
                        else f"{kind}: some occurrences"
                    )
                ),
            )

    def selection(self, path, text, lines, sites):
        uri = uri_of(path)
        for site in sites:
            if not self.take("selectionRange"):
                return
            found = self.audit.result(
                "textDocument/selectionRange",
                {"textDocument": {"uri": uri}, "positions": [lines.position(site.offset)]},
            )
            first = (found or [None])[0]
            depth = depth_of(first) if first else 0
            innermost = (first or {}).get("range") or {}
            widens = depth > 1
            self.score("selectionRange").record(
                f"{os.path.basename(path)} {site.shape}",
                depth > 0,
                widens,
                f"chain depth {depth}, innermost "
                f"{innermost.get('start', {}).get('character')}-"
                f"{innermost.get('end', {}).get('character')}",
                note=f"depth {depth}",
            )

    def signature(self, path, text, lines, masked):
        uri = uri_of(path)
        for offset, callee, index in C.stride(Extra.arguments(masked), 6):
            if not self.take("signatureHelp"):
                return
            found = self.audit.result(
                "textDocument/signatureHelp",
                {"textDocument": {"uri": uri}, "position": lines.position(offset)},
            )
            signatures = (found or {}).get("signatures") or []
            first = signatures[0] if signatures else {}
            parameters = first.get("parameters") or []
            active = (found or {}).get("activeParameter")
            tracks = active == index if parameters else False
            self.score("signatureHelp").record(
                f"{os.path.basename(path)} {callee}(arg {index})",
                bool(signatures),
                bool(parameters) and tracks,
                f"{first.get('label', '')[:70]!r} active={active} want={index}",
                note=None
                if not signatures
                else ("no parameters" if not parameters else ("tracks" if tracks else "wrong active")),
            )

    def folding(self, path, text, lines):
        if not self.take("foldingRange"):
            return
        score = self.score("foldingRange")
        found = (
            self.audit.result(
                "textDocument/foldingRange", {"textDocument": {"uri": uri_of(path)}}
            )
            or []
        )
        kinds = Counter(fold.get("kind") or "block" for fold in found)
        spans = [fold["endLine"] - fold["startLine"] for fold in found]
        # A file with N `{` at line starts has at least N foldable regions; answering with one is
        # the outermost brace and nothing else.
        braces = len(re.findall(r"\{\s*$", text, re.MULTILINE))
        score.record(
            os.path.basename(path),
            bool(found),
            len(found) > 1 and max(spans, default=0) > 1,
            f"{len(found)} ranges {dict(kinds)}, braces at eol {braces}",
            note=",".join(sorted(kinds)),
        )

    def tokens(self, path, text, lines):
        if not self.take("semanticTokens"):
            return
        score = self.score("semanticTokens")
        found = self.audit.result(
            "textDocument/semanticTokens/full", {"textDocument": {"uri": uri_of(path)}}
        )
        data = (found or {}).get("data") or []
        types = Counter(data[index + 3] for index in range(0, len(data), 5))
        identifiers = len(set(re.findall(r"[A-Za-z_]\w*", C.mask(text))))
        score.record(
            os.path.basename(path),
            bool(data),
            len(data) // 5 > identifiers // 4 and len(types) > 2,
            f"{len(data) // 5} tokens, {len(types)} types, {identifiers} distinct identifiers",
            note=f"{len(types)} types",
        )

    def hints(self, path, text, lines, masked):
        if not self.take("inlayHint"):
            return
        score = self.score("inlayHint")
        # A file with no calls in it has no parameter hints to draw, and counting that as a miss
        # would score the sample. Argument lists are what a parameter hint attaches to.
        #
        # And only the lists a hint is *for*. A hint that repeats the argument teaches a reader to
        # stop looking -- `f($id)` gets no `id:` over it, which the hints module states as its
        # opening rule -- so a file whose every call passes a bare variable has no hints to draw and
        # is right to draw none. `DatabaseRepository` is eight calls of
        # `$this->database->queryOne($sql, $params)` into `(string $sql, array $params)`, and it
        # scored as a capability that answered nothing.
        callable_sites = len(
            [site for site in Extra.arguments(masked) if not names_its_own_parameter(masked, site)]
        )
        found = (
            self.audit.result(
                "textDocument/inlayHint",
                {
                    "textDocument": {"uri": uri_of(path)},
                    "range": {
                        "start": {"line": 0, "character": 0},
                        "end": {"line": len(lines.starts), "character": 0},
                    },
                },
            )
            or []
        )
        kinds = Counter({1: "type", 2: "parameter"}.get(hint.get("kind"), "?") for hint in found)
        labels = [
            hint["label"] if isinstance(hint["label"], str)
            else "".join(part.get("value", "") for part in hint["label"])
            for hint in found[:3]
        ]
        score.record(
            os.path.basename(path),
            bool(found) or callable_sites == 0,
            bool(found) or callable_sites == 0,
            f"{len(found)} hints {dict(kinds)} over {callable_sites} argument lists e.g. {labels}",
            note=(
                "no calls to hint at"
                if callable_sites == 0
                else (",".join(sorted(kinds)) if found else "calls but no hints")
            ),
        )

    def actions(self, path, text, lines, masked):
        """Where a fix could exist: at a diagnostic, at a declaration, and on the import block.

        Asking at an arbitrary line and reporting the empty menu would be a fair test of nothing --
        an empty menu in the middle of a statement is the correct answer. These are the three places
        the providers say they offer something.
        """
        score = self.score("codeAction")
        uri = uri_of(path)
        found = self.audit.diagnostics.get(os.path.abspath(path)) or []
        spans = [item["range"] for item in found[:2]]
        for offset, name in Extra.methods(masked)[:1]:
            here = lines.position(offset)
            spans.append({"start": here, "end": here})
        imports = re.search(r"^use\s", masked, re.MULTILINE)
        if imports:
            here = lines.position(imports.start() + 4)
            spans.append({"start": here, "end": here})
        for span in spans:
            if not self.take("codeAction"):
                return
            here = [item for item in found if item["range"] == span]
            actions = (
                self.audit.result(
                    "textDocument/codeAction",
                    {
                        "textDocument": {"uri": uri},
                        "range": span,
                        "context": {"diagnostics": here},
                    },
                )
                or []
            )
            real = [
                action
                for action in actions
                if isinstance(action, dict) and (action.get("edit") or action.get("command"))
            ]
            titles = [action.get("title", "?") for action in actions[:3]]
            score.record(
                f"{os.path.basename(path)}:{span['start']['line'] + 1}"
                + (f" [{here[0].get('code')}]" if here else " [declaration or import]"),
                bool(actions),
                bool(real),
                f"{len(actions)} actions, {len(real)} with an edit: {titles}",
                note=("at a diagnostic" if here else "at a declaration or import")
                + (": offers" if real else ": empty"),
            )

    def formatting(self, path, text, lines):
        if not self.take("documentFormatting"):
            return
        """Formats the file as written, then formats a mangled copy and checks it comes back."""
        score = self.score("documentFormatting")
        uri = uri_of(path)
        options = {"tabSize": 2, "insertSpaces": True}
        edits = (
            self.audit.result(
                "textDocument/formatting", {"textDocument": {"uri": uri}, "options": options}
            )
            or []
        )
        clean = apply_edits(text, edits) if edits else text

        # The real question. A quote-swapper leaves a mangled buffer mangled.
        broken = mangle(text)
        self.audit.send(path, broken)
        repair = (
            self.audit.result(
                "textDocument/formatting", {"textDocument": {"uri": uri}, "options": options}
            )
            or []
        )
        repaired = apply_edits(broken, repair) if repair else broken
        self.audit.send(path, text)

        restored = sum(1 for line in repaired.split("\n") if line[:1] in (" ", "\t"))
        before = sum(1 for line in broken.split("\n") if line[:1] in (" ", "\t"))
        recovered = difflib.SequenceMatcher(None, clean, repaired).ratio()
        # The whole test: does a mangled buffer come back as the formatted original. Counting
        # restored indentation alone would pass a formatter that indents and nothing else, and
        # would fail a five-line file that has almost no indentation to restore.
        score.record(
            os.path.basename(path),
            bool(edits) or bool(repair),
            recovered > 0.99 and repaired != broken,
            f"as-written {len(edits)} edits; mangled -> {recovered:.3f} of the formatted "
            f"original, {restored} indented lines (was {before})",
            note="restores the buffer" if recovered > 0.99 else f"only {recovered:.2f} recovered",
        )

    def workspace_symbols(self, masked, path):
        score = self.score("workspaceSymbol")
        for offset, name, kind in Extra.declarations(masked)[:1]:
            if not self.take("workspaceSymbol"):
                return
            hits = self.audit.result("workspace/symbol", {"query": name}, timeout=60.0) or []
            exact = [
                hit
                for hit in hits
                if hit.get("name") == name
                or hit.get("name", "").rsplit("\\", 1)[-1] == name
            ]
            score.record(
                f"{kind} {name}",
                bool(hits),
                bool(exact),
                f"{len(hits)} hits, {len(exact)} named exactly",
                note="found it" if exact else "found something else",
            )

    def hierarchy(self, path, lines, masked):
        score = self.score("typeHierarchy")
        for offset, name, kind in Extra.declarations(masked)[:1]:
            if not self.take("typeHierarchy"):
                return
            where = {"textDocument": {"uri": uri_of(path)}, "position": lines.position(offset)}
            items = self.audit.result("textDocument/prepareTypeHierarchy", where) or []
            if not items:
                score.record(f"{kind} {name}", False, False, "prepare returned nothing")
                continue
            up = self.audit.result("typeHierarchy/supertypes", {"item": items[0]}) or []
            down = (
                self.audit.result("typeHierarchy/subtypes", {"item": items[0]}, timeout=60.0) or []
            )
            # What the source itself says it extends and implements. A standalone final class has no
            # supertypes and no subtypes, and scoring that as a miss would score the sample.
            clause = re.search(
                rf"\b(?:class|interface|trait|enum)\s+{name}\b([^{{]*)\{{", masked
            )
            # Only names after `extends` / `implements`. An enum's backing type is written the same
            # way -- `enum Purpose: string` -- and counting it made a correct answer read as a miss.
            wanted = 0
            if clause:
                for part in re.finditer(
                    r"\b(?:extends|implements)\b([^{]*?)(?=\bextends\b|\bimplements\b|$)",
                    clause.group(1),
                ):
                    wanted += len(re.findall(r"[A-Za-z_][\w\\]*", part.group(1)))
            enough = len(up) >= wanted
            score.record(
                f"{kind} {name}",
                True,
                enough,
                f"{len(up)} supertypes {[i['name'] for i in up[:3]]} (source names {wanted}), "
                f"{len(down)} subtypes {[i['name'] for i in down[:3]]}",
                note=(
                    "nothing to find, correctly silent"
                    if wanted == 0 and not up
                    else f"{len(up)}/{wanted} supertypes"
                ),
            )


class Provocations:
    """Buffers edited to create the condition each code action exists for.

    Real code that is kept tidy offers no quickfixes, and reporting the empty menu as an absence
    would be measuring the codebase. So each of these breaks one thing on purpose, in a buffer, and
    asks whether the fix for exactly that thing is on the menu. The file on disk is never touched:
    the modified text goes over `didOpen` and the original goes back after.

    Each returns `(why, text, offset, wanted)` or None when the file has no place to break. The
    offset is on the *token* the fix attaches to, never merely on its line: an offer is scoped to a
    position, so a cursor at the start of the line is a fair test of nothing.
    """

    @staticmethod
    def missing_docblock(text, masked):
        found = re.search(
            r"/\*\*(?:[^*]|\*(?!/))*\*/\s*\n\s*((?:public|protected|private)\s+function\s+(\w+))",
            text,
        )
        if not found:
            return None
        cut = text[: found.start()] + found.group(1) + text[found.end() :]
        return (
            f"method {found.group(2)} with its docblock deleted",
            cut,
            found.start() + found.group(1).index(found.group(2)) + 1,
            "docblock",
        )

    @staticmethod
    def unused_import(text, masked):
        found = re.search(r"^namespace [^;]+;\n", text, re.MULTILINE)
        if not found:
            return None
        added = "\nuse Drupal\\Core\\Session\\AccountInterface;\n"
        grown = text[: found.end()] + added + text[found.end() :]
        return (
            "an import nothing uses",
            grown,
            found.end() + added.index("Drupal") + 3,
            "unused import",
        )

    @staticmethod
    def unimported_name(text, masked):
        """A class the project really holds, written where the file has not imported it."""
        found = re.search(r"\n(\s*)(?:public|protected|private)\s+function\s+\w+[^\n]*\{\n", text)
        if not found or "EntityTypeManagerInterface" in text:
            return None
        indent = found.group(1) + "  "
        added = f"{indent}$probe = new EntityTypeManagerInterface();\n"
        grown = text[: found.end()] + added + text[found.end() :]
        return (
            "a class the file has not imported",
            grown,
            found.end() + added.index("EntityTypeManagerInterface") + 3,
            "import",
        )

    @staticmethod
    def missing_param_tag(text, masked):
        found = re.search(
            r"^(\s*)\* @param [^\n]*\n(?:\s*\*\s{2,}[^\n]*\n)*", text, re.MULTILINE
        )
        if not found:
            return None
        cut = text[: found.start()] + text[found.end() :]
        # The declaration the docblock belongs to, which is where the fix is offered.
        after = re.search(r"function\s+(\w+)", cut[found.start() :])
        if not after:
            return None
        return (
            "a docblock missing a @param",
            cut,
            found.start() + after.start(1) + 1,
            "@param",
        )


class Provoked:
    """Runs each provocation over the sample and records whether the fix was offered."""

    def __init__(self, audit, checks):
        self.audit = audit
        self.checks = checks

    def run(self, path, text, masked, lines):
        uri = uri_of(path)
        score = self.checks.score("codeAction/provoked")
        score.wanted = "the fix for a break made on purpose is on the menu"
        for name in ("missing_docblock", "unused_import", "unimported_name", "missing_param_tag"):
            if score.asked >= self.checks.limit or self.checks.spent[name] >= 1:
                continue
            made = getattr(Provocations, name)(text, masked)
            if made is None:
                continue
            self.checks.spent[name] += 1
            why, grown, offset, wanted = made
            self.audit.diagnostics.pop(os.path.abspath(path), None)
            self.audit.send(path, grown)
            deadline = time.monotonic() + 3.0
            while (
                os.path.abspath(path) not in self.audit.diagnostics
                and time.monotonic() < deadline
            ):
                self.audit.server.poll(0.3)
            reported = self.audit.diagnostics.get(os.path.abspath(path)) or []

            # The token, and every place the server itself flagged. An editor's lightbulb appears
            # at the diagnostic, so a fix offered only there is a fix a person really gets --
            # asking only at the token would report the feature missing because the probe stood in
            # the wrong place, which is how the first pass of this tool was wrong about all four.
            here = C.Lines(grown).position(offset)
            spans = [("the token", {"start": here, "end": here})] + [
                (f"the {item.get('code')} diagnostic", item["range"]) for item in reported
            ]
            offered = {}
            for label, span in spans:
                actions = (
                    self.audit.result(
                        "textDocument/codeAction",
                        {
                            "textDocument": {"uri": uri},
                            "range": span,
                            "context": {"diagnostics": reported},
                        },
                    )
                    or []
                )
                offered[label] = [action.get("title", "?") for action in actions]
            self.audit.send(path, text)

            hit = [
                (label, title)
                for label, titles in offered.items()
                for title in titles
                if wanted.lower() in title.lower()
            ]
            score.record(
                f"{os.path.basename(path)}: {why}",
                any(offered.values()),
                bool(hit),
                f"wanted a title mentioning {wanted!r}; got {offered}",
                note=f"{name}: " + (f"offered at {hit[0][0]}" if hit else "not offered"),
            )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("root", nargs="?", default=".")
    parser.add_argument("--filter", help="only files whose path contains this")
    parser.add_argument("--files", type=int, default=24)
    parser.add_argument("--sites", type=int, default=8)
    parser.add_argument("--limit", type=int, default=20, help="positions per capability")
    parser.add_argument("--warmup", type=float, default=600.0)
    parser.add_argument("--json", dest="out")
    parser.add_argument("--verbose", action="store_true")
    args = parser.parse_args()
    sys.stdout.reconfigure(line_buffering=True)

    root = os.path.abspath(args.root)
    binary = os.path.join(REPO, "target/release/phrost")
    if not os.path.exists(binary):
        sys.exit(f"not built: {binary}")

    files = C.php_files(root, args.filter, args.files)
    plan = {}
    for path in files:
        with open(path, encoding="utf-8", errors="replace") as handle:
            text = handle.read()
        masked = C.mask(text)
        chosen = C.stride(C.sites_of(masked), args.sites)
        if chosen:
            plan[path] = (text, masked, chosen)

    print(f"capability-audit  {root}")
    print(f"files: {len(plan)}   limit: {args.limit} positions per capability")
    probe = C.probe_of({p: (t, s) for p, (t, _, s) in plan.items()})
    print(f"probe: {probe[1]}" if probe else "probe: none")

    audit = Audit(root, [binary, "serve"], args.verbose)
    caps = audit.start(probe, args.warmup)
    print("  advertised:", ", ".join(sorted(k for k, v in caps.items() if v)))

    checks = Checks(audit, plan, args.limit, root)
    provoked = Provoked(audit, checks)
    for done, (path, (text, masked, sites)) in enumerate(plan.items(), 1):
        audit.send(path, text)
        # Diagnostics are pushed, and a quickfix asked for before they land finds nothing to fix.
        deadline = time.monotonic() + 3.0
        while os.path.abspath(path) not in audit.diagnostics and time.monotonic() < deadline:
            audit.server.poll(0.3)
        checks.next_file()
        lines = C.Lines(text)
        checks.navigation(path, text, lines, sites)
        checks.implementation(path, text, lines, masked, sites)
        checks.references(path, text, lines, masked)
        checks.highlight(path, text, lines, masked, sites)
        checks.selection(path, text, lines, sites)
        checks.signature(path, text, lines, masked)
        checks.folding(path, text, lines)
        checks.tokens(path, text, lines)
        checks.hints(path, text, lines, masked)
        checks.actions(path, text, lines, masked)
        provoked.run(path, text, masked, lines)
        checks.formatting(path, text, lines)
        checks.workspace_symbols(masked, path)
        checks.hierarchy(path, lines, masked)
        if args.verbose or done % 5 == 0:
            print(f"    {done}/{len(plan)} files", flush=True)

    audit.stop()

    published = sum(len(items) for items in audit.diagnostics.values())
    with_any = sum(1 for items in audit.diagnostics.values() if items)
    print(
        f"\ndiagnostics: {published} across {with_any} of {len(audit.diagnostics)} files "
        "(context for the code-action rows: a clean file has nothing to fix)"
    )

    print("\n| capability | answered | useful | useful means |")
    print("| --- | ---: | ---: | --- |")
    for name in sorted(checks.scores):
        score = checks.scores[name]
        print(
            f"| {name} | {score.answered}/{score.asked} | {score.useful}/{score.asked} "
            f"| {score.wanted} |"
        )

    for name in sorted(checks.scores):
        score = checks.scores[name]
        print(f"\n== {name}  answered {score.answered}/{score.asked}, useful {score.useful}")
        if score.notes:
            print("   notes: " + ", ".join(f"{k} x{v}" for k, v in score.notes.most_common(8)))
        for where, detail in score.good[:2]:
            print(f"   good  {where}\n         {detail}")
        for where, detail in score.bad[:3]:
            print(f"   bad   {where}\n         {detail}")

    print("\nlatency median/p95 ms")
    for method, samples in sorted(audit.timings.items()):
        ordered = sorted(samples)
        p95 = ordered[min(len(ordered) - 1, int(len(ordered) * 0.95))]
        print(f"  {method:<44} {ordered[len(ordered) // 2]:7.2f} {p95:8.2f}  n={len(ordered)}")

    if args.out:
        with open(args.out, "w", encoding="utf-8") as handle:
            json.dump(
                {name: checks.scores[name].row() for name in sorted(checks.scores)},
                handle,
                indent=2,
            )
        print(f"\nwritten to {args.out}")


if __name__ == "__main__":
    main()
