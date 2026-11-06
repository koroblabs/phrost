#!/usr/bin/env python3
"""Asks two language servers the same questions and reports where we lose.

    python3 tools/compare-lsp.py <project-root> --reference intelephense

The point is not a score. The point is the ranked list at the end: every site where the reference
server answered and we did not, grouped by the shape of the expression. "1043 reference-only" is a
number; "27 sites of the form `$this->t`" is a task.

Two rules make the report worth believing.

Both servers are black boxes, reached only over stdio. Nothing here links our index, so an answer
counted as ours is an answer an editor would have got.

And the positions are chosen from the file text, by a scan neither server can see. Sampling from
our own index would only ask about sites we already know about, which is how a harness comes to
flatter the thing it measures.
"""
import argparse
import glob
import json
import os
import re
import subprocess
import sys
import time
from bisect import bisect_right
from collections import Counter, defaultdict

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from lspclient import (  # noqa: E402
    Closed,
    Server,
    Timeout,
    is_dependency_dir,
    path_of,
    uri_of,
)

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SIBLINGS = os.path.dirname(REPO)
CACHE = os.path.expanduser("~/.cache/phrost-compare")

# Servers this knows how to start by name. Anything else goes through `--reference-cmd`.
PRESETS = {
    "phrost": [os.path.join(REPO, "target/release/phrost"), "serve"],
    "intelephense": ["intelephense", "--stdio"],
    "phpantom": [os.path.join(SIBLINGS, "phpantom_lsp/target/release/phpantom_lsp"), "--stdio"],
    # The upstream tag, built aside from the sibling checkout, which is a fork pinned to 0.9.0.
    "phpantom-0.10": ["/tmp/phpantom-0.10-target/release/phpantom_lsp", "--stdio"],
    "phrost-old": [os.path.join(SIBLINGS, "phrost-old/target/release/phrost"), "serve"],
}

def intelephense_options():
    """intelephense's initialization options, licence key included when one is installed.

    Two things it needs. It refuses to cache anything without being told where, and re-indexing a
    large project on every run would make this tool something nobody runs twice.

    And the key, which is not a nicety: without it intelephense answers `renameProvider: false`,
    `foldingRangeProvider: false` and six more, and a measurement that reads those as absences
    reports a paid product as a smaller one than it is. The key lives in a file whose *name* carries
    it, so nothing here ever reads the contents or prints the value.
    """
    options = {"storagePath": os.path.join(CACHE, "intelephense"), "clearCache": False}
    installed = glob.glob(os.path.expanduser("~/intelephense/intelephense_licence_key_*"))
    if installed:
        options["globalStoragePath"] = os.path.expanduser("~/intelephense")
        options["licenceKey"] = os.path.basename(installed[0]).rsplit("_", 1)[-1]
    return options


INIT_OPTIONS = {
    "intelephense": intelephense_options(),
}

# Dependencies and build output, plus the caches static-analysis tools leave behind.
#
# The sampler is blind on purpose -- see the module doc -- and that is about not confining it to the
# project's *own* code: reaching into contrib and into a vendored PHPStan is the point, because an
# editor's user reaches there too. A tool's cache is not source. `.phpstan/cache` holds one
# `<?php return [...];` blob per analysed file, with no declaration in any of them, and on the
# measured checkout that is **11 272 of the 25 147 non-vendor PHP files** -- 45 % of the sample space,
# drawn first because the stride runs over a sorted list and a dot sorts early. Every number in
# `docs/comparison.md` was diluted by it, and the outline count was dominated: 0 symbols is the right
# answer for a file with no symbols, from either server.
SKIP_DIRS = {
    ".git",
    "node_modules",
    "vendor",
    "target",
    ".idea",
    "dist",
    "build",
    "__pycache__",
    ".phpstan",
    ".psalm",
    ".phpunit.cache",
    ".php-cs-fixer.cache",
    ".rector.cache",
    ".cache",
    "coverage",
}


CAPABILITIES = {
    "window": {"workDoneProgress": True},
    "workspace": {"workspaceFolders": True, "symbol": {"dynamicRegistration": False}},
    "textDocument": {
        "synchronization": {"didSave": True, "willSave": False},
        "definition": {"linkSupport": True},
        "hover": {"contentFormat": ["markdown", "plaintext"]},
        "completion": {
            "completionItem": {"snippetSupport": True, "documentationFormat": ["markdown"]},
            "contextSupport": True,
        },
        "publishDiagnostics": {"relatedInformation": True},
        "documentSymbol": {"hierarchicalDocumentSymbolSupport": True},
    },
}


# -- picking the questions ----------------------------------------------------------------------

# Words that look like a class reference to the scan below but name something else.
NOT_A_TYPE = {
    "True", "False", "Null", "TRUE", "FALSE", "NULL", "Array", "String", "Int", "Integer",
    "Bool", "Boolean", "Float", "Double", "Void", "Mixed", "Never", "Iterable", "Object",
    "Callable", "Static", "Self", "Parent", "This", "PHP_EOL", "PHP_INT_MAX", "Closure",
}
# Words PHP spells like a call and does not resolve like one: a language construct has no
# declaration to go to, so asking about one is asking the server to answer nothing.
NOT_A_CALL = {
    "if", "elseif", "else", "while", "for", "foreach", "switch", "match", "catch", "fn",
    "function", "return", "echo", "print", "exit", "die", "list", "array", "isset", "unset",
    "empty", "eval", "include", "include_once", "require", "require_once", "declare", "and",
    "or", "xor", "new", "clone", "yield", "throw", "use", "static", "self", "parent", "do",
    "endif", "endwhile", "endforeach", "endfor", "endswitch", "class", "int", "float", "string",
    "bool", "object", "mixed", "void", "never", "callable", "iterable", "null", "true", "false",
}
# A name right after one of these is being declared, not referenced. `use` is not here: an
# import and a trait use are both places goto should work.
DECLARES = re.compile(r"(?:class|interface|trait|enum|function|const|namespace)\s+$")


def mask(text):
    """Blanks comments and string bodies, keeping every offset and newline where it was.

    Sites are read off the masked text so a `->` inside a doc comment is not asked about. The scan
    is approximate -- an interpolated `{$a->b}` inside a string is lost with the rest of it -- but
    it is the same approximation for both servers, so it cannot tilt the comparison.
    """
    out = list(text)
    length = len(text)
    at = 0

    def blank(start, end):
        for index in range(start, min(end, length)):
            if out[index] != "\n":
                out[index] = " "

    while at < length:
        char = text[at]
        nxt = text[at + 1] if at + 1 < length else ""
        if char == "/" and nxt == "/":
            end = text.find("\n", at)
            end = length if end < 0 else end
            blank(at, end)
            at = end
        elif char == "#" and nxt != "[":  # `#[` opens an attribute, which is code.
            end = text.find("\n", at)
            end = length if end < 0 else end
            blank(at, end)
            at = end
        elif char == "/" and nxt == "*":
            end = text.find("*/", at + 2)
            end = length if end < 0 else end + 2
            blank(at, end)
            at = end
        elif char in "'\"":
            end = at + 1
            while end < length:
                if text[end] == "\\":
                    end += 2
                    continue
                if text[end] == char:
                    end += 1
                    break
                end += 1
            blank(at + 1, end - 1)
            at = end
        elif text.startswith("<<<", at):
            header = re.match(r"<<<[ \t]*['\"]?([A-Za-z_]\w*)['\"]?\r?\n", text[at:])
            if not header:
                at += 3
                continue
            body = at + header.end()
            close = re.search(r"^[ \t]*" + header.group(1) + r"\b", text[body:], re.MULTILINE)
            end = body + (close.start() if close else len(text) - body)
            blank(body, end)
            at = end
        else:
            at += 1
    return "".join(out)


class Site:
    """One question to ask, and enough text to say what it was about in the report."""

    __slots__ = ("kind", "name", "offset", "trigger", "shape")

    def __init__(self, kind, name, offset, trigger, shape):
        self.kind = kind
        self.name = name
        self.offset = offset  # inside the identifier: what definition and hover are asked about
        self.trigger = trigger  # after the operator: what completion is asked at
        self.shape = shape  # the source text, for grouping the gaps

    def __repr__(self):
        return f"{self.kind}:{self.shape}"


CHAIN = re.compile(
    r"((?:\$[A-Za-z_]\w*|[A-Za-z_][\w\\]*)"
    r"(?:\s*(?:->|\?->|::)\s*[A-Za-z_]\w*(?:\([^()]*\))?)*)\s*$"
)


def receiver_of(masked, before):
    """The expression a `->` or `::` hangs off, as written. Used to group gaps by shape."""
    window = masked[max(0, before - 90) : before]
    found = CHAIN.search(window)
    return found.group(1).strip() if found else "?"


def inside(offset, name):
    """A position unambiguously within an identifier, since servers differ on its first column."""
    return offset + (1 if len(name) > 1 else 0)


def sites_of(masked):
    """Every place worth asking about, in file order."""
    found = []

    for match in re.finditer(r"(->|\?->)(\s*)([A-Za-z_]\w*)", masked):
        name = match.group(3)
        found.append(
            Site(
                "member",
                name,
                inside(match.start(3), name),
                match.end(1),
                f"{receiver_of(masked, match.start(1))}{match.group(1)}{name}",
            )
        )

    for match in re.finditer(r"::(\s*)(\$?[A-Za-z_]\w*)", masked):
        name = match.group(2)
        if name == "class":  # `::class` is a keyword, not a member.
            continue
        found.append(
            Site(
                "static",
                name,
                inside(match.start(2), name),
                match.end(1),
                f"{receiver_of(masked, match.start())}::{name}",
            )
        )

    for match in re.finditer(r"\bnew\s+(\\?(?:[A-Za-z_]\w*\\)*[A-Za-z_]\w*)", masked):
        path = match.group(1)
        last = path.rsplit("\\", 1)[-1]
        start = match.start(1) + len(path) - len(last)
        found.append(Site("new", last, inside(start, last), None, f"new {path}"))

    for match in re.finditer(
        r"(?<![\w$\\>:])(\\?(?:[A-Za-z_]\w*\\)*[A-Z]\w*)(?![\w\\])", masked
    ):
        path = match.group(1)
        last = path.rsplit("\\", 1)[-1]
        if last in NOT_A_TYPE or last.isupper():
            continue
        if DECLARES.search(masked[max(0, match.start() - 12) : match.start()]):
            continue
        start = match.start(1) + len(path) - len(last)
        found.append(Site("name", last, inside(start, last), None, path))

    # A global constant, which the class-like scan excludes by its own `isupper()` rule -- so
    # PHP's third symbol table was as invisible to every tool built on this as its second was.
    # Not followed by `(`, which would make it a call, and not preceded by `::` or `->`, which
    # would make it a class constant the `static` and `member` kinds already take.
    for match in re.finditer(
        r"(?<![\w$\\>:])((?:[A-Za-z_]\w*\\)*[A-Z][A-Z0-9_]{2,})(?![\w\\])\s*(?!\()", masked
    ):
        path = match.group(1)
        last = path.rsplit("\\", 1)[-1]
        if last in NOT_A_TYPE:
            continue
        if DECLARES.search(masked[max(0, match.start() - 12) : match.start()]):
            continue
        start = match.start(1) + len(path) - len(last)
        found.append(Site("constant", last, inside(start, last), None, path))

    # A free function called by name, which nothing above matches: the class-like scan takes only
    # a capitalised name, so `strlen()`, `alpha()` and every lowercase helper a project writes were
    # invisible to every tool built on this. That blindness is how find-references and rename on a
    # free function stayed broken -- the round trip scored 100 % without ever asking about one.
    for match in re.finditer(r"(?<![\w$\\>:])([a-z_]\w*)\s*\(", masked):
        name = match.group(1)
        if name in NOT_A_CALL:
            continue
        if DECLARES.search(masked[max(0, match.start() - 12) : match.start()]):
            continue
        found.append(Site("function", name, inside(match.start(1), name), None, f"{name}()"))

    for match in re.finditer(r"\$([A-Za-z_]\w*)", masked):
        name = match.group(1)
        if name == "this":
            continue
        found.append(Site("var", name, inside(match.start(), name), None, f"${name}"))

    found.sort(key=lambda site: site.offset)
    return found


DECLARED_CLASS = re.compile(
    r"^(?:final\s+|abstract\s+|readonly\s+)*(?:class|interface|trait|enum)\s+([A-Za-z_]\w*)",
    re.MULTILINE,
)


def probe_of(plan):
    """A class this project declares, for asking a server whether its index is built yet.

    Taken from the last file of the sample rather than the first, so a server that indexes in path
    order does not pass the probe while most of the project is still missing.
    """
    for path, (text, _) in reversed(list(plan.items())):
        found = DECLARED_CLASS.search(mask(text))
        if found:
            return path, found.group(1)
    return None


def stride(items, limit):
    """Up to `limit` items, spread across the whole list rather than taken off the front."""
    if limit <= 0 or len(items) <= limit:
        return items
    step = len(items) / limit
    return [items[int(index * step)] for index in range(limit)]


class Lines:
    """Offset to LSP position, counting characters the way the protocol does."""

    def __init__(self, text):
        self.text = text
        self.starts = [0] + [index + 1 for index, ch in enumerate(text) if ch == "\n"]

    def position(self, offset):
        line = bisect_right(self.starts, offset) - 1
        prefix = self.text[self.starts[line] : offset]
        return {"line": line, "character": len(prefix.encode("utf-16-le")) // 2}


def php_files(root, needle, limit):
    paths = []
    for directory, subdirs, names in os.walk(root):
        # Not `SKIP_DIRS` alone: a package manager's directory is not always `vendor/`, and
        # sampling Drupal's core made three instruments measure the wrong project. See
        # `lspclient.is_dependency_dir`.
        subdirs[:] = sorted(
            name
            for name in subdirs
            if name not in SKIP_DIRS
            and not is_dependency_dir(os.path.join(directory, name), root)
        )
        for name in sorted(names):
            if name.endswith(".php"):
                paths.append(os.path.join(directory, name))
    if needle:
        paths = [path for path in paths if needle in path]
    return stride(sorted(paths), limit)


# -- asking --------------------------------------------------------------------------------------

STILL_INDEXING = -32801
READY_SIGNALS = ("indexingended", "indexing ended", "indexing complete", "indexed")


class Session:
    """One server, driven through every question, with its answers kept for comparison."""

    def __init__(self, name, argv, root, requests, verbose):
        self.name = name
        self.requests = requests
        self.verbose = verbose
        self.ready_signal = None
        # Progress tokens the server has opened and not closed. A server that opens one is saying
        # it is busy, and phrost, intelephense and phpantom all report indexing in several phases --
        # so the first `end` means one phase finished, not that the server can answer.
        self.working = set()
        self.diagnostics = {}
        self.timings = defaultdict(list)
        self.answers = {}
        os.makedirs(CACHE, exist_ok=True)
        self.server = Server(
            name,
            argv,
            root,
            log=os.path.join(CACHE, f"{name}.stderr.log"),
            init_options=INIT_OPTIONS.get(name),
        )
        self.server.on_notification(self._heard)

    def _heard(self, method, params):
        if method == "textDocument/publishDiagnostics":
            self.diagnostics[path_of(params.get("uri", ""))] = params.get("diagnostics") or []
            return
        if method == "$/progress":
            token = json.dumps(params.get("token"))
            kind = (params.get("value") or {}).get("kind")
            if kind == "begin":
                self.working.add(token)
            elif kind == "end":
                self.working.discard(token)
                self.ready_signal = self.ready_signal or "$/progress end"
            return
        text = f"{method} {json.dumps(params)}".lower()
        if any(signal in text for signal in READY_SIGNALS):
            self.ready_signal = self.ready_signal or f"{method}"

    def _ready(self):
        """Whether the server has said it finished indexing and has nothing still in flight."""
        return self.ready_signal is not None and not self.working

    def start(self, warmup, probe):
        started = time.monotonic()
        capabilities = self.server.initialize(CAPABILITIES).get("capabilities", {})
        how = self._settle(warmup, probe)
        # Timed at the answer, before the notifications still in flight are drained -- the drain is
        # this tool's housekeeping, and it added 0.3 s to every server's figure.
        self.bootstrap = time.monotonic() - started
        self.server.pump(0.3)
        self.rss_ready = self.server.rss_mb()
        print(
            f"  {self.name}: ready in {self.bootstrap:.1f}s, rss {self.rss_ready} MB ({how})",
            flush=True,
        )
        return capabilities

    def _settle(self, warmup, probe):
        """Waits until the server can answer about the project, not until it says it can.

        A server's own progress messages are not enough. They arrive in phases, so the first `end`
        means one phase finished -- asking then once made a perfectly good server look like it
        answered nothing at all, 1032 times running. And one of the phases can be a background pass
        that never ends inside a run, so waiting for all of them hangs instead.

        So the last word goes to a question whose answer is already known: a class this project
        declares, asked for until the index has it. A server that cannot find it is not ready,
        whatever it says about itself.
        """
        deadline = time.monotonic() + warmup
        missing = "nothing to probe with"
        while time.monotonic() < deadline:
            # A twentieth of a second between probes, not a second: the probe is one cheap request,
            # and a one-second step put every server that was ready at 0.4 s at 1.3 and one ready at
            # 1.05 s at 2.3 -- which read as phrost starting a second behind PHPantom on BookStack
            # when, warm, it answered first (0.3 s against 0.4-0.65). Without a probe there is
            # nothing to ask, so the quiet that counts as ready is still a whole second of it.
            self.server.poll(0.05 if probe is not None else 1.0)
            if probe is None:
                if self._ready():
                    return self.ready_signal or "quiet"
                continue
            path, name = probe
            answer = self.server.request("workspace/symbol", {"query": name}, timeout=60.0)
            for hit in answer.get("result") or []:
                found = (hit.get("location") or {}).get("uri", "")
                if os.path.basename(path_of(found)) == os.path.basename(path):
                    return f"index has {name}"
            missing = f"{name} not indexed yet"
        return f"NOT READY after {warmup:.0f}s ({missing}) -- treat these answers as suspect"

    def _ask(self, method, params):
        """One request, retried while the server says it is still indexing."""
        for attempt in range(40):
            started = time.monotonic()
            try:
                answer = self.server.request(method, params, timeout=30.0)
            except (Timeout, Closed) as failed:
                return {"error": {"message": str(failed)}}
            if (answer.get("error") or {}).get("code") == STILL_INDEXING:
                time.sleep(0.5)
                continue
            self.timings[method].append((time.monotonic() - started) * 1000)
            return answer
        return {"error": {"message": "still indexing"}}

    def visit(self, path, text, sites):
        self.server.open(path, text)
        lines = Lines(text)
        uri = uri_of(path)
        for index, site in enumerate(sites):
            where = {"textDocument": {"uri": uri}, "position": lines.position(site.offset)}
            key = (path, index)
            if "definition" in self.requests:
                self.answers[("definition", key)] = targets_of(
                    self._ask("textDocument/definition", where).get("result")
                )
            if "hover" in self.requests:
                self.answers[("hover", key)] = hover_of(
                    self._ask("textDocument/hover", where).get("result")
                )
            if "completion" in self.requests and site.trigger is not None:
                trigger = {
                    "textDocument": {"uri": uri},
                    "position": lines.position(site.trigger),
                    "context": {"triggerKind": 1},
                }
                self.answers[("completion", key)] = labels_of(
                    self._ask("textDocument/completion", trigger).get("result")
                )
        if "outline" in self.requests:
            self.answers[("outline", (path, 0))] = outline_kinds(
                self._ask("textDocument/documentSymbol", {"textDocument": {"uri": uri}}).get(
                    "result"
                )
            )

    def finish(self):
        # Diagnostics are pushed, not asked for, and a slow checker is still working after the last
        # request is answered. Files are left open on purpose so nothing clears them.
        self.server.pump(3.0)
        self.rss_open = self.server.rss_mb()
        self.server.stop()


# -- reading the answers ------------------------------------------------------------------------


def targets_of(result):
    """A definition answer as a set of (path, line), whichever of the three shapes it came in."""
    if not result:
        return frozenset()
    if isinstance(result, dict):
        result = [result]
    found = set()
    for item in result:
        uri = item.get("targetUri") or item.get("uri")
        span = item.get("targetSelectionRange") or item.get("targetRange") or item.get("range")
        if uri and span:
            # By file name, since two servers store the standard library in different places and
            # some serve it from a virtual URI with no file behind it at all. That makes a
            # disagreement about a stub unavoidable and uninteresting, so `differ` rows naming a
            # stub are noise rather than a finding.
            found.add((os.path.basename(path_of(uri)), span["start"]["line"]))
    return frozenset(found)


def hover_of(result):
    """The text of a hover card, flattened out of the three shapes the protocol allows."""
    if not result:
        return ""
    contents = result.get("contents")
    if isinstance(contents, dict):
        text = contents.get("value", "")
    elif isinstance(contents, list):
        text = "\n".join(
            part.get("value", "") if isinstance(part, dict) else str(part) for part in contents
        )
    else:
        text = contents or ""
    return text.strip()


def labels_of(result):
    """The set of names a completion list offers, with each server's garnish removed.

    Servers label the same method differently and the difference is presentation: `id` here, `id()`
    there, and `t($string, $args = ..., $options = ...)` somewhere else. Comparing the raw strings
    made **every** method in the other list look like one only that server knew about -- a probe of
    one real `$this->` site returned nine items from each server, the same nine, and the harness
    scored seven of them as ours-missing. So the signature comes off with the parentheses.

    PHP has no overloading, so a name is a member and nothing is collapsed that was distinct.
    """
    if not result:
        return frozenset()
    items = result.get("items", []) if isinstance(result, dict) else result
    return frozenset(
        item.get("label", "").strip().split("(")[0].strip().lstrip("$")
        for item in items
        if isinstance(item, dict)
    )


# `textDocument/documentSymbol`'s kinds, by the numbers the protocol assigns them. Only the ones a
# PHP file produces; anything else prints as its number, which is enough to notice.
SYMBOL_KINDS = {
    2: "module", 3: "namespace", 5: "class", 6: "method", 7: "property", 8: "field",
    9: "constructor", 10: "enum", 11: "interface", 12: "function", 13: "variable",
    14: "constant", 22: "enum member", 23: "struct",
}


def outline_kinds(result):
    """Every symbol the document offers, counted by kind.

    By kind and not as a total, and the reason is the same one that makes the completion comparison
    normalise or not at all. A bare total read `phrost 285  intelephense 834` on real source and
    invited exactly one conclusion -- that two thirds of the outline is missing. Asked of one file,
    the two answers are 189 and 469, and **274 of intelephense's are local variables inside method
    bodies**. That is a different answer to a different question rather than more of the same one:
    an outline panel holding every `$row` of a 3 000-line class is a symbol dump, and which of the
    two a server should give is a position, not a score.

    So the row names the kinds. A reader then sees at once whether a difference is about
    declarations -- which would be a gap -- or about what belongs in an outline, which is not.
    """
    counts = Counter()

    def walk(entries):
        for entry in entries or []:
            kind = entry.get("kind")
            counts[SYMBOL_KINDS.get(kind, str(kind))] += 1
            walk(entry.get("children") or [])

    walk(result or [])
    return counts


def message_shape(diagnostic):
    """A diagnostic's message with the names stripped out, so like complaints group together."""
    text = diagnostic.get("message", "").split("\n")[0]
    text = re.sub(r"[\"'`][^\"'`]*[\"'`]", "X", text)
    text = re.sub(r"\$\w+", "$X", text)
    text = re.sub(r"\b[A-Z][\w\\]*\b", "X", text)
    return re.sub(r"\s+", " ", text).strip()[:90]


# -- the report ----------------------------------------------------------------------------------

VERDICTS = ("agree", "differ", "ours-only", "reference-only", "neither")


def verdict(mine, theirs):
    if not mine and not theirs:
        return "neither"
    if not theirs:
        return "ours-only"
    if not mine:
        return "reference-only"
    if isinstance(mine, frozenset):
        return "agree" if mine & theirs else "differ"
    return "agree"


def table(title, rows, columns):
    print(f"\n{title}")
    width = max(len(str(name)) for name in rows) if rows else 8
    print("  " + " " * width + "".join(f"{column:>16}" for column in columns))
    for name, counts in rows.items():
        cells = "".join(f"{counts.get(column, 0):>16}" for column in columns)
        print(f"  {str(name):<{width}}{cells}")


def ranked(title, counter, limit, total_note=None):
    if not counter:
        return
    print(f"\n{title}" + (f"  ({total_note})" if total_note else ""))
    for shape, count in counter.most_common(limit):
        print(f"  {count:>6}  {shape}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("root", nargs="?", default=".", help="project root")
    parser.add_argument("--reference", default="intelephense", help="preset name of the other server")
    parser.add_argument("--subject", default="phrost", help="preset name of the server under test")
    parser.add_argument("--reference-cmd", help="command line for the reference, overriding the preset")
    parser.add_argument("--subject-cmd", help="command line for the subject, overriding the preset")
    parser.add_argument("--files", type=int, default=40, help="how many files to sample")
    parser.add_argument("--sites", type=int, default=20, help="how many positions per file")
    parser.add_argument("--filter", help="only files whose path contains this")
    parser.add_argument("--kinds", default="member,static,new,name,var")
    parser.add_argument("--requests", default="definition,hover,completion,outline")
    parser.add_argument("--warmup", type=float, default=600.0, help="seconds to wait for indexing")
    parser.add_argument("--top", type=int, default=25, help="rows in each ranked list")
    parser.add_argument("--json", dest="out", help="write the numbers here as a baseline")
    parser.add_argument("--verbose", action="store_true")
    parser.add_argument(
        "--show-label",
        help="print every site where only the reference offers this completion label -- what turns "
        "the ranked list into places somebody can open",
    )
    parser.add_argument(
        "--sites-only",
        action="store_true",
        help="print the questions and stop, without starting a server",
    )
    args = parser.parse_args()
    # A long run into a pipe is worth watching, and block-buffered output arrives only at exit.
    sys.stdout.reconfigure(line_buffering=True)

    root = os.path.abspath(args.root)
    if not os.path.isdir(root):
        sys.exit(f"no such project root: {root}")

    def argv_for(name, override):
        if override:
            return override.split()
        if name not in PRESETS:
            sys.exit(f"unknown server {name!r}; known: {', '.join(PRESETS)} (or pass a --*-cmd)")
        argv = PRESETS[name]
        if os.path.isabs(argv[0]) and not os.path.exists(argv[0]):
            sys.exit(f"{name} is not built: {argv[0]}")
        return argv

    subject_argv = argv_for(args.subject, args.subject_cmd)
    reference_argv = argv_for(args.reference, args.reference_cmd)
    kinds = set(args.kinds.split(","))
    requests = set(args.requests.split(","))

    files = php_files(root, args.filter, args.files)
    if not files:
        sys.exit("no PHP files matched")

    plan = {}
    for path in files:
        with open(path, encoding="utf-8", errors="replace") as handle:
            text = handle.read()
        chosen = stride([site for site in sites_of(mask(text)) if site.kind in kinds], args.sites)
        if chosen:
            plan[path] = (text, chosen)

    total = sum(len(sites) for _, sites in plan.values())

    # The credibility of the whole report rests on the positions being chosen without either
    # server's help, so it has to be possible to look at them.
    if args.sites_only:
        for path, (text, chosen) in plan.items():
            lines = Lines(text)
            print(f"\n{os.path.relpath(path, root)}")
            for site in chosen:
                where = lines.position(site.offset)
                print(f"  {where['line'] + 1:>5}:{where['character']:<4} {site.kind:<7} {site.shape}")
        print(f"\n{len(plan)} files, {total} sites")
        return

    print(f"compare-lsp  {args.subject}  vs  {args.reference}")
    print(f"project: {root}")
    print(f"files: {len(plan)}   sites: {total}   requests: {','.join(sorted(requests))}")

    probe = probe_of(plan)
    print(f"probe: {probe[1]} in {os.path.relpath(probe[0], root)}" if probe else "probe: none")

    sessions = {}
    for name, argv in ((args.subject, subject_argv), (args.reference, reference_argv)):
        # Sequentially, one whole pass each: two indexers at once contend for the machine, and the
        # slower one looks like it cannot answer.
        session = Session(name, argv, root, requests, args.verbose)
        session.start(args.warmup, probe)
        started = time.monotonic()
        for done, (path, (text, sites)) in enumerate(plan.items(), 1):
            session.visit(path, text, sites)
            if args.verbose or done % 10 == 0:
                print(f"    {name}: {done}/{len(plan)} files", flush=True)
        print(f"  {name}: asked in {time.monotonic() - started:.1f}s", flush=True)
        session.finish()
        sessions[name] = session

    mine, theirs = sessions[args.subject], sessions[args.reference]
    report(args, plan, mine, theirs, requests)


def report(args, plan, mine, theirs, requests):
    sites = {(path, index): site for path, (_, chosen) in plan.items() for index, site in enumerate(chosen)}

    baseline = {"subject": args.subject, "reference": args.reference, "sites": len(sites)}

    for request in ("definition", "hover"):
        if request not in requests:
            continue
        rows = defaultdict(Counter)
        gaps = Counter()
        # Where the first gap of each shape is, and what the other server answered there: a shape
        # groups the work, and a location and a target are what let somebody start on it.
        gap_first = {}
        wins = Counter()
        clashes = []
        for key, site in sites.items():
            call = verdict(mine.answers.get((request, key)), theirs.answers.get((request, key)))
            rows[site.kind][call] += 1
            rows["total"][call] += 1
            if call == "reference-only":
                gaps[site.shape] += 1
                gap_first.setdefault(site.shape, (key[0], site.offset, theirs.answers[(request, key)]))
            elif call == "ours-only":
                wins[site.shape] += 1
            elif call == "differ" and len(clashes) < 8:
                clashes.append((site, mine.answers[(request, key)], theirs.answers[(request, key)]))
        table(f"{request}", rows, VERDICTS)
        baseline[request] = {kind: dict(counts) for kind, counts in rows.items()}
        ranked(f"{request}: {args.reference} answers and we do not, by shape", gaps, args.top,
               f"{sum(gaps.values())} sites, {len(gaps)} shapes")
        if gaps:
            print(f"\n{request}: first of each, and what {args.reference} answered")
            for shape, _ in gaps.most_common(args.top):
                path, offset, answer = gap_first[shape]
                line = Lines(plan[path][0]).position(offset)["line"] + 1
                shown = sorted(answer)[:2] if isinstance(answer, (set, frozenset, list, tuple)) else answer
                print(f"  {shape}  at {os.path.relpath(path, args.root)}:{line}\n      {shown}")
        ranked(f"{request}: we answer and {args.reference} does not", wins, 10)
        if clashes:
            print(f"\n{request}: both answered, different target")
            for site, ours, other in clashes:
                print(f"  {site.shape}\n      ours: {sorted(ours)}\n      {args.reference}: {sorted(other)}")

    if "completion" in requests:
        compare_completion(args, sites, plan, mine, theirs, baseline)

    if "outline" in requests:
        ours = Counter()
        other = Counter()
        for (kind, _), value in mine.answers.items():
            if kind == "outline":
                ours.update(value)
        for (kind, _), value in theirs.answers.items():
            if kind == "outline":
                other.update(value)
        print(f"\noutline entries: {args.subject} {sum(ours.values())}   "
              f"{args.reference} {sum(other.values())}")
        print(f"  {'kind':<14}{args.subject:>12}{args.reference:>14}")
        for name in sorted(set(ours) | set(other), key=lambda k: -(ours[k] + other[k])):
            print(f"  {name:<14}{ours[name]:>12}{other[name]:>14}")
        baseline["outline"] = {
            "subject": dict(ours),
            "reference": dict(other),
        }

    compare_diagnostics(args, set(plan), mine, theirs, baseline)

    print("\nlatency, median / p95 ms  (indicative: one pass each, not a perf run)")
    for name, session in ((args.subject, mine), (args.reference, theirs)):
        parts = []
        for method, samples in sorted(session.timings.items()):
            ordered = sorted(samples)
            p95 = ordered[min(len(ordered) - 1, int(len(ordered) * 0.95))]
            parts.append(f"{method.rsplit('/', 1)[-1]} {ordered[len(ordered) // 2]:.1f}/{p95:.1f}")
        print(
            f"  {name:<14} bootstrap {session.bootstrap:5.1f}s"
            f"   rss {session.rss_ready} -> {session.rss_open} MB   " + "   ".join(parts)
        )
        baseline.setdefault("latency", {})[name] = {
            "bootstrap_s": round(session.bootstrap, 2),
            "rss_ready_mb": session.rss_ready,
            "rss_open_mb": session.rss_open,
            **{m: round(sorted(s)[len(s) // 2], 2) for m, s in session.timings.items()},
        }

    if args.out:
        with open(args.out, "w", encoding="utf-8") as handle:
            json.dump(baseline, handle, indent=2, sort_keys=True)
        print(f"\nbaseline written to {args.out}")


def compare_completion(args, sites, plan, mine, theirs, baseline):
    """Compares the item lists, which is where a missing magic member shows up as a shorter list."""
    missing = Counter()
    extra = Counter()
    asked = both = 0
    # Sites and items counted apart: one number for both reads as a big gap at one position when it
    # is often a small gap at twenty, and the two call for completely different work.
    shorter_sites = Counter()
    shorter_items = Counter()
    # Where the first site of each shape is. A shape is what groups the gap; a location is what
    # lets somebody open it -- and this list exists to be worked through one entry at a time.
    shorter_first = {}
    for key, site in sites.items():
        ours = mine.answers.get(("completion", key))
        other = theirs.answers.get(("completion", key))
        if ours is None or other is None:
            continue
        asked += 1
        if ours and other:
            both += 1
        for label in other - ours:
            missing[label] += 1
            if args.show_label and label == args.show_label:
                text = plan[key[0]][0]
                line = Lines(text).position(site.offset)["line"] + 1
                print(f"  {args.reference} alone offers {label!r} at "
                      f"{os.path.relpath(key[0], args.root)}:{line}  {site.shape}")
        for label in ours - other:
            extra[label] += 1
        if other and len(other) > len(ours):
            shorter_sites[site.shape] += 1
            shorter_items[site.shape] += len(other) - len(ours)
            shorter_first.setdefault(site.shape, (key[0], site.offset))
    print(f"\ncompletion: {asked} sites, both non-empty at {both}")
    baseline["completion"] = {"sites": asked, "both": both, "missing_labels": len(missing)}
    ranked(f"completion: items only {args.reference} offers, by label", missing, args.top,
           f"{len(missing)} distinct labels")
    ranked("completion: items only we offer, by label", extra, 12)
    if shorter_items:
        print("\ncompletion: where their list is longer   (sites x missing items each)")
        for shape, items in shorter_items.most_common(12):
            count = shorter_sites[shape]
            print(f"  {count:>4} x {items / count:>4.1f}  {shape}")
            path, offset = shorter_first[shape]
            text = plan[path][0]
            line = Lines(text).position(offset)["line"] + 1
            print(f"        first at {os.path.relpath(path, args.root)}:{line}")


def compare_diagnostics(args, opened, mine, theirs, baseline):
    """Compares what each server said about the files this run opened.

    Only those files. A server may check the whole project and push a diagnostic for a file nobody
    asked about, and one that checks open buffers only is doing a different amount of work rather
    than finding fewer problems -- so counting every notification made a 59-against-10 045 row that
    reads as a capability gap and is a scope difference. Both are worth knowing, so the wider total
    is printed beside the comparable one instead of standing in for it.
    """

    def by_shape(diagnostics, paths):
        found = Counter()
        for path in paths:
            for item in diagnostics.get(path) or []:
                found[message_shape(item)] += 1
        return found

    ours = by_shape(mine.diagnostics, opened)
    other = by_shape(theirs.diagnostics, opened)
    print(
        f"\ndiagnostics on the {len(opened)} opened files: "
        f"{args.subject} {sum(ours.values())}   {args.reference} {sum(other.values())}"
    )
    for name, session in ((args.subject, mine), (args.reference, theirs)):
        elsewhere = set(session.diagnostics) - opened
        if elsewhere:
            total = sum(len(session.diagnostics[path] or []) for path in elsewhere)
            print(f"  {name} also pushed {total} for {len(elsewhere)} files nobody opened")
    baseline["diagnostics"] = {"subject": sum(ours.values()), "reference": sum(other.values())}
    ranked("diagnostics: ours, by message shape", ours, 15)
    ranked(f"diagnostics: {args.reference}'s, by message shape", other, 15)


if __name__ == "__main__":
    main()
