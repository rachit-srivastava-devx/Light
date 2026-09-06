#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 5 ]]; then
  echo "usage: $0 <seconds> <mic-audio-index> <blackhole-audio-index> <capture.wav> <schedule.json>" >&2
  exit 64
fi

DURATION_SECONDS="$1"
MIC_INDEX="$2"
BLACKHOLE_INDEX="$3"
CAPTURE_PATH="$4"
SCHEDULE_PATH="$5"

if ! command -v ffmpeg >/dev/null || ! command -v ffprobe >/dev/null; then
  echo "ffmpeg and ffprobe are required" >&2
  exit 69
fi

echo "BlackHole is GPL-3 local test tooling only; it is never bundled with Focus Orb."
echo "Use the Mac system default Multi-Output Device; iOS 17+ Simulator Audio Output redirect is broken."

ffmpeg -hide_banner -y \
  -f avfoundation -i ":${MIC_INDEX}" \
  -f avfoundation -i ":${BLACKHOLE_INDEX}" \
  -filter_complex \
    "[0:a]aresample=16000,pan=mono|c0=c0[mic];[1:a]aresample=16000,pan=mono|c0=c0[rendered];[mic][rendered]amerge=inputs=2[capture]" \
  -map "[capture]" -t "$DURATION_SECONDS" -ac 2 -ar 16000 -c:a pcm_s16le "$CAPTURE_PATH"

backend/relay-py/.venv/bin/python audio-gate/gate.py \
  --audio "$CAPTURE_PATH" \
  --schedule "$SCHEDULE_PATH"
