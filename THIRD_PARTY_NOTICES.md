# Third-party materials

The root MIT license applies to AgentEmu's original code, documentation and trained model weights. It does not relicense the materials below.

| Material | Source and terms | Location |
| --- | --- | --- |
| IBM Plex Sans Regular/SemiBold | IBM Plex, SIL Open Font License 1.1 | `crates/player-ui/assets/OFL.txt` and asset README |
| NES display palette | Pally / Persune, MIT-0 | `crates/nes-video/palettes/LICENSE-MIT-0` and README |
| Sampled NES 6502 CPU traces | SingleStepTests, MIT | `fixtures/cpu/LICENSE` and provenance |

Gallery title cards and the README diagram are original project artwork under MIT. The README hero is AI-generated artwork; its other captures show the actual AgentEmu interface. See [visual credits](docs/images/NOTICE.md). Game names identify compatibility; no affiliation or endorsement is implied. Commercial ROMs, game captures, save states and training datasets are not distributed.

`third-party/rust-dependencies.json` inventories the locked Cargo dependency graph across platforms. `third-party/rust-notices.txt` collects license/notice files from published crate sources and pinned upstream commits, including embedded font notices. `third-party/upstream/provenance.json` records source URLs and SHA256 digests. Regenerate with `python3 scripts/dependency-notices.py` after dependency changes. This is a comprehensive inventory, not a claim that every dependency is linked into every target. Crates with multiple license choices retain their upstream expressions. `option-ext` is MPL-2.0; its unmodified source is available at the recorded upstream location and crates.io version.

Python training and Node browser-test dependencies are installed separately by developers, not bundled into the player. Their installed distributions retain their own licenses. The current pinned direct dependencies are listed in `training/pyproject.toml` and `web/package-lock.json`.

Binary/static packages include this notice, the project and model licenses, and the collected Rust notices. Review the inventory whenever dependencies or packaging change.

The published packages for `malloc_buf` and `hexf-parse` omit license files;
this distribution includes their upstream license texts with pinned provenance.
`block` and `dispatch` identify Steven Sheldon as author and MIT as license in
their version-matched upstream manifests. Those manifests are retained, together
with the standard MIT permission text in `third-party/legacy-mit.txt`. Upstream
does not provide a separate copyright notice for these two packages; this is a
record of the declared terms, not a fabricated upstream notice.
