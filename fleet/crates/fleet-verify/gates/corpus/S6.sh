#!/usr/bin/env bash
FLEET_CORPUS_PRUNE="-path */target/* -o -path */.venv/* -o -path */node_modules/* -o -path */.git/* -o -path */.codebase-memory/*"
export FLEET_CORPUS_PRUNE
printf '%s\n' 'S6 remember and enforcement read different learning stores'
ROOT="$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)"
DIR="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
python3 -B - "$ROOT" "$DIR" <<'PY'
import json, re, sys
from pathlib import Path
root=Path(sys.argv[1])
hits=[]
sys.path.insert(0, sys.argv[2])
from _scan import lines
stores=[]
for p,ln,s in lines(root):
 if re.search(r'(?i)(remember|enforc)',s) and re.search(r'(?i)(store|path|file|db)',s): stores.append((p,ln,s.strip()))
if len(stores)>1 and len({s.split('=')[-1].strip() for _,_,s in stores})>1: hits.extend(stores[:2])
if hits:
 for p,n,s in hits[:8]: print(f'{p}:{n}: {s}')
 sys.exit(1)
sys.exit(0)
PY
rc=$?
case "$rc" in 0|1) exit "$rc";; *) exit 3;; esac

