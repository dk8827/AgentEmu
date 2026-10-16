# Original NES diagnostics

`nes_core::diagnostic::controller_rom()` builds an original NROM image from the documented instruction bytes in Rust. It contains no commercial code or graphics. The program strobes both controllers, stores their eight serial bits separately, writes through a CPU RAM mirror, increments a poll counter, and repeats.

`native.json` is the complete diagnostic report after a fresh boot with controller masks `0xa5` and `0x5a` and exactly 1,000 instructions. It includes CPU registers, PPU clock coordinates, the relevant RAM bytes, and a SHA256 of the serialized whole-machine state. Native CI regenerates and compares this report; compiled WASM executes the same sequence and compares it to this fixture.

```sh
cargo run --quiet -p nes-cli -- diagnose
cargo run -p nes-cli -- write-diagnostic .cache/controller-diagnostic.nes
cargo run -p nes-cli -- trace .cache/controller-diagnostic.nes 100
```

Create `.cache` first if needed. The writer refuses to overwrite an existing file. This controller program does not access PPU registers or the APU, although the PPU clock runs alongside the CPU.


## PPU, DMA and NMI diagnostic

`nes_core::diagnostic::ppu_rom()` waits through startup, checks buffered CHR and mirrored nametable reads, reads a palette alias, fills CPU RAM and transfers it to OAM with DMA, then enables NMI. Its handler increments a RAM counter and returns. It keeps rendering disabled and contains no third-party code or assets.

`ppu-native.json` captures 50,000 machine steps after boot. Expected RAM bytes 0–6 are `AB CD 42 2A 00 02 FF`; byte 0x20 is the completion marker `A5`, and byte 0x10 counts four NMIs. CPU cycles are 191,284 and PPU dots are 573,852. Tests assert these outcomes independently of the stored digest. Native and compiled WASM compare the complete report; both test snapshot/replay as well.

```sh
cargo run --quiet -p nes-cli -- diagnose-ppu
```

All fixture digests now use snapshot version 6, including graphics/audio pipelines, interrupt sampling and cartridge state. This changes the old controller digest without changing its 3,003-cycle CPU result. These fixtures validate deterministic parity of our implementation; they do not substitute for independent PPU timing/conformance tests or real-game compatibility checks.


## Background rendering diagnostic

`background_rom()` contains original program bytes and four original CHR tiles. It fills two nametables, sets four attribute quadrants and a palette, then enables background rendering at scroll (5,3). No commercial graphics or program code are included. `background-native.json` records eight completed frames (235,861 CPU cycles). Rust tests compare every pixel against geometric shapes independently of the fetch/shift implementation; WASM tests hash the actual returned color-code buffer and compare it with the native fixture.

Framebuffer digests cover 256×240 color codes in row-major order, each as an unsigned little-endian 16-bit value: palette color in bits 0–5, emphasis in bits 6–8. Display RGB conversion is outside the core and does not affect this digest.

```sh
cargo run --quiet -p nes-cli -- diagnose-background
```

## Original audio probe

`audio_rom()` configures pulse, triangle, noise and looping DMC output. `audio-native.json` records eight frames and `audio.f32` contains the resulting mono samples as little-endian IEEE float32 at 48 kHz. These bytes are synthesized by our emulator from original program bytes, with no commercial audio. The compiled-WASM test compares every sample byte and save/restore replay. Regenerate with `nes-cli diagnose-audio fixtures/diagnostic/audio.f32` and save its JSON stdout.

## Two-player playground

`playground_rom()` is an original, redistributable interactive diagnostic: two 16×16 sprite markers, independent controller polling, movement and pulse tones controlled by each player’s A button and X coordinate. Native regression tests verify independent movement, rendered colors, nonzero audio and deterministic replay. This diagnostic is generated only for tests; it is not offered in the player.
