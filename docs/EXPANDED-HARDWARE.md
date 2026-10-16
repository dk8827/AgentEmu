# Cartridge hardware and regional support

The core implements CNROM (3), AxROM (7), MMC5 (5), FME-7 (69),
SUROM's 512 KiB outer PRG bank and MMC6's mirrored 1 KiB RAM with independent
512-byte read/write permissions. MMC5 includes ROM/RAM PRG windows, CHR bank
sizes, separate background/sprite banks, extended attributes, ExRAM/fill/CIRAM
nametable routing, multiplication and scanline IRQ detection from PPU reads.
MMC5 expansion audio and vertical split are outside the US Castlevania III and
Gemfire target scope. FME-7 expansion audio is not included: the selected
European Mr. Gimmick cartridge does not use the Japanese Sunsoft 5B audio.

PAL runs 312 scanlines without NTSC's odd-frame skip, 16 PPU dots per five CPU
cycles, PAL noise/DMC timer tables and frame sequencer, and 48 kHz audio derived
from the PAL master clock. Native and browser pacing use the cartridge's rate.
The iNES TV bit selects PAL. Exact PRG hashes also identify the two selected
European dumps whose legacy headers omit that bit. Dendy is not implemented;
PAL's different analog color decoding and OAM refresh quirks remain limitations.

Duck Hunt automatically attaches a Zapper to port 2 using its exact PRG hash.
Mouse/touch coordinates map to the displayed game image. A short touch trigger
is retained for three frames, with aim retained through the game's hit-test
flashes. The light sensor samples the rendered beam around the aim point, with a
26-scanline decay approximation, rather than simply reporting a hit on a click.
Native mouse clicks use the same core sensor. Port 1 remains a joypad for menus
and steering a duck. Physical light guns are not supported.

MMC6 is identified by the two retail StarTropics PRG hashes because iNES 1.0
cannot distinguish MMC3 from MMC6. Its battery RAM persists as 1024 bytes; other
supported battery boards use 8192 bytes. Original ROM files are never modified.
The narrowly recognized `DiskDude!` signature and the specific cartridge
header exceptions documented below are interpreted in memory.

Regression tests cover bank windows, bus conflicts, single-screen mirroring,
RAM protection, multiplier, extended attributes, mapper IRQ acknowledgement,
PAL CPU/PPU clock ratio, audio sample counts, snapshot replay, and light/dark
sensor behavior. Save-state validation preserves the cartridge's region and
peripheral, and validates new mapper state. Older NTSC snapshots remain valid
except old MMC3-mode StarTropics snapshots, which lack the correct RAM layout.

Hardware references: [CNROM](https://www.nesdev.org/wiki/CNROM),
[AxROM](https://www.nesdev.org/wiki/AxROM),
[MMC1/SUROM](https://www.nesdev.org/wiki/MMC1),
[MMC5](https://www.nesdev.org/wiki/MMC5),
[MMC6](https://www.nesdev.org/wiki/MMC6),
[FME-7](https://www.nesdev.org/wiki/Sunsoft_FME-7),
[PPU rendering](https://www.nesdev.org/wiki/PPU_rendering),
[APU noise](https://www.nesdev.org/wiki/APU_Noise),
[APU DMC](https://www.nesdev.org/wiki/APU_DMC).

## RAMBO-1 and Camerica boards

Klax uses **RAMBO-1 (mapper 64)**: three switchable 8 KiB PRG banks,
independent CHR registers 8/9, CHR inversion, mirroring, filtered PPU A12 IRQs,
and a CPU-cycle IRQ prescaler with delayed assertion. IRQ timing follows the
compatibility model described by [NESdev](https://www.nesdev.org/wiki/RAMBO-1)
and cross-checked with [Mesen2](https://github.com/SourMesen/Mesen2/blob/master/Core/NES/Mappers/Tengen/Rambo1.h).
Tests cover the extra banks, both PRG modes, 1/2 KiB CHR modes, interrupts,
acknowledgement, the high-reload wraparound case and deterministic snapshots.
Actual-game coverage currently targets Klax; this is not a certification of all
RAMBO-1 titles or cycle-exact analog hardware behavior.

**Camerica (71)** implements conflict-free 16 KiB switching and optional Fire Hawk
single-screen control, with no PRG RAM. See [board registers](https://www.nesdev.org/wiki/INES_Mapper_071).
A narrowly pinned Micro Machines PRG identity identifies an old mapper-2 header
as this board. Separately, a pinned US Demon Sword PRG identity suppresses its
erroneous four-screen flag; MMC1 controls its nametable mirroring. These are
in-memory header interpretations, not modifications to users' ROMs. Header recovery alone does not validate a cartridge's CHR contents; visual
acceptance requires a verified dump. Original full-file hashes still identify
saves. Old Micro Machines snapshots made under mapper 2 need a fresh reset;
other existing mapper-state discriminants are preserved.
