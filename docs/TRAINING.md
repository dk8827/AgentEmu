# Train a new model

The public tools train fixed networks from structured game state. They include
Gymnasium environments, a Contra imitation/PPO example, game-specific imitation
trainers, safetensors export and native/WASM evaluation. Training runs locally;
there is no hosted training service or model upload service.

You need your own matching ROM. Each task pins its cartridge SHA-256 in
`integrations/TASK/task.json`. Check a file with `python3 scripts/rom-info.py
/path/to/game.nes`; do not alter a contract hash to accept another revision.

## Install

From the repository root, with the pinned Rust toolchain and Python 3.11+:

```sh
python3 -m venv .venv-training
# macOS / Linux
. .venv-training/bin/activate
# Windows PowerShell: .venv-training\Scripts\Activate.ps1
python -m pip install --upgrade pip
python -m pip install ./training
python scripts/check.py training
```

The install builds the Rust extension and installs PyTorch, Gymnasium,
Stable-Baselines3 and safetensors. The package is named `contra-training` for API
compatibility; it includes other games. No GPU is required. Run commands below
from the repository root with this environment activated. Outputs belong under
`.cache/`, which is ignored by Git. Every command requires a new output directory.

## First model: Contra Jungle

Start with a small end-to-end smoke run:

```sh
agentemu-train train --rom /path/to/Contra.nes \
  --output .cache/contra-smoke --bc-steps 256 --ppo-steps 0 \
  --horizon 240 --eval-seeds 1 --seed 7
```

This collects observations and teacher labels, fits the network, exports it,
checks PyTorch/native logits and decoded buttons, and evaluates trained,
scripted and random agents. It tests the pipeline; 240 frames cannot demonstrate
a stage clear.

For a larger candidate, use a separate output directory:

```sh
agentemu-train train --rom /path/to/Contra.nes \
  --output .cache/contra-candidate --bc-steps 12000 --ppo-steps 65536 \
  --envs 4 --horizon 9000 --eval-seeds 8 --seed 7
```

This example trains **co-op**: one learned controller with a scripted teammate.
Behavior cloning labels both players. PPO refines the imitation policy, then
validation reward selects between the two candidates. More steps or a higher
reward do not guarantee better gameplay. Inspect clears, deaths and progress;
compare the unchanged imitation baseline before keeping a PPO result.

The output includes `manifest.json`, `weights.safetensors`, `parity.json`,
`evaluation.json` and a local observation sample. The fixed actor is
256 → 64 → 64 → 10 with ReLU hidden layers and four action heads:
Left/Right/neutral, Up/Down/neutral, A and B. Start/Select belong to menu handling,
not the policy. The runtime neutralizes policy input outside supported gameplay.

## Real solo environments

An idle player-two controller in a two-player game is not solo play. Both APIs
below choose the game's one-player mode and enforce an absent second player:

```python
from contra_training.env import ContraEnv
from contra_training.tasks import TaskEnv

contra = ContraEnv("/path/to/Contra.nes", player=0, teammate="idle",
                   single_player=True, horizon=9000)
snow = TaskEnv("/path/to/SnowBros.nes", "snowbros-floor1",
               player=0, single_player=True, horizon=12000)
observation, info = contra.reset(seed=7)
observation, reward, terminated, truncated, info = contra.step([2, 0, 0, 1])
```

Use these with your own Gymnasium trainer. Their observations use the same
contract as deployment. `ActorNet`, `from_ppo` and `export` in
`contra_training.model` support the fixed network. SB3's default actor layout is
not compatible: use two 64-unit ReLU layers. PPO archives are training artifacts;
the player accepts the validated manifest and safetensors, not Python pickles.
The included CLI trains co-op by default; it must not be described as solo training.

## Other games

For Snow Bros., Bubble Bobble, Double Dragon II and TMNT II, the maintained
trainer collects game-specific teacher trajectories and iteratively adds labels
from the learned policy's states (DAgger):

```sh
python -m contra_training.task_train train --task snowbros-floor1 \
  --rom /path/to/SnowBros.nes --output .cache/snow-candidate \
  --rounds 6 --epochs 40 --horizon 12000 --seed 83
```

Task IDs are `snowbros-floor1`, `bubblebobble-round1`,
`doubledragon2-mission1`, `tmnt2-scene1` and `iceclimber-mountain1`.
These examples use two-player resets. `result.json` identifies the best selected
`round-NN` directory. `--resume MODEL` initializes weights, not optimizer state
or the old dataset. A teacher can fail; inspect its report before training longer.

Ice Climber requires successful controller demonstrations instead of the simple
scripted teacher. `python -m contra_training.trajectory_train --help` describes
the replay-based trainer. Demonstrations must use the declared reset and ROM,
contain per-frame controller pairs and matching input hashes, and replay to a
clear without state patches. The repository does not distribute demonstrations
or a demonstration-search service. See `collect` in that module for validation.
For Base 1's curriculum entry, see [task setup](AGENT-TASKS.md).

## Evaluate and try the exported policy

Run separate evaluation starts after selecting a candidate. For Contra:

```sh
agentemu-train evaluate-pair --rom /path/to/Contra.nes \
  --model .cache/contra-candidate --output .cache/contra-pair-test \
  --horizon 9000 --eval-seeds 8
```

For other tasks, substitute the chosen `round-NN` path:

```sh
python -m contra_training.task_train evaluate --task snowbros-floor1 \
  --rom /path/to/SnowBros.nes --model .cache/snow-candidate/round-00 \
  --output .cache/snow-test --horizon 12000
sh scripts/build-web.sh
node scripts/test-task-agent.mjs /path/to/SnowBros.nes \
  .cache/snow-candidate/round-00 .cache/snow-test/report.json
```

The WASM check independently infers each action and compares full episode inputs
and final state with native execution, including failed episodes. For Contra's
co-op result, use `scripts/test-contra-clear.mjs ROM MODEL REPORT` instead.
Check CLI help before changing evaluation timings. Once used for model selection,
a case is development data; choose a new held-out set for a new comparison.

Try an exported Contra model locally:

```sh
cargo run --release --locked -p frontend-native --bin agentemu -- \
  /path/to/Contra.nes --agent1 .cache/contra-candidate
```

Use the cartridge menu to select one or two players. To test a trained pair, add
`--agent2 .cache/contra-candidate`. `--help` lists the other developer arguments.
The shipped gallery contains reviewed bundles only. To propose a new bundle,
add its manifest, weights, license, honest model card and compact evaluation
record; register it in `catalog/models.json`, then run
`python3 scripts/sync-catalog.py`. Follow [adding an integration](ADDING-A-GAME.md)
if the game or observation schema is new. Review each added file in
`release-files.json`; do not include datasets, rollouts or candidate directories.

## What counts as improvement

Report section clears, completion time, deaths/lives remaining, failure modes
and solo/co-op/controller ownership separately. Include random, idle and scripted
baselines where meaningful. Test ordinary menu starts as well as prepared starts,
multiple timings, both active ports and native/WASM parity. Do not pool repeated
identical trajectories as independent evidence. Shared play needs human testing;
autonomous clear rates do not measure how helpful a model is to a person.

The bundled cards contain compact historical results, not exact training recipes
for those weights. The examples above create new candidates. See
[evaluation evidence](EVIDENCE.md) for what to record for your own model.
