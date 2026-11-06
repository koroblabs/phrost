#!/usr/bin/env python3
"""`phrost check` and a running server report the same findings about the same file.

    python3 tools/the-checker-and-the-server-agree.py <project-root> [--files 40]

Two entry points into one set of checks. The CLI builds a workspace, indexes, and sweeps the
project; the server bootstraps, checks every file nobody has open in the background, and re-checks a
file when the editor opens it. CLAUDE.md §8.1 calls a whole entry point reached by a second
spelling *two constructors of one rule*, and there is a standing note that the checker has to mirror
the server's setup or it over-reports -- which was found once, by hand, and has had no check since.

The stakes are asymmetric and easy to miss: every ratchet in this project is a CLI number, so a
drift between the two makes the instrument describe something nobody sees.

Three answers are compared, not two:

* what `phrost check` prints,
* what the server publishes for the file **closed**, from its background project pass,
* what it publishes once the file is **open**, which is `open == closed` asked through the protocol
  rather than through the harness.
"""
import argparse
import importlib.util
import json
import os
import re
import subprocess
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
from lspclient import Closed, Server, Timeout, path_of, uri_of  # noqa: E402


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
PROJECT_TOKEN = '"phrost/project-diagnostics"'

CAPABILITIES = {
    "window": {"workDoneProgress": True},
    "workspace": {"workspaceFolders": True},
    "textDocument": {
        "synchronization": {"didSave": True},
        "publishDiagnostics": {"relatedInformation": False},
        "definition": {"linkSupport": False},
    },
}

# `path:line:col: severity code: message`, which is what `check` prints per finding.
PRINTED = re.compile(r"^(.+?):(\d+):(\d+): (\w+) ([a-z0-9-]+): (.*)$")


def first_line(message):
    """A message's first line, which is all `check` prints on the line it prints the position on.

    A rule whose message runs over several lines -- the security ones spell out what to do instead
    -- would otherwise compare as different for a difference in neither the finding nor the place.
    """
    return message.strip().split("\n")[0].strip()


def read(path):
    with open(path, encoding="utf-8", errors="replace") as handle:
        return handle.read()


def checked(root, jobs):
    """Every finding `phrost check` prints, as {absolute path: {finding}}."""
    done = subprocess.run(
        [PHROST, "check", root, "--jobs", str(jobs)],
        capture_output=True,
        text=True,
    )
    found = {}
    for line in (done.stdout + done.stderr).splitlines():
        printed = PRINTED.match(line.strip())
        if not printed:
            continue
        where, line_no, column, severity, code, message = printed.groups()
        path = os.path.realpath(os.path.join(root, where))
        found.setdefault(path, set()).add(
            (int(line_no), int(column), severity, code, first_line(message))
        )
    return found


def published(text, diagnostics):
    """One publication as the same shape, so the two can be compared as sets.

    The protocol counts lines from zero and columns in UTF-16 code units; `check` counts both from
    one, in characters. Converted here rather than compared loosely -- a column that is quietly a
    little wrong is exactly what a loose comparison is for hiding.
    """
    rows = text.split("\n")
    found = set()
    for one in diagnostics:
        start = (one.get("range") or {}).get("start") or {}
        line = start.get("line", 0)
        units = start.get("character", 0)
        row = rows[line] if 0 <= line < len(rows) else ""
        column = len(row.encode("utf-16-le")[: units * 2].decode("utf-16-le", "ignore")) + 1
        found.add((
            line + 1,
            column,
            {1: "error", 2: "warning", 3: "note", 4: "hint"}.get(one.get("severity"), "warning"),
            str(one.get("code") or ""),
            first_line(one.get("message") or ""),
        ))
    return found


class Probe:
    def __init__(self, root):
        self.root = os.path.abspath(root)
        self.diagnostics = {}
        self.working = set()
        self.ended = set()
        os.makedirs(os.path.join(REPO, ".phrost-cache"), exist_ok=True)
        self.server = Server(
            "phrost",
            [PHROST, "serve"],
            self.root,
            log=os.path.join(REPO, ".phrost-cache", "checker-agrees.stderr.log"),
        )
        self.server.on_notification(self._heard)

    def _heard(self, method, params):
        if method == "textDocument/publishDiagnostics":
            self.diagnostics[os.path.realpath(path_of(params.get("uri", "")))] = (
                params.get("version"),
                params.get("diagnostics") or [],
            )
            return
        if method == "$/progress":
            token = json.dumps(params.get("token"))
            kind = (params.get("value") or {}).get("kind")
            if kind == "begin":
                self.working.add(token)
            elif kind == "end":
                self.working.discard(token)
                self.ended.add(token)

    def start(self):
        """Up to the point the background project pass has finished, which is what `check` is."""
        self.server.initialize(CAPABILITIES)
        deadline = time.monotonic() + 900
        while time.monotonic() < deadline:
            self.server.pump(0.5)
            if PROJECT_TOKEN in self.ended and not self.working:
                # One more turn, for the publications still in the pipe behind the `end`.
                self.server.pump(2.0)
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

    def opened_diagnostics(self, path, text):
        before = self.diagnostics.get(path)
        self.server.open(path, text)
        self.ask("textDocument/documentSymbol", {"textDocument": {"uri": uri_of(path)}})
        deadline = time.monotonic() + 20
        while time.monotonic() < deadline:
            held = self.diagnostics.get(path)
            if held is not None and held[0] == 1:
                return held[1]
            self.server.pump(0.2)
        # Nothing new for this version: the findings did not change when the buffer opened, so what
        # the editor shows is what the project pass gave it -- which is the answer, not a timeout.
        return (before or (None, []))[1]


def report(title, differences, root, shown=8):
    print(f"  {title}")
    for path, only_left, only_right in differences[:shown]:
        print(f"    {os.path.relpath(path, root)}")
        for one in sorted(only_left)[:3]:
            print(f"       check only : {one[0]}:{one[1]} {one[3]} {one[4][:70]}")
        for one in sorted(only_right)[:3]:
            print(f"       server only: {one[0]}:{one[1]} {one[3]} {one[4][:70]}")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("root")
    parser.add_argument("--files", type=int, default=40)
    args = parser.parse_args()
    root = os.path.abspath(args.root)

    print("asking the checker", flush=True)
    by_check = checked(root, 8)

    print("asking a server", flush=True)
    probe = Probe(root)
    if not probe.start():
        print("the server never finished checking the project")
        return 1

    # Every file either of them says anything about, so a finding one has and the other does not
    # counts -- comparing only the files both mentioned would score silence as agreement.
    closed = {
        path: published(read(path), diagnostics)
        for path, (_, diagnostics) in probe.diagnostics.items()
        if os.path.exists(path)
    }
    everywhere = sorted(set(by_check) | set(closed))
    differ_closed = [
        (path, by_check.get(path, set()) - closed.get(path, set()),
         closed.get(path, set()) - by_check.get(path, set()))
        for path in everywhere
    ]
    differ_closed = [row for row in differ_closed if row[1] or row[2]]

    # Spread over the project rather than drawn from the files that already have findings: a buffer
    # the editor opens is where a check runs against text the index did not write, so a file that is
    # clean closed and not clean open is exactly what this half is for.
    sample = compare.php_files(root, "", args.files)
    differ_open = []
    for path in sample:
        real = os.path.realpath(path)
        text = read(path)
        mine = published(text, probe.opened_diagnostics(real, text))
        theirs = by_check.get(real, set())
        if mine != theirs:
            differ_open.append((real, theirs - mine, mine - theirs))
        probe.server.close(real)
    probe.server.stop()

    findings = sum(len(one) for one in by_check.values())
    print()
    print(f"the checker found {findings} findings over {len(by_check)} files")
    print(f"closed  {len(everywhere) - len(differ_closed)} of {len(everywhere)} files agree")
    if differ_closed:
        report("files the server and the checker disagree about, closed:", differ_closed, root)
    print(f"open    {len(sample) - len(differ_open)} of {len(sample)} files agree")
    if differ_open:
        report("files that answer differently once opened:", differ_open, root)
    return 1 if differ_closed or differ_open else 0


if __name__ == "__main__":
    sys.exit(main())
