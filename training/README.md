# Training package

Python/Gymnasium APIs backed by the Rust emulator, fixed PyTorch actors,
imitation/PPO examples and validated model export.

Start with [Train a new model](../docs/TRAINING.md) for installation, a small
working example, solo environments, other games and deployment checks.
Install from the repository root with `python -m pip install ./training`.
The package name remains `contra-training`; the `agentemu-train` CLI is its
preferred entry point. Run `agentemu-train --help` after installing.

Run `python scripts/check.py training` with the environment activated.
Commercial-ROM tests are opt-in; set the ROM variables described in the tests.
Never commit ROMs, save states, datasets, optimizer checkpoints or run logs.
