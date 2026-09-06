# TIAGE — provenance

**Source URL:** https://github.com/HuiyuanXie/tiage
**Paper:** Xie, Huiyuan et al., "TIAGE: A Benchmark for Topic-Shift Aware Dialog Modeling",
Findings of ACL: EMNLP 2021. https://arxiv.org/abs/2109.04562
**Underlying dialogues:** PersonaChat (crowd-sourced open-domain chit-chat), re-annotated by
the TIAGE authors with per-utterance human topic-shift labels.

## Download command that worked

```bash
curl -s "https://raw.githubusercontent.com/HuiyuanXie/tiage/main/data/personachat/anno/train/anno_train.json" -o raw/anno_train.json
curl -s "https://raw.githubusercontent.com/HuiyuanXie/tiage/main/data/personachat/anno/dev/anno_dev.json"     -o raw/anno_dev.json
curl -s "https://raw.githubusercontent.com/HuiyuanXie/tiage/main/data/personachat/anno/test/anno_test.json"   -o raw/anno_test.json
```
(automated by `fetch_and_convert.py`, which also does the counting and conversion below)

## Licence

**MIT License**, copyright 2021 Huiyuan Xie — confirmed by reading the repo's `LICENSE` file
directly (`https://raw.githubusercontent.com/HuiyuanXie/tiage/main/LICENSE`). Permissive,
redistribution allowed. Committed to this repo.

## Counts (counted by `fetch_and_convert.py`, not the paper's claimed numbers)

| split | dialogues | turns | turns labelled `topic_shift=1` |
|---|---|---|---|
| train | 300 | 4,692 | 887 |
| dev | 100 | 1,546 | 311 |
| test | 100 | 1,564 | 315 |
| **total** | **500** | **7,802** | **1,513** |

- **500 dialogues total**, of which **479 (95.8%) contain at least one labelled topic shift.**
  (21 dialogues have zero human-labelled shifts — i.e. the annotators judged the whole
  conversation to stay on one topic.)
- No sampling needed — 500 < the 2,000-dialogue cap, so all three splits are kept whole.
- Label scheme, exactly as stored in the source JSON: `"-1"` = first turn (no prior turn to
  compare against), `"0"` = no topic shift at this turn, `"1"` = human-annotated topic shift
  at this turn. Speaker role (`user`/`other`) is assigned by turn position (PersonaChat has
  no inherent user/assistant asymmetry — both sides are crowd workers), alternating from
  turn 0.

## Example (verbatim, `tiage-train-1`, labelled shift at turn index 4)

```
[0] user:  hey ! do you love cats ?
[1] other: hey . . . i am a dog person , i have two
[2] user:  ah that is cool , i have two cats and got a collection of 1000 hats for them !
[3] other: wow ! ! ! that is a lot lol
[4] user:  yeah , i have a weakness for cats and vanilla ice cream , they are the best !   <-- labelled topic shift
[5] other: my weakness is eating when i am bored
```

## Orb-relevance

Exercises **topic-shift-and-return**: every turn carries a human ground-truth label for
whether the conversation just pivoted topic. It does *not* by itself test "return to the
earlier thread" — PersonaChat turns don't loop back to a prior subject in a labelled way, so
this dataset proves shift *detection*, not shift-and-recovery. See `evals/golden/INDEX.md`
for the explicit gap this leaves.
