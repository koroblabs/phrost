#!/usr/bin/env python3
"""Expanding the selection keeps the code whole, and keeps growing.

    python3 tools/a-selection-grows-without-breaking-the-code.py <project-root> [--files 25]

`textDocument/selectionRange` answers with a chain: the smallest thing round the cursor, then what
holds that, and so on out. Nobody writes the expected chain for a file -- but the chain has to obey
four rules whatever the file is, and each of them is checkable against the text alone:

  * every range in it holds the position that was asked about;
  * each parent holds its child and is strictly bigger, or the chain has stopped growing and should
    have ended;
  * a range is well-formed and inside the file;
  * the text a range covers is **balanced** -- a selection that opens a bracket it does not close is
    a selection a person cannot act on, and it is what an off-by-one in a node's range looks like
    from the outside.

Brackets are counted over the masked text, so one inside a string or a comment is not a bracket --
and only over a range that lies inside one PHP region. A file's inline HTML is one node to a PHP
parser, so the innermost selection in a Blade template is the whole template, and every `<` in it
would be read here as an unclosed bracket. Fifteen findings on one such file, all of them the
probe's.
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
# Deep enough for any real nesting, and finite because a cyclic chain is what this is looking for.
DEEPEST = 64
PAIRS = {")": "(", "]": "[", "}": "{"}

CAPABILITIES = {
    "window": {"workDoneProgress": True},
    "workspace": {"workspaceFolders": True},
    "textDocument": {"selectionRange": {}},
}


def read(path):
    with open(path, encoding="utf-8", errors="replace") as handle:
        return handle.read()


def php_regions(text):
    """The spans between `<?php`/`<?=` and the `?>` that closes each, or the end of the file."""
    out = []
    at = 0
    while True:
        opened = text.find("<?", at)
        if opened < 0:
            break
        closed = text.find("?>", opened + 2)
        end = len(text) if closed < 0 else closed
        out.append((opened, end))
        at = len(text) if closed < 0 else closed + 2
    return out


def balanced(masked):
    """Whether a masked slice opens and closes every bracket inside it."""
    stack = []
    for char in masked:
        if char in "([{":
            stack.append(char)
        elif char in PAIRS:
            if not stack or stack.pop() != PAIRS[char]:
                return False
    return not stack


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
            log=os.path.join(REPO, ".phrost-cache", "selection-range.stderr.log"),
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

    def start(self, anchor):
        self.server.initialize(CAPABILITIES)
        text = read(anchor)
        self.server.open(anchor, text)
        deadline = time.monotonic() + 900
        while time.monotonic() < deadline:
            self.server.pump(0.5)
            if self.ready is None or self.working:
                continue
            if self.ask("textDocument/selectionRange", {
                "textDocument": {"uri": uri_of(anchor)},
                "positions": [{"line": 0, "character": 0}],
            }) is not None:
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


def chain_of(answer):
    """The chain as a flat list, outermost last, stopping at `DEEPEST`."""
    out = []
    node = answer
    while isinstance(node, dict) and "range" in node and len(out) < DEEPEST:
        out.append(node["range"])
        node = node.get("parent")
    return out, isinstance(node, dict)


def offset_of(lines, position):
    """An LSP position back to an index into the text, or `None` past the end of its line.

    The protocol counts UTF-16 code units and python counts code points, so the walk is over the
    line's characters rather than a slice -- a name with an emoji in a docblock is not a thing to
    be off by two on.
    """
    line = position["line"]
    if not 0 <= line < len(lines.starts):
        return None
    start = lines.starts[line]
    end = lines.starts[line + 1] if line + 1 < len(lines.starts) else len(lines.text)
    units = position["character"]
    at = start
    while units > 0 and at < end:
        units -= len(lines.text[at].encode("utf-16-le")) // 2
        at += 1
    return at if units <= 0 else None


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("root")
    parser.add_argument("--files", type=int, default=25)
    parser.add_argument("--sites", type=int, default=20)
    args = parser.parse_args()

    paths = compare.php_files(args.root, "", args.files)
    if not paths:
        print("no php files in the sample")
        return 1

    probe = Probe(args.root)
    if not probe.start(paths[-1]):
        print("the server never answered about the project")
        return 1

    asked = 0
    depths = []
    findings = []
    for path in paths:
        text = read(path)
        masked = compare.mask(text)
        regions = php_regions(text)
        lines = compare.Lines(text)
        probe.server.open(path, text)
        sites = compare.stride(compare.sites_of(masked), args.sites)
        positions = [lines.position(site.offset) for site in sites]
        if not positions:
            probe.server.close(path)
            continue

        answer = probe.ask("textDocument/selectionRange", {
            "textDocument": {"uri": uri_of(path)}, "positions": positions
        })
        if not isinstance(answer, list) or len(answer) != len(positions):
            findings.append((os.path.relpath(path, args.root), "?",
                             f"asked about {len(positions)} positions, got "
                             f"{len(answer) if isinstance(answer, list) else answer!r}"))
            probe.server.close(path)
            continue

        for position, one in zip(positions, answer):
            asked += 1
            where = f"{os.path.relpath(path, args.root)}:{position['line'] + 1}"
            chain, truncated = chain_of(one)
            if truncated:
                findings.append((where, "", f"chain is deeper than {DEEPEST}"))
                continue
            if not chain:
                findings.append((where, "", "no range at all"))
                continue

            depths.append(len(chain))
            at = offset_of(lines, position)
            previous = None
            for depth, span in enumerate(chain):
                start = offset_of(lines, span["start"])
                end = offset_of(lines, span["end"])
                if start is None or end is None or start > end or end > len(text):
                    findings.append((where, f"[{depth}]", f"range is not well formed: {span}"))
                    break
                if not start <= at <= end:
                    findings.append((where, f"[{depth}]",
                                     f"does not hold the position asked about: {span}"))
                    break
                if previous is not None and not (start <= previous[0] and previous[1] <= end):
                    findings.append((where, f"[{depth}]", "does not hold its child"))
                    break
                if previous == (start, end):
                    findings.append((where, f"[{depth}]", "repeats its child"))
                    break
                inside = any(one <= start and end <= other for one, other in regions)
                if inside and not balanced(masked[start:end]):
                    findings.append((where, f"[{depth}]",
                                     f"unbalanced: {text[start:end][:70]!r}"))
                    break
                previous = (start, end)
        probe.server.close(path)
    probe.server.stop()

    print()
    # Printed because every rule here passes vacuously on a one-link chain: a probe that only ever
    # sees the innermost range is checking nothing about growing, and would say so by reporting 1.
    depth = (min(depths), sum(depths) / len(depths), max(depths)) if depths else (0, 0, 0)
    print(f"a selection grows without breaking the code  {asked - len(findings)} of {asked}"
          f"  (chain depth {depth[0]}-{depth[2]}, mean {depth[1]:.1f})")
    for where, depth, why in findings[:15]:
        print(f"  {where} {depth}  {why}")
    return 1 if findings else 0


if __name__ == "__main__":
    sys.exit(main())
