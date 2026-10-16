# NES display palette

`2c02.pal` is a 512-entry RGB lookup (64 colors × 8 emphasis combinations), generated using [Pally](https://github.com/Gumball2415/pally) at commit `9635e9b71505f969014c7efa15dca3ba35ade871` by Persune, under MIT-0. The license is retained alongside it.

Reproduce in a local Python environment with Pally's `requirements.txt`:

```sh
MPLBACKEND=Agg python pally.py --skip-plot -e -o 2c02.pal
```

These are Pally's default 2C02 composite signal / SMPTE C to sRGB settings, including its modeled emphasis colors. The table replaces the original illustrative HSV palette and per-channel dimming. It is shared by native and WASM presentation. This is a static NTSC color approximation, not a simulation of CRT scanlines, composite artifacts, or every television's decoder. Core framebuffer codes and save-state hashes are unchanged.
