import json
import unittest
from pathlib import Path
import numpy as np
from contra_training._native import Actor, Contra
from contra_training.env import buttons

ROOT = Path(__file__).resolve().parents[2]


class Contract(unittest.TestCase):
    def test_factorized_actions_and_invalid_input(self):
        self.assertEqual(buttons(np.array([2, 1, 1, 1])), 147)
        for invalid in ([3, 0, 0, 0], [-1, 0, 0, 0], [0, 0, 0], [0.0, 0.0, 0.0, 0.0]):
            with self.assertRaises(ValueError):
                buttons(invalid)

    def test_gym_gates_actions_and_teammate_outside_gameplay(self):
        from contra_training.env import ContraEnv

        class Core:
            def step_data(self, masks):
                from types import SimpleNamespace

                self.masks = masks
                return SimpleNamespace(
                    **{
                        **env.last,
                        "reward": 0,
                        "observations": np.asarray(env.last["observations"], dtype="<f4").tobytes(),
                    }
                )

        env = ContraEnv.__new__(ContraEnv)
        env.single_player = False
        env.core = Core()
        env.player, env.teammate = 0, "scripted"
        env.last = dict(
            observations=[[0.0] * 256] * 2,
            rewards=[0, 0],
            terminated=False,
            truncated=False,
            info=dict(state=dict(gameplay=False)),
        )
        env.step([2, 1, 1, 1])
        self.assertEqual(env.core.masks, [0, 0])
        with self.assertRaises(ValueError):
            env.step([9, 0, 0, 0])
        env.last["info"]["state"]["gameplay"] = True
        env.step([2, 1, 1, 1])
        self.assertEqual(env.core.masks, [147, 130])

    def test_native_python_actor_matches_reference(self):
        fixture = ROOT / "fixtures/contra-policy"
        actor = Actor(
            (fixture / "manifest.json").read_text(),
            (fixture / "weights.safetensors").read_bytes(),
        )
        for c in json.loads((fixture / "parity.json").read_text()):
            np.testing.assert_allclose(
                actor.logits(c["observation"]), c["logits"], atol=2e-5, rtol=2e-5
            )
            self.assertEqual(actor.buttons(c["observation"]), c["buttons"])

    def test_malformed_rom_and_bundle_rejected(self):
        with self.assertRaises(ValueError):
            Contra(b"bad ROM")
        with self.assertRaises(ValueError):
            Actor("{}", b"not tensors")
        with self.assertRaises(ValueError):
            Contra(b"bad ROM", 0)

    def test_shipped_contra_bundles_load_in_deployed_runtime(self):
        for directory in sorted((ROOT / "models").glob("*")):
            with self.subTest(model=directory.name):
                actor = Actor(
                    (directory / "manifest.json").read_text(),
                    (directory / "weights.safetensors").read_bytes(),
                )
                manifest = json.loads((directory / "manifest.json").read_text())
                inputs = 268 if "-268-shared-" in manifest["observation"] else 256
                logits = actor.logits([0.0] * inputs)
                self.assertEqual(len(logits), 10)
                self.assertTrue(np.isfinite(logits).all())
                self.assertEqual(actor.buttons([0.0] * inputs) & 12, 0)

    def test_expansion_contracts_are_pinned_as_complete_tuples(self):
        from contra_training.contracts import CONTRACTS

        fixture = ROOT / "fixtures/contra-policy"
        m = json.loads((fixture / "manifest.json").read_text())
        weights = (fixture / "weights.safetensors").read_bytes()
        for architecture, observation, rom in CONTRACTS.values():
            manifest = {
                **m,
                "architecture": architecture,
                "observation": observation,
                "rom_sha256": rom,
            }
            self.assertEqual(len(Actor(json.dumps(manifest), weights).logits([0.0] * 256)), 10)
            for key in ("architecture", "observation", "rom_sha256"):
                with self.assertRaises(ValueError):
                    Actor(json.dumps({**manifest, key: "wrong"}), weights)

    def test_expansion_episode_rejects_invalid_rom_and_task(self):
        from contra_training._native import TaskEpisode

        for task in ("contra-base1", "snowbros-floor1", "unknown"):
            with self.assertRaises(ValueError):
                TaskEpisode(b"bad ROM", task)


class EvaluationContract(unittest.TestCase):
    def test_pair_uses_same_frame_and_neutral_exit_animation(self):
        from contra_training.rollout import run_episode

        class Core:
            def reset(self, seed):
                self.n = 0
                self.inputs = []
                return self.result()

            def result(self):
                return json.dumps(
                    dict(
                        observations=[[self.n, 0], [self.n, 1]],
                        rewards=[0, 0],
                        terminated=self.n == 3,
                        truncated=False,
                        info=dict(
                            frames=self.n,
                            success=self.n == 3,
                            max_progress=[1, 2],
                            deaths=[0, 0],
                            state=dict(
                                gameplay=self.n == 0,
                                boss_defeated=self.n >= 1,
                                level=int(self.n == 3),
                                players=[dict(active=True)] * 2,
                            ),
                        ),
                    )
                )

            def step(self, masks):
                self.inputs.append(masks)
                self.n += 1
                return self.result()

            def snapshot(self):
                return b"final state"

        core = Core()
        seen = []

        def controller(obs):
            seen.append(obs)
            return 128 + obs[1]

        row, _, _, trace = run_episode(core, [controller, controller], 12)
        self.assertEqual(seen, [[0, 0], [0, 1]])
        self.assertEqual(core.inputs, [[128, 129], [0, 0], [0, 0]])
        self.assertEqual(trace, bytes([128, 129, 0, 0, 0, 0]))
        self.assertTrue(row["success"])
        self.assertEqual(row["frames"], 3)
        self.assertEqual(row["boss_defeated_frame"], 1)


class TeacherContract(unittest.TestCase):
    def test_training_teacher_grounding_threats_and_boss_aim(self):
        from contra_training.teacher import action, mask

        o = np.zeros(256, dtype=np.float32)
        o[13] = 1
        o[96 + (3 * 8 + 2) * 4 + 1] = 1  # floor under player, none ahead
        self.assertEqual(action(o).tolist(), [2, 0, 1, 1])
        o[13] = 0
        self.assertEqual(action(o)[2], 0)  # no repeated A while airborne
        o[4] = 3072 / 4096
        o[5] = 3208 / 4096
        o[10] = 196 / 240
        self.assertEqual(mask(o), 146)  # right/up/fire at the low boss approach
        o[10] = 140 / 240
        self.assertEqual(mask(o), 162)  # right/down/fire from above the target
        o[10] = 170 / 240
        self.assertEqual(mask(o), 130)


class TeamworkTeacherContract(unittest.TestCase):
    def test_waiting_never_interrupts_airborne_or_endangered_player(self):
        from contra_training.benchmark import cooperative_action, cooperative_action_v1

        o = np.zeros(256, dtype=np.float32)
        o[24] = 1
        o[9] = 0.8
        o[25] = 0.2
        o[13] = o[29] = 1
        self.assertEqual(cooperative_action(o)[0], 0)
        o[13] = 0
        self.assertEqual(cooperative_action(o)[0], 2)
        self.assertEqual(cooperative_action_v1(o)[0], 0)
        o[13] = 1
        o[40] = 1
        o[41] = 0.1
        self.assertEqual(cooperative_action(o)[0], 2)


if __name__ == "__main__":
    unittest.main()
