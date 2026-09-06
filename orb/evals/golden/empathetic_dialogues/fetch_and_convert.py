#!/usr/bin/env python3
"""
EmpatheticDialogues (Rashkin et al., ACL 2019) — Facebook AI / ParlAI.

Source (original, authoritative — NOT the HF loading-script mirror, which `datasets` 5.x
refuses to execute): https://dl.fbaipublicfiles.com/parlai/empatheticdialogues/empatheticdialogues.tar.gz
Licence: CC BY-NC 4.0 (facebookresearch/EmpatheticDialogues GitHub repo LICENSE file).
Allows redistribution with attribution, non-commercial use — flagged in PROVENANCE.md.

Raw format: train.csv / valid.csv / test.csv, one row per utterance:
  conv_id, utterance_idx, context (=emotion label), prompt (situation description,
  constant per conv_id), speaker_idx, utterance, selfeval, tags.
Known quirk of this dataset's own CSV export: literal commas inside `prompt`/`utterance`
are escaped as the substring "_comma_" (not real CSV escaping). We reverse that exact,
documented substitution back to "," — this is decoding the source's own encoding, not
inventing text.

conv_id numbering restarts inside each split file, so we prefix with the split name to keep
IDs globally unique; the underlying utterance text and ordering are untouched.
"""
import csv
import json
import random
import tarfile
import urllib.request
from collections import defaultdict
from pathlib import Path

HERE = Path(__file__).parent
RAW = HERE / "raw"
RAW.mkdir(exist_ok=True)

TARBALL_URL = "https://dl.fbaipublicfiles.com/parlai/empatheticdialogues/empatheticdialogues.tar.gz"
TARBALL_PATH = RAW / "empatheticdialogues.tar.gz"
EXTRACT_DIR = RAW / "empatheticdialogues"

CAP = 2000
SEED = 0


def download():
    if not TARBALL_PATH.exists():
        print(f"downloading {TARBALL_URL}")
        urllib.request.urlretrieve(TARBALL_URL, TARBALL_PATH)
    else:
        print(f"already have {TARBALL_PATH}")
    if not EXTRACT_DIR.exists():
        with tarfile.open(TARBALL_PATH) as tf:
            tf.extractall(RAW)


def undo_comma_escape(text):
    return text.replace("_comma_", ",")


def load_split(split):
    path = EXTRACT_DIR / f"{split}.csv"
    with open(path, newline="", encoding="utf-8") as f:
        return list(csv.DictReader(f))


def main():
    download()

    all_records = []
    per_split_dialogue_counts = {}
    per_split_turn_counts = {}
    emotion_counter = defaultdict(int)

    for split in ("train", "valid", "test"):
        rows = load_split(split)
        by_conv = defaultdict(list)
        for row in rows:
            by_conv[row["conv_id"]].append(row)

        per_split_dialogue_counts[split] = len(by_conv)
        turn_count = 0
        for conv_id, conv_rows in by_conv.items():
            conv_rows.sort(key=lambda r: int(r["utterance_idx"]))
            turns = []
            for i, row in enumerate(conv_rows):
                role = "user" if i % 2 == 0 else "other"
                turns.append({"role": role, "text": undo_comma_escape(row["utterance"])})
            turn_count += len(turns)
            emotion = conv_rows[0]["context"]
            emotion_counter[emotion] += 1
            all_records.append(
                {
                    "id": f"empatheticdialogues-{split}-{conv_id}",
                    "source": "EmpatheticDialogues",
                    "turns": turns,
                    "labels": {
                        "split": split,
                        "emotion": emotion,
                        "situation_prompt": undo_comma_escape(conv_rows[0]["prompt"]),
                    },
                }
            )
        per_split_turn_counts[split] = turn_count

    total_dialogues = len(all_records)

    rng = random.Random(SEED)
    if total_dialogues > CAP:
        # Sort first so the sample is reproducible independent of dict/OS ordering.
        all_records.sort(key=lambda r: r["id"])
        sampled_records = rng.sample(all_records, CAP)
        sampled = True
    else:
        sampled_records = all_records
        sampled = False

    out_path = HERE / "conversations.jsonl"
    with open(out_path, "w") as f:
        for r in sampled_records:
            f.write(json.dumps(r, ensure_ascii=False) + "\n")

    print("=== EmpatheticDialogues counts (real, counted from downloaded CSVs) ===")
    print("per-split dialogue (conv_id) counts:", per_split_dialogue_counts)
    print("per-split turn counts:", per_split_turn_counts)
    print("TOTAL dialogues (train+valid+test):", total_dialogues)
    print("distinct emotion labels:", len(emotion_counter))
    print(f"sampled down to cap={CAP} (seed={SEED}):", sampled, "-> kept", len(sampled_records))
    print("wrote:", out_path, "lines:", sum(1 for _ in open(out_path)))


if __name__ == "__main__":
    main()
