import unittest

import numpy as np

from contra_training.tasks import teacher


class SnowTeacher(unittest.TestCase):
    def observation(self):
        o = np.zeros(256, np.float32)
        o[1] = o[8] = o[13] = o[40] = 1
        o[41], o[42], o[45] = -0.1, -0.3, 2 / 63
        return o

    def test_platform_alignment_and_air_control(self):
        o = self.observation()
        o[96 + (8 + 3) * 4 + 1] = 1
        # Enemy is left, but the reachable platform is right.
        self.assertEqual(teacher("snowbros-floor1", o, "snow-platforms"), [2, 0, 0, 1])
        o[96 + (8 + 2) * 4 + 1] = 1
        self.assertEqual(teacher("snowbros-floor1", o, "snow-platforms"), [0, 0, 1, 1])
        o[13] = 0
        self.assertEqual(teacher("snowbros-floor1", o, "snow-platforms"), [0, 0, 0, 1])

    def test_standard_baseline_and_profile_validation(self):
        o = self.observation()
        self.assertEqual(teacher("snowbros-floor1", o), [1, 0, 1, 1])
        with self.assertRaisesRegex(ValueError, "not supported"):
            teacher("tmnt2-scene1", o, "snow-platforms")
