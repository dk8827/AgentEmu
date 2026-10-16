# Base 1 and Snow Bros. task setup

This page describes three separate, revision-pinned contracts. Tensor shapes and
action factorization stay fixed; a Jungle model never silently receives Base 1
or Snow Bros. features. The runtime pauses for a human handoff outside an active model's
supported gameplay. Both ports can use different supported models.

| Task | Observation | Architecture | Supported scope |
| --- | --- | --- | --- |
| Contra Jungle | contra-jungle-256-v1 | contra-mlp-256-64-64-10-v1 | Jungle |
| Contra Base 1 | contra-base1-256-v1 | contra-base1-mlp-256-64-64-10-v1 | Base 1 |
| Snow Bros. | snowbros-floor1-256-v1 | snowbros-mlp-256-64-64-10-v1 | First floor of the approved US dump |

All read immutable emulator RAM and encode 256 bounded floats. All use two hidden
layers of 64 ReLU units and 10 factorized outputs, one decision per frame. No
user-defined code or network architecture is accepted. Model cards distinguish
completed tasks from partial progress.

## Start and train

```sh
# Ordinary Jungle play generates a local Base 1 entry; no memory patches.
python -m contra_training.task_train prepare-base --rom /path/to/Contra.nes \
  --model models/contra-jungle-v2 --output .cache/base-entry
python -m contra_training.task_train train --task contra-base1 \
  --rom /path/to/Contra.nes --entry .cache/base-entry/entry.state \
  --output .cache/base-training --rounds 5 --epochs 50

# Snow Bros. starts a normal two-player game through menu inputs.
python -m contra_training.task_train train --task snowbros-floor1 \
  --rom /path/to/SnowBros.nes --output .cache/snow-training \
  --rounds 6 --epochs 60 --horizon 9000
python -m contra_training.task_train evaluate --task snowbros-floor1 \
  --rom /path/to/SnowBros.nes --model models/snowbros-floor1-v1 \
  --output .cache/snow-test --horizon 9000
```

Task training can continue an existing validated bundle with `--resume MODEL`.
The task, ROM and observation contracts must match. This starts a fresh optimizer
and gathers new teacher/DAgger examples; it does not restore an optimizer or an
old dataset. `--condition-observations` scales selected small input fields during
learning and folds that scale into the exported first-layer weights, so native
and browser inference still receive the original observation contract.
`--balance-directions` increases the training weight of rare direction labels.
The manifest records the source weight hash and learning scale. These switches
are available in both task and demonstration training.

Output directories must be new. The first Base entry is reached by a real v2
Jungle clear, then neutral input until the first room is ready. The entry file
is local and core-version dependent; provenance records ROM, model, input and
state hashes. This is a curriculum reset at Base 1, not a claim that the Base
policy can play Jungle. `models/contra-base1-v1` is an experimental baseline:
0/4 selection clears and 0/6 held-out clears, with mean held-out room progress
2.33. All six complete neural episodes match native/WASM inputs and final states.
It has not reached Waterfall.

`TaskEnv(rom, task, player=0, horizon=12000, start=None)` exposes Gymnasium's
256-float observation and `MultiDiscrete([3,3,2,2])` action spaces, with the
observation-only teacher on the other port. `TaskEpisode` exposes simultaneous
two-port observations and controller stepping through PyO3. Start/Select and
opposing directions are rejected during training. Finished episodes require a
reset; horizon exhaustion is truncation.

Teachers and DAgger labels use the same 256 features as the neural model. Six
initial timing variations supply demonstrations; training selects on 0/16/40/80.
Default evaluation uses separate 97/113/137/163/191/223 offsets. These vary timing
within a task, not level layouts. Trainers record loss, checkpoints, controller
inputs, hashes and evaluation reports. Evaluation runs have no
teacher intervention, branching or state restoration after reset.

Rewards: -0.001/frame, -1 per player death edge, +1 per new Base room or first-floor
Snow enemy-count high-water improvement, +10 for the actual next-stage/floor
transition. Base success is `level == 2` (Waterfall); Snow success is the floor
sequence advancing to 1. A boss flag or zero visible enemies alone is not success.

## Base 1 fields

RAM semantics were cross-checked against `vermiceli/nes-contra-us` at revision
`687d651c` and local controller experiments. Common player fields retain the
Jungle mapping. Additional fields are room (`$64`), room-clear / barrier flags
(`$37`), location (`$40`), transition (`$73`) and per-player advance (`$D0/$D1`).
Entities use all occupied enemy slots (`$4B8`), including background objectives;
health is `$578`, type `$528`, positions `$33E/$324`, and the crouched-bullet
collision requirement comes from `$598` bit 5. Type meanings are level-specific.

| Indices | Encoding |
| --- | --- |
| 0–15 | Slot, gameplay, phase/7, phase parity, room/8, room cleared, barrier removed, boss area, boss flag, P2 active, transition, own/other advancing, supported, two zero reservations |
| 16–47 | Same 16 player fields as Jungle, self then teammate |
| 48–255 | 16 stable entity slots × 13: valid, relative X/256, Y/240, VX/8, VY/8, HP/64, core, turret, projectile, roller, grenade, boss/plate, crouched-bullet requirement |

The teacher accounts for the perspective convergence of indoor shots, advances
only after the barrier clears, and attempts to duck bullets/jump rollers. It is
an experimental controller, not an expert demonstration of a complete stage.

## Snow Bros. fields

Pinned SHA256: `5c7ab65f5ed5a00f2c6193e060423f05b5b88c1e937f5514686627a5e9529c58`.
This is the exact tested cartridge variant; no assumption of a different reference
revision. Fields were established by independent P1/P2 movement, jumping,
ordinary deaths, first-floor completion and read-only inspection of local game
routines. No commercial code/data is included in this repository.

- Player X/Y: `$476/$463` plus slot. Velocity: signed `$534/$521` divided by 16.
  Ground contact: `$354` bit 7; death: `$369` bits 5/6; reserve lives:
  `$76/$77` (bit 7 means game over); spawn invincibility: `$3D2`; facing: `$4FB` bit 6.
- Floor sequence / layout: `$73/$74`; remaining enemy counter `$85`; phase `$50`.
  Gameplay is restricted to the first floor/layout and gameplay mode `$70 == 5`,
  with menu/pause gates. Slots 2–18 with `$450` bit 7 set supply entity positions,
  velocity, type (`$32A`) and ground contact. Object types are exposed as IDs;
  they are not silently labeled as generic enemies or collectibles.
- The 16×15 collision lattice begins at `$5FC`. Codes 1/2 are first-floor solid
  platform cells; the vector keeps their channels separate. Other codes are
  grouped without claiming their detailed behavior on untested floors.

| Indices | Encoding |
| --- | --- |
| 0–7 | Slot, gameplay, phase/7, phase parity, floor/50, enemy count/8, P2 active, zero |
| 8–39 | Self then other, 16 each: active, X/255, Y/240, VX/8, VY/8, grounded, dead, reserve lives/10, invincible, facing left, six zeros |
| 40–95 | Nearest eight entities × 7: valid, relative X/256, Y/240, VX/8, VY/8, type/63, grounded; distance then slot tie-break |
| 96–255 | Player-centered 8×5 grid, 16px spacing, columns -2…5 and rows -2…2; channels known, code 1, code 2, other nonzero code |

Both adapters clamp features to [-1,1]. Unused entities and out-of-screen terrain
are masked. The first Snow model is in `models/snowbros-floor1-v1`; it has not been
trained for later floors, bosses or human teammate behavior.

## Verify locally

```sh
python scripts/validate-tasks.py --snow /path/to/SnowBros.nes \
  --contra /path/to/Contra.nes --entry .cache/base-entry/entry.state
node scripts/test-task-agent.mjs /path/to/SnowBros.nes \
  .cache/snow-training/round-00 .cache/snow-test/report.json
```

The first command checks independent controls, semantic fields, exact reset,
read-only observation, bounds and truncation. The second reruns every recorded
neural episode through the application's WASM adapters and agent integration,
comparing controller bytes and the entire final emulator state to native results.
Base parity uses the optional fourth argument for the matching local entry file.

Expansion replays use the same player replay UI. Snow replays reset normally;
Base replays require loading their original local entry state first and verify
its hash before playback. No Base snapshot is bundled or published.
