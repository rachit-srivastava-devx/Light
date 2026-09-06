# B3 — audio-gate/gate.py `--max-gap-ms` threshold fix

## Task

`audio-gate/gate.py:431`'s `--max-gap-ms` CLI argument defaulted to `250.0`. The blueprint
(`blueprints/ADHD-Focus-Orb-L8-Deep-Dive/02-ALWAYS-ON-AUDIO-ENGINE.md`) specifies a 0 ms gap
budget (§1: "0 audio underruns per session, bounding the worst inter-buffer gap below one buffer
period (~5-10 ms)") and this exact gate implements §11.2's loudness-floor probe, which states the
tripwire explicitly: "assert no window > 20 ms falls below the bed's noise floor". 250 ms is
~12x looser than that stated tripwire and well above the ~120 ms users "reliably notice" per
§11.7's panel test — meaning a real, user-audible gap could pass this gate silently.

## Why 250 was there (git history + wiring check)

- `git log --follow -- audio-gate/gate.py` shows exactly one commit touching this file
  (`e942446 checkpoint: prior in-progress work`) — no history of a deliberate calibration
  decision, no commit message discussing the number. Consistent with an initial placeholder
  that was never tightened.
- `grep -rn "max-gap-ms\|max_gap_ms"` across `package.json`, `scripts/`, and every `.yml`/`.yaml`
  in the repo: the only invocation site is `scripts/capture-audio-gate.sh:30`, which calls
  `gate.py` with `--audio`/`--schedule` only — **`--max-gap-ms` is never passed explicitly
  anywhere**, so every real invocation of this gate has always relied on the loose default.
- No CI workflow or npm script invokes `audio-gate/gate.py` at all (grepped, 0 hits). It is a
  manual, device-capture-only tool run via `scripts/capture-audio-gate.sh`. This matters for the
  "did tightening reveal a real gap" question below.

## Fix

`audio-gate/gate.py`: default changed from `250.0` to `20.0`, with an inline comment citing
blueprint §11.2 and §1 verbatim so the number's provenance survives the next edit. Also renamed
the report's stale `turns_exceeding_250ms` JSON key to `turns_exceeding_max_gap_ms` (it hardcoded
the old number even though its value is `sum(value > args.max_gap_ms ...)` — a threshold-name
that no longer matched its own threshold). `audio-gate/README.md` updated to state the new
default and cite the same blueprint section instead of a bare "250 ms".

No fixture WAV existed anywhere in the repo for `audio-gate/gate.py` specifically (checked: only
`audio-gate/fixture-schedule.json`, a turn-schedule JSON with no paired audio). No existing
passing case was loosened or altered to accommodate the tightened default — nothing needed to be,
because nothing exercised this exact file's real-audio path in CI before or after this change.

## Real before/after proof (synthetic capture, isolates the threshold only)

Built two 2-channel 16 kHz WAV fixtures with raw PCM (via Python `wave`), under `fixtures/`:

- `capture2.wav` — rendered-output channel (ch1) is a continuous tone with exactly **one clean
  60.06 ms silence gap** inserted mid-capture (measured by ffmpeg `silencedetect` itself, see
  `longest_gap_ms` in the JSON below — not asserted, measured).
- `capture3_nogap.wav` — control: rendered channel has **zero gap** (fully continuous tone),
  proving the tightened threshold doesn't false-positive on clean audio.

Run with `--content-detector highpass` (Silero VAD isn't installed in either venv on this
machine; `highpass` is the repo's own documented synthetic-control mode for exactly this kind of
isolated check — see `audio-gate/README.md`'s existing caveat that it is not valid *content*
proof, but it does not affect the *continuity/gap* measurement path, which runs off
`silencedetect` on the same channel regardless of detector).

**Before fix** (`--max-gap-ms` default `250.0`, i.e. current code at the time of capture),
`before-max-gap-250ms.json`:

```
continuity.gap_events: 0
continuity.longest_gap_ms: 60.0619999999999
failures: ["fixed-cadence schedule expected 2 turns; captured 1",
           "turn t1 measured no rendered presence after user stop"]
```

The measured 60 ms real gap produced **zero** gap-related failures — it is well under 250 ms, so
`gap_events` stayed 0 and the specific `"N rendered-audio gap(s) exceeded 250.0 ms"` failure line
never appears. (The two failures present are from this synthetic fixture's single-continuous-tone
mic channel not producing two separate "turns" — unrelated to the gap check; included for
transparency, not cherry-picked away.)

**After fix** (`--max-gap-ms` default `20.0`), same exact WAV file, `after-max-gap-20ms.json`:

```
continuity.gap_events: 1
continuity.longest_gap_ms: 60.0619999999999
failures: ["1 rendered-audio gap(s) exceeded 20.0 ms",
           "fixed-cadence schedule expected 2 turns; captured 1",
           "turn t1 measured no rendered presence after user stop"]
```

Same measured 60.06 ms gap now produces the explicit `"1 rendered-audio gap(s) exceeded 20.0 ms"`
failure and `gap_events: 1`. This is the direct, reproducible demonstration that the old default
let a real, ffmpeg-measured rendered-audio gap pass the continuity check silently, and the new
default catches it.

**Control** (`capture3_nogap.wav`, zero real gap, new `20.0` default), `control-no-gap-20ms.json`:

```
continuity.gap_events: 0
continuity.longest_gap_ms: 0.0
failures: ["fixed-cadence schedule expected 2 turns; captured 1",
           "turn t1 measured no rendered presence after user stop"]
```

No gap-related failure on genuinely continuous audio — the tighter threshold is not a
false-positive generator; the two remaining failures are the same fixture-schedule artifact as
above, present in both the gap and no-gap captures, proving they are independent of `--max-gap-ms`.

## Regression check

`backend/relay-py` full pytest suite (`395 passed`, unchanged pass count) before and after the
edit — `gate.py`'s only production dependency, `AudioGapWatchdog`/`loudness_floor_ok` in
`orb_relay/observability/metrics.py`, was not modified, and its own unit tests
(`test_eval_observability.py`) construct `AudioGapWatchdog` with their own explicit
`max_gap_ms=250` argument — those are unaffected because they never read `gate.py`'s CLI default.

`pytest-full-after-fix.log`: `395 passed, 3 warnings in 56.53s`.

## Pre-existing, unrelated finding (not fixed, flagged only)

`ruff check audio-gate/gate.py` reports one `E402` (module-level import after `sys.path.insert`,
line 27) — confirmed present identically on unmodified `git show HEAD:audio-gate/gate.py` via
`ruff check` against a copy, i.e. **pre-existing, not introduced by this change** (`ruff-gate-py.log`).
Out of scope for this brief (not mentioned in the defect report; touching it would be a drive-by
refactor of an unrelated lint issue). Also noted but explicitly out of scope:
`backend/relay-py/src/orb_relay/eval/gates.py:349` instantiates a *separate*
`AudioGapWatchdog(max_gap_ms=300)` inside a synthetic corpus-replay eval gate (`voice_cases` from
a fixture corpus, not real captured audio) — a different mechanism from `audio-gate/gate.py`'s
real-audio CLI probe named in the defect. Flagged for a follow-up look, not touched here.

## Critical finding: did tightening this gate reveal a REAL previously-undetected gap in the actual audio pipeline?

**No — and the honest reason is that this question doesn't have real pipeline audio to answer it
with, not that the pipeline is clean.** `audio-gate/gate.py` is invoked nowhere in `package.json`,
no npm script, and no CI workflow (grepped, 0 hits) — its only invocation path is
`scripts/capture-audio-gate.sh`, a manual script requiring a physically connected Mac/Simulator
audio loopback (BlackHole) and a human to run it. No committed WAV capture of the real rendered
audio pipeline exists anywhere in this repo for this gate to be run against retroactively. So:
tightening the default could not and did not surface a previously-undetected gap in the real
pipeline in this task, because the real pipeline has never been captured through this gate at
all — verdict is **NOT-RUN / NO-DATA-UNKNOWN** for "does the real pipeline currently have a gap
between 20 ms and 250 ms", not a clean bill of health. The synthetic before/after proof above
demonstrates the *gate's own behavior change* is correct and real; it says nothing about whether
today's actual TTS/bed audio has such a gap, because nobody has ever pointed this gate at it. That
absence-of-CI-wiring is itself the more consequential finding here: a merge-blocking invariant
(§1: "0 ms... merge-blocking") has an automated prober that has never been run against production
audio in this repo's history.

## Reproduce from a fresh clone

```bash
cd company/products/adhd-focus-orb-worktrees/b3-gap-gate-threshold
# venv: symlink a sibling checkout's backend/relay-py/.venv (no venv is checked in; see
# FLEET-LEARNINGS.md's "no-fake-tts-default" entry, Gotcha 3, for the exact commands), or run
# `python3 -m venv backend/relay-py/.venv && backend/relay-py/.venv/bin/pip install -e "backend/relay-py[dev]"`.
cd backend/relay-py && ./.venv/bin/python -m pytest tests/ -q   # 395 passed
cd ../..
./backend/relay-py/.venv/bin/python audio-gate/gate.py \
  --audio evidence/fleet-2026-09-02/b3-gap-gate-threshold/fixtures/capture2.wav \
  --schedule evidence/fleet-2026-09-02/b3-gap-gate-threshold/fixtures/schedule2.json \
  --content-detector highpass --minimum-silence-ms 20
# expect: continuity.gap_events == 1, failures includes
# "1 rendered-audio gap(s) exceeded 20.0 ms"
```

## Commands run, exit codes

| Command | Exit |
| --- | --- |
| `ruff check audio-gate/gate.py` (before edit, copy of HEAD) | 1 (E402, pre-existing) |
| `ruff check audio-gate/gate.py` (after edit) | 1 (same E402, unchanged) |
| `backend/relay-py/.venv/bin/python -m pytest tests/ -q` (before edit) | 0 (395 passed) |
| `backend/relay-py/.venv/bin/python -m pytest tests/ -q` (after edit) | 0 (395 passed) |
| `gate.py` against `capture2.wav`, default 250ms (before fix) | 1 (fails, but NOT on the gap) |
| `gate.py` against `capture2.wav`, default 20ms (after fix) | 1 (fails, including on the gap) |
| `gate.py` against `capture3_nogap.wav`, default 20ms (after fix) | 1 (fails, but NOT on any gap) |

## Files changed

- `audio-gate/gate.py` — `--max-gap-ms` default `250.0` → `20.0` + provenance comment;
  `turns_exceeding_250ms` → `turns_exceeding_max_gap_ms` report key rename.
- `audio-gate/README.md` — updated stated default and cited blueprint section.
- `evidence/fleet-2026-09-02/b3-gap-gate-threshold/` — this report, before/after/control JSON,
  synthetic WAV fixtures, pytest log, ruff log, `diff.patch`.
