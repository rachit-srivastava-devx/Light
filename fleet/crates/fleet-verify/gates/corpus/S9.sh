#!/usr/bin/env bash
FLEET_CORPUS_PRUNE="-path */target/* -o -path */.venv/* -o -path */node_modules/* -o -path */.git/* -o -path */.codebase-memory/*"
export FLEET_CORPUS_PRUNE
printf '%s\n' 'S9 an offline embedding model lived in a purgeable temporary directory'
ROOT="$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)"
DIR="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
python3 -B - "$ROOT" "$DIR" <<'PY'
import json, re, sys
MODEL_ARTIFACT = re.compile(r'model|weight|embed|onnx|\\.safetensors|\\.gguf|fastembed|hf_hub', re.I)
from pathlib import Path
root=Path(sys.argv[1])
hits=[]
sys.path.insert(0, sys.argv[2])
from _scan import lines
for p,ln,s in lines(root):
 if re.search(r'(?i)(embedding|model).*(/tmp|mktemp|temp)',s) or re.search(r'(?i)(/tmp|mktemp|temp).*(embedding|model)',s): hits.append((p,ln,s.strip()))
if hits:
 for p,n,s in hits[:8]: print(f'{p}:{n}: {s}')
 sys.exit(1)
sys.exit(0)
PY
rc=$?
case "$rc" in 0|1) exit "$rc";; *) exit 3;; esac

# central exclusion (C15: a new detector fires on build artifacts before real code)
