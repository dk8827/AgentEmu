# Contra agents

Contra models use revision-pinned, structured game observations. Jungle policies
observe 256 bounded features and output four controller groups through a
256→64→64→10 ReLU network. Fields describe players, camera-relative terrain,
enemies and boss state; see `integrations/contra-jungle/observation.json` for
ordering and normalization. Policies act once per emulated frame.

Jungle and Base 1 have separate contracts. The session pauses for a handoff when
an assigned model reaches unsupported gameplay. Start/Select remain human inputs.
The [model catalog](MODELS.md) distinguishes recommended and experimental bundles;
model cards document measured clears and failures. Co-op success is not evidence
of solo competence or useful human assistance.

[Train a new model](TRAINING.md) covers the maintained imitation/PPO example,
real solo Gymnasium environments and export/evaluation checks.
[Shared control](SHARED-CONTROL.md) explains exclusive ownership of button groups.

For native/WASM observation and controller parity with a local ROM:

```sh
python scripts/record-contra-parity.py /path/to/Contra.nes models/contra-jungle-v3
node scripts/test-contra-local.mjs /path/to/Contra.nes models/contra-jungle-v3
```

These short checks test execution consistency, not first-stage completion.
