from __future__ import annotations

import unittest

from replay.timing import ReplayClock


class ReplayClockTests(unittest.TestCase):
    def test_real_time_and_accelerated_delays_are_deterministic_when_injected(self) -> None:
        observed: list[float] = []
        real_time = ReplayClock(speed=1.0, sleep_fn=observed.append)
        accelerated = ReplayClock(speed=4.0, sleep_fn=observed.append)

        self.assertAlmostEqual(real_time.wait_for_frame(20.0), 0.02)
        self.assertAlmostEqual(accelerated.wait_for_frame(20.0), 0.005)
        self.assertEqual(observed, [0.02, 0.005])

    def test_speed_must_be_positive(self) -> None:
        with self.assertRaises(ValueError):
            ReplayClock(speed=0.0)


if __name__ == "__main__":
    unittest.main()
