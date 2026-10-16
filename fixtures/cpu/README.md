# NES CPU instruction fixtures

These fixtures are sampled from the MIT-licensed [SingleStepTests NES 6502 corpus](https://github.com/SingleStepTests/65x02/tree/2f6980a2d95757486c7bee24355c360e40e2a224/nes6502). The upstream license is retained in `LICENSE`; `provenance.json` records the pinned revision and SHA256 of each full source file.

The checked-in subset takes cases 0, 625, 1250, and so on from each official opcode file: 16 cases per opcode, 2,416 total. They validate registers, memory and the exact ordered bus trace. These are flat 64 KiB CPU test memories, not game ROMs and not a test of PPU/APU integration.

Download the full 1,510,000-case corpus into the ignored local cache and regenerate the samples:

```sh
python3 scripts/fetch-cpu-tests.py
cargo test --release -p nes-core --test cpu_corpus -- --ignored --nocapture
```

The downloader uses eight concurrent requests and caches successful downloads. The emulator implementation is original; only test data and its license originate from the upstream project.
