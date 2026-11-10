#!/usr/bin/env python3
"""Whether a name typed halfway is shown by an editor, and whether accepting it writes the name.

    python3 tools/a-half-typed-name-is-shown-and-accepted.py <project-root> [--files 40]
                                                               [--limit 600] [--verbose]

Every name a file already writes is cut in half, the buffer is sent that way, and completion is
asked at the cut -- which is what an editor does on every keystroke, and what no other tool here
does: `completion-offers-what-is-written` asks right after the `->` with the file whole. Three
questions, and none needs an expectation written down, because the code says what the answer is:

* **offered** -- the written name is among the items. Counted, and not judged: a local at the
  place it is first bound and a method on a receiver nothing can type are not offered and should
  not be.
* **shown** -- the client keeps the item. A client filters the list by the word at the cursor
  against the item's `filterText`, or its label, by *its own* idea of a word. Zed's PHP language
  counts `$` in one. `$ap` answered with `$app` under `filterText: "app"` was dropped by the editor
  from a list the server had answered correctly, for every variable, in three releases.
* **accepted** -- applying the item's edit to the half-typed line writes the name that was there.
  A range that leaves the typed `$` standing writes `use ($$alpha)`; one that stops short of a
  namespace writes it twice.

The second is reported under two rules, because clients differ and the server cannot ask which it
is talking to: by the word (Zed), and by the text of the range the edit replaces (VS Code, and the
completion plugins that follow it). An item the first rule drops is a bug in an editor this project
ships an extension for. One only the second drops is recorded and listed apart.

Only what is offered is judged, so a miss here is completion's own and not a type the project
failed to state. And only the declaration the file means: where a `use` line names the class and
the index does not hold it -- a checkout with no `vendor/` -- the rows carrying that label are
namesakes, written with their whole path because the name is taken, and judging one of them
reports the server for being right.
"""
import argparse
import importlib.util
import json
import os
import re
import sys
import time
from collections import Counter, defaultdict

sys.dont_write_bytecode = True
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
STILL_INDEXING = -32801

# What a real client says it can do, snippets included: an item is inserted the way it is sent.
CAPABILITIES = {
    "window": {"workDoneProgress": True},
    "workspace": {"workspaceFolders": True},
    "textDocument": {
        "synchronization": {"didSave": True},
        "definition": {"linkSupport": False},
        "completion": {
            "completionItem": {"snippetSupport": True, "labelDetailsSupport": True},
            "contextSupport": True,
        },
    },
}

# The kinds the shared scanner finds whose name can be cut and asked for. A `name` and a
# `constant` are left out: a capitalised word in masked text is as often a type in a signature or
# a docblock's neighbour as a reference, and what is offered there is a different list.
KINDS = ("var", "member", "static", "function", "new")


def subsequence(needle, hay):
    """Whether every character of `needle` appears in `hay` in order, case aside -- the least any
    fuzzy matcher asks, so a pair that fails this is dropped by all of them."""
    rest = iter(hay.lower())
    return all(ch in rest for ch in needle.lower())


def word_before(text, extra):
    """The word a client reads at the end of `text`: letters, digits, `_`, and what the language
    adds. Zed's PHP adds `$`."""
    at = len(text)
    while at > 0 and (text[at - 1].isalnum() or text[at - 1] == "_" or text[at - 1] in extra):
        at -= 1
    return text[at:]


def plain(snippet):
    """A snippet as the text it leaves: placeholders by their defaults, tab stops gone."""
    text = re.sub(r"\$\{\d+:((?:[^{}\\]|\\.)*)\}", r"\1", snippet)
    text = re.sub(r"\$\{\d+\}|\$\d+", "", text)
    return re.sub(r"\\([$}\\])", r"\1", text)


class Session:
    def __init__(self, root):
        self.root = os.path.abspath(root)
        self.ready = None
        self.working = set()
        self.versions = {}
        os.makedirs(os.path.join(REPO, ".phrost-cache"), exist_ok=True)
        self.server = Server(
            "phrost",
            [os.path.join(REPO, "target/release/phrost"), "serve"],
            self.root,
            log=os.path.join(REPO, ".phrost-cache", "half-typed.stderr.log"),
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

    def start(self, probe_path, probe_name, text):
        self.server.initialize(CAPABILITIES)
        lines = compare.Lines(text)
        at = compare.mask(text).find(probe_name)
        self.show(probe_path, text)
        deadline = time.monotonic() + 600
        while time.monotonic() < deadline:
            self.server.pump(0.5)
            if self.ready is None or self.working:
                continue
            if self.ask("textDocument/definition", probe_path, lines.position(at)):
                return True
        return False

    def show(self, path, text):
        """The buffer as it now reads: opened the first time, changed after."""
        version = self.versions.get(path, 0) + 1
        self.versions[path] = version
        if version == 1:
            self.server.open(path, text, version=version)
            return
        self.server.notify(
            "textDocument/didChange",
            {
                "textDocument": {"uri": uri_of(path), "version": version},
                "contentChanges": [{"text": text}],
            },
        )

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


def written_names(text, masked):
    """Each name worth cutting: its kind, where the whole of it starts and ends, and how it reads
    in a label. The whole includes a `$`, which is part of what a person types."""
    found = []
    for site in compare.sites_of(masked):
        if site.kind not in KINDS:
            continue
        # The scanner hands back a position *inside* the name; walk out to both ends.
        start = site.offset
        while start > 0 and (text[start - 1].isalnum() or text[start - 1] == "_"):
            start -= 1
        if start > 0 and text[start - 1] == "$":
            start -= 1
        end = site.offset
        while end < len(text) and (text[end].isalnum() or text[end] == "_"):
            end += 1
        name = text[start:end]
        if len(name.lstrip("$")) < 3:
            continue
        found.append((site.kind, start, end, name))
    return found


IMPORT = re.compile(r"^use\s+(?:function\s+|const\s+)?\\?([\w\\]+?)(?:\s+as\s+(\w+))?\s*;", re.M)


def imports_of(text):
    """What each `use` line of a file binds: the local name to the path it names."""
    return {
        (match.group(2) or match.group(1).rsplit("\\", 1)[-1]).lower(): match.group(1)
        for match in IMPORT.finditer(text)
    }


def label_names(item):
    """What an item's label can be read as: itself, and itself without a signature or a colon."""
    label = item.get("label") or ""
    return {label, label.split("(")[0], label.rstrip(":"), label.lstrip("$")}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("root")
    parser.add_argument("--limit", type=int, default=600)
    parser.add_argument("--files", type=int, default=40)
    parser.add_argument("--needle", default="")
    parser.add_argument("--verbose", action="store_true")
    args = parser.parse_args()

    paths = compare.php_files(args.root, args.needle, args.files)
    texts = {}
    for path in paths:
        with open(path, encoding="utf-8", errors="replace") as handle:
            texts[path] = handle.read()
    probe = compare.probe_of({path: (text, None) for path, text in texts.items()})
    if probe is None:
        print("no class declared in the sample to probe readiness with")
        return 1

    session = Session(args.root)
    print(f"starting server over {args.root}", flush=True)
    if not session.start(probe[0], probe[1], texts[probe[0]]):
        print("server never answered about the project")
        return 1

    sites = []
    for path, text in texts.items():
        for kind, start, end, name in written_names(text, compare.mask(text)):
            sites.append((path, kind, start, end, name))
    sites = compare.stride(sites, args.limit)

    asked = Counter()
    offered = Counter()
    dropped_by_word = defaultdict(list)
    dropped_by_range = defaultdict(list)
    miswritten = defaultdict(list)

    imports = {path: imports_of(text) for path, text in texts.items()}

    for path, kind, start, end, name in sites:
        text = texts[path]
        sigil = 1 if name.startswith("$") else 0
        keep = sigil + max(1, (len(name) - sigil) // 2)
        cut = start + keep
        half = text[:cut] + text[end:]
        session.show(path, half)
        position = compare.Lines(half).position(cut)
        result = session.ask(
            "textDocument/completion", path, position, {"context": {"triggerKind": 1}}
        )
        asked[kind] += 1
        items = (result.get("items") if isinstance(result, dict) else result) or []
        bare = name.lstrip("$")
        hits = [item for item in items if name in label_names(item) or bare in label_names(item)]
        # The class the file's own `use` line names, where it names one: any other row with the
        # label is a namesake.
        imported = imports[path].get(bare.lower()) if kind == "new" else None
        if imported:
            hits = [item for item in hits if (item.get("detail") or "").lstrip("\\") == imported]
        if not hits:
            continue
        offered[kind] += 1
        item = hits[0]

        line_start = half.rfind("\n", 0, cut) + 1
        line_end = half.find("\n", cut)
        line_end = len(half) if line_end < 0 else line_end
        line = half[line_start:line_end]
        column = cut - line_start
        where = f"{os.path.relpath(path, args.root)}:{position['line'] + 1}"
        candidate = item.get("filterText") or item.get("label") or ""

        word = word_before(line[:column], "$")
        if not subsequence(word, candidate):
            dropped_by_word[kind].append(f"{where}  word {word!r} against {candidate!r}")

        edit = item.get("textEdit") or {}
        span = edit.get("range") or edit.get("replace")
        if span and span["start"]["line"] == position["line"]:
            from_column = span["start"]["character"]
            to_column = span["end"]["character"]
            if not subsequence(line[from_column:column], candidate):
                dropped_by_range[kind].append(
                    f"{where}  range {line[from_column:column]!r} against {candidate!r}"
                )
            new = edit.get("newText") or ""
            if item.get("insertTextFormat") == 2:
                new = plain(new)
            after = line[:from_column] + new + line[to_column:]
            # What was written, from where the name began: the edit may add a call's brackets after
            # it, and may not change what stands before or in it.
            name_column = start - line_start
            was = text[line_start:end]
            if not after.startswith(was):
                miswritten[kind].append(f"{where}  {line.strip()!r} -> {after.strip()!r}")

    session.server.stop()

    total = sum(asked.values())
    print(f"\nasked at {total} half-typed names over {len(paths)} files")
    print(f"{'kind':<10}{'asked':>7}{'offered':>9}{'hidden (word)':>15}{'hidden (range)':>16}{'miswritten':>12}")
    for kind in KINDS:
        if not asked[kind]:
            continue
        print(
            f"{kind:<10}{asked[kind]:>7}{offered[kind]:>9}{len(dropped_by_word[kind]):>15}"
            f"{len(dropped_by_range[kind]):>16}{len(miswritten[kind]):>12}"
        )

    failed = 0
    for title, found, fatal in (
        ("offered and hidden by a client that filters by the word (Zed)", dropped_by_word, True),
        ("accepting does not write what was there", miswritten, True),
        ("offered and hidden by a client that filters by the replaced range", dropped_by_range, False),
    ):
        count = sum(len(rows) for rows in found.values())
        if not count:
            continue
        failed += count if fatal else 0
        print(f"\n{title}: {count}")
        for kind, rows in found.items():
            shown = rows if args.verbose else rows[:6]
            for row in shown:
                print(f"  [{kind}] {row}")
            if len(rows) > len(shown):
                print(f"  [{kind}] ... and {len(rows) - len(shown)} more")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
