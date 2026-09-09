#!/usr/bin/env bash
FLEET_CORPUS_PRUNE="-path */target/* -o -path */.venv/* -o -path */node_modules/* -o -path */.git/* -o -path */.codebase-memory/*"
export FLEET_CORPUS_PRUNE
printf '%s\n' 'S9 an offline embedding model lived in a purgeable temporary directory'
ROOT="${FLEET_TARGET_REPO:-$(pwd)}"
DIR="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
python3 -B - "$ROOT" "$DIR" <<'PY'
import json, re, sys
MODEL_ARTIFACT = re.compile(r'model|weight|embed|onnx|\\.safetensors|\\.gguf|fastembed|hf_hub', re.I)
from pathlib import Path
root=Path(sys.argv[1])
hits=[]
sys.path.insert(0, sys.argv[2])
from _scan import lines
# Bare substring matching turned "model" and "temp" into a trap: "AttemptError" contains
# "temp" (Att-emp-t) and "ModelError" contains "model", so any line pairing the two --
# `AttemptError::Other(ModelError::new(...))`, and its copies inside mutants.out diffs -- hit
# with no /tmp or embedding model in sight. Lookaround requires each term be its own token: not
# glued to a surrounding letter (so "ModelError" and "Attempt" are excluded) but still allowed
# to sit next to "_" or "/" (so "embedding_model = \"/tmp/model\"", the self-test fixture, and
# a real path like /tmp/model.onnx still match).
TOKEN = r'(?<![A-Za-z])(?:embedding|model)(?![A-Za-z])'
TEMP = r'(?<![A-Za-z])(?:/tmp|mktemp|temp)(?![A-Za-z])'
for p,ln,s in lines(root):
 if re.search(f'(?i){TOKEN}.*{TEMP}',s) or re.search(f'(?i){TEMP}.*{TOKEN}',s): hits.append((p,ln,s.strip()))
if hits:
 for p,n,s in hits[:8]: print(f'{p}:{n}: {s}')
 sys.exit(1)
sys.exit(0)
PY
rc=$?
case "$rc" in 0|1) exit "$rc";; *) exit 3;; esac

# central exclusion (C15: a new detector fires on build artifacts before real code)
