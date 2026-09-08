#!/usr/bin/env bash
# M4 — a command function with no dispatch arm is dead code that every gate passes over.
# D35: the module-level reachability test cannot see this class -- main.rs IS reachable, so a
# function inside it with no dispatch arm passes every gate. `__agent` is legitimately internal
# (main.rs re-invokes the binary with it to spawn the fd-3 supervised worker), so an arm counts
# whether or not it appears in --help. What this catches is a *_command function with no arm at all.
set -u
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
SRC="$ROOT/keel/fleet/src/main.rs"
[ -r "$SRC" ] || exit 77
python3 - "$SRC" <<'PY'
import re, sys
src = open(sys.argv[1], encoding="utf-8").read()
fns = set(re.findall(r'^fn ([a-z_0-9]+_command)\(', src, re.M))
if not fns:
    print("M4: found no *_command functions -- measuring nothing is a failure"); sys.exit(1)
called = set()
for name in fns:
    # a dispatch arm, or any call site outside its own definition
    if re.search(r'=>\s*' + name + r'\(', src) or len(re.findall(r'\b' + name + r'\(', src)) > 1:
        called.add(name)
dead = sorted(fns - called)
print(f"M4: {len(called)} of {len(fns)} *_command functions are reachable from a dispatch arm")
if dead:
    print("M4: UNREACHABLE command functions (dead code no gate can see):")
    for d in dead: print(f"    {d}")
    sys.exit(1)
PY
