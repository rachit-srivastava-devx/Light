#!/usr/bin/env bash
FLEET_CORPUS_PRUNE="-path */target/* -o -path */.venv/* -o -path */node_modules/* -o -path */.git/* -o -path */.codebase-memory/*"
export FLEET_CORPUS_PRUNE
printf '%s\n' 'T1 a POSIX ERE used [^\\n] as if it meant a non-newline'
ROOT="$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)"
DIR="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
python3 -B - "$ROOT" "$DIR" <<'PY'
import json, re, sys
from pathlib import Path
root=Path(sys.argv[1])
hits=[]
sys.path.insert(0, sys.argv[2])
from _scan import lines
# The rule is about POSIX ERE (grep/sed/awk), where [^\n] means 'not backslash and not n'.
# Python's re honours \n inside a character class, so a Python pattern is NOT this defect.
# Narrowed, not disabled: a genuine grep/sed/awk use is still caught.
for p,ln,s in lines(root):
 if p.suffix == '.py' or re.search(r're\.(search|match|compile|sub|findall|split)', s): continue
 if p.suffix in {'.sh','.bash','.zsh'} and re.search(r'\[\^\\n\]',s): hits.append((p,ln,s.strip()))
if hits:
 for p,n,s in hits[:8]: print(f'{p}:{n}: {s}')
 sys.exit(1)
sys.exit(0)
PY
rc=$?
case "$rc" in 0|1) exit "$rc";; *) exit 3;; esac

