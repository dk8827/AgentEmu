"""Four exclusive gameplay action groups shared by training adapters."""

import numpy as np

FACTORS = (3, 3, 2, 2)


def buttons(action):
    a = np.asarray(action)
    if (
        a.shape != (4,)
        or not np.issubdtype(a.dtype, np.integer)
        or np.any(a < 0)
        or np.any(a >= FACTORS)
    ):
        raise ValueError("expected four integer factors within [3,3,2,2]")
    return (0, 64, 128)[a[0]] | (0, 16, 32)[a[1]] | int(a[2]) | int(a[3]) * 2


def factors(mask):
    if not 0 <= mask <= 255 or mask & 12 or mask & 192 == 192 or mask & 48 == 48:
        raise ValueError("invalid gameplay mask")
    return [
        1 if mask & 64 else 2 if mask & 128 else 0,
        1 if mask & 16 else 2 if mask & 32 else 0,
        int(bool(mask & 1)),
        int(bool(mask & 2)),
    ]
