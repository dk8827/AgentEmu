# Compatibility

AgentEmu distinguishes three kinds of support:

| Evidence | What it establishes |
| --- | --- |
| Hardware conformance | Results against independent, pinned NES test ROMs |
| Cartridge traces | Boot/input execution, selected opening gameplay, backend parity and restore determinism |
| Agent evaluation | A particular model's performance on a particular game section and benchmark |

A mapper implementation or a passing short cartridge trace does not establish
full-game compatibility. Matching native and WASM results can share the same bug.
Non-silent PCM and matching audio bytes do not establish sound fidelity.

Cartridge metadata is in `catalog/games.json`. Known identities select solo,
alternating-player or light-gun controls; unknown hashes use generic controls.
[Bundled agents](MODELS.md) require exact revisions independently of human play.
See [cartridge hardware](EXPANDED-HARDWARE.md) for mapper, regional and peripheral
limits. Four-player adapters and physical light guns are not supported.

## Test coverage

The [hardware suite](CONFORMANCE.md) contains 52 pinned independent test ROMs.
Cartridge target lists and controller scripts live under `fixtures/compatibility/`.
They include single-player, cooperative and larger NES cartridge suites; entries
are test targets, not a promise that every game is fully compatible. Some scripts
reach only menus or introductions. Consult the scope of each generated report.
The larger target lists overlap and should not be added as a unique-game count.

## Reproduce cartridge checks

Supply your own matching ROMs. From the repository root:

```sh
cargo build --release --locked -p nes-cli
sh scripts/build-web.sh
node scripts/test-top100.mjs /path/to/ROMS
node scripts/test-top100.mjs /path/to/ROMS --suite=ign100
node scripts/test-games.mjs /path/to/US/ROMs --solo --gameplay
```

Use `--ids=11,22,57` with the larger runner to select entries. Exact file hashes
are checked before execution. Generated state, audio and captures stay under
`.cache/`. For a new report bound to a clean source revision and built artifact
hashes, use `python3 scripts/record-baseline.py --rom-root /path/to/ROMS
--suite top100` (or `ign100`). See [evidence requirements](EVIDENCE.md).
