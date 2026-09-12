#!/usr/bin/env bash
printf '%s\n' 'T2 `shift 2` past end-of-args silently no-ops and the loop spins, ignoring SIGTERM (25 of 94 flags hung)'
ROOT="${FLEET_TARGET_REPO:-$(pwd)}"
python3 - "$ROOT" <<'PY'
import sys, re
from pathlib import Path
root = Path(sys.argv[1]); hits = []
SKIP = ('/target/','/var/','/.venv/','/node_modules/','/.git/','/.codebase-memory/','/site-packages/','/corpus/')
for p in root.rglob('*.sh'):
    if any(s in str(p) for s in SKIP): continue
    try: lines = p.read_text(errors='ignore').splitlines()
    except OSError: continue
    for i, l in enumerate(lines):
        if not re.search(r'^\s*shift\s+2\s*$', l): continue
        # guarded if any of the previous 6 lines checks argument cardinality
        window = '\n'.join(lines[max(0, i-6):i])
        # `"$#"` is idiomatic and the closing quote sat between $# and the operator, so a correctly
        # guarded `shift 2` was reported as unguarded. Allow an optional quote on either side.
        if re.search(r'\$#"?\s*(-ge|-gt|-lt|>=|>)\s*"?[12]|need_val|require_val', window):
            continue
        hits.append(f"{p}:{i+1}: unguarded `shift 2` (no $# cardinality check within 6 lines)")
if hits: print('\n'.join(hits))
sys.exit(1 if hits else 0)
PY
