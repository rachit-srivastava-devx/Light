from __future__ import annotations

from pathlib import Path
import json
import tempfile
import unittest

from replay.fixtures import ScriptedUtterance, load_fixture_manifest


class FixtureTests(unittest.TestCase):
    def test_loads_scripted_utterance_manifest(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "fixtures.json"
            path.write_text(
                json.dumps(
                    {
                        "version": 1,
                        "utterances": [
                            {
                                "id": "hello",
                                "phrase": "Hello",
                                "wav": "hello.wav",
                                "expected_transcript": "hello",
                                "tags": ["social"],
                            }
                        ],
                    }
                ),
                encoding="utf-8",
            )
            fixtures = load_fixture_manifest(path)
            resolved = fixtures[0].resolve_wav(directory)

        self.assertEqual(fixtures[0].id, "hello")
        self.assertEqual(fixtures[0].tags, ("social",))
        self.assertEqual(resolved.name, "hello.wav")

    def test_rejects_duplicate_ids_and_path_escape(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            manifest = Path(directory) / "fixtures.json"
            entry = {"id": "same", "phrase": "x", "wav": "x.wav", "expected_transcript": "x"}
            manifest.write_text(json.dumps({"version": 1, "utterances": [entry, entry]}), encoding="utf-8")
            with self.assertRaises(ValueError):
                load_fixture_manifest(manifest)

            fixture = ScriptedUtterance(
                id="escape",
                phrase="x",
                wav="../outside.wav",
                expected_transcript="x",
            )
            with self.assertRaises(ValueError):
                fixture.resolve_wav(directory)


if __name__ == "__main__":
    unittest.main()
