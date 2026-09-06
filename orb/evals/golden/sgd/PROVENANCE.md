# Schema-Guided Dialogue (SGD) — provenance

**Source URL:** https://github.com/google-research-datasets/dstc8-schema-guided-dialogue
**Paper:** Rastogi, Zang, Sunkara, Gupta, Khaitan, "Towards Scalable Multi-Domain
Conversational Agents: The Schema-Guided Dialogue Dataset", AAAI 2020 (DSTC8 task).

**Chosen over MultiWOZ 2.2:** both were candidates for "task-oriented multi-domain dialogue
with domain switching." SGD was picked because it downloads and licenses cleanly (plain
JSON files on GitHub, single tarball, no Hugging Face loading-script issue — the same
problem hit `daily_dialog` and `empathetic_dialogues`'s canonical HF repos, see those
PROVENANCE.md files) and its schema explicitly records, per dialogue, the full list of
services/domains touched — a direct, non-inferred signal for domain switching.

## Download command that worked

```bash
curl -sL --max-time 120 "https://codeload.github.com/google-research-datasets/dstc8-schema-guided-dialogue/tar.gz/refs/heads/master" -o raw/sgd.tar.gz
tar -xzf raw/sgd.tar.gz -C raw/extracted --include='dstc8-schema-guided-dialogue-master/train/*' --include='dstc8-schema-guided-dialogue-master/dev/*' --include='dstc8-schema-guided-dialogue-master/test/*'
```
(automated by `fetch_and_convert.py`, which also deletes the ~500MB extracted copy after
conversion and keeps only the 36MB tarball as a reproducible local cache — see `../.gitignore`,
neither is committed)

## Licence

**CC BY-SA 4.0** — declared in the repo's `LICENSE.txt` and restated in its `README.md`:
"The SGD and SGD-X datasets are released under CC BY-SA 4.0 license." Allows redistribution
with attribution and share-alike. No NonCommercial restriction. Committed to this repo.

## Counts (counted by `fetch_and_convert.py` directly from the downloaded JSON shards)

| split | dialogues | multi-domain (services > 1) |
|---|---|---|
| train | 16,142 | 10,739 |
| dev | 2,482 | 1,646 |
| test | 4,201 | 2,870 |
| **total** | **22,825** | **15,255** |

- **22,825 total dialogues, of which 15,255 (66.8%) touch 2 or more services/domains** —
  i.e. are real, human-collected examples of a conversation switching domains mid-session
  (e.g. booking a restaurant, then a movie, in the same conversation).
- **22,825 total → sampled to 2,000**, deterministic, `seed=0`, drawn from the *full,
  unfiltered* population (not pre-filtered to multi-domain only) so the sample's ratio still
  reflects the real corpus: **1,311 of the 2,000 sampled dialogues (65.6%) are multi-domain**
  — closely tracking the population's 66.8%, which is a sanity check that the sampling
  wasn't biased. Every record keeps `services` / `num_services` / `is_multi_domain` in
  `labels` so a consumer can filter to multi-domain-only downstream if a purer signal is
  wanted.

## Example (verbatim, `sgd-train-113_00009`, services=[`Restaurants_1`, `Movies_1`])

```
[0]  user:  I want to go out to eat. I'd like to try a pasta restaurant in SF.
[1]  other: I found 10 such places. You could try 54 Mint Ristorante Italiano in San Francisco.
...
[6]  user:  That works for me. I'd also like to see a movie when I'm in the area.
[7]  other: Well, films like Ash Is Purest White, Body Double, and Captain Marvel are in the
            theaters right now.
```
(turn 6 is where the conversation switches from the `Restaurants_1` domain to `Movies_1`,
inside one continuous session — a real example of the domain-switch property, not a
synthesised one)

## Orb-relevance

Exercises **topic-shift-and-return** in its task-oriented form (domain switching mid-session)
and **context-carry** (task-oriented dialogue state must persist correctly across the
switch — e.g. the restaurant city/cuisine choices from turns 0-5 remain valid state even
after the movie sub-conversation starts). It does not label an explicit "return to the
earlier thread" moment the way the orb needs (once SGD switches domain, dialogues typically
finish in the new domain rather than looping back) — see `evals/golden/INDEX.md` for that
gap.
