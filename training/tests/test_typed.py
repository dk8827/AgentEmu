"""Real-ROM parity is optional locally; CI never requires commercial ROMs."""

import json
import os
import unittest
from pathlib import Path
import numpy as np
from contra_training._native import Contra, EpisodeBatch
from contra_training.typed import unpack


def equal_values(test, a, b):
    if isinstance(a, float):
        test.assertAlmostEqual(a, b, delta=max(1e-7, abs(a) * 1e-7))
    elif isinstance(a, dict):
        test.assertEqual(a.keys(), b.keys())
        for k in a:
            equal_values(test, a[k], b[k])
    elif isinstance(a, list):
        test.assertEqual(len(a), len(b))
        for x, y in zip(a, b):
            equal_values(test, x, y)
    else:
        test.assertEqual(a, b)


class TypedTransport(unittest.TestCase):
    def test_batch_rejects_bad_dimensions_before_loading_rom(self):
        for count in (0, 129):
            with self.assertRaises(ValueError):
                EpisodeBatch(b"bad", "contra-jungle", count)

    @unittest.skipUnless(os.environ.get("CONTRA_ROM"), "set CONTRA_ROM for exact episode parity")
    def test_complete_episodes_json_typed_and_batch_match(self):
        rom = Path(os.environ["CONTRA_ROM"]).read_bytes()
        legacy, typed = Contra(rom, 180), Contra(rom, 180)
        batch = EpisodeBatch(rom, "contra-jungle", 2, 180)
        for warmup in (0, 17):
            left = json.loads(legacy.reset_timing(warmup))
            right = unpack(typed.reset_timing_data(warmup))
            rows = batch.reset([warmup, warmup])
            saved = rows[0].observations
            for frame in range(181):
                np.testing.assert_allclose(
                    left["observations"], right["observations"], atol=0, rtol=1e-7
                )
                equal_values(self, left["info"], right["info"])
                np.testing.assert_allclose(left["rewards"], right["rewards"], atol=1e-7, rtol=1e-7)
                for row in rows:
                    np.testing.assert_array_equal(
                        np.frombuffer(row.observations, "<f4").reshape(2, 256),
                        right["observations"],
                    )
                    self.assertEqual(row.info, right["info"])
                    self.assertEqual(row.terminated, left["terminated"])
                    self.assertEqual(row.truncated, left["truncated"])
                if left["terminated"] or left["truncated"]:
                    break
                masks = [130 | int(frame % 20 == 0), 2]
                left = json.loads(legacy.step(masks))
                right = unpack(typed.step_data(masks))
                rows = batch.step([masks, masks])
            self.assertTrue(left["terminated"] or left["truncated"])
            self.assertEqual(bytes(legacy.snapshot()), bytes(typed.snapshot()))
            self.assertEqual(bytes(batch.snapshot(0)), bytes(typed.snapshot()))
            self.assertEqual(bytes(batch.snapshot(1)), bytes(typed.snapshot()))
            with self.assertRaises(ValueError):
                batch.step([[0, 0], [0, 0]])
            self.assertEqual(len(saved), 2048)
            batch.reset([0, 0])
            before = bytes(batch.snapshot(0))
            for bad in ([[0, 0]], [[0, 0], [255, 0]]):
                with self.assertRaises(ValueError):
                    batch.step(bad)
                self.assertEqual(before, bytes(batch.snapshot(0)))
            batch.reset_at(0, 17)
            self.assertNotEqual(bytes(batch.snapshot(0)), bytes(batch.snapshot(1)))
