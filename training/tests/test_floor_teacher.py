import unittest
import numpy as np

try:
    import torch
except ImportError:
    torch = None


@unittest.skipIf(torch is None, "requires training dependencies")
class FloorTeacher(unittest.TestCase):
    def test_floor_and_slot_are_hard_constraints(self):
        from contra_training.trajectory_train import FloorDemonstration

        x = np.zeros((3, 256), np.float32)
        x[:, 13] = 1
        x[:, 14] = [1 / 255, 2 / 255, 1 / 255]
        x[2, 0] = 1
        y = np.array([[2, 0, 1, 0], [1, 0, 1, 0], [0, 0, 1, 0]])
        teacher = FloorDemonstration(x, y)
        for row, expected in zip(x, y):
            self.assertEqual(teacher.label(row), expected.tolist())
        airborne = x[0].copy()
        airborne[13], airborne[16] = 0, 2 / 255
        self.assertEqual(teacher.label(airborne), [1, 0, 0, 0])
