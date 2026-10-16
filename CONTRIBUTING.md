# Contributing to AgentEmu

Small, focused changes are welcome. Discuss architecture and observation-schema
changes before implementation. Contributions need clear provenance and license
terms compatible with the project; retain third-party notices.

## Setup

Install the pinned Rust toolchain, Node 22+, Python 3.11+ and the native libraries
listed in the [README](README.md). Run commands from the repository root. Use a
project-local Python environment; no commercial ROM is required for ordinary CI.

```sh
cargo install wasm-bindgen-cli --version 0.2.129 --locked
npm --prefix web ci
npm --prefix web exec -- playwright install chromium
```

Use `PLAYWRIGHT_CHROME=1` with an installed Chrome instead of downloaded Chromium.
See [training setup](docs/TRAINING.md) for the Python/Rust package.

## Checks

| Change | Command | Scope |
| --- | --- | --- |
| Rust | `python3 scripts/check.py core` | Format, Clippy and workspace tests |
| Browser | `python3 scripts/check.py browser` | Build, runtime tests and packaged desktop/mobile UI |
| Browser unit tests only | `python3 scripts/check.py browser-unit` | Existing WASM artifacts and JS reference tests |
| Training | `python scripts/check.py training` | Activated training environment and Python tests |
| Catalog/docs/release tree | `python3 scripts/check.py release` | Generated contracts, catalogs, links, licenses and tracked contents |

CI definitions run the corresponding checks on fresh checkouts, but GitHub
Actions is currently disabled. Run the named checks locally and report results
in your pull request. Browser acceptance serves
`dist/web` under `/AgentEmu/`, exercises real input/file pickers, captures desktop
and phone-sized screenshots, and measures generated audio. It does not replace
physical-device or screen-reader testing. JS reference helpers under `tests/`
preserve historical regression cases and are not the production implementation.

For Python formatting, install `ruff==0.15.0` in a local development environment
and run `ruff format scripts training`. Browser host formatting uses
`npm --prefix web run format:check`. Generated contracts have their own deterministic
format and are excluded from automatic formatting.

## Changing integrations or models

Read [Architecture](docs/ARCHITECTURE.md) and [Adding a game](docs/ADDING-A-GAME.md).
Edit `integrations/` and `catalog/`, then run their sync scripts. Never change
hashes to bypass compatibility checks. Model cards must state the training method,
selection/evaluation split, failure cases and commands. Follow [evidence provenance](docs/EVIDENCE.md)
when recording new runs.

## Optional local checks

`python3 scripts/test-conformance.py --fetch` fetches a pinned hardware-test-ROM
checkout into the ignored cache. Commercial-game suites require local matching
ROMs and write results under `.cache/`; see [Compatibility](docs/COMPATIBILITY.md).
Keep ROMs, states, datasets, credentials, generated builds and private paths out of Git.

## Pull requests and reports

Describe the problem, behavior change and relevant checks. Add regression coverage
for functional changes. Preserve unrelated work and avoid incidental formatting.
Bug reports need platform, app revision, cartridge hash/region and reproduction
steps. Do not attach ROMs or save-state payloads. See [SECURITY.md](SECURITY.md) for
vulnerability reporting.
