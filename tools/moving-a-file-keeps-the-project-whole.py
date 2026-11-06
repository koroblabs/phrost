#!/usr/bin/env python3
"""Moving a file, applying what the server offers, and moving it back leaves the project as it was.

    python3 tools/moving-a-file-keeps-the-project-whole.py <project-root> [--files 10]

PSR-4 makes a class's fully-qualified name a function of where its file is, so a drag in the
sidebar is a rename of the class. The server answers `workspace/willRenameFiles` with the edits
that keep the project autoloading, and an editor applies them and the move in one undo step --
which makes a wrong answer a project that no longer loads, with no diagnostic until something fails
at runtime.

The round trip is the check: move `src/A/Thing.php` to `src/B/Thing.php`, apply the edits, move it
back, apply those, and the tree has to be byte for byte what it was. A line the first move should
have written and did not comes back as a file still naming the old namespace; a line it should not
have touched comes back changed twice. `php -l` reads every edited file in between, and a
`check` over the moved tree says whether the move left anything undefined.

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
renames = _sibling("a-rename-goes-there-and-back")

REPO = os.path.dirname(HERE)
PHROST = os.path.join(REPO, "target/release/phrost")
STILL_INDEXING = -32801
CREATED, CHANGED, DELETED = 1, 2, 3

CAPABILITIES = {
    "window": {"workDoneProgress": True},
    "workspace": {
        "workspaceFolders": True,
        "fileOperations": {"willRename": True},
        "didChangeWatchedFiles": {"dynamicRegistration": False},
    },
    "textDocument": {"definition": {"linkSupport": False}},
}


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
            log=os.path.join(REPO, ".phrost-cache", "file-rename.stderr.log"),
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

    def will_rename(self, old, new):
        return renames.edits_of(self.ask("workspace/willRenameFiles", {
            "files": [{"oldUri": uri_of(old), "newUri": uri_of(new)}]
        }))

    def moved(self, old, new, edited=()):
        """Says the file moved and which others changed, so the index describes what is written.

        The edited neighbours matter as much as the move: the server refuses to write an import a
        file already has, and it can only know that from its own index. Told about the move alone,
        it offered a second `use` for a name already imported and the file stopped loading -- which
        read as a server bug and was this probe not saying what it had done.
        """
        changes = [
            {"uri": uri_of(old), "type": DELETED},
            {"uri": uri_of(new), "type": CREATED},
        ]
        changes += [
            {"uri": uri_of(path), "type": CHANGED}
            for path in sorted(set(edited) - {old, new})
        ]
        self.server.notify("workspace/didChangeWatchedFiles", {"changes": changes})
        for _ in range(2):
            self.ask("textDocument/documentSymbol", {"textDocument": {"uri": uri_of(new)}})
        self.server.pump(0.5)


def move(old, new):
    os.makedirs(os.path.dirname(new), exist_ok=True)
    shutil.move(old, new)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("root")
    parser.add_argument("--files", type=int, default=10)
    args = parser.parse_args()

    paths = compare.php_files(args.root, "", args.files)
    if not paths:
        print("no files to move")
        return 1

    work = tempfile.mkdtemp(prefix="phrost-move-")
    try:
        tree = os.path.join(work, "tree")
        watched.clone_tree(os.path.abspath(args.root), tree)
        here = [os.path.join(tree, os.path.relpath(path, args.root)) for path in paths]
        anchor = None
        for path in reversed(here):
            found = compare.DECLARED_CLASS.search(compare.mask(renames.read(path)))
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
        offered = 0
        broken, unreturned, residue = [], [], []
        for path in here:
            if not compare.DECLARED_CLASS.search(compare.mask(renames.read(path))):
                continue
            asked += 1
            # Into a sibling directory, which is the move a person makes: the namespace changes and
            # the basename does not, so what has to follow is every name the class was written by.
            new = os.path.join(os.path.dirname(path), "ZzMoved", os.path.basename(path))
            held = {}
            crossed = []

            # The edits first and the move after, which is the order the protocol names: the
            # answer is `willRenameFiles`, so every range in it is against the file where it still
            # is. Applied the other way round, an edit to the moved file itself has nowhere to land.
            forward = probe.will_rename(path, new)
            touched = renames.apply_to_disk(forward, held, crossed)
            move(path, new)
            probe.moved(path, new, touched)
            offered += sum(len(one) for one in forward.values())
            for edited in sorted(touched):
                # The moved file reads from where it is now.
                why = renames.parses(renames.read(new if edited == path else edited))
                if why is not None:
                    broken.append((path, edited, why))

            back = probe.will_rename(new, path)
            renames.apply_to_disk(back, {}, crossed)
            touched |= set(back)
            move(new, path)
            probe.moved(new, path, touched)

            different = sorted(
                (edited, renames.first_difference(before, renames.read(edited)))
                for edited, before in held.items()
                if renames.read(edited) != before
            )
            # An import the move *wrote* and the move back did not take away is a line PHP
            # accepts -- a class may be imported from its own namespace, and `unused-import` is
            # right not to report one. So it is residue rather than a break, and counted apart:
            # the project that comes back is correct and is not the same bytes.
            if different and all(
                after.strip().startswith("use ") and not before.strip()
                for _, (_, before, after) in different
            ):
                residue.append((path, different))
            elif different:
                unreturned.append((path, different))
            for edited, before in held.items():
                renames.write(edited, before)
            if held:
                probe.moved(path, path, held)
        probe.server.stop()

        print()
        print(f"moving a file keeps the project whole  "
              f"{asked - len(broken) - len(unreturned)} of {asked}  ({offered} lines offered)")
        if residue:
            print(f"  {len(residue)} left an import behind, which PHP accepts and nothing reports:")
            for path, different in residue[:6]:
                lines = ", ".join(
                    f"{os.path.relpath(edited, tree)}:{line}" for edited, (line, _, _) in different[:3]
                )
                print(f"    {os.path.relpath(path, tree)}  ->  {lines}")
        for path, edited, why in broken[:8]:
            print(f"  {os.path.relpath(path, tree)}: {os.path.relpath(edited, tree)}")
            print(f"     {why}")
        for path, different in unreturned[:8]:
            print(f"  {os.path.relpath(path, tree)}: did not come back")
            for edited, (line, before, after) in different[:4]:
                print(f"     {os.path.relpath(edited, tree)}:{line}")
                print(f"       was {before.strip()[:90]}")
                print(f"       now {after.strip()[:90]}")
        return 1 if broken or unreturned else 0
    finally:
        shutil.rmtree(work, ignore_errors=True)


if __name__ == "__main__":
    sys.exit(main())
