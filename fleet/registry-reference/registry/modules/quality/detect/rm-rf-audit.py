#!/usr/bin/env python3
"""Flag `rm -rf <path>` where <path> did NOT come from mktemp.

Corpus row C1. The original incident: snapshot.sh derived a temp dir from a content digest alone and
then rm -rf'd it, two concurrent runs resolved to the same path, one deleted the other's working
directory, and the machine was unusable for 40 minutes -- twice.

Why this is a script and not a grep: a line-local pattern cannot see that `rm -rf "$TMP_ROOT"` is
safe because TMP_ROOT was assigned from mktemp fifteen lines earlier. Three attempts at expressing
this as bash-inside-bash-inside-awk produced only false positives -- it flagged all three of the
repo's correctly-written cleanup traps. A detector whose every finding is a false positive gets
muted, which is worse than not having it.
"""
import re
import sys
import pathlib

MKTEMP_ASSIGN = re.compile(r'\b([A-Za-z_][A-Za-z0-9_]*)\s*=\s*[^\n]*\bmktemp\b')
RM_RF = re.compile(r'\brm\s+-[a-zA-Z]*r[a-zA-Z]*f|\brm\s+-[a-zA-Z]*f[a-zA-Z]*r')
VAR_REF = re.compile(r'\$\{?([A-Za-z_][A-Za-z0-9_]*)')
# Literals that are inherently scoped to a scratch area, so a bare path is fine.
SAFE_LITERAL = re.compile(r'\$\{?(TMPDIR|BATS_[A-Z_]+|RUNNER_TEMP)\b|/tmp/|mktemp')

def audit(paths):
    findings = []
    for p in paths:
        f = pathlib.Path(p)
        if not f.is_file():
            continue
        # A detector must not match its own source. Every previous version of this check flagged the
        # file that implements it, which is a finding that can never be cleared.
        if f.name in ('rm-rf-audit.py', 'corpus.sh'):
            continue
        try:
            text = f.read_text(errors='replace')
        except OSError:
            continue
        # Any variable ever assigned from mktemp anywhere in this file is trusted. File-scope is the
        # right granularity: these are short shell scripts, and a var named from mktemp in one
        # function is not re-pointed at $HOME in another.
        safe = set(MKTEMP_ASSIGN.findall(text))
        lines = text.splitlines()
        for n, line in enumerate(lines, 1):
            stripped = line.strip()
            if stripped.startswith('#') or not RM_RF.search(line):
                continue
            if SAFE_LITERAL.search(line):
                continue
            # An explicit, written justification clears the line. Some deletions ARE correct on a
            # non-mktemp path -- a lock directory, a test's own fixture -- and a rule with no escape
            # hatch gets bypassed wholesale rather than argued with. Requiring the reason in the
            # source makes the exception auditable instead of invisible.
            prev = lines[n - 2] if n >= 2 else ''
            if 'rm-rf-ok:' in line or 'rm-rf-ok:' in prev:
                continue
            # Only the vars AFTER the rm -rf token are the deletion target. Reading the whole line
            # flagged `echo "... $owner"; rm -rf "$lock"` on $owner -- the message, not the path.
            m = RM_RF.search(line)
            # Stop at the statement boundary. Reading to end-of-line blamed
            # `rm -rf "$snapshot"; return "$ec"` on $ec -- a variable in a DIFFERENT statement that
            # has nothing to do with the deletion target.
            tail = re.split(r';|&&|\|\||\}', line[m.end():])[0]
            # A computed path -- rm -rf "$(_receipt_lock)" -- is not a literal and not a bare var.
            # It cannot be traced statically; flagging it would be a permanent false positive.
            if '$(' in tail:
                continue
            refs = VAR_REF.findall(tail)
            if refs and all(r in safe for r in refs):
                continue
            if not refs:
                findings.append(f"{p}:{n}: rm -rf on a literal path: {stripped[:90]}")
            else:
                unsafe = [r for r in refs if r not in safe]
                findings.append(f"{p}:{n}: rm -rf on ${unsafe[0]}, never assigned from mktemp: {stripped[:70]}")
    return findings

if __name__ == '__main__':
    out = audit(sys.argv[1:])
    for line in out:
        print(line)
    sys.exit(1 if out else 0)
