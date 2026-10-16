# Bubble Bobble round 1 v2

A fixed neural policy for the cartridge and observation schema pinned in
[manifest.json](manifest.json). Inference uses structured state and weights only;
no scripted teacher or lookahead runs during play. Training method: mixed-teammate DAgger.

This policy covers only the section named in its manifest. Later sections pause for human control.

## Recorded performance

| Test | Start type | Controller profile | Clears |
| --- | --- | --- | ---: |
| evaluation | prepared-gameplay | agent-p1 | 10/10 |
| evaluation | prepared-gameplay | agent-p2 | 10/10 |
| evaluation | prepared-gameplay | idle | 0/10 |
| evaluation | prepared-gameplay | pair | 10/10 |
| evaluation | prepared-gameplay | random | 10/10 |
| evaluation | prepared-gameplay | teacher | 10/10 |
| startup | title-menu | agent-p1 | 1/1 |
| startup | title-menu | agent-p2 | 1/1 |
| startup | title-menu | pair | 1/1 |
| solo-startup | title-menu | solo | 4/4 |

[evaluation.json](evaluation.json) retains every case in these evaluations,
including failures, frame counts and available controller/state digests. These
are recorded outcomes, not new runs on the current source revision. The compact
file is not a replay proof. The suites use deterministic timings and scripted
partners; they do not measure arbitrary human partners or Shared play. Baseline
profiles are separate from neural profiles and should not be pooled with them.

## Solo play

3/3 first-section clears with P1 neural and P2 absent, at power-on delays
0, 83 and 179. All three complete native episodes match independent WASM
inference, including failures. These checks cover Round 1 only.

## Use and train

Choose the game in **AI games**, select AI or Shared, then choose a matching ROM.
Use the game's Start/Select menu. **No player 2** disables the second controller;
it does not itself change the game's player-count menu.

See [training a new model](../../docs/TRAINING.md) for the maintained tools,
export validation and evaluation commands. The training examples produce new
candidates; they do not reproduce these historical weights exactly.
Weights and project-authored documentation are [MIT licensed](LICENSE).
