# Deterministic microphone replay

This folder is a standalone Python standard-library module. It does not import the mobile app or
backend packages. A caller supplies a transport with two methods:

```python
send_control(frame: Mapping[str, object])  # JSON relay control frame
send_audio(pcm: bytes)                    # binary PCM frame
```

The replay engine validates WAV input as PCM16, mono, 16 kHz, splits it into 20 ms frames by
default, preserves a short final tail (`keep`), and emits stable JSONL evidence. `pad` and `drop`
tail policies are available through `FrameSpec`. `ReplayClock(speed=1.0)` is real-time; values
greater than one accelerate playback. Tests inject a no-op sleep function.

## Passing test command

Run from `company/products/adhd-focus-orb/`:

```bash
python3 -m unittest discover -s e2e-human-simulator -p 'test_*.py'
```

The command covers replay audio validation/splitting, exact relay control ordering, binary frame
metadata, JSONL output, fixture validation, timing, and the folder's import-independence check.

For replay-only discovery, run from `e2e-human-simulator/` so Python resolves the `replay/`
directory as a package:

```bash
python3 -m unittest discover -s replay -p 'test_*.py'
```

## Dry-run JSONL

From the same directory, replay a WAV without opening a socket:

```bash
python3 -m replay input.wav --events trace.jsonl --speed 8
```

The emitted trace contains control frames and audio byte counts plus SHA-256 hashes. It never
embeds raw microphone bytes.
