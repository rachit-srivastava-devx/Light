#!/usr/bin/env python3
"""
DailyDialog (Li et al., IJCNLP 2017) — everyday multi-turn chit-chat with dialogue-act and
emotion labels per utterance.

Source: the original host (yanran.li) is dead (domain parked/for sale, verified by HTTP
fetch returning a parking page). The canonical HF repos (li2017dailydialog/daily_dialog,
roskoN/dailydialog) both ship the data behind a Python *loading script*; `datasets` 5.x has
removed script execution entirely ("Dataset scripts are no longer supported" — reproduced
directly from this venv), so `load_dataset(...)` cannot be used. Instead we download the
raw train/validation/test .zip files that roskoN/dailydialog exposes as plain repo files
(no script execution needed) directly over HTTPS:
  https://huggingface.co/datasets/roskoN/dailydialog/resolve/main/{train,validation,test}.zip
This is the original Li et al. 2017 distribution (three plain-text files per split,
"__eou__"-separated utterances + parallel act/emotion label files) with no reformatting —
verified by inspecting the extracted files directly.

Licence: CC BY-NC-SA 4.0 (declared on the HF dataset pages) — allows redistribution with
attribution + share-alike, NonCommercial. Flagged in PROVENANCE.md; do not use as
commercially-distributed training data without checking this clause.

Raw format per split, three parallel line-aligned files (line N = dialogue N in all three):
  dialogues_{split}.txt         - utterances joined by literal "__eou__ "
  dialogues_act_{split}.txt     - space-separated act ids per utterance (1=inform, 2=question,
                                   3=directive, 4=commissive; 0 is an unused "__dummy__" code)
  dialogues_emotion_{split}.txt - space-separated emotion ids per utterance (0=no emotion,
                                   1=anger, 2=disgust, 3=fear, 4=happiness, 5=sadness, 6=surprise)
These id->name mappings are exactly as documented in the original DailyDialog paper/README.
"""
import json
import random
import urllib.request
import zipfile
from pathlib import Path

HERE = Path(__file__).parent
RAW = HERE / "raw"
RAW.mkdir(exist_ok=True)

BASE_URL = "https://huggingface.co/datasets/roskoN/dailydialog/resolve/main"
SPLITS = ["train", "validation", "test"]

CAP = 2000
SEED = 0

ACT_LABELS = {0: "__dummy__", 1: "inform", 2: "question", 3: "directive", 4: "commissive"}
EMOTION_LABELS = {
    0: "no emotion",
    1: "anger",
    2: "disgust",
    3: "fear",
    4: "happiness",
    5: "sadness",
    6: "surprise",
}


def download():
    for split in SPLITS:
        zip_path = RAW / f"{split}.zip"
        extract_dir = RAW / split
        if not zip_path.exists():
            url = f"{BASE_URL}/{split}.zip"
            print(f"downloading {url}")
            urllib.request.urlretrieve(url, zip_path)
        if not extract_dir.exists():
            with zipfile.ZipFile(zip_path) as zf:
                zf.extractall(extract_dir)


def read_lines(path):
    with open(path, encoding="utf-8") as f:
        return [line.rstrip("\n") for line in f]


def main():
    download()

    all_records = []
    per_split_counts = {}

    for split in SPLITS:
        d = RAW / split / split
        dialog_lines = read_lines(d / f"dialogues_{split}.txt")
        act_lines = read_lines(d / f"dialogues_act_{split}.txt")
        emotion_lines = read_lines(d / f"dialogues_emotion_{split}.txt")
        assert len(dialog_lines) == len(act_lines) == len(emotion_lines), (
            f"{split}: line-count mismatch across the three parallel files "
            f"({len(dialog_lines)}, {len(act_lines)}, {len(emotion_lines)})"
        )
        per_split_counts[split] = len(dialog_lines)

        for idx, (dtext, atext, etext) in enumerate(zip(dialog_lines, act_lines, emotion_lines)):
            utterances = [u.strip() for u in dtext.split("__eou__") if u.strip()]
            acts = [int(x) for x in atext.split()]
            emotions = [int(x) for x in etext.split()]
            assert len(utterances) == len(acts) == len(emotions), (
                f"{split} line {idx}: utterance/act/emotion count mismatch "
                f"({len(utterances)}, {len(acts)}, {len(emotions)})"
            )
            turns = []
            for i, text in enumerate(utterances):
                role = "user" if i % 2 == 0 else "other"
                turns.append({"role": role, "text": text})
            all_records.append(
                {
                    "id": f"dailydialog-{split}-{idx}",
                    "source": "DailyDialog",
                    "turns": turns,
                    "labels": {
                        "split": split,
                        "dialogue_act_ids": acts,
                        "dialogue_act_labels": [ACT_LABELS[a] for a in acts],
                        "emotion_ids": emotions,
                        "emotion_labels": [EMOTION_LABELS[e] for e in emotions],
                    },
                }
            )

    total_dialogues = len(all_records)

    rng = random.Random(SEED)
    if total_dialogues > CAP:
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

    print("=== DailyDialog counts (real, counted from downloaded files) ===")
    print("per-split dialogue counts:", per_split_counts)
    print("TOTAL dialogues (train+validation+test):", total_dialogues)
    print(f"sampled down to cap={CAP} (seed={SEED}):", sampled, "-> kept", len(sampled_records))
    print("wrote:", out_path, "lines:", sum(1 for _ in open(out_path)))


if __name__ == "__main__":
    main()
