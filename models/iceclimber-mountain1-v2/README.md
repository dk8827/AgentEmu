# Ice Climber mountain 1 v2

A fixed neural policy for the cartridge and observation schema pinned in
[manifest.json](manifest.json). Inference uses structured state and weights only;
no scripted teacher or lookahead runs during play. Training method: verified controller demonstrations + nearest-demonstration DAgger.

The default paired menu start fails. Historical death totals count animation transitions and can miss lives lost behind the scrolling screen. Use cartridge-life counters in new evaluations. Only Mountain 1 is supported.

## Recorded performance

| Test | Start type | Controller profile | Clears |
| --- | --- | --- | ---: |
| test | prepared-gameplay | pair | 3/6 |
| acceptance | prepared-gameplay | agent-p1 | 0/3 |
| acceptance | prepared-gameplay | agent-p2 | 2/3 |
| acceptance | prepared-gameplay | idle | 0/3 |
| acceptance | prepared-gameplay | pair | 2/3 |
| acceptance | prepared-gameplay | random | 1/3 |
| acceptance | prepared-gameplay | teacher | 0/3 |

[evaluation.json](evaluation.json) retains every case in these evaluations,
including failures, frame counts and available controller/state digests. These
are recorded outcomes, not new runs on the current source revision. The compact
file is not a replay proof. The suites use deterministic timings and scripted
partners; they do not measure arbitrary human partners or Shared play. Baseline
profiles are separate from neural profiles and should not be pooled with them.

## Solo play

0/3 first-section clears with P1 neural and P2 absent, at power-on delays
0, 83 and 179. All three complete native episodes match independent WASM
inference, including failures. Solo execution works, but this model is not reliable at clearing Mountain 1.

## Use and train

Choose the game in **AI games**, select AI or Shared, then choose a matching ROM.
Use the game's Start/Select menu. **No player 2** disables the second controller;
it does not itself change the game's player-count menu.

See [training a new model](../../docs/TRAINING.md) for the maintained tools,
export validation and evaluation commands. The training examples produce new
candidates; they do not reproduce these historical weights exactly.
Weights and project-authored documentation are [MIT licensed](LICENSE).
