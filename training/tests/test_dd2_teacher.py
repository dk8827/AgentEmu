import unittest
import numpy as np
from contra_training.tasks import teacher


class DragonTeacher(unittest.TestCase):
    def test_attack_uses_enemy_side_and_requires_reach(self):
        o = np.zeros(256, np.float32)
        o[3] = o[7] = o[40] = 1
        o[41] = 24 / 1024
        self.assertEqual(teacher("doubledragon2-mission1", o, "dd2-punch")[2:], [1, 0])
        o[41] = -24 / 1024
        self.assertEqual(teacher("doubledragon2-mission1", o, "dd2-punch")[2:], [0, 1])
        o[41] = 80 / 1024
        self.assertEqual(teacher("doubledragon2-mission1", o, "dd2-punch")[2:], [0, 0])
        self.assertEqual(teacher("doubledragon2-mission1", o)[2:], [1, 1])
