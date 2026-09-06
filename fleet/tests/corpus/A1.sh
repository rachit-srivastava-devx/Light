#!/usr/bin/env bash
FLEET_CORPUS_PRUNE="-path */target/* -o -path */.venv/* -o -path */node_modules/* -o -path */.git/* -o -path */.codebase-memory/*"
export FLEET_CORPUS_PRUNE
printf '%s\n' 'A1 hand-rolled solved primitives replaced adopted open-source components'
ROOT="$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)"
python3 -B - "$ROOT" <<'PY'
import json, re, sys
from pathlib import Path
root=Path(sys.argv[1])
hits=[]
sys.path.insert(0,str(root/"tests"/"corpus"))
from _scan import lines
import re
hits=[]
for p,ln,s in lines(root):
 if p.suffix in {'.sh','.py','.js','.ts','.rs','.go','.java','.rb'} and re.search(r'(?i)(blake3|blake2|sha256).*(iv|permutation|round|compress)',s): hits.append((p,ln,s.strip()))
if hits:
 for p,n,s in hits[:8]: print(f'{p}:{n}: {s}')
 sys.exit(1)
sys.exit(0)
PY
rc=$?
case "$rc" in 0|1) exit "$rc";; *) exit 3;; esac
