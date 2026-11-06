#!/usr/bin/env python3
"""The formatter answers the same whichever door it is reached through.

    python3 tools/one-formatter-two-front-doors.py <project-root> [--files 40]

Three readings of one reprint:

* `phrost reprint`, the command `tools/drupal-style.sh` measures the formatter with;
* `textDocument/formatting`, which is what format-on-save actually applies;
* `textDocument/rangeFormatting` over the whole document, which the server documents as a *subset*
  of the second -- so over the whole document it has to be the second exactly.

The first pair is the shape that has already been wrong twice here: a command line and a server
resolving one setting from two places. The third is checked for the thing a subset can still get
wrong, which is producing text PHP will not parse -- `php -l` reads it.
"""
import argparse
import importlib.util
import json
import os
import shutil
import subprocess
import sys
import tempfile
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
PHROST = os.path.join(REPO, "target/release/phrost")
STILL_INDEXING = -32801

CAPABILITIES = {
    "window": {"workDoneProgress": True},
    "workspace": {"workspaceFolders": True},
    "textDocument": {
        "formatting": {"dynamicRegistration": False},
        "rangeFormatting": {"dynamicRegistration": False},
        "definition": {"linkSupport": False},
    },
}


def read(path):
    with open(path, encoding="utf-8", errors="replace") as handle:
        return handle.read()


def applied(text, edits):
    """The text with every edit in it, applied back to front so earlier offsets stay valid."""
    lines = compare.Lines(text)
    placed = []
    for edit in edits or []:
        where = edit.get("range") or {}
        placed.append((
            offset_of(text, lines, where.get("start") or {}),
            offset_of(text, lines, where.get("end") or {}),
            edit.get("newText", ""),
        ))
    for start, end, replacement in sorted(placed, reverse=True):
        text = text[:start] + replacement + text[end:]
    return text


def offset_of(text, lines, position):
    """An LSP position as a byte offset into `text`, counting characters the way the protocol does."""
    line = position.get("line", 0)
    if line >= len(lines.starts):
        return len(text)
    start = lines.starts[line]
    end = lines.starts[line + 1] if line + 1 < len(lines.starts) else len(text)
    row = text[start:end]
    units = position.get("character", 0)
    return start + len(row.encode("utf-16-le")[: units * 2].decode("utf-16-le", "ignore"))


def parses(text):
    """What `php -l` says, or None when it is happy. PHP writes to both streams."""
    done = subprocess.run(
        ["php", "-l", "-d", "display_errors=1"],
        input=text,
        capture_output=True,
        text=True,
    )
    if done.returncode == 0:
        return None
    for line in (done.stdout + "\n" + done.stderr).splitlines():
        if line.strip():
            return line.strip()
    return f"php -l exited {done.returncode}"


class Probe:
    def __init__(self, root):
        self.root = os.path.abspath(root)
        self.ready = None
        self.working = set()
        os.makedirs(os.path.join(REPO, ".phrost-cache"), exist_ok=True)
        self.server = Server(
            "phrost",
            [PHROST, "serve"],
            self.root,
            log=os.path.join(REPO, ".phrost-cache", "two-front-doors.stderr.log"),
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

    def ask(self, method, params, timeout=60.0):
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

    def formatted(self, path, text):
        return applied(text, self.ask("textDocument/formatting", {
            "textDocument": {"uri": uri_of(path)},
            "options": {"tabSize": 2, "insertSpaces": True},
        }))

    def range_formatted(self, path, text):
        rows = text.split("\n")
        return applied(text, self.ask("textDocument/rangeFormatting", {
            "textDocument": {"uri": uri_of(path)},
            "range": {
                "start": {"line": 0, "character": 0},
                "end": {"line": max(len(rows) - 1, 0), "character": len(rows[-1]) if rows else 0},
            },
            "options": {"tabSize": 2, "insertSpaces": True},
        }))


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("root")
    parser.add_argument("--files", type=int, default=40)
    args = parser.parse_args()
    root = os.path.abspath(args.root)

    paths = compare.php_files(root, "", args.files)
    anchor = None
    for path in reversed(paths):
        found = compare.DECLARED_CLASS.search(compare.mask(read(path)))
        if found:
            anchor = (path, found.group(1))
            break
    if anchor is None:
        print("no file in the sample declares a class")
        return 1

    work = tempfile.mkdtemp(prefix="phrost-reprint-")
    try:
        print("reprinting through the command", flush=True)
        done = subprocess.run(
            [PHROST, "reprint", root, work], capture_output=True, text=True
        )
        if done.returncode != 0:
            print(f"reprint failed: {(done.stderr or done.stdout).strip()[:200]}")
            return 1
        print(done.stdout.strip(), flush=True)

        probe = Probe(root)
        if not probe.start(*anchor):
            print("the server never answered about the project")
            return 1

        asked = 0
        differ_command, differ_range, unparsed = [], [], []
        for path in paths:
            beside = os.path.join(work, "after", os.path.relpath(path, root))
            if not os.path.exists(beside):
                # The command walks the tree with its own list of extensions; a file it did not
                # take is not a disagreement.
                continue
            text = read(path)
            probe.server.open(path, text)
            asked += 1
            served = probe.formatted(path, text)
            if served != read(beside):
                differ_command.append(path)
            ranged = probe.range_formatted(path, text)
            if ranged != served:
                differ_range.append(path)
            why = parses(ranged)
            if why is not None:
                unparsed.append((path, why))
            probe.server.close(path)
        probe.server.stop()

        print()
        print(f"one formatter, two front doors  {asked - len(differ_command)} of {asked} files match the command")
        for path in differ_command[:10]:
            print(f"    {os.path.relpath(path, root)}")
        print(f"a whole-document range          {asked - len(differ_range)} of {asked} files match the document answer")
        for path in differ_range[:10]:
            print(f"    {os.path.relpath(path, root)}")
        print(f"the range result parses         {asked - len(unparsed)} of {asked}")
        for path, why in unparsed[:10]:
            print(f"    {os.path.relpath(path, root)}: {why}")
        return 1 if differ_command or differ_range or unparsed else 0
    finally:
        shutil.rmtree(work, ignore_errors=True)


if __name__ == "__main__":
    sys.exit(main())
