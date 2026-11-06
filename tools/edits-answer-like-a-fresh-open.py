#!/usr/bin/env python3
"""Whether a file edited into a state answers like a file opened at that state.

    python3 tools/edits-answer-like-a-fresh-open.py <project-root> [--files 12]

`incremental == from-scratch` is checked at the index level by `edit_fuzz`, over a skeleton built
in-process. This asks the same question of the **server**, through the protocol an editor uses: open
a file, type into it one `didChange` at a time, and then compare what it answers against a server
that opened the same text cold. Everything between the two -- the document store, the version
guards, which buffers count as open, the analysis a request runs over -- is inside what this
measures and outside what the index-level sweep can see.

Two things are asked after every edit, not only at the end:

* **Diagnostics stay inside the file.** A range past the end of the buffer is a stale answer about
  text that is no longer there, and a client draws it wherever it lands.
* **The server keeps answering.** A request that stops coming back is the failure a person
  experiences as the editor going quiet.

And one at the end, which is the invariant: the diagnostics after the edits equal the diagnostics a
fresh server gives for the same text.
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

REPO = os.path.dirname(HERE)
STILL_INDEXING = -32801

CAPABILITIES = {
    "window": {"workDoneProgress": True},
    "workspace": {"workspaceFolders": True},
    "textDocument": {
        "synchronization": {"didSave": True},
        "publishDiagnostics": {"relatedInformation": False},
        "definition": {"linkSupport": False},
        "hover": {"contentFormat": ["markdown"]},
        "completion": {"contextSupport": True},
    },
}

# What typing looks like, as whole-document replacements: the protocol's full sync, which is what
# this server declares. Each step is a small edit an author actually makes.
def typed(body):
    """A handful of states one file passes through while somebody works on it."""
    lines = body.split("\n")
    at = next((index for index, line in enumerate(lines) if line.strip().endswith("{")), 0)
    states = []
    # A half-written member access, which is what a completion request arrives on.
    states.append("\n".join(lines[: at + 1] + ["        $this->"] + lines[at + 1 :]))
    # The same line finished into something that parses.
    states.append("\n".join(lines[: at + 1] + ["        $x = 1;"] + lines[at + 1 :]))
    # A brace deleted, which is the commonest way a buffer stops parsing mid-edit.
    states.append(body.replace("{", "", 1))
    # A whole declaration pasted in.
    states.append(
        "\n".join(lines[: at + 1] + ["    public function zzAdded(): void {}"] + lines[at + 1 :])
    )
    # And back to where it started.
    states.append(body)
    return states


class Probe:
    def __init__(self, root, name):
        self.root = os.path.abspath(root)
        self.ready = None
        self.working = set()
        self.diagnostics = {}
        os.makedirs(os.path.join(REPO, ".phrost-cache"), exist_ok=True)
        self.server = Server(
            "phrost",
            [os.path.join(REPO, "target/release/phrost"), "serve"],
            self.root,
            log=os.path.join(REPO, ".phrost-cache", f"{name}.stderr.log"),
        )
        self.server.on_notification(self._heard)

    def _heard(self, method, params):
        if method == "textDocument/publishDiagnostics":
            # Keyed by the version the server stamped it with. A publication is asynchronous, so
            # one for the text before the last keystroke arrives after it -- and reading that as
            # the answer for the *current* buffer reports every range past the shorter text as a
            # diagnostic outside it, which is what the protocol's `version` is for.
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

    def start(self, probe_path, probe_name):
        self.server.initialize(CAPABILITIES)
        text = read(probe_path)
        lines = compare.Lines(text)
        at = compare.mask(text).find(probe_name)
        self.open(probe_path, text)
        deadline = time.monotonic() + 600
        while time.monotonic() < deadline:
            self.server.pump(0.5)
            if self.ready is None or self.working:
                continue
            if self.ask("textDocument/definition", {
                "textDocument": {"uri": uri_of(probe_path)},
                "position": lines.position(at),
            }):
                return True
        return False

    def open(self, path, text, version=1):
        self.server.open(path, text, version)

    def change(self, path, text, version):
        self.server.notify(
            "textDocument/didChange",
            {
                "textDocument": {"uri": uri_of(path), "version": version},
                "contentChanges": [{"text": text}],
            },
        )

    def close(self, path):
        self.server.close(path)

    def ask(self, method, params):
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

    def settled_diagnostics(self, path, text, version):
        """The diagnostics for the buffer as it stands, waited for rather than guessed at.

        A server publishes when the answer *changes*, so an edit that produces the same findings
        publishes nothing and the wait times out. Falling back to the last set it published is what
        an editor shows; reading the timeout as "no diagnostics" instead reported every such step
        as zero, which looked exactly like the bug this is hunting.
        """
        before = self.diagnostics.get(path)
        # A round trip the server answers only after it has processed the change before it.
        self.ask("textDocument/documentSymbol", {"textDocument": {"uri": uri_of(path)}})
        deadline = time.monotonic() + 15
        while time.monotonic() < deadline:
            held = self.diagnostics.get(path)
            if held is not None and held[0] == version:
                return held[1]
            self.server.pump(0.2)
        # Nothing new for this version: the findings did not change, so what the editor shows is
        # what it was last given.
        return (before or (None, []))[1]


def read(path):
    with open(path, encoding="utf-8", errors="replace") as handle:
        return handle.read()


def inside(text, span):
    rows = text.split("\n")
    for edge in ("start", "end"):
        position = span.get(edge) or {}
        line = position.get("line")
        if not isinstance(line, int) or line < 0 or line >= len(rows):
            return False
        width = len(rows[line].encode("utf-16-le")) // 2
        if not 0 <= position.get("character", -1) <= width:
            return False
    return True


def rendered(found):
    return sorted(
        f"{one['range']['start']['line']}:{one['range']['start']['character']} "
        f"{one.get('code')} {one.get('message')}"
        for one in found or []
    )


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("root")
    parser.add_argument("--files", type=int, default=12)
    parser.add_argument("--needle", default="")
    args = parser.parse_args()

    paths = compare.php_files(args.root, args.needle, args.files)
    plan = {path: read(path) for path in paths}
    probe = compare.probe_of({path: (text, None) for path, text in plan.items()})
    if probe is None:
        print("no class declared in the sample to probe readiness with")
        return 1

    edited = Probe(args.root, "edited")
    print(f"starting the server that is edited into shape over {args.root}", flush=True)
    if not edited.start(*probe):
        print("server never answered about the project")
        return 1

    asked = 0
    wrong = Counter()
    examples = {}
    final = {}
    for path, body in plan.items():
        edited.open(path, body)
        version = 1
        for state in typed(body):
            version += 1
            edited.change(path, state, version)
            asked += 1
            found = edited.settled_diagnostics(path, state, version)
            if found is None:
                wrong["the server stopped publishing diagnostics"] += 1
                examples.setdefault(
                    "the server stopped publishing diagnostics", os.path.relpath(path, args.root)
                )
                break
            outside = [one for one in found if not inside(state, one.get("range") or {})]
            if outside:
                wrong["a diagnostic outside the buffer"] += 1
                examples.setdefault(
                    "a diagnostic outside the buffer",
                    f"{os.path.relpath(path, args.root)}: {outside[0].get('message')}",
                )
        final[path] = edited.settled_diagnostics(path, body, version)
    edited.server.stop()

    # The same text, opened cold by a server that saw no edits.
    fresh = Probe(args.root, "fresh")
    if not fresh.start(*probe):
        print("the second server never answered about the project")
        return 1
    same = 0
    for path, body in plan.items():
        fresh.open(path, body)
        theirs = fresh.settled_diagnostics(path, body, 1)
        if rendered(final.get(path)) == rendered(theirs):
            same += 1
        else:
            wrong["edited and fresh disagree"] += 1
            mine = rendered(final.get(path))
            yours = rendered(theirs)
            only_mine = [one for one in mine if one not in yours]
            only_yours = [one for one in yours if one not in mine]
            examples.setdefault(
                "edited and fresh disagree",
                f"{os.path.relpath(path, args.root)}\n"
                + "".join(f"         edited only: {one}\n" for one in only_mine[:3])
                + "".join(f"         fresh only : {one}\n" for one in only_yours[:3]),
            )
    fresh.server.stop()

    print()
    print(f"after each edit     {asked - sum(wrong.values())} of {asked} answers well formed")
    print(f"edited == fresh     {same} of {len(plan)} files")
    for what, count in wrong.most_common():
        print(f"  {count:5}  {what}")
        print(f"         first in {examples[what]}")
    return 1 if wrong else 0


if __name__ == "__main__":
    sys.exit(main())
