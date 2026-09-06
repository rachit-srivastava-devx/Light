#!/usr/bin/env python3
"""
TIAGE — A Benchmark for Topic-Shift Aware Dialog Modeling (Xie et al., Findings of EMNLP 2021).

Source: https://github.com/HuiyuanXie/tiage (MIT License, verified in repo LICENSE file)
Underlying dialogues: PersonaChat (crowd-sourced open-domain chit-chat), re-annotated by TIAGE
authors with per-utterance human topic-shift labels.

Raw format (data/personachat/anno/{train,dev,test}/anno_{split}.json):
  {"<dialogue_id>": [[utterance_text, shift_label_str], ...], ...}
  shift_label: "-1" = first turn (no prior turn to compare against, undefined),
               "0"  = no topic shift at this turn relative to previous turn,
               "1"  = human-annotated topic shift at this turn.

This script downloads the three raw split files verbatim (no re-derivation of labels),
counts dialogues/turns/shift-labelled positions with code, and converts to the shared
golden-eval JSONL schema. No fabricated data — every turn and label is copied from the
downloaded file.
"""
import json
import random
import urllib.request
from pathlib import Path

HERE = Path(__file__).parent
RAW = HERE / "raw"
RAW.mkdir(exist_ok=True)

FILES = {
    "train": "https://raw.githubusercontent.com/HuiyuanXie/tiage/main/data/personachat/anno/train/anno_train.json",
    "dev": "https://raw.githubusercontent.com/HuiyuanXie/tiage/main/data/personachat/anno/dev/anno_dev.json",
    "test": "https://raw.githubusercontent.com/HuiyuanXie/tiage/main/data/personachat/anno/test/anno_test.json",
}

CAP = 2000
SEED = 0


def download():
    for split, url in FILES.items():
        dest = RAW / f"anno_{split}.json"
        if not dest.exists():
            print(f"downloading {url}")
            urllib.request.urlretrieve(url, dest)
        else:
            print(f"already have {dest}")


def load_split(split):
    with open(RAW / f"anno_{split}.json") as f:
        return json.load(f)


def main():
    download()

    all_records = []
    per_split_dialogue_counts = {}
    per_split_turn_counts = {}
    per_split_shift_counts = {}

    for split in ("train", "dev", "test"):
        data = load_split(split)
        per_split_dialogue_counts[split] = len(data)
        turn_count = 0
        shift_count = 0
        for dialogue_id, utterances in data.items():
            turns = []
            shift_labels = []
            for i, (text, label) in enumerate(utterances):
                role = "user" if i % 2 == 0 else "other"
                turns.append({"role": role, "text": text})
                shift_labels.append(int(label))
                if label == "1":
                    shift_count += 1
            turn_count += len(turns)
            shift_positions = [i for i, label_val in enumerate(shift_labels) if label_val == 1]
            all_records.append(
                {
                    "id": f"tiage-{split}-{dialogue_id}",
                    "source": "TIAGE (personachat)",
                    "turns": turns,
                    "labels": {
                        "split": split,
                        "topic_shift_labels": shift_labels,
                        "topic_shift_turn_indices": shift_positions,
                        "has_topic_shift": len(shift_positions) > 0,
                    },
                }
            )
        per_split_turn_counts[split] = turn_count
        per_split_shift_counts[split] = shift_count

    total_dialogues = len(all_records)
    total_with_shift = sum(1 for r in all_records if r["labels"]["has_topic_shift"])

    # Deterministic sampling if over cap (not needed here, but keep the mechanism honest
    # and reusable — TIAGE total is small, so this should be a no-op that we verify below).
    if total_dialogues > CAP:
        rng = random.Random(SEED)
        all_records = rng.sample(all_records, CAP)
        sampled = True
    else:
        sampled = False

    out_path = HERE / "conversations.jsonl"
    with open(out_path, "w") as f:
        for r in all_records:
            f.write(json.dumps(r, ensure_ascii=False) + "\n")

    print("=== TIAGE counts (real, counted from downloaded files) ===")
    print("per-split dialogue counts:", per_split_dialogue_counts)
    print("per-split turn counts:", per_split_turn_counts)
    print("per-split turns labelled shift=1 counts:", per_split_shift_counts)
    print("TOTAL dialogues (train+dev+test):", total_dialogues)
    print("TOTAL dialogues containing >=1 labelled topic shift:", total_with_shift)
    print("sampled down to cap:", sampled, f"(cap={CAP})")
    print("wrote:", out_path, "lines:", sum(1 for _ in open(out_path)))


if __name__ == "__main__":
    main()
