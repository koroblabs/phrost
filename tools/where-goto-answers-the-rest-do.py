#!/usr/bin/env python3
"""Where goto answers at a word, completion, hover and references answer there too.

    python3 tools/where-goto-answers-the-rest-do.py <project-root> [--files 80] [--limit 8000]
                                                     [--dump misses.json]

Every word of the project's own files is asked about -- strings and comments included, which is the
difference from `completion-offers-what-is-written`, whose sites are read off masked text and so
never reached `[$this, 'validate']`, a `$name` in a `@param`, or a key in an array literal. Where
goto answers, the index has proved it knows what the word is, and three other questions about the
same word must answer as well:

- completion asked at the end of the word offers it -- what a person who typed it would be shown;
- hover describes it;
- find-references from it lists it -- or a rename from anywhere else leaves it behind.

A word goto answers *on itself* is a declaration and is left out: completing a name being declared
is not a question. CLAUDE.md §8.1 calls the shape this finds *two constructors of one rule*, and its
tell is the divergence: one half answers and the other does not. The report groups the misses by
what stands around the word -- in code, a string, or a docblock, and the characters either side --
because a group is a rule and a single site is an anecdote.

Run first on 2026-10-06, over the reference project: 618 completion misses before declarations were
left out, and eight bugs behind them -- a callable pair's method, an array key read as a global
class, a docblock's parameters, an interpolated variable at a string's boundary, namespaces, the
ranking that lost `time()`, docblock tags, and a rename that left `@param` behind.
"""
import argparse
import json
import importlib.util
import os
import re
import sys
from collections import Counter, defaultdict

TOOLS = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, TOOLS)


def _sibling(name):
    spec = importlib.util.spec_from_file_location(
        name.replace("-", "_"), os.path.join(TOOLS, f"{name}.py")
    )
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


compare = _sibling("compare-lsp")
oracle = _sibling("completion-offers-what-is-written")

WORD = re.compile(r"\$?[A-Za-z_][A-Za-z0-9_]*")
DECLARES = re.compile(r"(?:class|interface|trait|enum|function|const|namespace|case)\s+$", re.I)
SKIP = {"$this", "this", "self", "static", "parent", "true", "false", "null", "TRUE", "FALSE", "NULL",
        "class", "function", "return", "public", "private", "protected", "new", "array", "string",
        "int", "bool", "float", "void", "mixed", "if", "else", "foreach", "as", "use", "namespace"}


def regions(text):
    """One code per character: c code, s string, m comment."""
    out = ["c"] * len(text)
    length = len(text)
    at = 0
    while at < length:
        ch = text[at]
        nxt = text[at + 1] if at + 1 < length else ""
        if ch == "/" and nxt == "/" or (ch == "#" and nxt != "["):
            end = text.find("\n", at)
            end = length if end < 0 else end
            out[at:end] = ["m"] * (end - at)
            at = end
        elif ch == "/" and nxt == "*":
            end = text.find("*/", at + 2)
            end = length if end < 0 else end + 2
            out[at:end] = ["m"] * (end - at)
            at = end
        elif ch in "'\"":
            end = at + 1
            while end < length:
                if text[end] == "\\":
                    end += 2
                    continue
                if text[end] == ch:
                    end += 1
                    break
                end += 1
            out[at:end] = ["s"] * (end - at)
            at = end
        else:
            at += 1
    return out


def context_of(text, where, start, end):
    line_start = text.rfind("\n", 0, start) + 1
    before = text[line_start:start]
    after = text[end:end + 1]
    if where[start] == "s":
        # Inside a string: what stands before the opening quote says what the string is for.
        quote = start - 1
        while quote >= 0 and where[quote] == "s" and text[quote] not in "'\"":
            quote -= 1
        lead = text[line_start:max(line_start, quote)].rstrip()[-4:]
        inner = text[quote + 1:start]
        return f"str  {lead!r} q{inner[-3:]!r}…{after!r}"
    if where[start] == "m":
        tag = re.search(r"@[\w-]+[^@]*$", before)
        return f"doc  {(tag.group(0).split()[0] if tag else before.strip()[-6:])!r}"
    return f"code {before.rstrip()[-3:]!r}…{after!r}"


def matches(word, offered):
    word = word.lstrip("$")
    for text in offered:
        if text == word or text.endswith("\\" + word) or word in re.split(r"[^\w]+", text):
            return True
    return False


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("root")
    parser.add_argument("--files", type=int, default=80)
    parser.add_argument("--limit", type=int, default=8000)
    parser.add_argument("--needle", default="")
    parser.add_argument("--dump", help="write every miss, grouped, to this JSON file")
    args = parser.parse_args()

    paths = compare.php_files(args.root, args.needle, args.files)
    plan = {}
    for path in paths:
        text = oracle.read(path)
        where = regions(text)
        lines = compare.Lines(text)
        found = []
        for match in WORD.finditer(text):
            word = match.group(0)
            if word in SKIP or len(word.lstrip("$")) < 2:
                continue
            start, end = match.start(), match.end()
            if DECLARES.search(text[max(0, start - 20):start]):
                continue
            found.append((word, start, end, context_of(text, where, start, end)))
        plan[path] = (text, lines, found)

    probe = compare.probe_of({path: (text, None) for path, (text, _, _) in plan.items()})
    if probe is None:
        # Nothing declared to ask readiness with -- an empty skeleton, or a sample of scripts.
        print(f"no class declared in {len(plan)} sampled file(s) to probe readiness with")
        return 1
    session = oracle.Probe(args.root)
    print(f"starting server over {args.root}", flush=True)
    if not session.start(*probe):
        print("server never answered")
        return 1

    sites = [(path, *site) for path, (_, _, found) in plan.items() for site in found]
    sites = compare.stride(sites, args.limit)
    for path in {site[0] for site in sites}:
        session.open(path)

    answered = Counter()
    no_completion = defaultdict(list)
    no_hover = defaultdict(list)
    no_refs = defaultdict(list)
    for index, (path, word, start, end, context) in enumerate(sites):
        lines = plan[path][1]
        inside = start + (1 if word.startswith("$") else 0) + min(1, end - start - 1)
        target = session.ask("textDocument/definition", path, lines.position(inside))
        if not target:
            continue
        first = target[0] if isinstance(target, list) else target
        rng = first.get("targetSelectionRange") or first.get("range") or {}
        here = lines.position(inside)
        if first.get("uri", first.get("targetUri")) == oracle.uri_of(path) and rng and \
                rng["start"]["line"] == here["line"] and rng["start"]["character"] <= here["character"] <= rng["end"]["character"]:
            continue  # a declaration: goto lands on itself
        answered[context] += 1
        offered, _ = oracle.labels(
            session.ask("textDocument/completion", path, lines.position(end),
                        {"context": {"triggerKind": 1}})
        )
        if offered is None or not matches(word, offered):
            no_completion[context].append((path, word, lines.position(end), len(offered or ())))
        refs = session.ask("textDocument/references", path, lines.position(inside),
                           {"context": {"includeDeclaration": True}}) or []
        mine = lines.position(inside)
        covered = any(r.get("uri") == oracle.uri_of(path)
                      and r["range"]["start"]["line"] == mine["line"]
                      and r["range"]["start"]["character"] <= mine["character"] <= r["range"]["end"]["character"]
                      for r in refs)
        if not covered:
            no_refs[context].append((path, word, mine, len(refs)))
        hover = session.ask("textDocument/hover", path, lines.position(inside))
        if not hover:
            no_hover[context].append((path, word, lines.position(inside)))
        if index % 1000 == 0:
            print(f"  {index}/{len(sites)}", flush=True)

    session.server.stop()
    if args.dump:
        with open(args.dump, "w") as out:
            json.dump(
                {"completion": no_completion, "hover": no_hover, "references": no_refs},
                out,
                indent=1,
            )
    total = sum(answered.values())
    print(f"\ngoto answered {total} of {len(sites)} words")
    for title, misses in (("completion does not offer it", no_completion), ("hover says nothing", no_hover), ("references from it leave it out", no_refs)):
        count = sum(len(v) for v in misses.values())
        print(f"\n{title}: {count}")
        for context, items in sorted(misses.items(), key=lambda kv: -len(kv[1]))[:40]:
            path, word, pos, *rest = items[0]
            rel = os.path.relpath(path, args.root)
            print(f"  {len(items):5}/{answered[context]:<5} {context:44} e.g. {word} {rel}:{pos['line']+1}:{pos['character']+1} {rest}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
