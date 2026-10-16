import os
import unittest
import numpy as np


@unittest.skipUnless(os.environ.get("CONTRA_ROM"), "set CONTRA_ROM for vector episode acceptance")
class VectorContract(unittest.TestCase):
    def test_route_partner_matches_frozen_steady_benchmark(self):
        from contra_training.vector import RustVecEnv
        from contra_training.benchmark import teammate
        from unittest.mock import patch

        env = RustVecEnv(
            os.environ["CONTRA_ROM"], count=2, player="alternate", teammate="route", horizon=8
        )
        try:
            env.seed(43)
            env.reset()
            for frame in range(4):
                expected = [teammate(env.last[i, 1 - i], frame, "steady") for i in range(2)]
                with patch.object(env.batch, "step", wraps=env.batch.step) as step:
                    env.step(np.zeros((2, 4), dtype=np.int64))
                    masks = step.call_args.args[0]
                self.assertEqual(masks, [[0, expected[0]], [expected[1], 0]])
        finally:
            env.close()

    def test_self_play_partner_uses_only_the_other_port(self):
        from contra_training.vector import RustVecEnv
        from types import SimpleNamespace
        from unittest.mock import patch

        env = RustVecEnv(
            os.environ["CONTRA_ROM"],
            count=4,
            player="alternate",
            horizon=2,
            policy_partner=SimpleNamespace(buttons=lambda o: 130),
        )
        try:
            env.seed(7)
            env.reset()
            with patch.object(env.batch, "step", wraps=env.batch.step) as step:
                env.step(np.zeros((4, 4), dtype=np.int64))
                masks = step.call_args.args[0]
            self.assertEqual(masks[2], [0, 130])
            self.assertEqual(masks[3], [130, 0])
            self.assertEqual(masks[0][0], 0)
            self.assertEqual(masks[1][1], 0)
        finally:
            env.close()

    def test_alternating_slots_survive_autoreset(self):
        from contra_training.vector import RustVecEnv

        env = RustVecEnv(
            os.environ["CONTRA_ROM"],
            count=2,
            player="alternate",
            horizon=2,
            teammate="idle",
            warmup_range=97,
        )
        try:
            env.seed(43)
            before = env.reset()
            np.testing.assert_array_equal(before[:, 0], [0, 1])
            self.assertEqual(env.get_attr("player"), [0, 1])
            for _ in range(2):
                obs, _, done, infos = env.step(np.zeros((2, 4), dtype=np.int64))
            self.assertTrue(done.all())
            np.testing.assert_array_equal(obs[:, 0], [0, 1])
            self.assertEqual([i["terminal_observation"][0] for i in infos], [0, 1])
            env.seed(43)
            np.testing.assert_array_equal(before, env.reset())
        finally:
            env.close()

    def test_terminal_observation_autoreset_and_short_ppo_run(self):
        from contra_training.vector import RustVecEnv
        from stable_baselines3 import PPO

        env = RustVecEnv(os.environ["CONTRA_ROM"], count=2, horizon=2, teammate="idle")
        try:
            env.seed(11)
            before = env.reset()
            self.assertEqual(before.shape, (2, 256))
            for _ in range(2):
                obs, reward, done, infos = env.step(np.zeros((2, 4), dtype=np.int64))
            self.assertTrue(done.all())
            for info in infos:
                self.assertTrue(info["TimeLimit.truncated"])
                self.assertEqual(info["terminal_observation"].shape, (256,))
                self.assertEqual(info["frames"], 2)
            env.seed(11)
            np.testing.assert_array_equal(before, env.reset())
            model = PPO(
                "MlpPolicy",
                env,
                n_steps=4,
                batch_size=4,
                n_epochs=1,
                seed=11,
                device="cpu",
                policy_kwargs=dict(net_arch=[16]),
            )
            model.learn(total_timesteps=8)
            self.assertEqual(model.num_timesteps, 8)
        finally:
            env.close()


@unittest.skipUnless(os.environ.get("DD2_ROM"), "set DD2_ROM for scripted-partner parity")
class DoubleDragonPartner(unittest.TestCase):
    def test_declared_partner_matches_scalar_episodes_for_both_slots(self):
        import json
        from pathlib import Path
        from contra_training._native import TaskEpisode
        from contra_training.vector import RustVecEnv
        from contra_training.env import buttons
        from contra_training.tasks import teacher

        raw = Path(os.environ["DD2_ROM"]).read_bytes()
        task = "doubledragon2-mission1"
        env = RustVecEnv(
            os.environ["DD2_ROM"],
            task=task,
            count=2,
            player="alternate",
            horizon=8,
            warmup_range=97,
            teacher_profile="lateral-jumps",
        )
        try:
            env.seed(7)
            env.reset()
            cores = [TaskEpisode(raw, task, 8) for _ in range(2)]
            rows = [json.loads(c.reset(7 + i)) for i, c in enumerate(cores)]
            for _ in range(4):
                for slot, core in enumerate(cores):
                    mask = buttons(
                        teacher(task, rows[slot]["observations"][1 - slot], "lateral-jumps")
                    )
                    masks = [0, mask] if slot == 0 else [mask, 0]
                    rows[slot] = json.loads(core.step(masks))
                obs, _, _, info = env.step(np.zeros((2, 4), np.int64))
                for slot in range(2):
                    np.testing.assert_allclose(
                        obs[slot], rows[slot]["observations"][slot], atol=1e-7
                    )
                    self.assertEqual(info[slot]["state"], rows[slot]["info"]["state"])
        finally:
            env.close()
