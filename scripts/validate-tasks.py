"""Local-ROM semantic tests; no game data is written to the repository."""

import argparse, json
from pathlib import Path
from contra_training._native import ResearchMachine, TaskEpisode


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--snow", required=True)
    p.add_argument("--contra", required=True)
    p.add_argument("--entry", required=True)
    a = p.parse_args()
    for task, path, entry in [
        ("snowbros-floor1", a.snow, None),
        ("contra-base1", a.contra, Path(a.entry).read_bytes()),
    ]:
        rom = Path(path).read_bytes()
        core = TaskEpisode(rom, task, 1, entry)
        first = json.loads(core.reset(0))
        initial = bytes(core.snapshot())
        assert all(len(o) == 256 for o in first["observations"])
        for masks in [[8, 0], [192, 0], [0, 48]]:
            try:
                core.step(masks)
                raise AssertionError("invalid action accepted")
            except ValueError:
                pass
        assert bytes(core.snapshot()) == initial
        result = json.loads(core.step([0, 0]))
        assert result["truncated"] and not result["terminated"]
        try:
            core.step([0, 0])
            raise AssertionError("stepped after truncation")
        except ValueError:
            pass
        core.reset(0)
        assert bytes(core.snapshot()) == initial
        core.reset(40)
        assert bytes(core.snapshot()) != initial
        m = ResearchMachine(rom)
        m.restore(initial)
        before = bytes(m.snapshot())
        f = json.loads(m.observe(task))
        assert bytes(m.snapshot()) == before
        assert f["gameplay"]
        # A neutral reference separates motion caused by gravity/enemies from controller effects.
        if task == "snowbros-floor1":
            m.step([0, 0], 80)
        baseline = bytes(m.snapshot())
        positions = []
        for masks in [[0, 0], [128, 0], [0, 64], [1, 0]]:
            m.restore(baseline)
            m.step(masks, 12)
            positions.append(json.loads(m.observe(task))["state"]["players"])
        assert positions[1][0]["x"] > positions[0][0]["x"]
        assert positions[1][1]["x"] == positions[0][1]["x"]
        assert positions[2][1]["x"] < positions[0][1]["x"]
        assert positions[2][0]["x"] == positions[0][0]["x"]
        if task == "snowbros-floor1":
            assert positions[3][0]["y"] < positions[0][0]["y"]
            assert first["info"]["state"]["remaining_enemies"] == 4
            assert [p["reserve_lives"] for p in first["info"]["state"]["players"]] == [
                2,
                2,
            ]
        else:
            cores = [e for e in f["state"]["entities"] if e["kind"] == 0x14]
            assert cores and cores[0]["hp"] == 8
            assert cores[0]["kind"] == 0x14
            assert isinstance(cores[0]["requires_crouch"], bool)
        print(
            task
            + ": independent controls, semantic fields, reset determinism, limits and read-only observations passed"
        )


if __name__ == "__main__":
    main()
