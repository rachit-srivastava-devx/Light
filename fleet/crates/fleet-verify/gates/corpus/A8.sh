#!/usr/bin/env bash
FLEET_CORPUS_PRUNE="-path */target/* -o -path */.venv/* -o -path */node_modules/* -o -path */.git/* -o -path */.codebase-memory/*"
export FLEET_CORPUS_PRUNE
printf '%s\n' 'A8 published percentages omitted their denominator'
ROOT="${FLEET_TARGET_REPO:-$(pwd)}"
DIR="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
python3 -B - "$ROOT" "$DIR" <<'PY'
import json, re, sys
from pathlib import Path
root=Path(sys.argv[1])
hits=[]
sys.path.insert(0, sys.argv[2])
from _scan import lines
for p,ln,s in lines(root):
 if s.lstrip().startswith('#'): continue  # 4th detector needing this (T6/B10/T20/A8): a detector that reports the comment documenting a fix as the defect is noise
 if p.suffix in {'.sh','.py','.js','.ts','.rs','.md'} and re.search(r'(?i)(percent|percentage|rate)',s) and re.search(r'(?<![A-Za-z])\d+(?:\.\d+)?\s*%',s) and not re.search(r'(/|denominator|total|checked)',s,re.I): hits.append((p,ln,s.strip()))
if hits:
 for p,n,s in hits[:8]: print(f'{p}:{n}: {s}')
 sys.exit(1)
sys.exit(0)
PY
rc=$?
case "$rc" in 0|1) exit "$rc";; *) exit 3;; esac

