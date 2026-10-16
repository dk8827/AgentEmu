import unittest
from contra_training.task_benchmark import controllers, scripted_partner
from contra_training.task_rollout import rollout
from contra_training.tasks import teacher
import json


class Actor:
    def buttons(self, observation):
        return 129 if observation[0] == 0 else 66


class TaskBenchmarkTests(unittest.TestCase):
    def test_worldcup_attack_direction_switches_halves_and_kickoff_passes(self):
        o = [0.0] * 256
        o[9], o[10] = 500 / 1024, 224 / 512
        # Close to the ball: carry it toward the opponent goal.
        self.assertEqual(teacher("worldcup-firstmatch", o)[:2], [1, 0])
        o[4] = 1
        self.assertEqual(teacher("worldcup-firstmatch", o)[:2], [2, 0])
        o[7] = 1
        self.assertEqual(teacher("worldcup-firstmatch", o)[2], 0)
        o[5] = 1
        self.assertEqual(teacher("worldcup-firstmatch", o)[2], 1)
        o[3], o[9] = 1, 700 / 1024
        self.assertEqual(teacher("worldcup-firstmatch", o)[3], 1)

    def test_punch_edges_release_held_input_and_preserve_route_controls(self):
        obs = [0.0] * 256
        obs[4] = 5 / 34
        obs[40] = obs[46] = 1
        obs[41] = 20 / 1024
        self.assertEqual(teacher("rivercity-moose", obs, "punch-edges")[2], 1)
        obs[20] = 1
        self.assertEqual(teacher("rivercity-moose", obs, "punch-edges")[2], 0)
        obs[41] = 80 / 1024
        obs[20] = 0
        self.assertEqual(teacher("rivercity-moose", obs, "punch-edges")[2], 0)
        obs[4] = 1 / 34
        self.assertEqual(
            teacher("rivercity-moose", obs, "punch-edges"),
            teacher("rivercity-moose", obs),
        )
        with self.assertRaises(ValueError):
            teacher("tmnt2-scene1", obs, "punch-edges")

    def test_walk_and_punch_route_ignores_clock_and_keeps_exit_correction(self):
        obs = [0.0] * 256
        obs[1] = obs[8] = 1.0
        obs[9], obs[10] = 300 / 1024, 80 / 255
        for clock in range(256):
            obs[2] = clock / 255
            self.assertEqual(teacher("rivercity-moose", obs, "walk-and-punch"), [2, 0, 0, 0])
        obs[4], obs[9] = 1 / 34, 480 / 1024
        self.assertEqual(teacher("rivercity-moose", obs, "walk-and-punch"), [1, 1, 0, 0])
        with self.assertRaises(ValueError):
            teacher("tmnt2-scene1", obs, "walk-and-punch")

    def test_lateral_jump_profile_avoids_up_at_early_ledge_but_keeps_ladder(self):
        obs = [0.0] * 256
        obs[4] = 5 / 24
        obs[7] = 1
        self.assertEqual(teacher("doubledragon2-mission1", obs, "lateral-jumps"), [2, 0, 1, 1])
        obs[4] = 9 / 24
        self.assertEqual(teacher("doubledragon2-mission1", obs, "lateral-jumps")[1], 1)
        with self.assertRaises(ValueError):
            teacher("rivercity-moose", obs, "lateral-jumps")

    def test_neural_port_assignment_and_seeded_baselines(self):
        obs = [0.0] * 256
        obs[1] = 1
        obs[7] = 1
        actor = Actor()
        p1 = controllers("bubblebobble-round1", "agent-p1", actor, 42)
        p2 = controllers("bubblebobble-round1", "agent-p2", actor, 42)
        self.assertEqual(p1[0](obs), 129)
        obs[0] = 1
        self.assertEqual(p2[1](obs), 66)
        a = controllers("bubblebobble-round1", "random", actor, 42)
        b = controllers("bubblebobble-round1", "random", actor, 42)
        self.assertEqual(
            [[f(obs) for f in a] for _ in range(20)],
            [[f(obs) for f in b] for _ in range(20)],
        )
        self.assertEqual(
            [f(obs) for f in controllers("bubblebobble-round1", "idle", actor, 42)],
            [0, 0],
        )
        with self.assertRaises(ValueError):
            controllers("bubblebobble-round1", "bad", actor, 42)

    def test_partner_profile_is_explicit_and_original_default_is_unchanged(self):
        task = "doubledragon2-mission1"
        obs = [0.0] * 256
        obs[4], obs[7] = 5 / 24, 1
        self.assertEqual(scripted_partner({}, task), "standard")
        suite = dict(scripted_partners={task: "lateral-jumps"})
        declared = scripted_partner(suite, task)
        self.assertEqual([f(obs) for f in controllers(task, "teacher", Actor(), 1)], [147, 147])
        self.assertEqual(
            [f(obs) for f in controllers(task, "teacher", Actor(), 1, teacher_profile=declared)],
            [131, 131],
        )
        self.assertEqual(
            [f(obs) for f in controllers(task, "agent-p1", Actor(), 1, teacher_profile=declared)],
            [129, 131],
        )
        with self.assertRaises(ValueError):
            scripted_partner(dict(scripted_partners={task: "unknown"}), task)

    def test_non_gameplay_does_not_call_policy_or_teacher(self):
        class Core:
            def reset(self, warmup):
                return json.dumps({"observations": [[0.0] * 256, [0.0] * 256]})

            def step(self, masks):
                self.masks = masks
                return json.dumps(
                    {
                        "terminated": True,
                        "truncated": False,
                        "reward": 0,
                        "info": {
                            "frames": 1,
                            "success": True,
                            "max_progress": 3,
                            "deaths": [0, 0],
                            "state": {"players": [{"active": True}, {"active": True}]},
                        },
                    }
                )

            def snapshot(self):
                return b"test"

        def forbidden(o):
            raise AssertionError("policy called outside gameplay")

        core = Core()
        row, trace, _, _ = rollout(
            core, "bubblebobble-round1", 0, controllers=[forbidden, forbidden]
        )
        self.assertEqual(trace, [[0, 0]])
        self.assertTrue(row["success"])


if __name__ == "__main__":
    unittest.main()
