# Live app WebSocket transport

`ws_proxy.py` is the black-box bridge that makes the visible simulator run a real app session.
The app connects to port `8091`; the proxy forwards that socket to relay-rs on `8092` and exposes
control port `8093`. `live_replay.py` sends strict PCM16 mono 16 kHz WAV frames through the
control port into the backend session owned by the app connection. Returned transcripts and TTS
chunks travel back through the app socket and are recorded in the evaluator trace.

Start it before launching the app:

```bash
PYTHONPATH=.. python3 -m transport.ws_proxy \
  --backend-url ws://127.0.0.1:8092 \
  --trace runs/<run-id>/trace.jsonl \
  --expected-transcript "Hello, how are you?"
```

Then replay a fixture:

```bash
PYTHONPATH=.. python3 -m transport.live_replay runs/<run-id>/hello.wav \
  --control-url ws://127.0.0.1:8093 --session-id local-session
```

App audio is suppressed by default while a deterministic fixture is injected, so host silence
or simulator ambient audio cannot contaminate the STT result. Use `--forward-app-audio` only when
testing the actual microphone path. The proxy never imports app or backend modules.
