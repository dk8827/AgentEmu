import unittest
import hashlib
import json
from contra_training.contracts import CONTRACTS, RESETS
import numpy as np
import torch
from contra_training.model import ActorNet
from contra_training.env import buttons
from contra_training.trajectory_train import (
    factors,
    NearestDemonstration,
    collect,
    balanced_dataset,
    feature_scale,
    runtime_actor,
    direction_class_weights,
)


class TrajectoryTests(unittest.TestCase):
    def test_rare_direction_corrections_are_weighted_and_absent_classes_are_finite(
        self,
    ):
        labels = np.zeros((101, 4), np.int64)
        labels[0, 0] = 1
        weights = direction_class_weights(labels)
        np.testing.assert_allclose(weights[0], [1, 10, 1])
        np.testing.assert_array_equal(weights[1], [1, 1, 1])

    def test_learning_scale_is_folded_without_changing_logits_or_training_weights(self):
        torch.manual_seed(7)
        model = ActorNet()
        original = model.layers[0].weight.detach().clone()
        for task in (
            "worldcup-firstmatch",
            "chipndale-zone0",
            "jackal-stage1",
            "lifeforce-stage1",
            "iceclimber-mountain1",
            "rivercity-moose",
            "doubledragon2-mission1",
        ):
            scale = feature_scale(task)
            runtime = runtime_actor(model, scale)
            observations = torch.rand(100, 256) * 2 - 1
            with torch.no_grad():
                expected = model(observations * torch.from_numpy(scale))
                actual = runtime(observations)
            torch.testing.assert_close(actual, expected, atol=1e-5, rtol=1e-5)
            torch.testing.assert_close(model.layers[0].weight, original)
            # Resuming a runtime bundle with a different learning scale must
            # preserve its policy before the first optimizer update.
            replacement = feature_scale("rivercity-moose")
            with torch.no_grad():
                runtime.layers[0].weight.div_(torch.from_numpy(replacement))
                resumed = runtime_actor(runtime, replacement)
                torch.testing.assert_close(resumed(observations), actual, atol=1e-5, rtol=1e-5)
        np.testing.assert_array_equal(feature_scale("bubblebobble-round1"), 1)

    def test_factor_inverse_and_reject_impossible_or_menu_inputs(self):
        for mask in range(256):
            if mask & 12 or mask & 48 == 48 or mask & 192 == 192:
                with self.assertRaises(ValueError):
                    factors(mask)
            else:
                self.assertEqual(buttons(factors(mask)), mask)
        for value in [True, -1, 256, 1.0, "1"]:
            with self.assertRaises(ValueError):
                factors(value)

    def test_nearest_teacher_uses_observation_and_separates_ports(self):
        x = np.zeros((2, 256), np.float32)
        x[1, 0] = 1
        y = np.array([[1, 0, 1, 0], [2, 0, 0, 0]])
        teacher = NearestDemonstration(x, y, "iceclimber-mountain1")
        np.testing.assert_array_equal(teacher.label(x[0]), y[0])
        np.testing.assert_array_equal(teacher.label(x[1]), y[1])

    def test_river_city_route_recovery_does_not_copy_a_stuck_right_action(self):
        observation = np.zeros(256, np.float32)
        observation[4] = 1 / 34
        observation[9] = 504 / 1024
        observation[10] = 83 / 255
        labels = np.array([[2, 0, 0, 0]])
        expert = NearestDemonstration(
            observation[None, :], labels, "rivercity-moose", route_controls=True
        )
        self.assertEqual(expert.label(observation)[:2].tolist(), [1, 1])
        observation[4] = 5 / 34
        np.testing.assert_array_equal(expert.label(observation), labels[0])

    def test_river_city_combat_releases_punch_despite_nearest_demo_label(self):
        observation = np.zeros(256, np.float32)
        observation[4] = 5 / 34
        observation[40] = observation[46] = 1
        observation[41] = 20 / 1024
        observation[20] = 1
        expert = NearestDemonstration(
            observation[None, :],
            np.array([[2, 0, 1, 0]]),
            "rivercity-moose",
            combat_controls=True,
        )
        self.assertEqual(expert.label(observation)[2], 0)
        observation[20] = 0
        self.assertEqual(expert.label(observation)[2], 1)

    def test_failed_rollouts_cannot_displace_demonstrations(self):
        dx, dy = np.zeros((3, 256)), np.zeros((3, 4), dtype=np.int64)
        rx, ry = np.ones((100, 256)), np.ones((100, 4), dtype=np.int64)
        a = balanced_dataset((dx, dy), (rx, ry), 4, 8, np.random.default_rng(7))
        b = balanced_dataset((dx, dy), (rx, ry), 4, 8, np.random.default_rng(7))
        self.assertEqual(a[0].shape, (20, 256))
        np.testing.assert_array_equal(a[0][:12], 0)
        np.testing.assert_array_equal(a[0], b[0])

    def test_mixed_demonstrations_do_not_train_on_the_scripted_partner(self):
        task = "iceclimber-mountain1"
        obs = np.zeros((2, 256)).tolist()
        obs[1][0] = 1
        obs[0][1] = obs[1][1] = 1

        class Core:
            def reset(self, warmup):
                return json.dumps(dict(observations=obs, info=dict(state=dict(gameplay=True))))

            def step(self, pair):
                return json.dumps(dict(terminated=True, truncated=False, info=dict(success=True)))

            def snapshot(self):
                return b"final"

        for profile, slot in (("agent-p1", 0), ("agent-p2", 1)):
            replay = dict(
                version=1,
                game=task,
                rom_sha256=CONTRACTS[task][2],
                reset=RESETS[task],
                warmup=0,
                inputs=[[129, 64]],
                result=dict(
                    profile=profile,
                    success=True,
                    input_sha256=hashlib.sha256(bytes([129, 64])).hexdigest(),
                    final_state_sha256=hashlib.sha256(b"final").hexdigest(),
                ),
            )
            xs, ys = collect(Core(), task, replay)
            self.assertEqual(xs.shape, (1, 256))
            self.assertEqual(xs[0, 0], slot)
            self.assertEqual(buttons(ys[0]), [129, 64][slot])

    def test_recovery_demonstration_replays_but_does_not_learn_failed_prefix(self):
        task = "iceclimber-mountain1"
        obs = np.zeros((2, 256)).tolist()
        obs[0][1] = obs[1][1] = 1

        class Core:
            seen = []

            def reset(self, warmup):
                return json.dumps(dict(observations=obs, info=dict(state=dict(gameplay=True))))

            def step(self, pair):
                self.seen.append(pair)
                return json.dumps(
                    dict(
                        observations=obs,
                        terminated=len(self.seen) == 2,
                        truncated=False,
                        info=dict(success=len(self.seen) == 2, state=dict(gameplay=True)),
                    )
                )

            def snapshot(self):
                return b"recovered"

        core = Core()
        replay = dict(
            version=1,
            game=task,
            rom_sha256=CONTRACTS[task][2],
            reset=RESETS[task],
            warmup=0,
            inputs=[[64, 64], [129, 129]],
            result=dict(
                profile="pair",
                success=True,
                training_start_frame=1,
                input_sha256=hashlib.sha256(bytes([64, 64, 129, 129])).hexdigest(),
                final_state_sha256=hashlib.sha256(b"recovered").hexdigest(),
            ),
        )
        xs, ys = collect(core, task, replay)
        self.assertEqual(core.seen, replay["inputs"])
        self.assertEqual(len(xs), 2)
        self.assertEqual([buttons(y) for y in ys], [129, 129])
        replay["result"]["training_start_frame"] = 2
        with self.assertRaisesRegex(ValueError, "training start"):
            collect(None, task, replay)

    def test_bad_contract_does_not_step_emulator(self):
        with self.assertRaisesRegex(ValueError, "contract mismatch"):
            collect(None, "iceclimber-mountain1", {"version": 1, "game": "snowbros-floor1"})


if __name__ == "__main__":
    unittest.main()
