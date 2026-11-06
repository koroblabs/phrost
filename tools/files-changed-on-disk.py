#!/usr/bin/env python3
"""Whether a server told a file changed on disk answers like one that started after the change.

    python3 tools/files-changed-on-disk.py <project-root> [--files 14]

A branch switch, a `composer install`, a code generator running: the editor touched none of those
files, so the only thing the server hears about them is `workspace/didChangeWatchedFiles`. Nothing
else here measures that path -- `edit_fuzz` edits a skeleton in process and
`edits-answer-like-a-fresh-open` edits through open buffers -- and a wrong answer from it survives
until somebody restarts, which is the kind nobody traces back to the change that caused it.

Four changes are made to a **copy** of the project, each followed by the notification an editor
would send and then compared against a server started fresh on the same tree:

* a file **created**, declaring a class the project did not have;
* a method **added** to it, which nothing could have known about before;
* the method **removed**, which must stop being offered;
* the file **deleted**, whose class must stop being found at all.

The subject is written by this tool rather than picked out of the project, so that "created" is a
real creation and every answer has a known shape. The file it is asked *about* from is an ordinary
open buffer, because that is where a person would see the staleness. Nothing is written to the
project given on the command line: the tree is copied first.
"""
import argparse
import importlib.util
import json
import os
import re
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

REPO = os.path.dirname(HERE)
STILL_INDEXING = -32801
CREATED, CHANGED, DELETED = 1, 2, 3
SUBJECT, ASKER = "ZzWatchedSubject", "ZzWatchedAsker"

CAPABILITIES = {
    "window": {"workDoneProgress": True},
    "workspace": {
        "workspaceFolders": True,
        "didChangeWatchedFiles": {"dynamicRegistration": False},
    },
    "textDocument": {
        "synchronization": {"didSave": True},
        "definition": {"linkSupport": False},
        "completion": {"contextSupport": True},
    },
}


def read(path):
    with open(path, encoding="utf-8", errors="replace") as handle:
        return handle.read()


def write(path, text):
    with open(path, "w", encoding="utf-8") as handle:
        handle.write(text)


def clone_tree(source, destination):
    """The project, without its `.git`, copied cheaply enough to do on a gigabyte checkout.

    `.git` is left behind because nothing a server reads is in it and a repository with a running
    fsmonitor holds a socket, which a copy cannot carry. `cp -c` clones on APFS, which turns a
    minute into a moment; anywhere it is not understood the same command without it is a copy.
    """
    os.makedirs(destination)
    for name in sorted(os.listdir(source)):
        if name == ".git":
            continue
        here = os.path.join(source, name)
        for flags in ("-Rpc", "-Rp"):
            done = subprocess.run(
                ["cp", flags, here, destination + os.sep],
                capture_output=True,
            )
            if done.returncode == 0:
                break


class Probe:
    """One server, and the three questions asked of it after every change."""

    def __init__(self, root, name, asker):
        self.root = os.path.abspath(root)
        self.asker = asker
        self.ready = None
        self.working = set()
        self.opened = set()
        os.makedirs(os.path.join(REPO, ".phrost-cache"), exist_ok=True)
        self.server = Server(
            "phrost",
            [os.path.join(REPO, "target/release/phrost"), "serve"],
            self.root,
            log=os.path.join(REPO, ".phrost-cache", f"watched-{name}.stderr.log"),
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

    def start(self, anchor_path, anchor_name):
        self.server.initialize(CAPABILITIES)
        text = read(anchor_path)
        at = compare.mask(text).find(anchor_name)
        self.server.open(anchor_path, text)
        self.opened.add(anchor_path)
        deadline = time.monotonic() + 600
        while time.monotonic() < deadline:
            self.server.pump(0.5)
            if self.ready is None or self.working:
                continue
            if self.ask("textDocument/definition", {
                "textDocument": {"uri": uri_of(anchor_path)},
                "position": compare.Lines(text).position(at),
            }):
                self.open(self.asker)
                return True
        return False

    def open(self, path):
        if path not in self.opened:
            self.opened.add(path)
            self.server.open(path, read(path))

    def watched(self, changes):
        self.server.notify(
            "workspace/didChangeWatchedFiles",
            {"changes": [{"uri": uri_of(path), "type": kind} for path, kind in changes]},
        )
        # The handler applies every change before it answers anything else, so a round trip is the
        # wait. Two, because a configuration change hands the reindex to a background pass.
        for _ in range(2):
            self.ask("textDocument/documentSymbol", {
                "textDocument": {"uri": uri_of(self.asker)}
            })
        self.server.pump(0.5)

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

    def answers(self, asker_text):
        """Where the subject's name goes, what it offers, and whether the project lists it."""
        lines = compare.Lines(asker_text)
        masked = compare.mask(asker_text)
        where = self.ask("textDocument/definition", {
            "textDocument": {"uri": uri_of(self.asker)},
            "position": lines.position(masked.index(SUBJECT) + 2),
        })
        return {
            "goto the class": self._locations(where),
            "members offered": self._members(lines.position(masked.index("->") + 2)),
            "workspace symbol": {
                one.get("name", "").split("\\")[-1]
                for one in (self.ask("workspace/symbol", {"query": SUBJECT}) or [])
            },
        }

    def _locations(self, where):
        found = where if isinstance(where, list) else ([where] if where else [])
        return {
            os.path.relpath(path_of(one["uri"]), self.root) + f":{one['range']['start']['line']}"
            for one in found
            if isinstance(one, dict) and "uri" in one
        }

    def _members(self, cursor):
        result = self.ask("textDocument/completion", {
            "textDocument": {"uri": uri_of(self.asker)},
            "position": cursor,
            "context": {"triggerKind": 1},
        })
        items = result if isinstance(result, list) else (result or {}).get("items") or []
        offered = {
            (item.get("insertText") or item.get("label") or "").split("(")[0].strip()
            for item in items
        }
        # Only the subject's own members: what a project offers everywhere else is not the question,
        # and a magic-method list differs between two servers for reasons this probe does not own.
        return {name for name in offered if name.startswith("zz")}


NAMESPACE = re.compile(r"^\s*namespace\s+([^;{]+)[;{]", re.MULTILINE)


def a_user_file(root, limit):
    """A file of the project's own code, with the namespace its directory is mapped to."""
    for path in reversed(compare.php_files(root, "", limit)):
        text = read(path)
        if compare.DECLARED_CLASS.search(compare.mask(text)):
            found = NAMESPACE.search(compare.mask(text))
            return path, (found.group(1).strip() if found else "")
    return None


def subject_text(namespace, methods):
    prelude = f"namespace {namespace};\n\n" if namespace else ""
    body = "".join(f"    public function {name}(): void {{}}\n" for name in methods)
    return f"<?php\n\n{prelude}class {SUBJECT}\n{{\n{body}}}\n"


def asker_text(namespace):
    prelude = f"namespace {namespace};\n\n" if namespace else ""
    return (
        f"<?php\n\n{prelude}class {ASKER}\n{{\n"
        f"    public function go({SUBJECT} $it): void\n    {{\n"
        f"        $it->zzAlpha();\n    }}\n}}\n"
    )


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("root")
    parser.add_argument("--files", type=int, default=14)
    args = parser.parse_args()

    chosen = a_user_file(args.root, args.files)
    if chosen is None:
        print("no file in the sample declares a class")
        return 1
    neighbour, namespace = chosen

    work = tempfile.mkdtemp(prefix="phrost-watched-")
    try:
        tree = os.path.join(work, "tree")
        clone_tree(args.root, tree)
        here = os.path.join(tree, os.path.dirname(os.path.relpath(neighbour, args.root)))
        subject = os.path.join(here, f"{SUBJECT}.php")
        asker = os.path.join(here, f"{ASKER}.php")
        write(asker, asker_text(namespace))
        anchor = os.path.join(tree, os.path.relpath(neighbour, args.root))
        anchor_name = compare.DECLARED_CLASS.search(compare.mask(read(neighbour))).group(1)

        print(f"the subject is {os.path.relpath(subject, tree)}", flush=True)
        running = Probe(tree, "running", asker)
        if not running.start(anchor, anchor_name):
            print("the server never answered about the project")
            return 1

        steps = [
            ("the file created", ["zzAlpha"], CREATED),
            ("a method added", ["zzAlpha", "zzBeta"], CHANGED),
            ("the method removed", ["zzAlpha"], CHANGED),
            ("the file deleted", None, DELETED),
        ]
        text = read(asker)
        wrong = []
        for label, methods, kind in steps:
            if methods is None:
                os.remove(subject)
            else:
                write(subject, subject_text(namespace, methods))
            running.watched([(subject, kind)])

            fresh = Probe(tree, "fresh", asker)
            if not fresh.start(anchor, anchor_name):
                print("the fresh server never answered about the project")
                return 1
            mine, theirs = running.answers(text), fresh.answers(text)
            fresh.server.stop()

            for question in sorted(mine):
                if mine[question] != theirs[question]:
                    wrong.append((
                        label,
                        question,
                        sorted(mine[question] - theirs[question]),
                        sorted(theirs[question] - mine[question]),
                    ))
        running.server.stop()

        asked = len(steps) * 3
        print()
        print(f"after a change on disk  {asked - len(wrong)} of {asked} answers match a fresh server")
        for label, question, only_running, only_fresh in wrong:
            print(f"  {label}: {question}")
            if only_running:
                print(f"     told about it : {only_running[:6]}")
            if only_fresh:
                print(f"     started after : {only_fresh[:6]}")
        return 1 if wrong else 0
    finally:
        shutil.rmtree(work, ignore_errors=True)


if __name__ == "__main__":
    sys.exit(main())
