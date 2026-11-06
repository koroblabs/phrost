#!/usr/bin/env python3
"""Whether every edit a code action offers still leaves the file PHP can parse.

    python3 tools/code-actions-apply-cleanly.py <project-root> [--files 20] [--limit 400]

The one request where a wrong answer destroys somebody's code rather than merely disappointing
them, and the one whose result can be checked without any expectation being written down: apply
what the action says, hand the text to `php -l`, and read what the runtime says. A file that parsed
before and does not after is a bug however plausible the diff looks.

Three things are asked of each offer, and the second is the one that catches a range bug before it
reaches anyone: every edit's range has to lie inside the file, the edits must not overlap, and the
text that comes out has to parse. Nothing is written to disk -- the edits are applied to a copy in
memory and the copy is linted through stdin.

Cursors come from a text scan, so the sample is what a person's caret actually lands on: a name, a
member, a variable. An action that offers no edit at all -- a command, a refusal -- is counted and
skipped, since there is nothing to apply.
"""
import argparse
import importlib.util
import json
import os
import subprocess
import sys
import time
from collections import Counter

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
STILL_INDEXING = -32801

CAPABILITIES = {
    "window": {"workDoneProgress": True},
    "workspace": {"workspaceFolders": True, "applyEdit": True},
    "textDocument": {
        "synchronization": {"didSave": True},
        "definition": {"linkSupport": False},
        "codeAction": {
            "codeActionLiteralSupport": {
                "codeActionKind": {
                    "valueSet": [
                        "quickfix",
                        "refactor",
                        "refactor.extract",
                        "refactor.inline",
                        "refactor.rewrite",
                        "source",
                        "source.organizeImports",
                        "source.fixAll",
                    ]
                }
            },
            "resolveSupport": {"properties": ["edit"]},
            "dataSupport": True,
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
            log=os.path.join(REPO, ".phrost-cache", "actions-probe.stderr.log"),
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
            if self.request("textDocument/definition", {
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

    def request(self, method, params):
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


class Text:
    """One file's text, with the protocol's positions turned into offsets."""

    def __init__(self, body):
        self.body = body
        self.starts = [0] + [index + 1 for index, ch in enumerate(body) if ch == "\n"]

    def offset(self, position):
        line = position["line"]
        if line >= len(self.starts):
            return None
        at, used = self.starts[line], 0
        end = self.starts[line + 1] if line + 1 < len(self.starts) else len(self.body)
        while at < end and used < position["character"]:
            used += len(self.body[at].encode("utf-16-le")) // 2
            at += 1
        return at if used == position["character"] else None


def edits_of(action, uri):
    """The text edits an action makes to one file, or None when it makes none there."""
    edit = action.get("edit") or {}
    changes = edit.get("changes") or {}
    if uri in changes:
        return changes[uri]
    for document in edit.get("documentChanges") or []:
        if (document.get("textDocument") or {}).get("uri") == uri:
            return document.get("edits") or []
    return None


def applied(body, edits):
    """The text after the edits, or a reason it cannot be applied."""
    text = Text(body)
    spans = []
    for one in edits:
        start = text.offset(one["range"]["start"])
        end = text.offset(one["range"]["end"])
        if start is None or end is None:
            return None, "a range outside the file"
        if start > end:
            return None, "a range that ends before it starts"
        spans.append((start, end, one.get("newText", "")))
    spans.sort(key=lambda span: span[0])
    for (_, before_end, _), (after_start, _, _) in zip(spans, spans[1:]):
        if after_start < before_end:
            return None, "two edits over the same text"
    out, at = [], 0
    for start, end, new in spans:
        out.append(body[at:start])
        out.append(new)
        at = end
    out.append(body[at:])
    return "".join(out), None


def parses(body):
    done = subprocess.run(
        ["php", "-l"], input=body, capture_output=True, text=True
    )
    # The first *non-empty* line: `php -l` writes the same complaint to both streams and stdout
    # opens with a blank one, so taking line zero reported every failure with no reason at all.
    said = [line for line in (done.stdout + done.stderr).split("\n") if line.strip()]
    return done.returncode == 0, said[0] if said else "php said nothing"


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("root")
    parser.add_argument("--files", type=int, default=20)
    parser.add_argument("--limit", type=int, default=400)
    parser.add_argument("--needle", default="")
    parser.add_argument("--verbose", action="store_true")
    args = parser.parse_args()

    paths = compare.php_files(args.root, args.needle, args.files)
    plan = {}
    for path in paths:
        body = read(path)
        ok, _ = parses(body)
        # Only files PHP already accepts: an action cannot be blamed for a file that was broken.
        if ok:
            plan[path] = (body, compare.sites_of(compare.mask(body)))
    if not plan:
        print("no file in the sample parses")
        return 1

    probe = compare.probe_of({path: (body, None) for path, (body, _) in plan.items()})
    if probe is None:
        print("no class declared in the sample to probe readiness with")
        return 1

    session = Probe(args.root)
    print(f"starting server over {args.root}", flush=True)
    if not session.start(*probe):
        print("server never answered about the project")
        return 1

    sites = []
    for path, (body, found) in plan.items():
        lines = compare.Lines(body)
        for site in found:
            sites.append((path, lines.position(site.offset)))
    sites = compare.stride(sites, args.limit)
    for path, _ in sites:
        session.open(path)

    offered = 0
    no_edit = 0
    broken = []
    for path, position in sites:
        uri = uri_of(path)
        actions = session.request(
            "textDocument/codeAction",
            {
                "textDocument": {"uri": uri},
                "range": {"start": position, "end": position},
                "context": {"diagnostics": []},
            },
        )
        for action in actions or []:
            if not isinstance(action, dict):
                continue
            edits = edits_of(action, uri)
            if edits is None and action.get("data") is not None:
                resolved = session.request("codeAction/resolve", action)
                edits = edits_of(resolved, uri) if resolved else None
            if not edits:
                no_edit += 1
                continue
            offered += 1
            body = session.opened[path]
            after, why = applied(body, edits)
            if after is None:
                broken.append((action.get("title", "?"), path, position, why))
                continue
            if after == body:
                broken.append((action.get("title", "?"), path, position, "no change at all"))
                continue
            ok, message = parses(after)
            if not ok:
                broken.append((action.get("title", "?"), path, position, message))

    session.server.stop()
    print()
    print(f"code actions        {offered - len(broken)} of {offered} edits leave the file parsing")
    print(f"  offered no edit   {no_edit:5}")
    if broken:
        print()
        for why, count in Counter(why for *_, why in broken).most_common(8):
            print(f"  {count:5}  {why[:70]}")
        print()
        seen = set()
        for title, path, position, why in broken:
            if title in seen and not args.verbose:
                continue
            seen.add(title)
            rel = os.path.relpath(path, args.root)
            print(f"  {title[:48]:50} {rel}:{position['line'] + 1}:{position['character'] + 1}")
            print(f"     {why[:90]}")
    return 1 if broken else 0


if __name__ == "__main__":
    sys.exit(main())
