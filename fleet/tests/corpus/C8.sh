#!/usr/bin/env bash
FLEET_CORPUS_PRUNE="-path */target/* -o -path */.venv/* -o -path */node_modules/* -o -path */.git/* -o -path */.codebase-memory/*"
export FLEET_CORPUS_PRUNE
printf '%s\n' 'C8 a stdin-reading command consumed a while-read loop'
ROOT="$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)"
python3 -B - "$ROOT" <<'PY'
import json, re, sys
from pathlib import Path
root=Path(sys.argv[1])
hits=[]
sys.path.insert(0,str(root/"tests"/"corpus"))
from _scan import lines
for p,ln,s in lines(root):
 if p.suffix in {'.sh','.bash','.zsh'} and re.search(r'while\s+(IFS=\S+\s+)?read\b',s) and re.search(r'\b(cat|read|head|sed|awk|sort)\b',s): hits.append((p,ln,s.strip()))
if hits:
 for p,n,s in hits[:8]: print(f'{p}:{n}: {s}')
 sys.exit(1)
sys.exit(0)
PY
rc=$?
case "$rc" in 0|1) exit "$rc";; *) exit 3;; esac

