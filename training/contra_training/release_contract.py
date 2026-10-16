"""Check that a release protocol matches the loaded emulator extension."""

from . import _native


def check_death_metric(suite, task):
    if task != "iceclimber-mountain1":
        return
    actual = getattr(_native, "ICE_DEATH_METRIC", "death-transitions-v1")
    expected = suite.get("death_metric", "death-transitions-v1")
    if actual != expected:
        raise ValueError(
            f"Ice death metric mismatch: loaded extension uses {actual}, suite requires {expected}. "
            "Rebuild the extension and use fixtures/benchmarks/agentemu-ice-release-v2.json."
        )
