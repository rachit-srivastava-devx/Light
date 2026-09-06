#!/usr/bin/env python3
"""
AnnoMI — Anno-MI: A Dataset of Expert-Annotated Counselling Dialogues (Wu, Balloccu et al.,
ICASSP 2022 / Future Internet 2023).

Source: https://github.com/uccollab/AnnoMI (AnnoMI-full.csv, downloaded verbatim)

*** LICENSE WARNING ***
The uccollab/AnnoMI GitHub repo has NO LICENSE file and the README/papers make no explicit
redistribution grant (checked: README.md, both cited papers' abstracts). A third-party HF
reupload tags it "openrail", but that is the re-uploader's own guess, not an authoritative
grant from the dataset authors. Per the project's hard rule ("if a licence forbids
redistribution, keep the script, do not commit the data"), this script treats "no licence
found" the same as "redistribution not clearly granted": conversations.jsonl is written
locally (needed to count and sanity-check) but is listed in ../.gitignore and MUST NOT be
committed. Only this script, PROVENANCE.md, and 2-3 short verbatim lines for sanity-checking
are committed.

Raw format (AnnoMI-full.csv): one row per (transcript_id, utterance_id, annotator_id).
133 transcripts of real motivational-interviewing (MI) counselling sessions, each utterance
labelled with mi_quality (high/low, per transcript), interlocutor (therapist/client),
main_therapist_behaviour, client_talk_type. A subset of utterances were annotated by more
than one of the 10 annotators (used for inter-annotator-agreement in the paper); where that
happens we deterministically keep the row from the lowest annotator_id present for that
utterance (a real annotator's real recorded label — not a synthesised majority vote) and
report how many utterances that affected.
"""
import csv
import json
import urllib.request
from collections import defaultdict
from pathlib import Path

HERE = Path(__file__).parent
RAW = HERE / "raw"
RAW.mkdir(exist_ok=True)

CSV_URL = "https://raw.githubusercontent.com/uccollab/AnnoMI/main/AnnoMI-full.csv"
CSV_PATH = RAW / "AnnoMI-full.csv"


def download():
    if not CSV_PATH.exists():
        print(f"downloading {CSV_URL}")
        urllib.request.urlretrieve(CSV_URL, CSV_PATH)
    else:
        print(f"already have {CSV_PATH}")


def main():
    download()

    with open(CSV_PATH, newline="", encoding="utf-8") as f:
        rows = list(csv.DictReader(f))

    total_rows = len(rows)

    # Group by (transcript_id, utterance_id); keep the lowest annotator_id's row when an
    # utterance was annotated more than once.
    by_key = defaultdict(list)
    for r in rows:
        by_key[(r["transcript_id"], int(r["utterance_id"]))].append(r)

    multi_annotated_utterances = sum(1 for rs in by_key.values() if len(rs) > 1)

    def annotator_sort_key(row):
        try:
            return int(row["annotator_id"])
        except (ValueError, TypeError):
            return 0

    dedup_rows = {}
    for key, rs in by_key.items():
        rs_sorted = sorted(rs, key=annotator_sort_key)
        dedup_rows[key] = rs_sorted[0]

    by_transcript = defaultdict(list)
    for (transcript_id, utterance_id), row in dedup_rows.items():
        by_transcript[transcript_id].append((utterance_id, row))

    records = []
    mi_quality_counts = {"high": 0, "low": 0}
    for transcript_id, items in by_transcript.items():
        items.sort(key=lambda x: x[0])
        turns = []
        turn_labels = []
        for _utt_id, row in items:
            role = "user" if row["interlocutor"] == "client" else "other"
            turns.append({"role": role, "text": row["utterance_text"]})
            turn_labels.append(
                {
                    "main_therapist_behaviour": row["main_therapist_behaviour"],
                    "client_talk_type": row["client_talk_type"],
                }
            )
        mi_quality = items[0][1]["mi_quality"]
        mi_quality_counts[mi_quality] = mi_quality_counts.get(mi_quality, 0) + 1
        records.append(
            {
                "id": f"annomi-{transcript_id}",
                "source": "AnnoMI-full.csv",
                "turns": turns,
                "labels": {
                    "mi_quality": mi_quality,
                    "topic": items[0][1]["topic"],
                    "video_title": items[0][1]["video_title"],
                    "video_url": items[0][1]["video_url"],
                    "per_turn": turn_labels,
                },
            }
        )

    out_path = HERE / "conversations.jsonl"
    with open(out_path, "w") as f:
        for r in records:
            f.write(json.dumps(r, ensure_ascii=False) + "\n")

    print("=== AnnoMI counts (real, counted from downloaded CSV) ===")
    print("total CSV data rows (incl. multi-annotator duplicates):", total_rows)
    print("distinct (transcript_id, utterance_id) turns:", len(dedup_rows))
    print("utterance keys with >1 annotator row (deduped to lowest annotator_id):", multi_annotated_utterances)
    print("distinct transcripts (dialogues):", len(records))
    print("mi_quality distribution over transcripts:", mi_quality_counts)
    print("wrote (LOCAL ONLY, gitignored — see PROVENANCE.md licence note):", out_path)
    print("lines:", sum(1 for _ in open(out_path)))


if __name__ == "__main__":
    main()
