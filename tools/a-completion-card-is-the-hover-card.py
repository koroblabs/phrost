#!/usr/bin/env python3
"""The card a completion item resolves to is the card hovering that member shows.

    python3 tools/a-completion-card-is-the-hover-card.py <project-root> [--files 25]

Two descriptions of one declaration, reached by different code: `describe_completion` renders what
a client shows beside the highlighted item, and `hover` renders what it shows over the same member
written out. CLAUDE.md §8.1 calls that shape *two constructors of one rule*, and both halves are
here for the same reason -- a list is built per keystroke and a card is built for one item -- so
they will keep drifting unless something compares them.

Asked at every `->` and `::` whose member a completion list offers: resolve that item, hover the
member itself, and require the declaration line, the prose and the tags to match. Hover wraps its
declaration in the class that holds it and a completion card does not, so the comparison is of the
member line rather than of the whole fence.
"""
import argparse
import importlib.util
import json
import os
import re
import sys
import time

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

CAPABILITIES = {
    "window": {"workDoneProgress": True},
    "workspace": {"workspaceFolders": True},
    "textDocument": {
        "hover": {"contentFormat": ["markdown"]},
        "completion": {
            "completionItem": {
                "documentationFormat": ["markdown"],
                "resolveSupport": {"properties": ["documentation", "detail"]},
            }
        },
    },
}

FENCE = re.compile(r"```(?:php)?\n(.*?)```", re.DOTALL)
# A member line inside a rendered class body, which is the one line both cards state.
DECLARED = re.compile(
    r"^\s*(?:(?:final|abstract|readonly|public|protected|private|static)\s+)*"
    r"(?:function|const|case|\??[\w\\|]+\s+\$)"
)


def read(path):
    with open(path, encoding="utf-8", errors="replace") as handle:
        return handle.read()


def markdown_of(result):
    if not result:
        return ""
    contents = result.get("contents") if isinstance(result, dict) else None
    if isinstance(contents, dict):
        return contents.get("value", "")
    if isinstance(result, dict) and isinstance(result.get("documentation"), dict):
        return result["documentation"].get("value", "")
    return ""


# The member a rendered declaration line is about, which is what both cards have to agree on.
# With its kind, because a class may declare `$hasErrors` and `hasErrors()` at once and the list
# labels both with the bare name -- matching on the name alone compares a property against a method.
NAMED = re.compile(r"\bfunction\s+(\w+)|\bconst\s+(?:[\w\\|?<>, ]+\s+)?(\w+)|\bcase\s+(\w+)|\$(\w+)")
# CompletionItemKind, in the order the groups above are written.
KINDS = (2, 21, 20, 10)


def named(declaration):
    found = NAMED.search(declaration)
    if not found:
        return None, None
    for kind, one in zip(KINDS, found.groups()):
        if one:
            return one, kind
    return None, None


def card(text):
    """A card as (declaration line, everything under it), both normalised for whitespace."""
    fence = FENCE.search(text)
    if not fence:
        return None
    body = [line for line in fence.group(1).split("\n") if line.strip()]
    declaration = next((line.strip() for line in body if DECLARED.match(line)), None)
    if declaration is None:
        return None
    rest = text[fence.end():].strip()
    # What the access gives back *where it is written* is a hover's alone: a completion item names a
    # member nobody has called yet, so there is no "here" for its card to describe. The line is
    # about the site, not the declaration, and is left out of the comparison.
    rest = re.sub(r"^_?(?:returns|holds) .+ here_?\s*$", "", rest, flags=re.MULTILINE).strip()
    # The tag fence renders identically in both, so it needs no separate reading -- only the
    # blank-line differences a markdown assembler introduces.
    return declaration, re.sub(r"\n{2,}", "\n", rest)


class Probe:
    def __init__(self, root):
        self.root = os.path.abspath(root)
        self.ready = None
        self.working = set()
        os.makedirs(os.path.join(REPO, ".phrost-cache"), exist_ok=True)
        self.server = Server(
            "phrost",
            [os.path.join(REPO, "target/release/phrost"), "serve"],
            self.root,
            log=os.path.join(REPO, ".phrost-cache", "completion-card.stderr.log"),
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

    def start(self, anchor, anchor_name):
        self.server.initialize(CAPABILITIES)
        text = read(anchor)
        at = compare.mask(text).find(anchor_name)
        self.server.open(anchor, text)
        deadline = time.monotonic() + 900
        while time.monotonic() < deadline:
            self.server.pump(0.5)
            if self.ready is None or self.working:
                continue
            if self.ask("textDocument/definition", {
                "textDocument": {"uri": uri_of(anchor)},
                "position": compare.Lines(text).position(at),
            }):
                return True
        return False

    def ask(self, method, params, timeout=30.0):
        for _ in range(60):
            try:
                answer = self.server.request(method, params, timeout=timeout)
            except (Timeout, Closed):
                return None
            if answer.get("error", {}).get("code") == STILL_INDEXING:
                self.server.pump(0.5)
                continue
            return answer.get("result")
        return None


def offered(result):
    if isinstance(result, dict):
        return result.get("items") or []
    return result or []


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("root")
    parser.add_argument("--files", type=int, default=25)
    parser.add_argument("--sites", type=int, default=12)
    args = parser.parse_args()

    paths = compare.php_files(args.root, "", args.files)
    anchor = None
    for path in reversed(paths):
        found = compare.DECLARED_CLASS.search(compare.mask(read(path)))
        if found:
            anchor = (path, found.group(1))
            break
    if anchor is None:
        print("no file in the sample declares a class")
        return 1

    probe = Probe(args.root)
    if not probe.start(*anchor):
        print("the server never answered about the project")
        return 1

    compared = 0
    differ = []
    for path in paths:
        text = read(path)
        lines = compare.Lines(text)
        probe.server.open(path, text)
        chosen = compare.stride(
            [s for s in compare.sites_of(compare.mask(text)) if s.kind in ("member", "static")],
            args.sites,
        )
        for site in chosen:
            position = lines.position(site.offset)
            hovered = card(markdown_of(probe.ask("textDocument/hover", {
                "textDocument": {"uri": uri_of(path)}, "position": position
            })))
            if hovered is None:
                continue
            # Taken from the card hover just rendered rather than from the text. A window round the
            # site holds whatever else the line calls, and matching the wrong one of them compares
            # two members and reports every site -- which is what it did, and what the finding
            # looked like.
            wanted, kind = named(hovered[0])
            if wanted is None:
                continue
            items = offered(probe.ask("textDocument/completion", {
                "textDocument": {"uri": uri_of(path)}, "position": position
            }))
            item = next(
                (one for one in items
                 if one.get("label", "").lstrip("$").rstrip("()") == wanted
                 and one.get("kind") == kind
                 and one.get("data")),
                None,
            )
            if item is None:
                continue
            resolved = card(markdown_of(probe.ask("completionItem/resolve", item)))
            if resolved is None:
                continue
            compared += 1
            where = f"{os.path.relpath(path, args.root)}:{position['line'] + 1}"
            if resolved != hovered:
                differ.append((where, wanted, hovered, resolved))
        probe.server.close(path)
    probe.server.stop()

    print()
    print(f"a completion card is the hover card  {compared - len(differ)} of {compared}")
    for where, name, hovered, resolved in differ[:12]:
        print(f"  {where}  {name}")
        if hovered[0] != resolved[0]:
            print(f"     hover    {hovered[0]}")
            print(f"     resolve  {resolved[0]}")
        else:
            print(f"     same declaration, different text under it")
            print(f"     hover    {hovered[1][:160]!r}")
            print(f"     resolve  {resolved[1][:160]!r}")
    return 1 if differ else 0


if __name__ == "__main__":
    sys.exit(main())
