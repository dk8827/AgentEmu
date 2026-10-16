# Hardware conformance inventory

The acceptance inventory contains 52 independent hardware tests. Run the command below on your build; a passing inventory is not a claim that every NES test ROM or commercial game works.

| Suite | Result |
| --- | --- |
| `apu_test/rom_singles` | 8/8 |
| `sprite_hit_tests_2005.10.05` | 11/11 |
| `sprite_overflow_tests` | 5/5 |
| `ppu_vbl_nmi/rom_singles` | 10/10 |
| `cpu_interrupts_v2/rom_singles` | 5/5 |
| `mmc3_test_2/rom_singles` | 6/6, including alternate IRQ mode for test 6 |
| `ppu_open_bus` and `oam_read` | 2/2 |
| `sprdma_and_dmc_dma` | 2/2 |
| `dmc_dma_during_read4`: controller read, PPUDATA write, read/write | 3/3 |

Source: [public NES test collection](https://github.com/christopherpow/nes-test-roms/tree/95d8f621ae55cee0d09b91519a8989ae0e64753b), pinned at `95d8f621ae55cee0d09b91519a8989ae0e64753b`. Tests and their documentation are by their respective authors, including Shay Green (blargg). These external ROMs are downloaded only into `.cache`, not redistributed in this repository. Source code was consulted to interpret failing tests; the emulator implementation is original.

Run after loading the project Rust environment:

```sh
python3 scripts/test-conformance.py --fetch
```

The runner builds the release CLI, verifies the test checkout revision, executes each ROM with a 1,200-frame limit and writes `.cache/conformance.json`. A nonzero test result, core error or timeout fails the run. It accepts a result only from the test's documented signature/status protocol or completed on-screen `PASSED` text; it never treats merely running for a fixed duration as success.

The original checked-in diagnostics continue to run in ordinary native/WASM CI. The CPU additionally has a 2,416-case official-opcode bus corpus subset for fast regression coverage. Reset-button behavior, unofficial CPU opcodes, DMC stop bugs and sub-dot PPUDATA races remain outside this passing inventory. The collection’s `double_2007_read` and `dma_2007_read` print phase-dependent CRCs, so they are not classified by the generic pass/fail runner. Our adjacent-PPUDATA result does not yet match the documented hardware variants; it remains a known limitation. The larger read-buffer test has not been accepted into this passing inventory; CNROM itself is implemented. Older `dmc_tests` binaries did not produce an interpreted result and are not counted as passes.

Commercial cartridges have separate, local input/replay and native/WASM parity checks; see [compatibility](COMPATIBILITY.md).
