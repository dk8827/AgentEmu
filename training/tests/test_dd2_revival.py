import hashlib
import json
import os
from pathlib import Path
import unittest

from contra_training._native import ResearchMachine
from contra_training.env import buttons
from contra_training.tasks import teacher


@unittest.skipUnless(os.environ.get("DD2_ROM"), "set DD2_ROM for the boss-revival regression")
class BossRevival(unittest.TestCase):
    def test_recorded_stall_exposes_boss_and_can_finish_with_ordinary_controls(self):
        root = Path(__file__).resolve().parents[2]
        fixture = json.loads((root / "fixtures/regressions/dd2-reviving-boss.json").read_text())
        raw = Path(os.environ["DD2_ROM"]).read_bytes()
        self.assertEqual(hashlib.sha256(raw).hexdigest(), fixture["rom_sha256"])
        machine = ResearchMachine(raw)
        inputs, frames = hashlib.sha256(), 0
        for count, p1, p2 in fixture["runs"]:
            inputs.update(bytes([p1, p2]) * count)
            frames += count
            machine.step([p1, p2], count)
        self.assertEqual(frames, fixture["frames"])
        self.assertEqual(inputs.hexdigest(), fixture["input_sha256"])
        self.assertEqual(
            hashlib.sha256(bytes(machine.snapshot())).hexdigest(), fixture["final_state_sha256"]
        )
        task = fixture["task"]
        step = json.loads(machine.observe(task))
        self.assertFalse(step["success"])
        self.assertEqual(len(step["state"]["enemies"]), 1)
        boss = step["state"]["enemies"][0]
        self.assertEqual((boss["kind"], boss["state"], boss["health"]), (7, 0x28, 0))
        self.assertTrue(boss["active"])
        for _ in range(1800):
            masks = [buttons(teacher(task, o, "dd2-punch")) for o in step["observations"]]
            machine.step(masks if step["gameplay"] else [0, 0], 1)
            step = json.loads(machine.observe(task))
            if step["success"]:
                break
        self.assertTrue(step["success"], "visible boss must be finishable with the task teacher")
