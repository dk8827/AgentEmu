"""Preserve the last complete local artifact when a write is interrupted."""

from contextlib import contextmanager
import json
import os
from pathlib import Path
import tempfile

import numpy as np


@contextmanager
def atomic_file(path):
    path = Path(path)
    temporary = None
    try:
        descriptor, name = tempfile.mkstemp(dir=path.parent, prefix=path.name + ".", suffix=".tmp")
        temporary = Path(name)
        with os.fdopen(descriptor, "wb") as output:
            yield output
            output.flush()
            os.fsync(output.fileno())
        temporary.replace(path)
    finally:
        if temporary is not None:
            temporary.unlink(missing_ok=True)


def write_json(path, value, *, compact=False):
    text = (
        json.dumps(value, separators=(",", ":")) if compact else json.dumps(value, indent=2)
    ) + "\n"
    with atomic_file(path) as output:
        output.write(text.encode("utf-8"))


def save_dataset(path, observations, actions):
    with atomic_file(path) as output:
        np.savez_compressed(output, observations=observations, actions=actions)
