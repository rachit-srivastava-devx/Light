# DailyDialog — provenance

**Paper:** Li, Su, Shen, Li, Cao, Niu, "DailyDialog: A Manually Labelled Multi-turn Dialogue
Dataset", IJCNLP 2017.
**Original host (dead):** `yanran.li` — verified dead: `curl -sL http://yanran.li/files/ijcnlp_dailydialog.zip`
returns HTTP 200 but the body is a domain-parking HTML page ("This domain may be for
sale."), not the zip.
**Canonical HF repos** (`li2017dailydialog/daily_dialog`, `roskoN/dailydialog`) both ship the
data behind a Python loading script. Tried loading the canonical one first:

```
$ ./.venv/bin/python -c "from datasets import load_dataset; load_dataset('li2017dailydialog/daily_dialog', trust_remote_code=True)"
RuntimeError: Dataset scripts are no longer supported, but found daily_dialog.py
```

(`datasets` 5.0.1 has removed script execution entirely — this is not a transient error.)
**Source actually used:** `roskoN/dailydialog` exposes the same original Li et al. 2017
files as plain per-split zip archives (not behind the script), which sidesteps the issue:
https://huggingface.co/datasets/roskoN/dailydialog

## Download command that worked

```bash
curl -sL "https://huggingface.co/datasets/roskoN/dailydialog/resolve/main/train.zip"      -o raw/train.zip
curl -sL "https://huggingface.co/datasets/roskoN/dailydialog/resolve/main/validation.zip" -o raw/validation.zip
curl -sL "https://huggingface.co/datasets/roskoN/dailydialog/resolve/main/test.zip"       -o raw/test.zip
unzip -o -q raw/train.zip -d raw/train   # etc. for validation, test
```
(automated by `fetch_and_convert.py`)

Verified this is the unmodified original distribution by inspecting the extracted files
directly: three parallel, line-aligned per-split files —
`dialogues_{split}.txt` (utterances joined by literal `"__eou__ "`),
`dialogues_act_{split}.txt`, `dialogues_emotion_{split}.txt` (space-separated integer codes)
— exactly the format documented in the original paper/README, with no reformatting.

## Licence

**CC BY-NC-SA 4.0** — declared on both HF dataset pages. Allows redistribution with
attribution and share-alike; **NonCommercial only.** Committed to this repo for eval/research
use; do not fold into any commercially-distributed training corpus, and any derivative must
carry the same licence, without separately clearing this term.

## Counts (counted by `fetch_and_convert.py` directly from the downloaded files)

| split | dialogues |
|---|---|
| train | 11,118 |
| validation | 1,000 |
| test | 1,000 |
| **total** | **13,118** |

- All three parallel files (`dialogues_*.txt`, `dialogues_act_*.txt`,
  `dialogues_emotion_*.txt`) were asserted line-count-equal and per-utterance-count-equal
  during conversion — no silent misalignment.
- **13,118 total dialogues** → **sampled to 2,000**, deterministic, `seed=0`.
- Dialogue-act codes: 1=inform, 2=question, 3=directive, 4=commissive (0=unused
  `__dummy__`). Emotion codes: 0=no emotion, 1=anger, 2=disgust, 3=fear, 4=happiness,
  5=sadness, 6=surprise. Both mappings are exactly as documented in the original paper —
  stored as both the raw ids and the mapped label strings in `labels`.

## Example (verbatim, `dailydialog-train-4772`)

```
user:  Good evening , Mr . Frank . Bourbon on the rocks ?          [act=question, emotion=no emotion]
other: No . This time I'll try Chinese wine .                       [act=inform,   emotion=no emotion]
user:  What about Mao Tai , one of the most famous liquors in China ?
       It's good indeed . It never goes to t[o your head] ...        [act=directive, emotion=no emotion]
```

## Orb-relevance

Exercises **open-domain, everyday multi-turn chit-chat** as a baseline of ordinary
conversational flow (topic changes happen naturally across turns but are not labelled as
such — unlike TIAGE, there is no ground-truth shift annotation here). Dialogue-act labels
(`question`/`directive`/`commissive`/`inform`) are a weak, indirect signal for
**load-direction** (a `directive` from the assistant role is closer to "proposing the next
step" than a bare `question` handing it back), but this is not an expert-annotated
load-direction label the way AnnoMI's is — treat it as a volume complement, not a substitute.
