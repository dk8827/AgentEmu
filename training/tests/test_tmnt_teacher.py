import unittest
import numpy as np
from contra_training.tasks import teacher


class TmntTeacher(unittest.TestCase):
    def test_pixel_boundaries_match_json_teacher_and_float32_training_labels(self):
        # Cover both sides of each movement/attack boundary, both facing states
        # and both pulse states using the cartridge's normalized pixel fields.
        for dx in (-49, -48, -47, -25, -24, -23, -1, 0, 1, 23, 24, 25, 47, 48, 49):
            for dy in (-17, -16, -15, -4, -3, -2, 0, 2, 3, 4, 15, 16, 17):
                for facing in (0, 1):
                    for pulse in (0, 1):
                        o = np.zeros(256, np.float32)
                        o[40], o[41], o[42] = 1, dx / 255, dy / 255
                        o[17], o[3] = facing, pulse
                        expected = teacher("tmnt2-scene1", o.tolist(), "tmnt-bit-jump")
                        self.assertEqual(teacher("tmnt2-scene1", o, "tmnt-pixel-bit"), expected)
                        self.assertEqual(
                            teacher("tmnt2-scene1", o.tolist(), "tmnt-pixel-bit"), expected
                        )

    def test_faces_nearby_target_and_does_not_attack_across_lanes(self):
        o = np.zeros(256, np.float32)
        o[3] = o[40] = 1
        o[41] = -8 / 255
        o[2] = 1 / 255
        self.assertEqual(teacher("tmnt2-scene1", o, "tmnt-fast-jump"), [1, 0, 0, 1])
        o[17] = 1
        self.assertEqual(teacher("tmnt2-scene1", o, "tmnt-fast-jump"), [0, 0, 0, 1])
        o[2] = 16 / 255
        self.assertEqual(teacher("tmnt2-scene1", o, "tmnt-fast-jump")[2:], [1, 0])
        o[42] = 32 / 255
        self.assertEqual(teacher("tmnt2-scene1", o, "tmnt-fast-jump")[2:], [0, 0])

    def test_bit_pulse_alternates_attack_and_jump_without_a_modulo_prediction(self):
        o = [0.0] * 256
        o[40], o[41] = 1, 20 / 255
        for pulse, expected in ((0, [1, 0]), (1, [0, 1])):
            o[3] = pulse
            self.assertEqual(teacher("tmnt2-scene1", o, "tmnt-bit-jump")[2:], expected)
        o[42] = 17 / 255
        self.assertEqual(teacher("tmnt2-scene1", o, "tmnt-bit-jump")[2:], [0, 0])
