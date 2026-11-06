#!/usr/bin/env python3
"""Renaming a declaration and renaming it back leaves the project byte for byte as it was.

    python3 tools/a-rename-goes-there-and-back.py <project-root> [--files 12] [--subjects 2]

Rename is the one request where a wrong answer destroys somebody's code rather than merely
disappointing them. `tools/rename-audit.py` measures what a rename *leaves behind*, which is the
under-editing half; nothing measured the other half, and nothing at all checked that what comes out
still parses.

A round trip checks both at once. Rename `Foo` to `ZzFooRenamed`, apply every edit to disk, tell the
server the files changed, then rename it back -- and the tree has to be exactly what it was. A site
the first rename missed comes back as a file that still says `Foo` where the second rename had
nothing to change; a site it should not have touched comes back changed twice. `php -l` reads every
edited file in between, because a syntactically broken result is the failure nobody can undo.

Nothing is written to the project given on the command line: the tree is copied first.
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
from lspclient import Closed, Server, Timeout, path_of, uri_of  # noqa: E402


def _sibling(name):
    spec = importlib.util.spec_from_file_location(
        name.replace("-", "_"), os.path.join(HERE, f"{name}.py")
    )
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


compare = _sibling("compare-lsp")
watched = _sibling("files-changed-on-disk")

REPO = os.path.dirname(HERE)
PHROST = os.path.join(REPO, "target/release/phrost")
STILL_INDEXING = -32801
CHANGED = 2
# What a rename is asked of: the declarations a person renames, and not a property or a case, whose
# names a framework reads out of configuration as often as a call site writes them.
RENAMEABLE = {5: "class", 11: "interface", 10: "enum", 23: "trait", 6: "method", 12: "function"}

CAPABILITIES = {
    "window": {"workDoneProgress": True},
    "workspace": {
        "workspaceFolders": True,
        "didChangeWatchedFiles": {"dynamicRegistration": False},
        "workspaceEdit": {"documentChanges": True},
    },
    "textDocument": {
        "rename": {"prepareSupport": True},
        "documentSymbol": {"hierarchicalDocumentSymbolSupport": True},
        "definition": {"linkSupport": False},
    },
}


def read(path):
    with open(path, encoding="utf-8", errors="replace") as handle:
        return handle.read()


def write(path, text):
    with open(path, "w", encoding="utf-8") as handle:
        handle.write(text)


def parses(text):
    done = subprocess.run(
        ["php", "-l", "-d", "display_errors=1"], input=text, capture_output=True, text=True
    )
    if done.returncode == 0:
        return None
    for line in (done.stdout + "\n" + done.stderr).splitlines():
        if line.strip():
            return line.strip()
    return f"php -l exited {done.returncode}"


def edits_of(answer):
    """Every edit of a workspace edit, as {path: [(start, end, text)]}, whichever shape it came in."""
    out = {}
    if not isinstance(answer, dict):
        return out
    # By the path exactly as the server spelled it, never through `realpath`. A macOS temporary
    # directory is `/var/...` and resolves to `/private/var/...`, and a probe that canonicalised
    # told the server a *second* path for a file it had already indexed -- which produced a second
    # file id, a rename naming one file under two uris, and a corrupted round trip that read
    # exactly like a rename bug. The project's own rule is that a client spells a path one way and
    # keeps to it; an instrument has to keep to it too.
    for uri, edits in (answer.get("changes") or {}).items():
        out.setdefault(path_of(uri), []).extend(edits)
    for change in answer.get("documentChanges") or []:
        document = change.get("textDocument") or {}
        if "uri" in document:
            out.setdefault(path_of(document["uri"]), []).extend(change.get("edits") or [])
    return out


def placed_edits(text, edits):
    lines = compare.Lines(text)
    return sorted(
        (
            offset_of(text, lines, (edit.get("range") or {}).get("start") or {}),
            offset_of(text, lines, (edit.get("range") or {}).get("end") or {}),
            edit.get("newText", ""),
        )
        for edit in edits
    )


def overlaps(placed):
    """The first pair of edits that cover the same character, which the protocol forbids.

    Worth its own answer rather than a corrupted file: an editor applies a workspace edit as given,
    so two edits over one range come out as text nobody wrote -- and read as a rename that mangles
    a docblock rather than as a malformed answer.
    """
    for (start, end, _), (next_start, next_end, _) in zip(placed, placed[1:]):
        if next_start < end:
            return (start, end), (next_start, next_end)
    return None


def applied(text, edits):
    placed = placed_edits(text, edits)
    for start, end, replacement in reversed(placed):
        text = text[:start] + replacement + text[end:]
    return text


def offset_of(text, lines, position):
    line = position.get("line", 0)
    if line >= len(lines.starts):
        return len(text)
    start = lines.starts[line]
    end = lines.starts[line + 1] if line + 1 < len(lines.starts) else len(text)
    units = position.get("character", 0)
    row = text[start:end]
    return start + len(row.encode("utf-16-le")[: units * 2].decode("utf-16-le", "ignore"))


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
            log=os.path.join(REPO, ".phrost-cache", "rename-round-trip.stderr.log"),
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
                self.server.close(anchor)
                return True
        return False

    def looking_at(self, path):
        """A buffer open for the length of one question, and closed before the disk changes.

        The editor owns an open buffer, so the server deliberately ignores a watched change under
        one -- a probe that left files open would tell it about edits it is right to refuse, and
        then ask the stale index what it thinks.
        """
        return _Looking(self, path)

    def ask(self, method, params, timeout=90.0):
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

    def told(self, paths):
        """Says the files changed on disk, and waits for the index to have taken it."""
        self.server.notify("workspace/didChangeWatchedFiles", {
            "changes": [{"uri": uri_of(path), "type": CHANGED} for path in sorted(paths)]
        })
        for _ in range(2):
            self.ask("textDocument/documentSymbol", {
                "textDocument": {"uri": uri_of(next(iter(paths)))}
            })
        self.server.pump(0.5)

    def declarations(self, path):
        """Every renameable declaration in a file, as (kind, name, position)."""
        found = []

        def walk(nodes):
            for node in nodes or []:
                kind = RENAMEABLE.get(node.get("kind"))
                where = (node.get("selectionRange") or {}).get("start")
                if kind and where and node.get("name", "").isidentifier():
                    found.append((kind, node["name"], where))
                walk(node.get("children"))

        walk(self.ask("textDocument/documentSymbol", {"textDocument": {"uri": uri_of(path)}}))
        return found

    def renamed(self, path, position, to):
        return edits_of(self.ask("textDocument/rename", {
            "textDocument": {"uri": uri_of(path)},
            "position": position,
            "newName": to,
        }))


class _Looking:
    def __init__(self, probe, path):
        self.probe, self.path = probe, path

    def __enter__(self):
        self.probe.server.open(self.path, read(self.path))
        return self.probe

    def __exit__(self, *_):
        self.probe.server.close(self.path)
        return False


def first_difference(before, after):
    """The first line the round trip did not put back, as (line number, was, now)."""
    was, now = before.split("\n"), after.split("\n")
    for index in range(max(len(was), len(now))):
        left = was[index] if index < len(was) else "(nothing)"
        right = now[index] if index < len(now) else "(nothing)"
        if left != right:
            return index + 1, left, right
    return 0, "", ""


def apply_to_disk(by_path, held, crossed):
    """Applies a workspace edit, remembering what each file said first."""
    for path, edits in by_path.items():
        if not os.path.exists(path):
            continue
        held.setdefault(path, read(path))
        text = read(path)
        if (pair := overlaps(placed_edits(text, edits))) is not None:
            crossed.append((path, pair))
        write(path, applied(text, edits))
    return set(by_path)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("root")
    parser.add_argument("--files", type=int, default=12)
    parser.add_argument("--subjects", type=int, default=2,
                        help="how many declarations per file to round-trip")
    args = parser.parse_args()

    paths = compare.php_files(args.root, "", args.files)
    if not paths:
        print("no files to rename anything in")
        return 1

    work = tempfile.mkdtemp(prefix="phrost-rename-")
    try:
        tree = os.path.join(work, "tree")
        watched.clone_tree(os.path.abspath(args.root), tree)
        here = [os.path.join(tree, os.path.relpath(path, args.root)) for path in paths]
        anchor = None
        for path in reversed(here):
            found = compare.DECLARED_CLASS.search(compare.mask(read(path)))
            if found:
                anchor = (path, found.group(1))
                break
        if anchor is None:
            print("no file in the sample declares a class")
            return 1

        probe = Probe(tree)
        if not probe.start(*anchor):
            print("the server never answered about the project")
            return 1

        asked = 0
        refused = 0
        moved = 0
        crossed = []
        broken, unreturned = [], []
        for path in here:
            with probe.looking_at(path):
                subjects = probe.declarations(path)[: args.subjects]
            for kind, name, position in subjects:
                with probe.looking_at(path):
                    allowed = probe.ask("textDocument/prepareRename", {
                        "textDocument": {"uri": uri_of(path)}, "position": position
                    })
                    forward = (
                        probe.renamed(path, position, f"Zz{name}Renamed") if allowed else {}
                    )
                if not allowed:
                    refused += 1
                    continue
                asked += 1
                held = {}
                # There, applied to disk and announced, so the index describes what is now written.
                touched = apply_to_disk(forward, held, crossed)
                moved += sum(len(edits) for edits in forward.values())
                if touched:
                    probe.told(touched)
                for edited in sorted(touched):
                    why = parses(read(edited))
                    if why is not None:
                        broken.append((path, name, edited, why))

                # And back, from the same place: a name keeps the position it started at.
                with probe.looking_at(path):
                    back = probe.renamed(path, position, name)
                touched |= apply_to_disk(back, held, crossed)
                if touched:
                    probe.told(touched)

                different = sorted(
                    (edited, first_difference(before, read(edited)))
                    for edited, before in held.items()
                    if read(edited) != before
                )
                if different:
                    unreturned.append((path, name, different))
                for edited, before in held.items():
                    write(edited, before)
                if held:
                    probe.told(set(held))
        probe.server.stop()

        print()
        print(f"a rename goes there and back  {asked - len(broken) - len(unreturned)} of {asked}"
              f"  ({moved} sites moved, {refused} refused before anything was edited)")
        if crossed:
            print(f"  {len(crossed)} workspace edits hold two edits over one range:")
            for edited, (left, right) in crossed[:6]:
                print(f"    {os.path.relpath(edited, tree)}  {left} and {right}")
        for path, name, edited, why in broken[:10]:
            print(f"  {os.path.relpath(path, tree)} {name}: {os.path.relpath(edited, tree)}")
            print(f"     {why}")
        for path, name, different in unreturned[:10]:
            print(f"  {os.path.relpath(path, tree)} {name}: did not come back")
            for edited, (line, before, after) in different[:4]:
                print(f"     {os.path.relpath(edited, tree)}:{line}")
                print(f"       was {before.strip()[:100]}")
                print(f"       now {after.strip()[:100]}")
        return 1 if broken or unreturned else 0
    finally:
        shutil.rmtree(work, ignore_errors=True)


if __name__ == "__main__":
    sys.exit(main())
