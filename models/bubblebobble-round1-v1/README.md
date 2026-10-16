# Bubble Bobble round 1 v1

A fixed neural policy for the cartridge and observation schema pinned in
[manifest.json](manifest.json). Inference uses structured state and weights only;
no scripted teacher or lookahead runs during play. Training method: observation-only teacher + DAgger.

This policy covers only the section named in its manifest. Later sections pause for human control.

## Recorded performance

| Test | Start type | Controller profile | Clears |
| --- | --- | --- | ---: |
| test | prepared-gameplay | pair | 6/6 |
| acceptance | prepared-gameplay | agent-p1 | 3/3 |
| acceptance | prepared-gameplay | agent-p2 | 3/3 |
| acceptance | prepared-gameplay | idle | 0/3 |
| acceptance | prepared-gameplay | pair | 3/3 |
| acceptance | prepared-gameplay | random | 3/3 |
| acceptance | prepared-gameplay | teacher | 3/3 |

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
