#!/usr/bin/env bash
printf '%s\n' 'H1 TEST-HARNESS bug produced a false reading (3x today: $? after pipe, $? after command-substitution, zsh no-word-split)'
ROOT="${FLEET_TARGET_REPO:-$(pwd)}"
DIR="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
python3 - "$ROOT" "$DIR" <<'PY'
import sys, re
from pathlib import Path
root = Path(sys.argv[1]); hits = []
# The detector corpus (this script's own directory) embeds literal copies of the bad-code
# patterns it detects, as fixtures for _selftest.sh -- it must be skipped by its real, current,
# script-relative location, not a repo layout (`tests/corpus`) that already moved once.
corpus_dir = str(Path(sys.argv[2]))
SKIP = ('/target/','/var/','/.venv/','/node_modules/','/.git/','/.codebase-memory/','/site-packages/','/docs/','.md', corpus_dir + '/')
for p in root.rglob('*.sh'):
    if any(s in str(p) for s in SKIP): continue
    try: lines = p.read_text(errors='ignore').splitlines()
    except OSError: continue
    for i, l in enumerate(lines):
        # (a) $? read on a line that also contains a command substitution BEFORE it -> clobbered
        # SAFE: VAR="$(cmd)"; RC=$?  (the assignment's status IS the command's status)
        # UNSAFE: echo "$(other) exit=$?"  — the substitution runs first and clobbers $?
        if not re.match(r'^\s*\w+="?\$\(', l):
            m = re.search(r'\$\((?!\()', l)
            if m and re.search(r'\$\?(?!\w)', l[m.end():]):
                hits.append(f"{p}:{i+1}: $? read AFTER a command substitution in the same expression — clobbered")
        # (b) invoking a binary with an unquoted multi-word var (zsh does not word-split)
        if re.search(r'"\$[A-Za-z_]\w*"\s+\$[a-z_]\w*\b', l) and 'for ' not in l:
            hits.append(f"{p}:{i+1}: unquoted $var as argv — zsh does NOT word-split; pass args explicitly")
if hits: print('\n'.join(hits))
sys.exit(1 if hits else 0)
PY
