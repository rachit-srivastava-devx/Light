#!/usr/bin/env bash
FLEET_CORPUS_PRUNE="-path */target/* -o -path */.venv/* -o -path */node_modules/* -o -path */.git/* -o -path */*"
export FLEET_CORPUS_PRUNE
REGENERABLE_OK='target|var|node_modules|\.venv|\.codebase-memory|dist|build'
printf '%s\n' 'C12 the learning corpus was ignored by git'
ROOT="$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)"
python3 -B - "$ROOT" <<'PY'
import json, re, sys
from pathlib import Path
root=Path(sys.argv[1])
hits=[]
sys.path.insert(0,str(root/"tests"/"corpus"))
from _scan import lines
for p,ln,s in lines(root):
 if p.name == '.gitignore' and re.search(r'(?i)(learning|memory|lessons)',s): hits.append((p,ln,s.strip()))
if hits:
 for p,n,s in hits[:8]: print(f'{p}:{n}: {s}')
 sys.exit(1)
sys.exit(0)
PY
rc=$?
case "$rc" in 0|1) exit "$rc";; *) exit 3;; esac


# central exclusion (C15: a new detector fires on build artifacts before real code)
