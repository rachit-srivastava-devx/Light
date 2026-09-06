# Replay fixtures

`utterances.json` is a checked-in scripted journey definition. Place matching user-recorded or
generated WAV files beside it when running the fixture. Each WAV must be PCM16, mono, 16 kHz.

The transcript is an expectation for a later evaluator; it is never injected into the relay.
The replay engine sends only the WAV bytes and the relay control frames.
