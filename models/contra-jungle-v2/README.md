# Contra Jungle stage-clear v2

A fixed neural policy for the cartridge and observation schema pinned in
[manifest.json](manifest.json). Inference uses structured state and weights only;
no scripted teacher or lookahead runs during play. Training method: DAgger + weighted behavior cloning.

The 32 recorded starts overlap training and selection; they are regression cases, not an independent test set. Only Jungle is supported.

## Recorded performance

| Test | Start type | Controller profile | Clears |
| --- | --- | --- | ---: |
| evaluation | prepared-gameplay | pair | 26/32 |

[evaluation.json](evaluation.json) retains every case in these evaluations,
including failures, frame counts and available controller/state digests. These
are recorded outcomes, not new runs on the current source revision. The compact
file is not a replay proof. The suites use deterministic timings and scripted
partners; they do not measure arbitrary human partners or Shared play. Baseline
profiles are separate from neural profiles and should not be pooled with them.

## Solo play

No solo menu evaluation is recorded for this version. Co-op results do not establish solo performance.

## Use and train

Choose the game in **AI games**, select AI or Shared, then choose a matching ROM.
Use the game's Start/Select menu. **No player 2** disables the second controller;
it does not itself change the game's player-count menu.

See [training a new model](../../docs/TRAINING.md) for the maintained tools,
export validation and evaluation commands. The training examples produce new
candidates; they do not reproduce these historical weights exactly.
Weights and project-authored documentation are [MIT licensed](LICENSE).
