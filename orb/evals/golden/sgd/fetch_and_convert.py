#!/usr/bin/env python3
"""
Schema-Guided Dialogue (SGD) — Rastogi et al., AAAI 2020 (DSTC8 task).

Source: https://github.com/google-research-datasets/dstc8-schema-guided-dialogue
(codeload tarball of the `master` branch — one download, extracted locally).
Licence: CC BY-SA 4.0, as declared in the repo's LICENSE.txt / README ("The SGD and SGD-X
datasets are released under CC BY-SA 4.0 license") — allows redistribution with
attribution + share-alike. No NonCommercial restriction.

Picked over MultiWOZ 2.2 because it downloads and licenses cleanly (plain JSON on GitHub,
no HF loading-script issue) and its task construction is explicitly built around domain
switching: many dialogues span 2+ "services" (domains), which is exactly the orb property
("topic-shift-and-return" analog for task-oriented, multi-domain dialogue) we need evidence
for.

Raw format: train/, dev/, test/ directories of dialogues_NNN.json shards, each a JSON list
of dialogues:
  {"dialogue_id": str, "services": [str, ...], "turns": [{"speaker": "USER"|"SYSTEM",
   "utterance": str, "frames": [...slot/state/action detail, dropped here...]}, ...]}
`services` is the ground-truth list of domains/APIs touched during the dialogue — a
dialogue with len(services) > 1 is a real, human-collected example of a conversation that
switches domains. We keep that count (and the `services` list itself) as the label rather
than re-deriving or inferring anything.
"""
import json
import random
import shutil
import tarfile
import urllib.request
from pathlib import Path

HERE = Path(__file__).parent
RAW = HERE / "raw"
RAW.mkdir(exist_ok=True)

TARBALL_URL = (
    "https://codeload.github.com/google-research-datasets/"
    "dstc8-schema-guided-dialogue/tar.gz/refs/heads/master"
)
TARBALL_PATH = RAW / "sgd.tar.gz"
EXTRACT_DIR = RAW / "extracted"
REPO_DIR = EXTRACT_DIR / "dstc8-schema-guided-dialogue-master"

CAP = 2000
SEED = 0


def download():
    if not TARBALL_PATH.exists():
        print(f"downloading {TARBALL_URL}")
        urllib.request.urlretrieve(TARBALL_URL, TARBALL_PATH)
    else:
        print(f"already have {TARBALL_PATH}")
    if not REPO_DIR.exists():
        EXTRACT_DIR.mkdir(exist_ok=True)
        with tarfile.open(TARBALL_PATH) as tf:
            members = [
                m
                for m in tf.getmembers()
                if any(
                    f"dstc8-schema-guided-dialogue-master/{p}" in m.name
                    for p in ("train/", "dev/", "test/", "LICENSE.txt")
                )
            ]
            tf.extractall(EXTRACT_DIR, members=members)


def load_split(split):
    split_dir = REPO_DIR / split
    dialogues = []
    for shard_path in sorted(split_dir.glob("dialogues_*.json")):
        with open(shard_path, encoding="utf-8") as f:
            dialogues.extend(json.load(f))
    return dialogues


def main():
    download()

    all_records = []
    per_split_counts = {}
    per_split_multi_domain = {}

    for split in ("train", "dev", "test"):
        dialogues = load_split(split)
        per_split_counts[split] = len(dialogues)
        multi = 0
        for dlg in dialogues:
            services = dlg["services"]
            is_multi = len(services) > 1
            if is_multi:
                multi += 1
            turns = []
            for turn in dlg["turns"]:
                role = "user" if turn["speaker"] == "USER" else "other"
                turns.append({"role": role, "text": turn["utterance"]})
            all_records.append(
                {
                    "id": f"sgd-{split}-{dlg['dialogue_id']}",
                    "source": "Schema-Guided Dialogue (SGD)",
                    "turns": turns,
                    "labels": {
                        "split": split,
                        "services": services,
                        "num_services": len(services),
                        "is_multi_domain": is_multi,
                    },
                }
            )
        per_split_multi_domain[split] = multi

    total_dialogues = len(all_records)
    total_multi_domain = sum(1 for r in all_records if r["labels"]["is_multi_domain"])

    # Unbiased deterministic sample of the FULL population (not filtered to multi-domain
    # only) so the sampled set's multi-domain ratio still reflects the real corpus; every
    # record still carries `is_multi_domain` so a consumer can filter further downstream.
    rng = random.Random(SEED)
    if total_dialogues > CAP:
        all_records.sort(key=lambda r: r["id"])
        sampled_records = rng.sample(all_records, CAP)
        sampled = True
    else:
        sampled_records = all_records
        sampled = False

    sampled_multi_domain = sum(1 for r in sampled_records if r["labels"]["is_multi_domain"])

    out_path = HERE / "conversations.jsonl"
    with open(out_path, "w") as f:
        for r in sampled_records:
            f.write(json.dumps(r, ensure_ascii=False) + "\n")

    print("=== SGD counts (real, counted from downloaded files) ===")
    print("per-split dialogue counts:", per_split_counts)
    print("per-split multi-domain (services>1) counts:", per_split_multi_domain)
    print("TOTAL dialogues (train+dev+test):", total_dialogues)
    print("TOTAL multi-domain dialogues (services > 1):", total_multi_domain)
    print(f"sampled down to cap={CAP} (seed={SEED}):", sampled, "-> kept", len(sampled_records))
    print("multi-domain dialogues within the sampled set:", sampled_multi_domain)
    print("wrote:", out_path, "lines:", sum(1 for _ in open(out_path)))

    # Disk hygiene: the extracted JSON is ~500MB+; keep only the compact tarball as the
    # reproducible cache and drop the extracted copy once conversations.jsonl is written.
    if EXTRACT_DIR.exists():
        shutil.rmtree(EXTRACT_DIR)
        print("cleaned up:", EXTRACT_DIR, "(kept raw/sgd.tar.gz as the reproducible cache)")


if __name__ == "__main__":
    main()
