import copy
import os
import unittest
from types import SimpleNamespace
from unittest.mock import patch
import numpy as np
from contra_training.vector import individual_outcome, RustVecEnv


def info(active=(True, True), success=False):
    return dict(success=success, state=dict(players=[dict(active=a) for a in active]))


class IndividualRewardTests(unittest.TestCase):
    def test_partner_progress_cannot_reward_learner(self):
        self.assertEqual(
            individual_outcome([0.1, 12.0], False, False, info(), 0), (0.1, False, False)
        )
        self.assertEqual(
            individual_outcome([12.0, 0.1], False, False, info(), 1), (0.1, False, False)
        )

    def test_learner_elimination_is_terminal_not_timeout(self):
        state = info((False, True))
        self.assertEqual(
            individual_outcome([-1.001, 5.0], False, True, state, 0), (-1.001, True, False)
        )
        self.assertTrue(state["learner_eliminated"])
        self.assertFalse(state["success"])
        self.assertEqual(
            individual_outcome([-1.0, 0.2], False, False, info((False, True)), 1),
            (0.2, False, False),
        )

    def test_team_clear_bonus_requires_surviving_learner(self):
        state = info((False, True), True)
        reward, terminal, _ = individual_outcome([9.999, 10.2], True, False, state, 0)
        self.assertAlmostEqual(reward, -0.001)
        self.assertTrue(terminal and state["team_success"])
        self.assertFalse(state["success"])
        self.assertEqual(
            individual_outcome([9.999, 10.2], True, False, info((False, True), True), 1),
            (10.2, True, False),
        )


@unittest.skipUnless(os.environ.get("CONTRA_ROM"), "local Contra required")
class IndividualVectorTests(unittest.TestCase):
    def test_real_deaths_allow_respawn_but_elimination_stops_credit(self):
        from pathlib import Path
        from contra_training._native import Contra
        from contra_training.typed import unpack
        from contra_training.env import buttons
        from contra_training.teacher import action

        env = RustVecEnv(
            os.environ["CONTRA_ROM"],
            count=1,
            player=0,
            contra_individual=True,
            teammate="route",
            warmup_range=97,
        )
        core = Contra(Path(os.environ["CONTRA_ROM"]).read_bytes(), 9000)
        try:
            env.seed(811)
            env.reset()
            core.reset_timing_data(811 % 97)
            saw_respawn = False
            for _ in range(9000):
                partner = buttons(action(env.last[0, 1]))
                native = unpack(core.step_data([128, partner]))
                _, rewards, done, infos = env.step(np.array([[2, 0, 0, 0]], dtype=np.int64))
                player = native["info"]["state"]["players"][0]
                if player["dead"] and player["active"]:
                    saw_respawn = True
                    self.assertFalse(done[0])
                expected = individual_outcome(
                    native["rewards"],
                    native["terminated"],
                    native["truncated"],
                    copy.deepcopy(native["info"]),
                    0,
                )
                self.assertAlmostEqual(float(rewards[0]), expected[0], places=6)
                if done[0]:
                    self.assertFalse(player["active"])
                    self.assertTrue(infos[0]["learner_eliminated"])
                    self.assertFalse(infos[0]["TimeLimit.truncated"])
                    break
            else:
                self.fail("Fixture did not exercise learner elimination")
            self.assertTrue(saw_respawn)
        finally:
            env.close()

    def test_elimination_preserves_terminal_observation_and_resets_only_learner(self):
        env = RustVecEnv(
            os.environ["CONTRA_ROM"],
            count=2,
            player="alternate",
            contra_individual=True,
            teammate="idle",
        )
        try:
            env.seed(811)
            before = env.reset()
            real = env.batch.step

            def altered(masks):
                obs, reward, term, trunc, rows = real(masks)
                changed = copy.deepcopy(rows[0].info)
                changed["state"]["players"][0]["active"] = False
                changed["success"] = False
                rows = [
                    SimpleNamespace(
                        info=changed, rewards=[-1.001, 3.0], skipped_transition_frames=0
                    ),
                    rows[1],
                ]
                return obs, reward, term, trunc, rows

            with (
                patch.object(env.batch, "step", side_effect=altered),
                patch.object(env.batch, "reset_at", wraps=env.batch.reset_at) as reset,
            ):
                obs, reward, done, details = env.step(np.zeros((2, 4), np.int64))
            self.assertEqual(done.tolist(), [True, False])
            self.assertAlmostEqual(float(reward[0]), -1.001, places=6)
            self.assertFalse(details[0]["TimeLimit.truncated"])
            self.assertEqual(details[0]["terminal_observation"].shape, (256,))
            self.assertEqual(reset.call_args.args[0], 0)
            self.assertEqual(reset.call_count, 1)
            self.assertEqual(obs.shape, before.shape)
        finally:
            env.close()
