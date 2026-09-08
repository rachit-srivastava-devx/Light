#!/usr/bin/env bash
FLEET_CORPUS_PRUNE="-path */target/* -o -path */.venv/* -o -path */node_modules/* -o -path */.git/* -o -path */.codebase-memory/*"
export FLEET_CORPUS_PRUNE
printf '%s\n' 'C11 a regenerable giant hash cache was nearly committed'
ROOT="$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)"
DIR="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
python3 -B - "$ROOT" "$DIR" <<'PY'
import json, re, sys
from pathlib import Path
root=Path(sys.argv[1])
hits=[]
sys.path.insert(0, sys.argv[2])
from _scan import lines
for p,ln,s in lines(root):
 try:
  if p.stat().st_size > 2000000 and re.search(r'(?i)(cache|hash|digest)',p.name): hits.append((p,1,f'{p.stat().st_size} bytes'))
 except OSError: pass
if hits:
 for p,n,s in hits[:8]: print(f'{p}:{n}: {s}')
 sys.exit(1)
sys.exit(0)
PY
rc=$?
case "$rc" in 0|1) exit "$rc";; *) exit 3;; esac

