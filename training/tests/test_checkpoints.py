import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import numpy as np
from contra_training.checkpoints import atomic_file, save_dataset, write_json


class CheckpointTests(unittest.TestCase):
    def test_failed_write_preserves_previous_checkpoint_and_removes_partial(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "checkpoint.json"
            write_json(path, {"complete": True})
            with self.assertRaises(OSError):
                with atomic_file(path) as output:
                    output.write(b"partial")
                    raise OSError("disk full")
            self.assertEqual(json.loads(path.read_text()), {"complete": True})
            self.assertEqual(list(Path(directory).iterdir()), [path])

    def test_dataset_replacement_is_complete_or_unchanged(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "dataset.npz"
            x, y = np.zeros((4, 256)), np.ones((4, 4), np.int64)
            save_dataset(path, x, y)
            previous = path.read_bytes()
            with patch(
                "contra_training.checkpoints.np.savez_compressed", side_effect=OSError("disk full")
            ):
                with self.assertRaises(OSError):
                    save_dataset(path, x + 1, y)
            self.assertEqual(path.read_bytes(), previous)
            save_dataset(path, x + 1, y)
            with np.load(path, allow_pickle=False) as data:
                np.testing.assert_array_equal(data["observations"], x + 1)
                np.testing.assert_array_equal(data["actions"], y)
            self.assertEqual(list(Path(directory).iterdir()), [path])

    def test_ppo_archive_can_use_atomic_stream(self):
        import zipfile
        from stable_baselines3 import PPO

        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "optimizer.zip"
            model = PPO("MlpPolicy", "CartPole-v1", n_steps=2, batch_size=2)
            try:
                with atomic_file(path) as output:
                    model.save(output)
                with zipfile.ZipFile(path) as archive:
                    self.assertIn("policy.pth", archive.namelist())
                    self.assertIn("policy.optimizer.pth", archive.namelist())
                    self.assertIsNone(archive.testzip())
            finally:
                model.get_env().close()
