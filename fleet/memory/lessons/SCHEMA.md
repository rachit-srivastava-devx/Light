# Lesson store — schema

One JSON file per lesson row, matching `Speed-of-Thought-L8-Deep-Dive/11-THREE-MEMORY-LAYERS.md`
§4.1/§4.3 (fields) and the `fleet-rs` `AGENTS.md` hard-rules convention (id + evidence + path:line).

```json
{
  "id": "E1",
  "signature": "one-sentence description of the mistake shape, not the specific instance",
  "phrasings": ["independently-worded restatement 1", "restatement 2", "restatement 3"],
  "pattern_regex": "the regex `bin/recur-gate.sh` evaluates against added diff lines, or null if not yet mechanisable",
  "evidence": "where this was seen / who paid for it, with a citation",
  "path_line": "file:line where the rule is also stated in prose (the human-facing index)",
  "exemptions": ["a line shape the pattern must NOT flag"],
  "status": "Observed | Candidate | Promoted | Enforced | Retired | Unpromotable",
  "author": "who authored the pattern + fixture",
  "note": "free text — status caveats, e.g. self-authored fixture pending independent review"
}
```

## Funnel (§4.3)

`Observed` (caught once, no pattern yet) → `Candidate` (pattern + fixture exist, not yet proven
both directions by someone other than the pattern's author) → `Promoted` (independent bad fixture +
control both pass) → `Enforced` (wired into `bin/recur-gate.sh` and run in `verify.sh`) →
`Retired` (superseded by a later row; never deleted, never edited in place).

**A lesson stays `Candidate` forever if nobody but its author reviews it.** That is the honest
state for a solo-operator session — see `11-THREE-MEMORY-LAYERS.md` §4.3, §8. Do not hand-promote
a self-reviewed lesson to `Enforced` to make a dashboard look complete; a green gate nobody
adversarially tried to fool is a rehearsal, not a measurement.

## What does NOT belong here

Judgement failures — "built breadth instead of finishing one path," "declared victory early" — have
no diff signature. They stay `Unpromotable`, listed with their reason, and are handled by review
discipline (this repo's `verify.sh` + `lld-ready`-equivalent gates), not by `recur`. Forcing a
grep pattern onto a judgement call is how a gate gets a false-positive rate high enough that it
gets muted within a week — see `11-THREE-MEMORY-LAYERS.md` §4.4.
