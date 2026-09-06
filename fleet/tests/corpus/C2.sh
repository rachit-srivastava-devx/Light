#!/usr/bin/env bash
FLEET_CORPUS_PRUNE="-path */target/* -o -path */.venv/* -o -path */node_modules/* -o -path */.git/* -o -path */.codebase-memory/*"
export FLEET_CORPUS_PRUNE
printf '%s\n' 'C2 a test wrote production ledger rows'
ROOT="$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)"
python3 -B - "$ROOT" <<'PY'
import json, re, sys
from pathlib import Path
root=Path(sys.argv[1])
hits=[]
sys.path.insert(0,str(root/"tests"/"corpus"))
from _scan import lines
# A file that roots FLEET_STATE in `mktemp -d` cannot touch the production ledger: every path it
# opens is under a throwaway directory. Exempt those files rather than the individual lines, and
# only when BOTH markers are present -- narrowing the detector, not disabling it.
def temp_rooted(path):
 try: t=path.read_text(errors='replace')
 except OSError: return False
 return ('mktemp -d' in t) and ('FLEET_STATE' in t)
# B16: temp_rooted is a whole-file read. Calling it once per line re-reads every file once per
# yielded line (O(lines x bytes); NUL-shredded PNGs turn into tens of thousands of fake lines).
# Memoize per path -- it must be a full-file read exactly once per unique file, never per line --
# so the exempt set is computed identically, just not quadratically.
rooted_cache = {}
for p,ln,s in lines(root):
 rooted = rooted_cache.get(p)
 if rooted is None:
  rooted = temp_rooted(p)
  rooted_cache[p] = rooted
 if rooted: continue
 if p.suffix in {'.sh','.bash','.zsh'} and re.search(r'ledger',s,re.I) and re.search(r'(>>|append|write|open)',s,re.I) and not re.search(r'(override|LEDGER_PATH|ledger_path)',s): hits.append((p,ln,s.strip()))
if hits:
 for p,n,s in hits[:8]: print(f'{p}:{n}: {s}')
 sys.exit(1)
sys.exit(0)
PY
rc=$?
case "$rc" in 0|1) exit "$rc";; *) exit 3;; esac
