# TMNT II Scene 1 v1

A fixed neural policy for the cartridge and observation schema pinned in
[manifest.json](manifest.json). Inference uses structured state and weights only;
no scripted teacher or lookahead runs during play. Training method: observation-only teacher + DAgger.

Menu timing and partner behavior affect success. Only Scene 1 is supported.

## Recorded performance

| Test | Start type | Controller profile | Clears |
| --- | --- | --- | ---: |
| test | prepared-gameplay | pair | 5/6 |
| acceptance | prepared-gameplay | agent-p1 | 1/3 |
| acceptance | prepared-gameplay | agent-p2 | 1/3 |
| acceptance | prepared-gameplay | idle | 0/3 |
| acceptance | prepared-gameplay | pair | 1/3 |
| acceptance | prepared-gameplay | random | 0/3 |
| acceptance | prepared-gameplay | teacher | 1/3 |

[evaluation.json](evaluation.json) retains every case in these evaluations,
including failures, frame counts and available controller/state digests. These
are recorded outcomes, not new runs on the current source revision. The compact
file is not a replay proof. The suites use deterministic timings and scripted
partners; they do not measure arbitrary human partners or Shared play. Baseline
profiles are separate from neural profiles and should not be pooled with them.

## Solo play

0/3 first-section clears with P1 neural and P2 absent, at power-on delays
0, 83 and 179. All three complete native episodes match independent WASM
inference, including failures. All three solo cases ended in player elimination. Co-op evidence does not establish solo competence.

## Use and train

Choose the game in **AI games**, select AI or Shared, then choose a matching ROM.
Use the game's Start/Select menu. **No player 2** disables the second controller;
it does not itself change the game's player-count menu.

See [training a new model](../../docs/TRAINING.md) for the maintained tools,
export validation and evaluation commands. The training examples produce new
candidates; they do not reproduce these historical weights exactly.
Weights and project-authored documentation are [MIT licensed](LICENSE).
