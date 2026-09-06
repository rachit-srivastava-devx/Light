# AnnoMI — provenance

**Source URL:** https://github.com/uccollab/AnnoMI
**Papers:**
- Wu, Balloccu, Kumar, Helaoui, Reiter, Reforgiato Recupero, Riboni, "Anno-MI: A Dataset of
  Expert-Annotated Counselling Dialogues", ICASSP 2022.
- Wu et al., "Creation, Analysis and Evaluation of AnnoMI, a Dataset of Expert-Annotated
  Counselling Dialogues", Future Internet 15(3), 2023 (the "full" release used here).

## Download command that worked

```bash
curl -s "https://raw.githubusercontent.com/uccollab/AnnoMI/main/AnnoMI-full.csv" -o raw/AnnoMI-full.csv
```
(automated by `fetch_and_convert.py`)

## Licence — ⚠ NOT COMMITTED, no clear redistribution grant

The `uccollab/AnnoMI` GitHub repo has **no `LICENSE` file** and its `README.md` and both
cited papers state only a citation request, not a redistribution licence. A third-party
Hugging Face re-upload (`to-be/annomi-motivational-interviewing-therapy-conversations`)
tags itself `license:openrail`, but that is the re-uploader's own label, not a grant from
the dataset's authors — it carries no authority.

Per the task's hard rule ("if a licence forbids redistribution, keep the download script and
do not commit the data"), this project treats "no licence found" the same way: unclear is
not the same as granted. **`conversations.jsonl` is generated locally by
`fetch_and_convert.py` (so it can be counted and sanity-checked, see below) but is listed in
`../.gitignore` and must not be committed.** Only this file, the script, and the three short
verbatim lines below are checked in. If you need the actual data, run the script yourself
after confirming terms of use with the dataset's authors.

## Counts (counted by `fetch_and_convert.py` directly from the downloaded CSV)

- **13,551 total CSV data rows** (`AnnoMI-full.csv`, excluding header).
- **133 distinct transcripts (dialogues)** — independently confirms the dataset card's
  claimed "133 conversations."
- **9,699 distinct (transcript, utterance) turns.**
- **428 utterance-keys (4.4%) were annotated by more than one of the 10 annotators**
  (multi-annotator inter-rater-agreement rows); of those, **258 had actual disagreement**
  between annotators on `main_therapist_behaviour`/`client_talk_type`. Rather than fabricate
  a synthetic majority-vote label, we deterministically keep the real row from the lowest
  `annotator_id` present for that utterance — a real annotator's real recorded label.
- **mi_quality split: 110 transcripts "high", 23 transcripts "low".**
- No sampling needed — 133 dialogues is far under the 2,000 cap.

## Example (verbatim, first 2 turns of one `high` and one `low` mi_quality transcript — both on the same topic, "reducing alcohol consumption")

```
HIGH quality (annomi-0):
  other: Thanks for filling it out. We give this form to everyone once a year regardless of
         why they come in. It helps us provide better care. Is it okay if I take a look at
         what you put down?
  user:  Sure.

LOW quality (annomi-9):
  other: So now that we've discussed a little bit of your medical history and- and taking
         your vitals, I'd like to ask you a few more questions. The- the rise in your blood
         pressure is concerning to me, so I'd like to gather a bit more information about
         your lifestyle.
  user:  Okay.
```

## Orb-relevance

This is the highest-value dataset for **load-direction** (does the assistant carry the
executive/decision load, or hand it back?): every therapist utterance is labelled
`main_therapist_behaviour` (reflection / question / therapist_input / other) and every
transcript is labelled `mi_quality` (high/low). Low-quality transcripts are real,
professionally-annotated examples of a counsellor doing the thing the orb must never do
(directing without inviting, or leaving the client without a concrete next move); high-
quality ones show the alternative. This is real expert-annotated evidence of the property,
not a proxy — contingent on resolving the licence question above before using it beyond
local counting/inspection.
