import json
import unittest
from contra_training._native import observation_schema
from contra_training.contracts import CONTRACTS
from contra_training.observation_contracts import LAYOUTS


class ObservationSchema(unittest.TestCase):
    def test_installed_python_indices_match_native_schema_for_every_task(self):
        for game, (_, schema, _) in CONTRACTS.items():
            spec = json.loads(observation_schema(game))
            self.assertEqual(spec["schema"], schema)
            actual = {}
            for group in spec["groups"]:
                for repeat in range(group["repeat"]):
                    prefix = group["name"] + (f"[{repeat}]" if group["repeat"] > 1 else "")
                    for offset, field in enumerate(group["fields"]):
                        if not field["reserved"]:
                            actual[prefix + "." + field["name"]] = (
                                group["start"] + repeat * group["stride"] + offset,
                                field["encoding"],
                            )
            self.assertEqual(LAYOUTS[game], actual)
        with self.assertRaises(ValueError):
            observation_schema("unknown")
