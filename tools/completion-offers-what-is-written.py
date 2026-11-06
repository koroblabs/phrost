#!/usr/bin/env python3
"""Whether completion offers the member the code at that very position already writes.

    python3 tools/completion-offers-what-is-written.py <project-root> [--limit 400]

The invariant, like `navigation-roundtrip`'s, needs no expected output: at `$repo->find(1)`, if goto
on `find` answers a declaration, then completion asked right after the `->` must offer `find`. The
receiver is the same expression in both questions and the index has already proved it knows what it
is -- so a miss is completion's own, in the ranking, the cap or the filter, and not a type the
project failed to state.

Asked the other way round it would say nothing: completion offers members the file does not write
yet, which is the whole point of it.

Only sites goto answers are counted. A receiver nothing can type is a gap in inference, which
`phrost coverage` measures and this deliberately does not.
"""
import argparse
import importlib.util
import json
import os
import sys
import time
from collections import Counter, defaultdict

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
        "definition": {"linkSupport": False},
        "completion": {
            "completionItem": {"snippetSupport": False, "labelDetailsSupport": True},
            "contextSupport": True,
        },
    },
}

# `->` and `::` sites only: those are the ones with a receiver the index had to resolve, which is
# what makes the two questions the same question.
KINDS = ("member", "static")


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
            log=os.path.join(REPO, ".phrost-cache", "completion-probe.stderr.log"),
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
            if self.ask("textDocument/definition", probe_path, lines.position(at)):
                return True
        return False

    def open(self, path):
        if path not in self.opened:
            self.opened[path] = read(path)
            self.server.open(path, self.opened[path])
        return self.opened[path]

    def ask(self, method, path, position, extra=None):
        params = {"textDocument": {"uri": uri_of(path)}, "position": position}
        if extra:
            params.update(extra)
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


def labels(result):
    """The names a completion answer offers, with the decoration an editor strips before inserting."""
    if result is None:
        return None, False
    items = result if isinstance(result, list) else result.get("items") or []
    incomplete = isinstance(result, dict) and bool(result.get("isIncomplete"))
    out = set()
    for item in items:
        # What is actually inserted, where the server says so: a label may carry a signature or a
        # kind beside it, and comparing the decoration is how `compare-lsp` once reported a gap of
        # 968 labels that was 201.
        text = item.get("insertText") or item.get("filterText") or item.get("label") or ""
        if isinstance(item.get("textEdit"), dict):
            text = item["textEdit"].get("newText") or text
        out.add(text.split("(")[0].lstrip("$").strip())
    return out, incomplete


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("root")
    parser.add_argument("--limit", type=int, default=400)
    parser.add_argument("--files", type=int, default=40)
    parser.add_argument("--needle", default="")
    parser.add_argument("--verbose", action="store_true")
    args = parser.parse_args()

    paths = compare.php_files(args.root, args.needle, args.files)
    plan = {path: (read(path),) for path in paths}
    plan = {path: (text, compare.sites_of(compare.mask(text))) for path, (text,) in plan.items()}
    probe = compare.probe_of({path: (text, None) for path, (text, _) in plan.items()})
    if probe is None:
        print("no class declared in the sample to probe readiness with")
        return 1

    session = Probe(args.root)
    print(f"starting server over {args.root}", flush=True)
    if not session.start(*probe):
        print("server never answered about the project")
        return 1

    sites = []
    for path, (text, found) in plan.items():
        lines = compare.Lines(text)
        for site in found:
            if site.kind in KINDS and site.trigger is not None:
                sites.append((path, lines.position(site.offset), lines.position(site.trigger), site))
    sites = compare.stride(sites, args.limit)
    for path, _, _, _ in sites:
        session.open(path)

    asked = 0
    missing = []
    empty = Counter()
    incompletes = 0
    for path, at, trigger, site in sites:
        if not session.ask("textDocument/definition", path, at):
            continue
        asked += 1
        offered, incomplete = labels(
            session.ask(
                "textDocument/completion",
                path,
                trigger,
                {"context": {"triggerKind": 1}},
            )
        )
        incompletes += 1 if incomplete else 0
        if offered is None:
            empty["refused"] += 1
            continue
        if not offered:
            empty["nothing offered"] += 1
            missing.append((site, path, trigger, 0))
            continue
        # A static property is written `::$name` and offered as `$name`; the `$` is stripped from
        # both sides rather than from one, which is the half-normalisation `docs/08` records.
        if site.name.lstrip("$") not in offered:
            missing.append((site, path, trigger, len(offered)))

    session.server.stop()

    print()
    print(f"offered             {asked - len(missing)} of {asked} names the code already writes")
    for why, count in empty.most_common():
        print(f"  {why:18}{count:5}")
    if incompletes:
        print(f"  marked incomplete {incompletes:5}")
    if missing:
        print()
        print("missed, by receiver shape")
        for shape, count in Counter(site.shape.split("->")[0][:40] for site, *_ in missing).most_common(12):
            print(f"  {count:5}  {shape}")
        print()
        print("first fifteen")
        for site, path, trigger, offered in missing[:15]:
            rel = os.path.relpath(path, args.root)
            print(f"  {site.shape:52} {rel}:{trigger['line'] + 1} ({offered} offered)")
    return 1 if missing else 0


if __name__ == "__main__":
    sys.exit(main())
