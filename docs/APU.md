# Audio and interrupt implementation

PAL cartridges use their own frame-sequencer, noise and DMC periods and master clock while retaining 48 kHz output. Both frontends pace from the cartridge region. See [expanded hardware](EXPANDED-HARDWARE.md) for limits, including unimplemented expansion audio.

The original APU implements both pulse channels, triangle, noise and DMC. Pulse envelopes, duty patterns, sweep including the pulse-1 negate difference, triangle linear/length gating, noise feedback modes, DMC buffer/output/loop/IRQ and the four/five-step frame sequencer run with CPU time. Register reads distinguish the frame IRQ acknowledgement from the persistent DMC IRQ.

The CPU bus handles DMC requests at read cycles, with halt, dummy, alignment and sample cycles. OAM DMA, DMC arbitration and controller read conflicts still need the full dedicated DMA suite. CPU interrupt polls preserve CLI/SEI/PLP latency, special branch polling and IRQ/BRK vector hijacking by NMI; all five CPU interrupt ROMs in the current inventory pass.

Audio uses an integer nonlinear mixer approximation and integrates CPU-cycle outputs into 48 kHz samples, with a simple high-pass/low-pass presentation filter. This is not a band-limited analog reconstruction. Frontends can drain a bounded two-second output queue. Hardware and filter state are saved; the host output queue is excluded from snapshots and clears on restore. Headless execution still clocks all audio hardware.

Seven original APU tests cover lengths, envelopes, linear counters, sweep arithmetic, DMC wrapping/looping/IRQ, DMA clock agreement, sample rate/replay and malformed snapshots. All eight `apu_test` ROMs in the pinned public test collection pass, including frame-counter jitter and IRQ/length timing. Public fixtures remain in the ignored cache, with source provenance in `scripts/test-conformance.py`.

Hardware references (implementation written for this project):

- [NESdev APU](https://www.nesdev.org/wiki/APU)
- [APU sweep](https://www.nesdev.org/wiki/APU_Sweep)
- [APU triangle](https://www.nesdev.org/wiki/APU_Triangle)
- [DMA](https://www.nesdev.org/wiki/DMA)
- [CPU interrupts](https://www.nesdev.org/wiki/CPU_interrupts)
