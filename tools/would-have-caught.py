"""Would phrost have seen what a fix commit fixed?

    python3 tools/would-have-caught.py <project> <commit> [<own code dir>] [--keep <dir>]

Builds two scratch projects -- symlinks to everything but the project's own code, and a copy of
that code with the commit's files at their state before and after it -- runs `phrost check` on
each, and prints the findings in the changed files that the commit made go away (`fixed`) and
that it brought (`new`). A bug fix phrost has nothing to say about is a candidate for a check;
one it reported is the checker earning its keep.

Never writes into the project: the copies live under a temporary directory (or `--keep`), and
are clones rather than links, so the overlay cannot reach the working tree.
The own-code directory defaults to Drupal's `docroot/modules/custom`; a project whose code is
elsewhere names it.
"""
import os, shutil, subprocess, sys

import tempfile

args = [a for a in sys.argv[1:]]
keep = None
if "--keep" in args:
    at = args.index("--keep"); keep = args[at + 1]; del args[at:at + 2]
if len(args) < 2:
    sys.exit(__doc__)
LIVE = os.path.abspath(os.path.expanduser(args[0]))
PHROST = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "target/release/phrost")
BASE = keep or tempfile.mkdtemp(prefix="would-have-caught-")
CUSTOM = args[2] if len(args) > 2 else "docroot/modules/custom"

def git(*args):
    return subprocess.run(["git", "-C", LIVE, *args], capture_output=True, text=True).stdout

def build(root):
    """A copy of the project without `.git`, cloned rather than linked.

    Cloned because the server's walks do not follow a symlink -- deliberately, since a Drupal
    checkout's `web -> docroot` would index everything twice -- so a linked `docroot/core` is a
    core with no entity types in it, and every `getStorage('node')` reads as a finding. `cp -c`
    is APFS's copy-on-write clone: seconds for a whole checkout and no disk. Elsewhere it falls
    back to an ordinary copy.
    """
    if os.path.exists(root):
        shutil.rmtree(root)
    os.makedirs(root)
    for entry in os.listdir(LIVE):
        if entry in (".git", "phrost-baseline.toml"):
            continue
        source, target = os.path.join(LIVE, entry), os.path.join(root, entry)
        if subprocess.run(["cp", "-cR", source, target], capture_output=True).returncode != 0:
            subprocess.run(["cp", "-R", source, target], check=True)

def overlay(root, commit, files):
    for path in files:
        target = os.path.join(root, path)
        blob = subprocess.run(["git", "-C", LIVE, "show", f"{commit}:{path}"], capture_output=True)
        if blob.returncode != 0:
            if os.path.exists(target):
                os.remove(target)
            continue
        os.makedirs(os.path.dirname(target), exist_ok=True)
        with open(target, "wb") as handle:
            handle.write(blob.stdout)

def findings(root, files):
    out = subprocess.run([PHROST, "check", root], capture_output=True, text=True).stdout
    wanted = set(files)
    return sorted(line for line in out.splitlines() if line.split(":", 1)[0] in wanted)

commit = args[1]
files = [f for f in git("diff-tree", "--no-commit-id", "--name-only", "-r", commit).split() if f.startswith(CUSTOM) and f.endswith((".php", ".module", ".install", ".inc"))]
before, after = os.path.join(BASE, "before"), os.path.join(BASE, "after")
build(before); overlay(before, commit + "^", files)
build(after); overlay(after, commit, files)
pre, post = findings(before, files), findings(after, files)
strip = lambda line: line.split(": ", 1)[-1]
print(f"{commit}: {len(files)} files, {len(pre)} findings before, {len(post)} after")
post_msgs = [strip(l) for l in post]
for line in pre:
    if strip(line) not in post_msgs:
        print("  fixed  ", line[:220])
for line in post:
    if strip(line) not in [strip(l) for l in pre]:
        print("  new    ", line[:220])
if keep is None:
    shutil.rmtree(BASE, ignore_errors=True)
