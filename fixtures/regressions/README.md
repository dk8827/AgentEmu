# Cartridge regressions

`dd2-reviving-boss.json` contains controller inputs from a real stalled Mission 1
run, encoded as `[frame_count, player_1_mask, player_2_mask]` groups. It starts at
power-on and reaches the boss's temporary zero-health pose. Earlier adapters hid
that still-present boss from both agents; they walked away and never finished.

The regression verifies the complete input and emulator-state digests, checks
that the boss remains observable, then finishes the fight with ordinary controller
inputs. It never edits game RAM or restores a supplied save state.

Run it with the activated training environment and your matching cartridge:

```sh
DD2_ROM=/path/to/DoubleDragonII.nes python -m unittest discover \
  -s training/tests -p test_dd2_revival.py
```

This fixture contains no ROM, save state, screenshot or executable model.
