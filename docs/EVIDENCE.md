# Evaluation evidence

Record the source revision, clean/dirty status, tool versions, ROM and model
hashes, reset procedure, input timing, horizon and every outcome, including
failures. Keep model selection and held-out cases separate. A new source release
does not retroactively validate an older evaluation.

## Bundled model records

Each `models/` bundle has a compact `evaluation.json`. It retains per-case
outcomes and available controller/final-state digests from recorded evaluations,
with totals recomputed by `scripts/model_evidence.py`. These records are bound
to the shipped weights and cartridge hash. They are not executable replays or
independent proof that a current build reproduces historical results.

Prepared gameplay starts, title-menu starts and solo play are different scopes.
Scripted partners are proxies, not human participants. Failure cases remain in
both the model cards and outcome records. Training outputs and study logs are
not part of the public distribution.

## New evaluations

Use the maintained training/evaluation tools to generate local controller
replays and native reports. Then run independent WASM inference over complete
episodes, comparing applied inputs and final machine state. Keep full local
records for diagnosis; publish a concise model card with the declared protocol,
complete case outcomes, limitations and commands. Never publish commercial ROMs,
save states, credentials or personal paths.

Hardware conformance needs independent NES test ROMs. Native/WASM agreement
checks consistency, not hardware accuracy. Non-silent PCM is an audio smoke test,
not a perceptual comparison with original hardware. See [compatibility](COMPATIBILITY.md).
