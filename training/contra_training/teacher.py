"""Jungle demonstration teacher, using only the public 256-feature observation.

This is training code, never a fallback in deployed neural inference. The final
boss arena threshold is stage-specific route knowledge, explicitly disclosed.
"""

import numpy as np
from .observation_contracts import LAYOUTS

FEATURES = {name: spec[0] for name, spec in LAYOUTS["contra-jungle"].items()}
NAME = "terrain-threat-jungle-v3"


def action(observation):
    o = observation

    def feature(name):
        return o[FEATURES[name]]

    def floor(row, column):
        cell = row * 8 + column
        return feature(f"terrain[{cell}].floor") > 0.5 or feature(f"terrain[{cell}].solid") > 0.5

    edge = not floor(3, 3) and (floor(3, 2) or floor(4, 2))
    obstacle = floor(2, 3) or floor(1, 3)
    threat = any(
        feature(f"entities[{i}].present") > 0.5
        and 0 < feature(f"entities[{i}].dx") * 256 < 32
        and abs(feature(f"entities[{i}].dy") * 240) < 24
        and feature(f"entities[{i}].kind0") < 0.5
        for i in range(8)
    )
    boss_arena = feature("header.camera_x") * 4096 >= 2970
    jump = feature("players[0].grounded") > 0.5 and (edge or obstacle or threat) and not boss_arena
    # The final descent approaches the door target from above, then below.
    # Aim diagonally while advancing; standing fire passes below its weak point.
    vertical = 0
    if boss_arena and feature("header.world_x") * 4096 > 3180:
        if feature("players[0].y") * 240 > 180:
            vertical = 1
        elif feature("players[0].y") * 240 < 160:
            vertical = 2
    return np.array(
        [2, vertical, int(jump), int(feature("header.fire_phase") < 0.5)], dtype=np.int64
    )


def mask(observation):
    a = action(observation)
    return 128 | (0, 16, 32)[a[1]] | int(a[2]) | (int(a[3]) << 1)


def raised_platform_action(o):
    """Wider overhead lookahead; integer pixels avoid rounding-dependent thresholds.

    This new supervision profile leaves the frozen v3 benchmark teacher intact.
    The local six-start pilot cleared all starts, with both players alive in four.
    """

    def f(name):
        return o[FEATURES[name]]

    def pixel(name, scale):
        return round(float(f(name)) * scale)

    def floor(r, c):
        return f(f"terrain[{r * 8 + c}].floor") > 0.5 or f(f"terrain[{r * 8 + c}].solid") > 0.5

    edge = not floor(3, 3) and (floor(3, 2) or floor(4, 2))
    obstacle = floor(2, 3) or floor(1, 3)
    es = [
        (
            f(f"entities[{i}].present") > 0.5,
            pixel(f"entities[{i}].dx", 256),
            pixel(f"entities[{i}].dy", 240),
            f(f"entities[{i}].kind0"),
        )
        for i in range(8)
    ]
    threat = any(p and 0 < dx < 32 and abs(dy) < 24 and kind < 0.5 for p, dx, dy, kind in es)
    camera = pixel("header.camera_x", 4096)
    world = pixel("header.world_x", 4096)
    y = pixel("players[0].y", 240)
    jump = (
        f("players[0].grounded") > 0.5
        and (edge or obstacle or floor(0, 3) or floor(0, 4) or floor(1, 4) or threat)
        and camera < 2970
    )
    v = 1 if y > 180 else 2 if y < 160 else 0
    if camera < 2970 or world <= 3180:
        v = 0
    safe = not any(p and abs(dx) < 64 and abs(dy) < 32 for p, dx, dy, kind in es)
    wait = (
        f("players[1].active") > 0.5
        and f("players[1].dead") < 0.5
        and f("players[0].grounded") > 0.5
        and f("players[1].grounded") > 0.5
        and safe
        and pixel("players[0].x", 255) - pixel("players[1].x", 255) > 64
        and camera < 2970
    )
    return np.array([0 if wait else 2, v, int(jump), int(f("header.fire_phase") < 0.5)], np.int64)


def island_takeoff_action(o):
    """Training supervisor with an explicit takeoff point on the last small island.

    A threat-triggered jump immediately after landing can fall short of the next
    platform. Walk to its right edge first. This uses stage-specific route
    knowledge, like the boss aiming rule; it never overrides a deployed policy.
    """
    result = raised_platform_action(o)
    world = round(float(o[FEATURES["header.world_x"]]) * 4096)
    if 2850 <= world < 2920 and o[FEATURES["players[0].grounded"]] > 0.5:
        result[0] = 2
        result[2] = int(world >= 2900)
    return result
