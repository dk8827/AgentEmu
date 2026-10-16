# Contra Jungle teammate v3

A fixed neural policy for the cartridge and observation schema pinned in
[manifest.json](manifest.json). Inference uses structured state and weights only;
no scripted teacher or lookahead runs during play. Training method: cooperative DAgger.

Retreating scripted partners are a known failure case. Only Jungle is supported; use the separate Base 1 model for the next section.

## Recorded performance

| Test | Start type | Controller profile | Clears |
| --- | --- | --- | ---: |
| test | prepared-gameplay | jumper | 1/2 |
| test | prepared-gameplay | pair | 4/4 |
| test | prepared-gameplay | pause | 1/2 |
| test | prepared-gameplay | retreat | 0/2 |
| test | prepared-gameplay | steady | 2/2 |
| legacy | prepared-gameplay | pair | 26/32 |

[evaluation.json](evaluation.json) retains every case in these evaluations,
including failures, frame counts and available controller/state digests. These
are recorded outcomes, not new runs on the current source revision. The compact
file is not a replay proof. The suites use deterministic timings and scripted
partners; they do not measure arbitrary human partners or Shared play. Baseline
profiles are separate from neural profiles and should not be pooled with them.

## Solo play

0/3 first-section clears with P1 neural and P2 absent, at power-on delays
0, 83 and 179. All three complete native episodes match independent WASM
inference, including failures. This is the bundled cooperative model, not a research solo checkpoint. The earlier attract-demo false clears were excluded.

## Use and train

Choose the game in **AI games**, select AI or Shared, then choose a matching ROM.
Use the game's Start/Select menu. **No player 2** disables the second controller;
it does not itself change the game's player-count menu.

See [training a new model](../../docs/TRAINING.md) for the maintained tools,
export validation and evaluation commands. The training examples produce new
candidates; they do not reproduce these historical weights exactly.
Weights and project-authored documentation are [MIT licensed](LICENSE).
