#!/usr/bin/env python3
"""The type drawn beside a variable is the type hovering it reports.

    python3 tools/a-hint-and-a-hover-agree.py <project-root> [--files 60]

An inlay hint and a hover are two renderings of one fact -- what a variable was given -- reached
through two different projections (`assigned_types` and `variable_at`). CLAUDE.md §8.1 calls that
shape *two constructors of one rule*, and its tell is a divergence: one half answers and the other
does not, or they answer differently, along a boundary nobody drew on purpose. Neither half can be
checked against a written expectation here, because PHP never writes the type down -- but they can
be checked against each other, and a disagreement means one of them is wrong.

A hint too wide to draw is folded, and the whole of what it left out is in its tooltip -- so the
tooltip is what is compared there, which aims the check at exactly the types that grow: the shapes
and the long unions.
"""
import argparse
import json
import os
import re
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
from lspclient import Closed, Server, Timeout, uri_of  # noqa: E402
import importlib.util  # noqa: E402


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
TYPE_HINT = 1  # InlayHintKind.Type

CAPABILITIES = {
    "window": {"workDoneProgress": True},
    "workspace": {"workspaceFolders": True},
    "textDocument": {
        "hover": {"contentFormat": ["markdown", "plaintext"]},
        "inlayHint": {"resolveSupport": {"properties": ["tooltip"]}},
    },
}


def read(path):
    with open(path, encoding="utf-8", errors="replace") as handle:
        return handle.read()


def same(one, other):
    """Two spellings of a type, compared without the whitespace either may have wrapped at."""
    return "".join(one.split()) == "".join(other.split())


FENCE = re.compile(r"```(?:php)?\n(.*?)```", re.DOTALL)


def whole_type(hint):
    """What the hint claims the type is, taking the tooltip over the label when one was folded."""
    tooltip = hint.get("tooltip")
    if tooltip is not None:
        value = tooltip.get("value", "") if isinstance(tooltip, dict) else str(tooltip)
        found = FENCE.search(value)
        return (found.group(1) if found else value).strip()
    label = hint.get("label")
    drawn = label if isinstance(label, str) else "".join(
        part.get("value", "") for part in label or []
    )
    return drawn.lstrip(":").strip()


def hovered_type(result, name):
    """The type out of a variable's hover card: `T $name`, rendered inside a php fence."""
    if not result:
        return None
    contents = result.get("contents")
    text = contents.get("value", "") if isinstance(contents, dict) else str(contents)
    found = FENCE.search(text)
    if not found:
        return None
    card = found.group(1).strip()
    tail = f"${name}"
    if not card.endswith(tail):
        return None
    return card[: -len(tail)].strip() or None


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
            log=os.path.join(REPO, ".phrost-cache", "hint-hover.stderr.log"),
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
        deadline = time.monotonic() + 600
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

    def ask(self, method, params):
        for _ in range(60):
            try:
                answer = self.server.request(method, params, timeout=30.0)
            except (Timeout, Closed):
                return None
            if answer.get("error", {}).get("code") == STILL_INDEXING:
                self.server.pump(0.5)
                continue
            return answer.get("result")
        return None


VARIABLE_BEFORE = re.compile(r"\$([A-Za-z_]\w*)\s*$")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("root")
    parser.add_argument("--files", type=int, default=60)
    args = parser.parse_args()

    paths = compare.php_files(args.root, "", args.files)
    if not paths:
        print("no files to ask about")
        return 1
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

    asked = 0
    disagreed = []
    unhovered = []
    for path in paths:
        text = read(path)
        lines = text.splitlines()
        probe.server.open(path, text)
        hints = probe.ask("textDocument/inlayHint", {
            "textDocument": {"uri": uri_of(path)},
            "range": {
                "start": {"line": 0, "character": 0},
                "end": {"line": max(len(lines), 1), "character": 0},
            },
        }) or []
        for hint in hints:
            if hint.get("kind") != TYPE_HINT:
                continue
            drawn = whole_type(hint)
            at = hint["position"]
            before = lines[at["line"]][: at["character"]] if at["line"] < len(lines) else ""
            named = VARIABLE_BEFORE.search(before)
            if not named:
                # The hint is drawn after the name it types; anything else is this probe reading the
                # position wrongly, and is reported rather than skipped.
                unhovered.append((path, at, drawn, "no variable name before the hint"))
                continue
            asked += 1
            hovered = hovered_type(
                probe.ask("textDocument/hover", {
                    "textDocument": {"uri": uri_of(path)},
                    "position": {"line": at["line"], "character": at["character"] - 1},
                }),
                named.group(1),
            )
            if hovered is None:
                unhovered.append((path, at, drawn, "hover says nothing about the variable"))
            elif not same(drawn, hovered):
                disagreed.append((path, at, named.group(1), drawn, hovered))
        probe.server.close(path)
    probe.server.stop()

    print()
    print(f"a hint and a hover agree  {asked - len(disagreed) - len(unhovered)} of {asked}")
    for path, at, name, drawn, hovered in disagreed[:20]:
        where = f"{os.path.relpath(path, args.root)}:{at['line'] + 1}"
        print(f"  {where} ${name}")
        print(f"     hint  {drawn}")
        print(f"     hover {hovered}")
    for path, at, drawn, why in unhovered[:20]:
        where = f"{os.path.relpath(path, args.root)}:{at['line'] + 1}"
        print(f"  {where} hint `{drawn}` -- {why}")
    return 1 if disagreed or unhovered else 0


if __name__ == "__main__":
    sys.exit(main())
