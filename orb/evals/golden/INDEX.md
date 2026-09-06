# Golden eval set — real human-conversation datasets

Every row below is a real, publicly-downloaded dataset — nothing in `evals/golden/` was
authored, synthesised, or hand-written. Each dataset directory has its own
`PROVENANCE.md` with the exact source URL, the exact download command that worked, the
licence, counts obtained by running code against the actual downloaded files (not the
numbers claimed on a dataset card), and 2-3 verbatim example turns.

## Summary table

| Dataset | Population (real total) | In this eval set | Turns in set | Licence | Committed to repo? | Orb propert(y/ies) exercised |
|---|---|---|---|---|---|---|
| [TIAGE](tiage/PROVENANCE.md) | 500 dialogues | 500 (no sampling needed) | 7,802 | MIT | **Yes** | topic-shift (detection half only — see gap below) |
| [AnnoMI](annomi/PROVENANCE.md) | 133 dialogues | 133 (no sampling needed) | 9,699 | **No licence found in source repo** | **No — script + docs only, see below** | **load-direction** (strongest evidence of the five) |
| [EmpatheticDialogues](empathetic_dialogues/PROVENANCE.md) | 23,149 dialogues | 2,000 (sampled, seed=0) | 8,679 | CC BY-NC 4.0 | Yes | open-domain (emotional register) |
| [DailyDialog](daily_dialog/PROVENANCE.md) | 13,118 dialogues | 2,000 (sampled, seed=0) | 15,535 | CC BY-NC-SA 4.0 | Yes | open-domain (everyday chit-chat); weak load-direction proxy via dialogue-act labels |
| [Schema-Guided Dialogue (SGD)](sgd/PROVENANCE.md) | 22,825 dialogues (15,255 multi-domain) | 2,000 (sampled, seed=0; 1,311 multi-domain) | 40,658 | CC BY-SA 4.0 | Yes | topic-shift-and-return (task-domain-switch form); context-carry |

**Totals:** 6,633 dialogues obtained and converted across all five sources (500 + 133 + 2,000
+ 2,000 + 2,000); **6,500 of those are actually committed to this repo** (AnnoMI's 133 are
counted, documented, and verified but withheld — see below). **72,674 turns total across the
four committed sets** (7,802 + 8,679 + 15,535 + 40,658 for TIAGE / EmpatheticDialogues /
DailyDialog / SGD respectively); AnnoMI's 9,699 turns were counted and verified the same way
but are not part of that committed total, since the data itself isn't committed.

All four sampled/capped datasets used `random.Random(seed=0).sample(...)` over the full,
deterministically-sorted population of dialogue records — reproducible by re-running each
`fetch_and_convert.py`.

## The five orb properties, mapped to real evidence (and the gaps, honestly)

The orb needs: **(a)** context-carry, **(b)** topic-shift-and-return, **(c)**
interruption/"stop", **(d)** load-direction (never hand the mental load back), **(e)**
open-domain discussion-then-return-to-task.

| Property | Real data found? | Which dataset(s) | Caveat |
|---|---|---|---|
| (a) context-carry | Partial | All five, implicitly (every dialogue here is multi-turn and only coheres if context is carried) | **No dataset in this set has an explicit "context was/wasn't carried correctly" label.** SGD comes closest — slot/state values must persist correctly across turns, including across the domain switch — but that's a task-state signal, not a general conversational-memory label. Multi-turn text alone is raw material for a context-carry eval, not a ready-made one. |
| (b) topic-shift-and-return | **Partial — shift yes, return no** | TIAGE (per-turn human shift label, open-domain); SGD (services list proves domain switching, task-oriented) | Neither dataset labels the *return* half. TIAGE's shift label is binary (shifted / didn't) with no marker for "this shift is a return to an earlier subject." SGD dialogues that switch domain (e.g. restaurant → movie, see `sgd/PROVENANCE.md`) generally finish in the new domain rather than looping back. **We found no real dataset, among the five candidates or the papers/benchmarks they cite, that labels a conversation shifting away from a topic and then explicitly returning to it.** This is a genuine gap, not a failure to search — see the note on TurnBench/Full-Duplex-Bench/SpokenTOD below for why we didn't chase it further right now. |
| (c) interruption / "stop" | **None found** | — | All five datasets are clean, non-overlapping, turn-by-turn text — no barge-in, no "user says stop mid-response," no yielding behaviour of any kind. This property has **zero coverage** in this golden set. A supplementary search (not one of the five requested candidates, so not pursued further here) surfaced audio/prosody turn-taking benchmarks — **TurnBench** (turnbench.sesame.com), **Full-Duplex-Bench**, and **SpokenTOD/SpokenUS** (barge-in-augmented spoken task dialogue) — as possible leads for a *follow-up* task. None of these were downloaded or verified; they are unverified leads, not obtained data, and should not be treated as if they were. |
| (d) load-direction | **Yes — real, expert-annotated** | **AnnoMI** (`main_therapist_behaviour`, `mi_quality` high/low) | The single strongest piece of evidence in this set: 133 real counselling transcripts where a professional either does or does not carry the direction of the session, with expert labels for both. **Licence is unresolved (see below) — usable for local counting/inspection now, not yet cleared for anything beyond that.** DailyDialog's dialogue-act labels (`directive` vs `question`) are a much weaker secondary signal, included as a volume complement only. |
| (e) open-domain, discuss-then-return-to-task | **Partial — open-domain yes, return-to-task no** | EmpatheticDialogues, DailyDialog (open-domain); SGD (task-oriented, for contrast) | We have open-domain conversation at volume and we have task-oriented conversation at volume, but **no dataset in this set — or that we're aware of — has the specific hybrid structure the orb needs: a session that starts on-task, digresses into open-domain chat, and is then steered back to the task.** That composite structure appears to be genuinely novel to this product; the closest existing proxy is TIAGE/SGD's shift-detection (property b) plus EmpatheticDialogues/DailyDialog's open-domain register (property e), used together rather than any single dataset covering the full loop. |

**Bottom line:** of the five orb properties, two have solid real-data coverage (load-direction
via AnnoMI, open-domain via EmpatheticDialogues/DailyDialog), two have half-coverage (shift
detection and domain-switching exist; the "return" half does not, for either the
conversational or task-oriented case), and one — interruption/"stop" handling — has no
coverage at all in this set. The half-coverage and no-coverage findings are the two most
useful outputs of this task: they tell you exactly what a golden eval can't yet catch, rather
than padding the set with synthetic data that would quietly hide the gap.

## Why AnnoMI's data isn't committed

`github.com/uccollab/AnnoMI` has no `LICENSE` file, and neither its `README.md` nor its two
cited papers state redistribution terms — a third-party HF mirror's self-applied `openrail`
tag is not an authoritative grant. The task's hard rule is "if a licence forbids
redistribution, keep the download script and do not commit the data"; we extended that same
caution to "no licence statement found at all," rather than assume permission by default.
`annomi/fetch_and_convert.py` and `annomi/PROVENANCE.md` are committed; `annomi/conversations.jsonl`
is generated locally (so the counts above are real, not estimated) but is excluded via
`.gitignore` and must not be committed. If AnnoMI's authors are contacted and confirm terms,
remove that `.gitignore` line and re-run the script.

## NonCommercial-licensed datasets — flag for downstream use

EmpatheticDialogues (CC BY-NC 4.0) and DailyDialog (CC BY-NC-SA 4.0) are committed here
because eval/research use is exactly what their licences permit, but **neither may be folded
into a commercially-distributed training corpus** without separately clearing that term.
TIAGE (MIT) and SGD (CC BY-SA 4.0) carry no such restriction.

## Reproducing this set

```bash
cd company/products/adhd-focus-orb/evals/golden
python3.12 -m venv .venv   # fresh, isolated — never backend/relay-py/.venv or evals/.venv
./.venv/bin/pip install --upgrade pip
./.venv/bin/pip install datasets huggingface_hub pandas pyarrow
./.venv/bin/python tiage/fetch_and_convert.py
./.venv/bin/python annomi/fetch_and_convert.py
./.venv/bin/python empathetic_dialogues/fetch_and_convert.py
./.venv/bin/python daily_dialog/fetch_and_convert.py
./.venv/bin/python sgd/fetch_and_convert.py
```

Note: the `datasets`/`huggingface_hub` install succeeded here (Python 3.12, `datasets`
5.0.1), but ended up unused by three of the five scripts — `datasets` 5.x has removed
Python-loading-script execution entirely, which is exactly how DailyDialog's and
EmpatheticDialogues's canonical HF repos ship. Those two scripts, and SGD, fetch raw
files over plain HTTPS instead (see each dataset's `PROVENANCE.md` for the exact
error/reasoning). TIAGE and AnnoMI were plain-HTTPS from GitHub from the start.
