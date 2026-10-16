import unittest
import numpy as np

try:
    import torch
except ImportError:
    torch = None


@unittest.skipIf(torch is None, "requires training dependencies")
class TeacherFeatures(unittest.TestCase):
    def test_omitted_features_cannot_change_teacher_decisions(self):
        from contra_training.conditioning import contra_teacher_scale
        from contra_training.benchmark import cooperative_action

        scale = contra_teacher_scale()
        self.assertEqual(np.count_nonzero(scale), 52)
        rng = np.random.default_rng(29)
        for _ in range(1000):
            observation = rng.uniform(-1, 1, 256).astype(np.float32)
            modified = observation.copy()
            modified[scale == 0] = rng.uniform(-1, 1, np.count_nonzero(scale == 0))
            np.testing.assert_array_equal(
                cooperative_action(observation), cooperative_action(modified)
            )

    def test_zero_feature_scales_fold_into_ordinary_weights(self):
        from contra_training.conditioning import contra_teacher_scale, runtime_actor
        from contra_training.model import ActorNet

        scale = contra_teacher_scale()
        actor = ActorNet()
        deployed = runtime_actor(actor, scale)
        inputs = torch.randn(16, 256)
        with torch.no_grad():
            torch.testing.assert_close(actor(inputs * torch.from_numpy(scale)), deployed(inputs))
        self.assertTrue(torch.all(deployed.layers[0].weight[:, scale == 0] == 0))

    def test_tmnt_features_only_keep_declared_combat_inputs(self):
        from contra_training.conditioning import teacher_feature_scale
        from contra_training.tasks import teacher

        rng = np.random.default_rng(19)
        for profile, count in (
            ("tmnt-bit-jump", 35),
            ("tmnt-fast-jump", 36),
            ("tmnt-nearest-bit", 5),
            ("tmnt-pixel-bit", 5),
        ):
            scale = teacher_feature_scale("tmnt2-scene1", profile)
            self.assertEqual(np.count_nonzero(scale), count)
            for _ in range(1000):
                observation = rng.uniform(-1, 1, 256).astype(np.float32)
                modified = observation.copy()
                modified[scale == 0] = rng.uniform(-1, 1, np.count_nonzero(scale == 0))
                self.assertEqual(
                    teacher("tmnt2-scene1", observation, profile),
                    teacher("tmnt2-scene1", modified, profile),
                )
        with self.assertRaises(ValueError):
            teacher_feature_scale("snowbros-floor1")


class RaisedPlatforms(unittest.TestCase):
    def test_island_takeoff_uses_grounded_pixel_position(self):
        from contra_training.teacher import island_takeoff_action, raised_platform_action, FEATURES

        o = np.zeros(256, np.float32)
        o[FEATURES["players[0].grounded"]] = 1
        o[FEATURES["terrain[4].floor"]] = 1  # The old supervisor wants to jump.
        for world, jump in ((2849, 1), (2850, 0), (2899, 0), (2900, 1), (2919, 1), (2920, 1)):
            o[FEATURES["header.world_x"]] = world / 4096
            for observation in (o, o.tolist()):
                self.assertEqual(island_takeoff_action(observation)[2], jump)
                self.assertEqual(island_takeoff_action(observation)[0], 2)
        o[FEATURES["players[0].grounded"]] = 0
        o[FEATURES["header.world_x"]] = 2900 / 4096
        np.testing.assert_array_equal(island_takeoff_action(o), raised_platform_action(o))

    def test_upper_platform_requires_jump_only_before_boss_on_ground(self):
        from contra_training.teacher import raised_platform_action, FEATURES

        o = np.zeros(256, np.float32)
        o[FEATURES["players[0].grounded"]] = 1
        self.assertEqual(raised_platform_action(o)[2], 0)
        o[FEATURES["terrain[4].floor"]] = 1
        self.assertEqual(raised_platform_action(o)[2], 1)
        o[FEATURES["players[0].grounded"]] = 0
        self.assertEqual(raised_platform_action(o)[2], 0)
        o[FEATURES["players[0].grounded"]] = 1
        o[FEATURES["header.camera_x"]] = 2970 / 4096
        self.assertEqual(raised_platform_action(o)[2], 0)

    @unittest.skipIf(torch is None, "requires training dependencies")
    def test_all_supervisor_inputs_are_retained(self):
        from contra_training.teacher import raised_platform_action, island_takeoff_action
        from contra_training.conditioning import teacher_feature_scale

        rng = np.random.default_rng(21)
        for profile, labeler in (
            ("contra-raised-platforms", raised_platform_action),
            ("contra-island-takeoff", island_takeoff_action),
        ):
            scale = teacher_feature_scale("contra-jungle", profile)
            self.assertEqual(np.count_nonzero(scale), 58)
            for _ in range(1000):
                o = rng.uniform(-1, 1, 256).astype(np.float32)
                changed = o.copy()
                changed[scale == 0] = rng.uniform(-1, 1, np.count_nonzero(scale == 0))
                np.testing.assert_array_equal(labeler(o), labeler(changed))


@unittest.skipIf(torch is None, "requires training dependencies")
class UniqueExamples(unittest.TestCase):
    def test_removes_only_repeated_active_inputs_with_identical_labels(self):
        from contra_training.conditioning import unique_supervision

        xs = np.array([[1, 9], [1, 2], [1, 4], [2, 8]], np.float32)
        ys = np.array([[0, 0, 1, 0], [0, 0, 1, 0], [0, 0, 0, 0], [0, 0, 1, 0]])
        x, y = unique_supervision(xs, ys, np.array([1, 0]))
        np.testing.assert_array_equal(x, xs[[0, 2, 3]])
        np.testing.assert_array_equal(y, ys[[0, 2, 3]])
