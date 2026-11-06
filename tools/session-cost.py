#!/usr/bin/env python3
"""What one editing session costs, phase by phase, for two servers on one project.

    python3 tools/session-cost.py <project-root> --servers phrost,phpantom

`compare-lsp.py` answers *what a server knows*. This answers *what it costs to know it*, which is
a different question and needs a different shape of measurement: two numbers at the ends of a run
cannot tell a server that finished indexing from one that is still going, and the difference is
most of what a memory comparison is about.

So the run is phases, each with a name, and every phase samples memory and processor time rather
than reading them once at its end:

  ready     start to the first correct answer about a class this project declares
  settle    idle, asked nothing -- what a server is still doing after it says it is ready
  open      N files opened as an editor restoring its tabs would, and nothing else
  work      one hover and one completion in each of them
  rest      idle again -- whether what the work cost is given back

Two properties make the numbers worth believing. Every server is asked the same questions in the
same order, one at a time, because two indexers on one machine each make the other look slow. And
memory is read from `ps` rather than from the server, since that is the only form of the question
every server answers the same way.
"""
import argparse
import os
import re
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from lspclient import (  # noqa: E402
    Closed,
    Server,
    Timeout,
    is_dependency_dir,
    path_of,
    uri_of,
)

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SIBLINGS = os.path.dirname(REPO)
CACHE = os.path.expanduser("~/.cache/phrost-compare")

PRESETS = {
    "phrost": [os.path.join(REPO, "target/release/phrost"), "serve"],
    "intelephense": ["intelephense", "--stdio"],
    "phpantom": [os.path.join(SIBLINGS, "phpantom_lsp/target/release/phpantom_lsp"), "--stdio"],
}

SKIP_DIRS = {
    ".git", "node_modules", "vendor", "target", ".idea", "dist", "build", "__pycache__",
    ".phpstan", ".psalm", ".phpunit.cache", ".cache", "coverage",
}

CAPABILITIES = {
    "window": {"workDoneProgress": True},
    "workspace": {"workspaceFolders": True, "symbol": {"dynamicRegistration": False}},
    "textDocument": {
        "synchronization": {"didSave": True, "willSave": False},
        "definition": {"linkSupport": True},
        "hover": {"contentFormat": ["markdown", "plaintext"]},
        "completion": {"completionItem": {"snippetSupport": True}, "contextSupport": True},
        "publishDiagnostics": {"relatedInformation": True},
        "documentSymbol": {"hierarchicalDocumentSymbolSupport": True},
    },
}

DECLARED_CLASS = re.compile(
    r"^(?:final\s+|abstract\s+|readonly\s+)*(?:class|interface|trait|enum)\s+([A-Za-z_]\w*)",
    re.MULTILINE,
)
STILL_INDEXING = -32801


class Phase:
    """What one named stretch of a session cost.

    Processor time as the phase's *own* share rather than the running total, so an idle phase reads
    as the near-zero it is instead of inheriting what indexing spent.
    """

    def __init__(self, name, cpu_before):
        self.name = name
        self.seconds = 0.0
        self.peak = 0.0
        self.ended = 0.0
        self.cpu = 0.0
        self.before = cpu_before or 0.0
        # When memory last climbed by more than a megabyte, as seconds into the phase. In `settle`,
        # where nothing is asked, this is the answer to "when is the server actually ready" -- which
        # is not what it says about itself: one of the two measured here reports readiness at a
        # second and then quadruples its memory over the following minute, unasked.
        self.grew_until = 0.0


class Watched:
    """A server, driven through the phases, sampled while it works.

    Sampling happens inside the waits rather than around them, so a phase that is mostly idle is
    still measured throughout it -- which is the whole point of `settle`, where nothing is asked
    and the interesting number is whether memory keeps climbing anyway.
    """

    def __init__(self, name, argv, root):
        self.name = name
        self.phases = []
        self.ready_signal = None
        self.working = set()
        self.diagnostics = {}
        os.makedirs(CACHE, exist_ok=True)
        self.server = Server(name, argv, root, log=os.path.join(CACHE, f"{name}.stderr.log"))
        self.server.on_notification(self._heard)

    def _heard(self, method, params):
        if method == "textDocument/publishDiagnostics":
            self.diagnostics[path_of(params.get("uri", ""))] = params.get("diagnostics") or []
        elif method == "$/progress":
            kind = (params.get("value") or {}).get("kind")
            token = str(params.get("token"))
            if kind == "begin":
                self.working.add(token)
            elif kind == "end":
                self.working.discard(token)

    # -- phases ---------------------------------------------------------------------------------

    def begin(self, name):
        # Seeded with where the last phase left memory, so the first sample of a quiet phase is not
        # read as growth from zero -- which would put a `grew until` on every phase alike and make
        # the one phase the number exists for say nothing.
        carried = self.phases[-1].ended if self.phases else 0.0
        self.current = Phase(name, self.server.cpu_seconds())
        self.current.ended = carried
        self.current.peak = carried
        self.phases.append(self.current)
        self.started = time.monotonic()

    def sample(self):
        elapsed = time.monotonic() - self.started
        rss = self.server.rss_mb()
        if rss is not None:
            if rss > self.current.ended + 1.0:
                self.current.grew_until = elapsed
            self.current.peak = max(self.current.peak, rss)
            self.current.ended = rss
        cpu = self.server.cpu_seconds()
        if cpu is not None:
            self.current.cpu = round(cpu - self.current.before, 1)
        self.current.seconds = time.monotonic() - self.started

    def idle(self, seconds):
        """Waits, reading whatever the server sends, and samples once a second while doing it."""
        until = time.monotonic() + seconds
        while time.monotonic() < until:
            self.server.poll(1.0)
            self.sample()

    def ready(self, probe, warmup):
        """Until the server answers correctly about a class the project declares.

        Its own progress messages are not enough: they arrive in phases, and one phase can be a
        background pass that never ends inside a run. A question whose answer is already known is
        the only thing that settles it.
        """
        self.begin("ready")
        self.server.initialize(CAPABILITIES)
        self.sample()
        path, name = probe
        until = time.monotonic() + warmup
        while time.monotonic() < until:
            answer = self.server.request("workspace/symbol", {"query": name}, timeout=60.0)
            self.sample()
            for hit in answer.get("result") or []:
                found = (hit.get("location") or {}).get("uri", "")
                if os.path.basename(path_of(found)) == os.path.basename(path):
                    return True
            self.server.poll(0.5)
        return False

    def open_all(self, plan):
        self.begin("open")
        for path, text in plan.items():
            self.server.open(path, text)
            self.server.poll(0.05)
            self.sample()

    def work(self, plan):
        """One hover and one completion in each file, at a position chosen from the text."""
        self.begin("work")
        for path, text in plan.items():
            at = first_member(text)
            if at is None:
                continue
            uri = uri_of(path)
            where = {"textDocument": {"uri": uri}, "position": at[0]}
            self._ask("textDocument/hover", where)
            self._ask(
                "textDocument/completion",
                {
                    "textDocument": {"uri": uri},
                    "position": at[1],
                    "context": {"triggerKind": 1},
                },
            )
            self.sample()

    def _ask(self, method, params):
        for _ in range(40):
            try:
                answer = self.server.request(method, params, timeout=30.0)
            except (Timeout, Closed):
                return None
            if (answer.get("error") or {}).get("code") == STILL_INDEXING:
                time.sleep(0.5)
                continue
            return answer.get("result")
        return None

    def stop(self):
        self.server.stop()


def first_member(text):
    """A `->name` in the file, as an (identifier, after-the-arrow) pair of LSP positions."""
    for number, line in enumerate(text.split("\n")):
        found = re.search(r"->([A-Za-z_]\w*)", line)
        if found and not line.lstrip().startswith(("*", "//", "#")):
            return (
                {"line": number, "character": found.start(1) + 1},
                {"line": number, "character": found.start(1)},
            )
    return None


def php_files(root, limit):
    paths = []
    for directory, subdirs, names in os.walk(root):
        # Not `SKIP_DIRS` alone: a package manager's directory is not always `vendor/`, and
        # sampling Drupal's core made three instruments measure the wrong project. See
        # `lspclient.is_dependency_dir`.
        subdirs[:] = sorted(
            name
            for name in subdirs
            if name not in SKIP_DIRS
            and not is_dependency_dir(os.path.join(directory, name), root)
        )
        for name in sorted(names):
            if name.endswith(".php"):
                paths.append(os.path.join(directory, name))
    paths.sort()
    if len(paths) <= limit:
        return paths
    step = len(paths) / limit
    return [paths[int(index * step)] for index in range(limit)]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("root", nargs="?", default=".")
    parser.add_argument("--servers", default="phrost,phpantom")
    parser.add_argument("--cmd", action="append", default=[], help="name=command, for one not preset")
    parser.add_argument("--files", type=int, default=20, help="how many files the session opens")
    parser.add_argument("--settle", type=float, default=60.0, help="idle seconds after ready")
    parser.add_argument("--rest", type=float, default=30.0, help="idle seconds after the work")
    parser.add_argument("--warmup", type=float, default=900.0)
    args = parser.parse_args()
    sys.stdout.reconfigure(line_buffering=True)

    root = os.path.abspath(args.root)
    if not os.path.isdir(root):
        sys.exit(f"no such project root: {root}")

    overrides = dict(entry.split("=", 1) for entry in args.cmd)
    files = php_files(root, args.files)
    plan = {}
    for path in files:
        with open(path, encoding="utf-8", errors="replace") as handle:
            plan[path] = handle.read()

    probe = None
    for path, text in reversed(list(plan.items())):
        found = DECLARED_CLASS.search(text)
        if found:
            probe = (path, found.group(1))
            break
    if probe is None:
        sys.exit("no class to probe with in the sample")

    print(f"session-cost   project: {root}")
    print(f"files opened: {len(plan)}   probe: {probe[1]}")

    watched = []
    for name in args.servers.split(","):
        argv = overrides[name].split() if name in overrides else PRESETS.get(name)
        if argv is None:
            sys.exit(f"unknown server {name!r}; known: {', '.join(PRESETS)} (or pass --cmd)")
        if os.path.isabs(argv[0]) and not os.path.exists(argv[0]):
            sys.exit(f"{name} is not built: {argv[0]}")

        # One at a time, whole session each: two indexers on one machine each make the other slow.
        print(f"\n{name}")
        session = Watched(name, argv, root)
        if not session.ready(probe, args.warmup):
            print(f"  NOT READY after {args.warmup:.0f}s -- treat these numbers as suspect")
        session.sample()
        session.begin("settle")
        session.idle(args.settle)
        session.open_all(plan)
        session.work(plan)
        session.begin("rest")
        session.idle(args.rest)
        session.stop()
        watched.append(session)
        report(session)

    if len(watched) > 1:
        compare(watched)


def report(session):
    print(f"  {'phase':<8}{'seconds':>9}{'cpu s':>9}{'peak MB':>10}{'end MB':>9}{'grew until':>12}")
    for phase in session.phases:
        print(
            f"  {phase.name:<8}{phase.seconds:>9.1f}{phase.cpu:>9.1f}"
            f"{phase.peak:>10.1f}{phase.ended:>9.1f}{phase.grew_until:>12.1f}"
        )
    findings = sum(len(items) for items in session.diagnostics.values())
    print(f"  {len(session.diagnostics)} files diagnosed, {findings} findings")


def compare(watched):
    def of(session, name):
        for phase in session.phases:
            if phase.name == name:
                return phase
        return Phase(name, 0.0)

    print("\nside by side")
    width = max(len(session.name) for session in watched)
    print(
        f"  {'':<{width}}{'says ready':>12}{'truly quiet':>13}{'total cpu':>11}"
        f"{'idle MB':>9}{'open cost':>11}{'end MB':>9}"
    )
    for session in watched:
        settle, opened, rest = of(session, "settle"), of(session, "open"), of(session, "rest")
        ready = of(session, "ready")
        cpu = sum(phase.cpu for phase in session.phases)
        # What the server said, and what it was still doing afterwards. A server whose memory keeps
        # climbing through a minute of being asked nothing was not ready when it said so, and the
        # sum of the two is the number an editor's user waits through.
        print(
            f"  {session.name:<{width}}{ready.seconds:>12.1f}"
            f"{ready.seconds + settle.grew_until:>13.1f}{cpu:>11.1f}"
            f"{settle.ended:>9.1f}{opened.ended - settle.ended:>+11.1f}{rest.ended:>9.1f}"
        )


if __name__ == "__main__":
    main()
