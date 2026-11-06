#!/usr/bin/env python3
"""Whether the positional answers a client draws with are well formed.

    python3 tools/positions-are-well-formed.py <project-root> [--files 40]

Four requests answer with *positions* rather than with facts, and a client draws them without
checking: semantic tokens colour the text, folding ranges collapse it, selection ranges are what
"expand selection" walks, and inlay hints are written between the characters. A range outside the
file, a chain that does not nest, a token that overlaps its neighbour -- none of those is caught by
anything the server itself asks, and each shows up as an editor behaving strangely rather than as a
wrong answer anybody can point at.

So the questions here are about shape, and none needs an expectation written down:

* **semantic tokens** -- inside the file, in order, and never overlapping. A client rebuilds absolute
  positions by adding up deltas, so one bad row moves everything after it.
* **folding ranges** -- inside the file, start before end, and properly nested with each other.
* **selection ranges** -- each parent strictly contains its child, and the innermost contains the
  cursor that was asked about.
* **inlay hints** -- inside the file, and not repeating the text already written at that position,
  which is what a parameter-name hint over `id: 1` would do.
"""
import argparse
import importlib.util
import json
import os
import sys
import time
from collections import Counter

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
from lspclient import Closed, Server, Timeout, uri_of  # noqa: E402


def _sibling(name):
    spec = importlib.util.spec_from_file_location(
        name.replace("-", "_"), os.path.join(HERE, f"{name}.py")
    )
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


compare = _sibling("compare-lsp")

REPO = os.path.dirname(HERE)
STILL_INDEXING = -32801

TOKEN_TYPES = [
    "namespace", "type", "class", "enum", "interface", "struct", "typeParameter", "parameter",
    "variable", "property", "enumMember", "event", "function", "method", "macro", "keyword",
    "modifier", "comment", "string", "number", "regexp", "operator", "decorator",
]
CAPABILITIES = {
    "window": {"workDoneProgress": True},
    "workspace": {"workspaceFolders": True},
    "textDocument": {
        "synchronization": {"didSave": True},
        "definition": {"linkSupport": False},
        "foldingRange": {"lineFoldingOnly": False},
        "selectionRange": {},
        "inlayHint": {},
        "semanticTokens": {
            "requests": {"full": True, "range": True},
            "tokenTypes": TOKEN_TYPES,
            "tokenModifiers": ["declaration", "definition", "readonly", "static", "deprecated",
                               "abstract", "async", "modification", "documentation",
                               "defaultLibrary"],
            "formats": ["relative"],
        },
    },
}


class Probe:
    def __init__(self, root):
        self.root = os.path.abspath(root)
        self.ready = None
        self.working = set()
        self.opened = {}
        os.makedirs(os.path.join(REPO, ".phrost-cache"), exist_ok=True)
        self.server = Server(
            "phrost",
            [os.path.join(REPO, "target/release/phrost"), "serve"],
            self.root,
            log=os.path.join(REPO, ".phrost-cache", "positions-probe.stderr.log"),
        )
        self.server.on_notification(self._heard)

    def _heard(self, method, params):
        if method == "$/progress":
            token = json.dumps(params.get("token"))
            kind = (params.get("value") or {}).get("kind")
            if kind == "begin":
                self.working.add(token)
            elif kind == "end":
                self.working.discard(token)
                self.ready = self.ready or "progress"

    def start(self, probe_path, probe_name):
        self.server.initialize(CAPABILITIES)
        text = read(probe_path)
        lines = compare.Lines(text)
        at = compare.mask(text).find(probe_name)
        self.open(probe_path)
        deadline = time.monotonic() + 600
        while time.monotonic() < deadline:
            self.server.pump(0.5)
            if self.ready is None or self.working:
                continue
            if self.ask("textDocument/definition", {
                "textDocument": {"uri": uri_of(probe_path)},
                "position": lines.position(at),
            }):
                return True
        return False

    def open(self, path):
        if path not in self.opened:
            self.opened[path] = read(path)
            self.server.open(path, self.opened[path])
        return self.opened[path]

    def ask(self, method, params):
        for _ in range(40):
            try:
                answer = self.server.request(method, params, timeout=30.0)
            except (Timeout, Closed):
                return None
            if answer.get("error", {}).get("code") == STILL_INDEXING:
                self.server.pump(0.5)
                continue
            return answer.get("result")
        return None


def read(path):
    with open(path, encoding="utf-8", errors="replace") as handle:
        return handle.read()


class Lines:
    """The file's lines, in the units the protocol counts."""

    def __init__(self, body):
        self.rows = body.split("\n")

    def holds(self, position):
        line = position.get("line")
        if not isinstance(line, int) or line < 0 or line >= len(self.rows):
            return False
        width = len(self.rows[line].encode("utf-16-le")) // 2
        return 0 <= position.get("character", -1) <= width

    def key(self, position):
        return (position["line"], position["character"])

    def text_at(self, line, character, length):
        if line >= len(self.rows):
            return ""
        row = self.rows[line].encode("utf-16-le")
        return row[character * 2 : (character + length) * 2].decode("utf-16-le", "replace")


def check_tokens(lines, answer, say):
    """Absolute rows out of the delta encoding, each checked against the file."""
    data = (answer or {}).get("data") or []
    if len(data) % 5:
        say("a token row that is not five numbers")
        return
    line = character = 0
    previous = None
    for index in range(0, len(data), 5):
        delta_line, delta_start, length, kind, _ = data[index : index + 5]
        line += delta_line
        character = delta_start if delta_line else character + delta_start
        if not lines.holds({"line": line, "character": character}):
            say("a token outside the file")
            return
        if length <= 0:
            say("a token of no width")
            return
        if kind >= len(TOKEN_TYPES):
            say("a token of a kind the client did not declare")
            return
        if previous is not None and (line, character) < previous:
            say("a token before the one in front of it")
            return
        end = character + length
        if previous is not None and previous[0] == line and previous[1] > character:
            say("two tokens over the same characters")
            return
        previous = (line, end)


def check_folding(lines, answer, say):
    ranges = []
    for one in answer or []:
        start, end = one.get("startLine"), one.get("endLine")
        if not lines.holds({"line": start, "character": 0}) or not lines.holds(
            {"line": end, "character": 0}
        ):
            say("a folding range outside the file")
            return
        if end < start:
            say("a folding range that ends before it starts")
            return
        ranges.append((start, end))
    # Widest first at each start, which is the order a well-nested set closes in. Two regions may
    # share a line freely -- `): T {` ends a parameter list and opens a body, and `assertSame([`
    # opens a call and an array at once -- so the only shape worth reporting is one that begins
    # inside another and ends outside it, which is what a stack finds and a pairwise test does not.
    ranges.sort(key=lambda span: (span[0], -span[1]))
    open_ends = []
    for start, end in ranges:
        # `<=`, not `<`: a region ending on the line the next one starts has closed as far as
        # nesting goes. `endLine` is inclusive and PHP puts two boundaries on one line constantly
        # -- `): T {`, `], $defaults->all()`, `}) ->` -- so `<` reports every one of them. Written
        # three ways before this one, each looking like a finding on real code.
        while open_ends and open_ends[-1] <= start:
            open_ends.pop()
        if open_ends and open_ends[-1] < end:
            say(f"two folding ranges that overlap without nesting ({open_ends[-1]} and {start}-{end})")
            return
        open_ends.append(end)


def check_selection(lines, answer, cursor, say):
    for one in answer or []:
        seen = []
        node = one
        while node:
            span = node.get("range") or {}
            if not lines.holds(span.get("start", {})) or not lines.holds(span.get("end", {})):
                say("a selection range outside the file")
                return
            if lines.key(span["start"]) > lines.key(span["end"]):
                say("a selection range that ends before it starts")
                return
            seen.append((lines.key(span["start"]), lines.key(span["end"])))
            node = node.get("parent")
        if seen and not (seen[0][0] <= lines.key(cursor) <= seen[0][1]):
            say("an innermost selection range that does not hold the cursor")
            return
        for (child_start, child_end), (parent_start, parent_end) in zip(seen, seen[1:]):
            if parent_start > child_start or parent_end < child_end:
                say("a selection range not held by its parent")
                return


def check_hints(lines, answer, say):
    for one in answer or []:
        position = one.get("position") or {}
        if not lines.holds(position):
            say("an inlay hint outside the file")
            return
        label = one.get("label")
        text = label if isinstance(label, str) else "".join(
            part.get("value", "") for part in label or []
        )
        # A parameter-name hint is `name:`, and what would repeat it is the *named argument*
        # spelling `name: value` written at the same place. Comparing the bare word instead
        # matched `array` against `array_values(`, which shares five characters and nothing else.
        name = text.strip().rstrip(":").strip()
        if not name:
            continue
        ahead = lines.text_at(position["line"], position["character"], len(name) + 2)
        if ahead.startswith(name) and ahead[len(name) :].lstrip().startswith(":"):
            say("an inlay hint repeating the text it sits on")
            return


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("root")
    parser.add_argument("--files", type=int, default=40)
    parser.add_argument("--needle", default="")
    args = parser.parse_args()

    paths = compare.php_files(args.root, args.needle, args.files)
    plan = {path: read(path) for path in paths}
    probe = compare.probe_of({path: (text, None) for path, text in plan.items()})
    if probe is None:
        print("no class declared in the sample to probe readiness with")
        return 1

    session = Probe(args.root)
    print(f"starting server over {args.root}", flush=True)
    if not session.start(*probe):
        print("server never answered about the project")
        return 1

    asked = Counter()
    wrong = Counter()
    examples = {}
    for path, body in plan.items():
        session.open(path)
        lines = Lines(body)
        document = {"textDocument": {"uri": uri_of(path)}}

        def say(kind, what, path=path):
            wrong[f"{kind}: {what}"] += 1
            examples.setdefault(f"{kind}: {what}", os.path.relpath(path, args.root))

        asked["semantic tokens"] += 1
        check_tokens(lines, session.ask("textDocument/semanticTokens/full", document),
                     lambda what: say("semantic tokens", what))
        asked["folding ranges"] += 1
        check_folding(lines, session.ask("textDocument/foldingRange", document),
                      lambda what: say("folding ranges", what))
        asked["inlay hints"] += 1
        whole = {"start": {"line": 0, "character": 0},
                 "end": {"line": max(0, len(lines.rows) - 1), "character": 0}}
        check_hints(lines, session.ask("textDocument/inlayHint", {**document, "range": whole}),
                    lambda what: say("inlay hints", what))

        # A handful of cursors per file, which is what "expand selection" is asked at.
        cursors = [compare.Lines(body).position(site.offset)
                   for site in compare.sites_of(compare.mask(body))]
        for cursor in compare.stride(cursors, 12):
            asked["selection ranges"] += 1
            check_selection(lines, session.ask(
                "textDocument/selectionRange", {**document, "positions": [cursor]}
            ), cursor, lambda what: say("selection ranges", what))

    session.server.stop()
    print()
    for kind, count in sorted(asked.items()):
        bad = sum(value for key, value in wrong.items() if key.startswith(f"{kind}: "))
        print(f"{kind:18} {count - bad} of {count} well formed")
    if wrong:
        print()
        for what, count in wrong.most_common():
            print(f"  {count:5}  {what}")
            print(f"         first in {examples[what]}")
    return 1 if wrong else 0


if __name__ == "__main__":
    sys.exit(main())
