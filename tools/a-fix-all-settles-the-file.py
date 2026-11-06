#!/usr/bin/env python3
"""`source.fixAll` leaves the file parsing, says no more than before, and settles.

    python3 tools/a-fix-all-settles-the-file.py <project-root> [--files 40]

This is the save path. An editor configured with `code_actions_on_format` asks for
`source.fixAll` over the whole document on every save and applies whatever comes back, unread --
so an edit that overlaps another, or one that fixes one thing and breaks a second, is text nobody
wrote appearing in a file somebody just saved.

Four things are required of the answer, and the first is the one no diff would show:

* every edit is its own place, which the protocol requires and a client does not check;
* the result parses -- `php -l` reads it;
* the file says no *more* than it did, by count and by code;
* asking again **settles**, because a save loop asks on every save. Not that the second answer is
  empty: `doc-missing-throws` is transitive, so documenting one method makes its caller owe the
  same tag, and each save takes one step up that chain. What must not happen is that the steps
  never end.

`tools/code-actions-apply-cleanly.py` asks at a cursor and applies one offer at a time. This asks
for the whole document and applies the composition, which is where edits meet.
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
from lspclient import Closed, Server, Timeout, path_of, uri_of  # noqa: E402


def _sibling(name):
    spec = importlib.util.spec_from_file_location(
        name.replace("-", "_"), os.path.join(HERE, f"{name}.py")
    )
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


compare = _sibling("compare-lsp")
renames = _sibling("a-rename-goes-there-and-back")

REPO = os.path.dirname(HERE)
STILL_INDEXING = -32801

CAPABILITIES = {
    "window": {"workDoneProgress": True},
    "workspace": {"workspaceFolders": True},
    "textDocument": {
        "synchronization": {"didSave": True},
        "publishDiagnostics": {"relatedInformation": False},
        "definition": {"linkSupport": False},
        "codeAction": {
            "codeActionLiteralSupport": {
                "codeActionKind": {"valueSet": ["quickfix", "source.fixAll"]}
            },
            "resolveSupport": {"properties": ["edit"]},
        },
    },
}


class Probe:
    def __init__(self, root):
        self.root = os.path.abspath(root)
        self.ready = None
        self.working = set()
        self.diagnostics = {}
        os.makedirs(os.path.join(REPO, ".phrost-cache"), exist_ok=True)
        self.server = Server(
            "phrost",
            [os.path.join(REPO, "target/release/phrost"), "serve"],
            self.root,
            log=os.path.join(REPO, ".phrost-cache", "fix-all.stderr.log"),
        )
        self.server.on_notification(self._heard)

    def _heard(self, method, params):
        if method == "textDocument/publishDiagnostics":
            self.diagnostics[path_of(params.get("uri", ""))] = (
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
                self.ready = self.ready or "progress"

    def start(self, anchor, anchor_name):
        self.server.initialize(CAPABILITIES)
        text = renames.read(anchor)
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
                self.server.close(anchor)
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

    def fix_all(self, path, text):
        """The one workspace edit `source.fixAll` offers over the whole document."""
        rows = text.split("\n")
        offers = self.ask("textDocument/codeAction", {
            "textDocument": {"uri": uri_of(path)},
            "range": {
                "start": {"line": 0, "character": 0},
                "end": {"line": max(len(rows) - 1, 0), "character": len(rows[-1]) if rows else 0},
            },
            "context": {"diagnostics": [], "only": ["source.fixAll"]},
        }) or []
        edits = {}
        for offer in offers:
            if not isinstance(offer, dict):
                continue
            if offer.get("edit") is None and offer.get("data") is not None:
                offer = self.ask("codeAction/resolve", offer) or offer
            for where, these in renames.edits_of(offer.get("edit")).items():
                edits.setdefault(where, []).extend(these)
        return edits

    def settled_diagnostics(self, path, text, version):
        before = self.diagnostics.get(path)
        self.ask("textDocument/documentSymbol", {"textDocument": {"uri": uri_of(path)}})
        deadline = time.monotonic() + 15
        while time.monotonic() < deadline:
            held = self.diagnostics.get(path)
            if held is not None and held[0] == version:
                return held[1]
            self.server.pump(0.2)
        return (before or (None, []))[1]


def codes(diagnostics):
    return Counter(str(one.get("code") or "") for one in diagnostics)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("root")
    parser.add_argument("--files", type=int, default=40)
    parser.add_argument("--passes", type=int, default=6,
                        help="how many saves a chain of fixes may take to end")
    args = parser.parse_args()

    paths = compare.php_files(args.root, "", args.files)
    anchor = None
    for path in reversed(paths):
        found = compare.DECLARED_CLASS.search(compare.mask(renames.read(path)))
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

    asked = offered = 0
    crossed, broken, worse, unsettled = [], [], [], []
    for path in paths:
        text = renames.read(path)
        probe.server.open(path, text)
        before = codes(probe.settled_diagnostics(path, text, 1))
        edits = probe.fix_all(path, text)
        asked += 1
        here = edits.get(path) or []
        if not here:
            probe.server.close(path)
            continue
        offered += len(here)

        placed = renames.placed_edits(text, here)
        if (pair := renames.overlaps(placed)) is not None:
            crossed.append((path, pair))
        fixed = renames.applied(text, here)

        why = renames.parses(fixed)
        if why is not None:
            broken.append((path, why))
            probe.server.close(path)
            continue

        # Every save is another pass, so the check is that the passes end -- and that no pass
        # makes the file say more than it did before any of them.
        version = 1
        settled = False
        for _ in range(args.passes):
            version += 1
            probe.server.notify("textDocument/didChange", {
                "textDocument": {"uri": uri_of(path), "version": version},
                "contentChanges": [{"text": fixed}],
            })
            after = codes(probe.settled_diagnostics(path, fixed, version))
            rose = {code: (before.get(code, 0), count) for code, count in after.items()
                    if count > before.get(code, 0)}
            if rose:
                worse.append((path, rose))
                break
            more = probe.fix_all(path, fixed).get(path) or []
            if not more:
                settled = True
                break
            offered += len(more)
            placed = renames.placed_edits(fixed, more)
            if (pair := renames.overlaps(placed)) is not None:
                crossed.append((path, pair))
                break
            fixed = renames.applied(fixed, more)
            why = renames.parses(fixed)
            if why is not None:
                broken.append((path, why))
                break
        else:
            unsettled.append(path)
        if not settled and not (worse or crossed or broken):
            pass
        probe.server.close(path)
    probe.server.stop()

    print()
    kept = asked - len(crossed) - len(broken) - len(worse) - len(unsettled)
    print(f"a fix-all settles the file  {kept} of {asked}  ({offered} edits offered)")
    for path, pair in crossed[:8]:
        print(f"  {os.path.relpath(path, args.root)}: two edits over one range {pair}")
    for path, why in broken[:8]:
        print(f"  {os.path.relpath(path, args.root)}: {why}")
    for path, rose in worse[:8]:
        print(f"  {os.path.relpath(path, args.root)}: says more than before {rose}")
    for path in unsettled[:8]:
        print(f"  {os.path.relpath(path, args.root)}: still offering after {args.passes} saves")
    return 1 if crossed or broken or worse or unsettled else 0


if __name__ == "__main__":
    sys.exit(main())
