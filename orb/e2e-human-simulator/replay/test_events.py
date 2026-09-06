from __future__ import annotations

from io import StringIO
import json
import unittest

from replay.events import JsonlEventWriter


class EventWriterTests(unittest.TestCase):
    def test_jsonl_is_flushable_stable_and_indexed(self) -> None:
        stream = StringIO()
        writer = JsonlEventWriter(stream)
        writer.emit("audio.frame.sent", frame_seq=0, text="café")
        writer.emit("replay.completed", frames_sent=1)

        lines = stream.getvalue().splitlines()
        self.assertEqual(len(lines), 2)
        first = json.loads(lines[0])
        second = json.loads(lines[1])
        self.assertEqual(first["schema"], "focus-orb.replay.event.v1")
        self.assertEqual(first["event_index"], 0)
        self.assertEqual(first["event"], "audio.frame.sent")
        self.assertEqual(first["text"], "café")
        self.assertEqual(second["event_index"], 1)
        self.assertNotIn("wall_clock", first)

    def test_reserved_fields_cannot_be_overridden(self) -> None:
        writer = JsonlEventWriter(StringIO())
        with self.assertRaises(ValueError):
            writer.emit("bad", event_index=99)


if __name__ == "__main__":
    unittest.main()
