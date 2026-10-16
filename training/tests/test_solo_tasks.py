"""Solo training must reset the cartridge to one player, not park a partner."""

import json
import os
from pathlib import Path
import unittest
from unittest.mock import patch
from contra_training.tasks import TaskEnv

GAMES = {
    "snowbros-floor1": "Snow Bros (U).nes",
    "bubblebobble-round1": "Bubble Bobble (U).nes",
    "iceclimber-mountain1": "Ice Climber (U).nes",
    "doubledragon2-mission1": "Double Dragon 2 - The Revenge (U).nes",
    "tmnt2-scene1": "Teenage Mutant Ninja Turtles 2 (U).nes",
}


class SoloEnvTests(unittest.TestCase):
    def test_solo_requires_first_player(self):
        with self.assertRaisesRegex(ValueError, "player 0"):
            TaskEnv("unused", "tmnt2-scene1", player=1, single_player=True)

    @patch("contra_training.tasks.unpack", side_effect=lambda x: x)
    @patch("contra_training.tasks.teacher", side_effect=AssertionError("solo has no teammate"))
    @patch("contra_training.tasks.TaskEpisode")
    @patch("pathlib.Path.read_bytes", return_value=b"rom")
    def test_solo_never_invents_a_scripted_partner(self, _read, episode, _teacher, _unpack):
        env = TaskEnv("unused", "tmnt2-scene1", single_player=True)
        episode.assert_called_once_with(b"rom", "tmnt2-scene1", 12000, None, True)
        state = dict(
            observations=[[0.0] * 256] * 2, info={}, reward=0, terminated=False, truncated=False
        )
        env.last = state
        episode.return_value.step_data.return_value = state
        env.step([2, 0, 1, 1])
        episode.return_value.step_data.assert_called_once_with([131, 0])


@unittest.skipUnless(os.environ.get("SOLO_ROM_DIR"), "set SOLO_ROM_DIR for five local cartridges")
class SoloCartridgeTests(unittest.TestCase):
    def test_native_resets_and_neutral_second_port(self):
        from contra_training._native import TaskEpisode

        for task, filename in GAMES.items():
            with self.subTest(task=task):
                raw = (Path(os.environ["SOLO_ROM_DIR"]) / filename).read_bytes()
                env = TaskEpisode(raw, task, single_player=True)
                state = json.loads(env.reset(0))["info"]["state"]
                self.assertTrue(state["gameplay"])
                self.assertEqual([p["active"] for p in state["players"]], [True, False])
                self.assertFalse(state.get("two_players", False))
                self.assertFalse(state.get("demo", False))
                snapshot = env.snapshot()
                with self.assertRaisesRegex(ValueError, "neutral P2"):
                    env.step([0, 1])
                self.assertEqual(env.snapshot(), snapshot)
                for _ in range(30):
                    result = json.loads(env.step([0, 0]))
                    self.assertFalse(result["info"]["state"]["players"][1]["active"])
                    self.assertEqual(result["info"]["deaths"][1], 0)
                env.reset(0)
                self.assertEqual(env.snapshot(), snapshot)
