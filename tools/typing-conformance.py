#!/usr/bin/env python3
"""Runs phrost through php-typing-conformance and ranks it beside the engines that suite tracks.

    python3 tools/typing-conformance.py <clone of zonuexe/php-typing-conformance> [--phrost BIN]

The suite is twelve PHP engines answering the same 250 files, each stating inline what a correct
engine reports (`// E`), stays quiet about (`// Q`), and accepts (`// V`). It is the one corpus here
written by somebody else *for* comparing engines, so a pass rate is a number nobody here chose the
questions for -- and it is published, with every other engine's result already in the clone.

What this does to the clone: installs `tools/typing-conformance/PhrostChecker.php` into its checker
directory and registers it in `conformance/src/main.php`, both idempotently, then runs
`main.php --tool phrost`, which writes `conformance/results/phrost/` and touches no other engine's
results. Needs `composer install` run once inside `conformance/`.

Prints every engine's pass rate from the stored results, then phrost's failing tests with which of
PHPStan and phpantom pass them -- a test both of those fail is usually a position, not a gap.
"""
import argparse
import os
import re
import shutil
import subprocess
import sys

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
CHECKER = os.path.join(REPO, "tools", "typing-conformance", "PhrostChecker.php")


def install(clone):
    shutil.copyfile(CHECKER, os.path.join(clone, "conformance", "src", "Checker", "PhrostChecker.php"))
    main = os.path.join(clone, "conformance", "src", "main.php")
    text = open(main).read()
    if "PhrostChecker" in text:
        return
    text = text.replace("use Conformance\\Checker\\PhpantomChecker;\n",
                        "use Conformance\\Checker\\PhpantomChecker;\nuse Conformance\\Checker\\PhrostChecker;\n", 1)
    text = text.replace("$mirChecker = new MirChecker(", """$phrostChecker = new PhrostChecker(
    binaryPath: (($phrost = getenv('PHROST')) !== false && $phrost !== '') ? $phrost : 'phrost',
    testsDir: $testsDir,
);
$mirChecker = new MirChecker(""", 1)
    text = text.replace("$phpantomChecker, $mirChecker,", "$phpantomChecker, $phrostChecker, $mirChecker,", 1)
    if "$phrostChecker, $mirChecker" not in text:
        sys.exit("conformance/src/main.php changed shape; register PhrostChecker by hand")
    open(main, "w").write(text)


def verdicts(results, tool):
    found = {}
    directory = os.path.join(results, tool)
    for name in os.listdir(directory):
        if name.endswith(".toml") and name != "version.toml":
            match = re.search(r'^conformance_automated = "([^"]*)"', open(os.path.join(directory, name)).read(), re.M)
            found[name[:-5]] = match.group(1) if match else "?"
    return found


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("clone")
    parser.add_argument("--phrost", default=os.path.join(REPO, "target", "release", "phrost"))
    args = parser.parse_args()
    clone = os.path.abspath(args.clone)

    install(clone)
    run = subprocess.run(["php", "conformance/src/main.php", "--tool", "phrost"], cwd=clone,
                         env=dict(os.environ, PHROST=args.phrost), capture_output=True, text=True)
    if run.returncode != 0:
        sys.exit(run.stdout[-2000:] + run.stderr[-2000:])

    results = os.path.join(clone, "conformance", "results")
    rows = []
    for tool in sorted(os.listdir(results)):
        if tool in ("lsp", "tests", "scaffold") or not os.path.isdir(os.path.join(results, tool)):
            continue
        found = verdicts(results, tool)
        passed = sum(1 for verdict in found.values() if verdict == "Pass")
        rows.append((passed / max(len(found), 1), tool, passed, len(found)))
    for rate, tool, passed, total in sorted(rows, reverse=True):
        print(f"{tool:16s} {rate * 100:5.1f}%  {passed}/{total}")

    ours = verdicts(results, "phrost")
    others = {tool: verdicts(results, tool) for tool in ("phpstan", "phpantom") if os.path.isdir(os.path.join(results, tool))}
    print("\nfailing for phrost:")
    for name, verdict in sorted(ours.items()):
        if verdict != "Pass":
            passing = [tool for tool, found in others.items() if found.get(name) == "Pass"]
            print(f"  {name}  (passes: {', '.join(passing) or 'neither'})")


if __name__ == "__main__":
    main()
