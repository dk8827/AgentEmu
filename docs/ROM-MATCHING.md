# Matching a cartridge to an agent

Human play uses the emulator's supported cartridge hardware. Neural agents also
require an exact ROM revision because their observations read game-specific
memory layouts. A game that works with human controls can therefore be rejected
by an agent.

The expected SHA-256 is `rom_sha256` in the model's `manifest.json`. It covers the
entire file, including the iNES header. Region, revisions, translations, patches
and header changes can all change that identity. Do not edit a manifest or ROM
header to bypass the check.

Compute your local file's hash without uploading it:

```sh
python3 scripts/rom-info.py /path/to/game.nes
```

Gallery mismatch details show expected and selected hashes. To try the file with
human controls, return to the player and use **Open ROM** without selecting an
agent. A mismatch does not replace the current game.

The bundled Snow Bros. model targets a particular US variant whose program hash
differs from the reference cartridge; its graphics hash matches. It does not claim
compatibility with every US dump. The exact identity is in its model card and
manifest. AgentEmu does not supply ROMs or download sources.
