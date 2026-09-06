# EmpatheticDialogues — provenance

**Source URL (original, authoritative):**
https://dl.fbaipublicfiles.com/parlai/empatheticdialogues/empatheticdialogues.tar.gz
**Paper:** Rashkin, Smith, Li, Boureau, "Towards Empathetic Open-domain Conversation Models:
A New Benchmark and Dataset", ACL 2019.
**Repo:** https://github.com/facebookresearch/EmpatheticDialogues

We used the original Facebook/ParlAI distribution directly rather than the Hugging Face
mirror (`facebook/empathetic_dialogues`), because that HF repo is script-based
(`empathetic_dialogues.py`) and requires `trust_remote_code=True`; newer `datasets` releases
(we're on 5.0.1) have removed script execution entirely (see `daily_dialog/PROVENANCE.md`
for the exact error). Plain HTTPS to the original tarball sidesteps that completely and is
the more authoritative source anyway.

## Download command that worked

```bash
curl -s --max-time 60 "https://dl.fbaipublicfiles.com/parlai/empatheticdialogues/empatheticdialogues.tar.gz" -o raw/empatheticdialogues.tar.gz
tar -xzf raw/empatheticdialogues.tar.gz -C raw/
```
(automated by `fetch_and_convert.py`)

## Licence

**CC BY-NC 4.0** — declared in the `LICENSE` file of
`github.com/facebookresearch/EmpatheticDialogues`. Allows redistribution with attribution;
**NonCommercial only.** Committed to this repo for eval/research use; do not fold into any
commercially-distributed training corpus without separately clearing this term.

## Counts (counted by `fetch_and_convert.py` directly from the downloaded CSVs)

| split | dialogues (`conv_id`) | turns |
|---|---|---|
| train | 17,844 | 76,673 |
| valid | 2,763 | 12,030 |
| test | 2,542 | 10,943 |
| **total** | **23,149** | **99,646** |

(The per-split turn counts match the HF dataset card's row counts exactly — 76,673 /
12,030 / 10,943 — which cross-checks that the CSV parsing above is correct.)

- **32 distinct emotion labels** across the corpus.
- **23,149 total dialogues** → **sampled to 2,000** (the ~2,000-dialogue cap), deterministic,
  `seed=0`, via `random.Random(0).sample(...)` over the full sorted list of dialogue records.

## Example (verbatim, `empatheticdialogues-train-hit:2723_conv:5446`, emotion=`grateful`)

Situation prompt: *"I was having dinner and just praised the Lord for it."*

```
user:  Hi, I was having dinner yesterday and just stopped for a moment to thank the Lord.
other: That is gracious of you. I haven't done that in awhile.
user:  You should do it more often, I was thankful to have food on the table!
```

Note: the source CSV encodes literal commas inside `utterance`/`prompt` text as the literal
substring `_comma_` (a quirk of how the original authors wrote the CSV). We reverse that
exact substitution back to `,` — this is decoding the source's own encoding, not adding or
inventing any text.

## Orb-relevance

Exercises **open-domain discussion** and, in aggregate as a distribution shift, the general
tone of empathetic listening the orb needs when a user brings up something emotionally
loaded mid-task. It does not by itself test topic-shift-and-return, interruption, or
load-direction — see `evals/golden/INDEX.md`.
