#!/usr/bin/env bash
FLEET_CORPUS_PRUNE="-path */target/* -o -path */.venv/* -o -path */node_modules/* -o -path */.git/* -o -path */.codebase-memory/*"
export FLEET_CORPUS_PRUNE
printf '%s\n' 'C19 a package script pointed to a missing file'
ROOT="$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)"
python3 -B - "$ROOT" <<'PY'
import json, re, sys
from pathlib import Path
root=Path(sys.argv[1])
hits=[]
sys.path.insert(0,str(root/"tests"/"corpus"))
from _scan import lines
for p,ln,s in lines(root):
 if p.name == 'package.json':
  try:
   data=json.loads(p.read_text())
   for name,cmd in data.get('scripts',{}).items():
    for ref in re.findall(r'(?<![A-Za-z0-9_./-])([A-Za-z0-9_.-]+\.(?:js|ts|sh))(?![A-Za-z0-9_./-])',cmd):
     if not (p.parent/ref).exists(): hits.append((p,1,f'script {name} references missing {ref}'))
  except Exception: pass
if hits:
 for p,n,s in hits[:8]: print(f'{p}:{n}: {s}')
 sys.exit(1)
sys.exit(0)
PY
rc=$?
case "$rc" in 0|1) exit "$rc";; *) exit 3;; esac

