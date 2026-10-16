import json
import os
from pathlib import Path
import unittest

import numpy as np

from contra_training._native import TaskEpisode
from contra_training.typed import Batch


@unittest.skipUnless(
    os.environ.get("ICE_ROM") and os.environ.get("ICE_REPLAY"),
    "set ICE_ROM and ICE_REPLAY for summit-transition replay",
)
class IceTransitionSteps(unittest.TestCase):
    def test_coalesced_transition_preserves_completion_truncation_rewards_and_state(self):
        rom = Path(os.environ["ICE_ROM"]).read_bytes()
        replay = json.loads(Path(os.environ["ICE_REPLAY"]).read_bytes())
        # This local regression route enters its bonus transition after frame 1104.
        entry = 1103
        self.assertTrue(all(pair == [0, 0] for pair in replay["inputs"][entry + 1 :]))
        for horizon in (12000, 1600):
            with self.subTest(horizon=horizon):
                full = TaskEpisode(rom, "iceclimber-mountain1", horizon)
                fast = Batch(
                    rom, "iceclimber-mountain1", 1, horizon=horizon, skip_non_gameplay=True
                )
                full.reset(replay["warmup"])
                fast.reset([0])
                for masks in replay["inputs"][:entry]:
                    full.step_data(masks)
                start = fast.reset_with_prefix_at(0, replay["warmup"], replay["inputs"][:entry])
                self.assertEqual(start["info"]["frames"], entry)
                self.assertTrue(start["info"]["state"]["gameplay"])
                merged = fast.step([replay["inputs"][entry]])[4][0]
                self.assertGreater(merged.skipped_transition_frames, 400)
                rewards = np.zeros(2, np.float64)
                for frame in range(entry, merged.info["frames"]):
                    last = full.step_data(replay["inputs"][frame])
                    rewards += last.rewards
                np.testing.assert_allclose(merged.rewards, rewards, atol=0.0003, rtol=0.00001)
                self.assertEqual(merged.info, last.info)
                self.assertEqual(merged.truncated, horizon == 1600)
                self.assertEqual(merged.terminated, horizon == 12000)
                self.assertEqual(merged.info["success"], horizon == 12000)
                self.assertEqual(
                    merged.info["frames"] - entry, merged.skipped_transition_frames + 1
                )
                self.assertEqual(fast.core.snapshot(0), full.snapshot())


@unittest.skipUnless(os.environ.get("CONTRA_ROM"), "set CONTRA_ROM for gameplay cadence")
class GameplayCadence(unittest.TestCase):
    def test_transition_mode_still_steps_once_per_playable_frame(self):
        rom = Path(os.environ["CONTRA_ROM"]).read_bytes()
        slow = Batch(rom, "contra-jungle", 1, horizon=4)
        fast = Batch(rom, "contra-jungle", 1, horizon=4, skip_non_gameplay=True)
        slow.reset([0])
        fast.reset([0])
        for frame in range(1, 5):
            a, b = slow.step([[130, 130]]), fast.step([[130, 130]])
            self.assertEqual(b[4][0].skipped_transition_frames, 0)
            self.assertEqual(b[4][0].info["frames"], frame)
            self.assertEqual(a[4][0].info, b[4][0].info)
            np.testing.assert_array_equal(a[0], b[0])
        self.assertEqual(slow.core.snapshot(0), fast.core.snapshot(0))
