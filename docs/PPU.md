# Picture processing unit

The PPU implements background and sprite rendering, scrolling, register behavior,
OAM DMA and interrupt timing. Native and browser players use the same core.

The timing details below describe NTSC. PAL cartridges now use 312 lines, a 16:5 PPU/CPU ratio and no odd-frame skip; see [expanded hardware](EXPANDED-HARDWARE.md) for implementation and remaining regional limits. The [compatibility guide](COMPATIBILITY.md) records the current cartridge checks.

## Implemented behavior

The PPU maps 2 KiB nametable RAM (or four independent KiB for MMC3 four-screen boards), palette RAM, primary OAM, the shared scroll/address latch, a PPUDATA read buffer and a separate PPU I/O latch. CHR accesses and nametable arrangement use the current cartridge mapping. Register aliases, palette aliases, six-bit palette storage, grayscale reads, OAM wrapping and unwired attribute bits are covered by tests. Side-effect-free inspection is separate from CPU reads.

Each CPU bus access advances three PPU dots, including reset, dummy accesses, interrupt entry and OAM DMA. The disabled-rendering clock uses 341 dots × 262 scanlines per frame. Vblank is set at scanline 241 dot 1 and cleared at pre-render scanline 261 dot 1. Coordinates identify the next dot to execute; CPU register access occurs before its three PPU ticks. Status reads just before vblank can suppress that frame's flag. With background rendering enabled, odd frames omit the final pre-render dot and last 89,341 dots instead of 89,342.

Background nametable, attribute and pattern reads run in eight-dot groups on visible and pre-render scanlines, including two prefetched tiles. Pattern and attribute shift registers produce 256×240 pixels with fine-X scrolling and left-edge clipping. Coarse X advances per tile, Y at dot 256, horizontal scroll copies at dot 257, and pre-render vertical scroll copies at dots 280–304. Coarse Y 29 and 31 have distinct wrap behavior. PPUDATA accesses during rendering increment both scroll axes.

A completed front buffer is published at vblank independently of the vblank status flag. Each `u16` pixel contains the six-bit palette color after grayscale masking and emphasis flags in bits 6–8. Transparent background pixels use the universal backdrop; forced blanking can select a palette address. The frontend converts these codes with a shared 512-entry NTSC 2C02 lookup generated with Pally, including all emphasis combinations; see [palette provenance](../crates/nes-video/palettes/README.md). This is a static color approximation, not a full analog NTSC decoder. `run_frame()` returns at the first CPU instruction boundary after publication.

OAM DMA stalls for 513 or 514 cycles and copies through the CPU bus and OAMDATA. The deterministic phase uses odd CPU cycles for DMA reads. Consecutive CPU read-modify-write writes finish before a single transfer from the final selected page. The PPU continues ticking throughout the transfer.

NMI output is sampled once per CPU cycle, then latched and polled at the CPU instruction-specific polling points. Eligible requests use the seven-cycle interrupt sequence. Pending NMI state survives snapshots, including edges during DMA. One `run_instructions` budget unit can be either an instruction or NMI entry; DMA cycles are included in the initiating instruction's step.

## Explicit limits

- Sprite evaluation, 8×8/8×16 fetches, priority, flipping, clipping, sprite-zero hits and the diagonal overflow scan are implemented. Active-rendering OAM writes are suppressed and OAMDATA exposes the internal read bus. OAMADDR corruption, unusual mid-scanline enable changes, PPUCTRL/PPUADDR write delays and bus-conflict races still need independent timing validation.
- CPU polling latency, NMI pulse suppression, BRK/IRQ hijacking, branch latency and IRQ timing around OAM DMA pass the current independent suites. This does not establish all silicon-race behavior. PPUMASK rendering changes take effect after a short dot delay, checked by the odd-frame timing ROM.
- Startup uses zeroed RAM and a fixed 29,658-CPU-cycle write-inhibit approximation. The PPU I/O latch decays per bit after a deterministic 0.6-second interval; actual analog decay varies by silicon and temperature. Randomized power-on phase and revision-specific PPU corruption are not modeled.
- DMA is atomic to the public instruction-step API. DMC transfers are implemented; combined DMA arbitration and obscure DMA/I/O address conflicts require additional tests.
- MMC3 observes the PPU address bus during background, sprite and dummy fetches and CPU VRAM access. The A12 low-time filter is a dot-level approximation, validated by the current six-ROM MMC3 suite; exact sub-dot M2 phases remain a limitation. Debugger peeks never clock mapper IRQs.

## Snapshots and validation

Snapshot version **6** includes PPU memory, fetch latches, shift registers, sprite evaluation/output state, both framebuffers, frame coordinates, skipped-dot accounting NMI/IRQ polling, APU/filter state and MMC3 state, plus per-bit PPU bus refresh timestamps. Versions 1–5 are deliberately rejected because they lack required hardware state. The JSON input limit is 1 MiB. Restore validates memory/framebuffer dimensions, register and color-code ranges, palette/OAM values, completed-frame counts and CPU/PPU clock agreement before changing the live machine. Snapshots are instruction-boundary snapshots; a fabricated pending DMA transfer is rejected.

Eleven register tests, eight background tests, seven sprite tests and five machine integration tests cover these behaviors. They include both DMA lengths, consecutive writes, repeated NMI/RTI, malformed snapshots and replay from partial addresses/read buffers/pending NMIs and mid-tile fetch state. A geometric oracle checks all 61,440 pixels of the original background ROM and every fine-scroll offset; other tests cover clipping, attributes, nametable wrap, fetch addresses and odd-frame timing. The original `ppu_rom()` and `background_rom()` diagnostics are checked against [native and compiled WASM fixtures](../fixtures/diagnostic/README.md), including complete state and framebuffer digests. Independent sprite-hit (11), sprite-overflow (5), vblank/NMI (10) and CPU-interrupt (5) ROM suites also pass; see [conformance inventory](CONFORMANCE.md).

## Hardware references

Implementation was written for this project from the documented behavior; no existing emulator code or commercial ROM was copied.

- [NESdev PPU registers](https://www.nesdev.org/wiki/PPU_registers): register protocol, buffering and palette reads.
- [NESdev PPU rendering](https://www.nesdev.org/wiki/PPU_rendering): fetch schedule, shift registers, pixels and odd-frame skip.
- [NESdev PPU scrolling](https://www.nesdev.org/wiki/PPU_scrolling): address/scroll bit layout and shared write toggle.
- [NESdev PPU frame timing](https://www.nesdev.org/wiki/PPU_frame_timing): frame lengths and timing model.
- [NESdev PPU power-up state](https://www.nesdev.org/wiki/PPU_power_up_state): initial write inhibition.
- [NESdev PPU OAM](https://www.nesdev.org/wiki/PPU_OAM): attribute storage.
- [NESdev DMA](https://www.nesdev.org/wiki/DMA) and [hardware DMA tests](https://forums.nesdev.org/viewtopic.php?t=14120): halt, alignment and consecutive-write behavior.

- [NESdev sprite evaluation](https://www.nesdev.org/wiki/PPU_sprite_evaluation): secondary OAM, per-dot evaluation and overflow scan.

DMC setup now overlaps OAM transfers; only the sample read steals the bus, with the required OAM re-alignment afterward. Both independent overlap-timing ROMs pass. Repeated DMC dummy/alignment reads hold the NES-001 joypad output-enable line active, giving one extra shifted bit rather than three. Consecutive PPUDATA access races, DMC stop bugs and unusual DMA register/address conflicts remain outside the validated model.
