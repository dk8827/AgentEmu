# AgentEmu player

The desktop app and browser share the same controls, menus and settings.

## Open and play

- Browser: `sh scripts/build-web.sh`, then `python3 scripts/serve-web.py`; open `/play.html`.
- Desktop: `cargo run --release --locked -p frontend-native`. No ROM argument is required: use **Choose game ROM** or **Explore AI games**.
- macOS: `sh scripts/build-macos.sh` produces `target/dist/AgentEmu.app`, an ad-hoc signed development bundle (not Developer ID signed or notarized).

Open or drop a local `.nes` file, then choose **Play**. Use the separate **Start / Select** buttons to operate the cartridge's menu and select its player count. **Pause / Resume** runs the emulator without sending NES buttons. ROMs stay on your device.

Opening any settings panel pauses. Closing it stays paused. Escape opens/closes the menu; F11 toggles fullscreen; Command/Ctrl+O opens a game. Focus loss pauses and clears input/audio; rotating a touch layout also pauses. Native window close exits the application.

## Players and controls

Click a player's heading or **Player setup** to choose **Human**, **AI**, **Shared**, or **No input**, then keyboard, touch or an explicitly assigned gamepad. Changing controllers does not select a two-player game mode inside the cartridge. Use the visible Start/Select controls to choose the cartridge’s one- or two-player mode.

| Action | Player 1 | Player 2 |
| --- | --- | --- |
| Move / aim | W A S D | Arrow keys |
| A / B | J / L | Comma / Period |
| Select / Start | Space / Enter | 0 / Backspace |

Keys can be remapped; duplicate bindings are rejected. Escape and F5/F8/F11 are
reserved for application controls. If your keyboard misses a particular combination,
try remapping one of its keys or use a gamepad.

Gamepads use the left stick or D-pad, south/west buttons for A/B, and Select/Start. Disconnects pause instead of silently reassigning a port. Held inputs are suppressed through ownership changes until released.

Phones start with P1 touch. Portrait puts the game above the controller; landscape puts controls beside it. The D-pad supports diagonals, independent fingers can hold A and B, and A+B holds both without turbo. **Touch controls** changes size, mirroring and the assigned port. **P2 menu** switches only the Start/Select strip. One touch deck controls one player; a second human can use a gamepad. Physical iOS/Android comfort and gamepad testing remain separate acceptance work.

Exact cartridge identities determine solo, alternating and light-gun layouts. Solo games show one player by default. **Settings → Advanced → Enable controller 2 for special game commands** exposes a second port for commands such as Zelda's save menu. Duck Hunt hides the unused A/B deck: click or tap the game to aim and fire. Start/Select remain visible.

## Agents and shared control

**AI games** opens the game gallery. Click anywhere on a **Set up game** card, then
choose Human, AI or Shared separately for each player. Select **No player 2** to
play alone, including Shared control of Player 1. **Choose game ROM** loads your
matching cartridge and applies your chosen controls. Agent scope and model
selection are under **Agent details**. Open `/play.html?view=ai-games` or run
`agentemu --ai-games`. Disabling P2 blocks its controller; it does not remove an
existing character from a running two-player game. Choose one-player mode in the
game itself.

Bundled agents work offline and require a matching cartridge revision. Choose an included agent in player setup. The browser does not upload or download models; native training and CLI tools support locally trained bundles.

**Shared** defaults to human movement/aim and A, with AI controlling B. Presets assign A, B, or A+B to the AI; custom ownership keeps direction pairs together. Start/Select always remain human. A compact ownership summary stays beside the game. Expand **Inputs** to see human input, AI proposals (crossed out for human-owned groups), and applied buttons. AI-owned touch buttons carry a robot icon. Touch size and left-handed layout apply in both orientations. **Take full control** returns gameplay to the human. Unsupported stages, bad inference, and fatal emulation errors stop play; resuming cannot bypass the fault.

**Help & about → Training tools** reads training report JSON and links to native Python/Rust training instructions. Version one uses included agents only: no model upload, download, community catalog, or account setup.

## Sound, saves and display

Sound starts on **Play / Resume**. **Display & sound** offers mute, volume, original 4:3 proportions, square pixels, whole-pixel scaling, fullscreen and advanced frame stepping. Native output resamples to the device rate; browser audio uses Web Audio. Pauses flush the queue. If initialization fails, the settings show the error; another Resume retries initialization while silent play remains possible.

**Saved games** provides three local slots per exact cartridge, with screenshots and timestamps. F5/F8 save/load Slot 1. Slots survive closing or reloading; reopen the same ROM to access them. Overwriting a slot and loading a state require confirmation (F5 remains the explicit quick-save shortcut). **Export save file** and **Import save file** provide portable backups. Replays sit in a separate disclosure and verify input/final-state hashes. `--state FILE` loads an existing state at startup.

Battery RAM persists by exact ROM hash about every ten seconds, when pausing/replacing a game, and on native exit. Browser storage keeps the previous player's battery keys. Native files remain in the application's `battery` data directory. Private browsing or clearing storage may remove browser saves; download a state for backup. Corrupt or unavailable battery storage is reported, not silently discarded.

The advanced MMC3A counter option is available for mapper 4 cartridges and asks before restarting. `--mmc3-alt` also remains available. Core support and compatibility limits are unchanged; see [hardware limits](EXPANDED-HARDWARE.md) and the [recorded cartridge checks](COMPATIBILITY.md). PAL and NTSC use their own frame clocks.

## Development checks

```sh
python3 scripts/check.py core
python3 scripts/check.py browser
```

The canvas browser test uses original homebrew and real input/file-picker interactions. It covers keyboard focus, pause/release, sound queue, invalid replacement files, multi-touch diagonal firing, phone rotation, 320px width and WebGL2. Screenshots are written to ignored `web/test-results/rust-ui/`. Set `PLAYWRIGHT_CHROME=1` to use installed Chrome.

For native captures: `agentemu /path/to/game.nes --screenshot /absolute/path.png`.
`--frames N` runs N additional emulated frames and exits, including when unfocused.
See [contributor setup](../CONTRIBUTING.md) for dependencies and optional checks.

## Navigation and accessibility

The welcome screen offers a local ROM or the included AI game gallery. Menus pause play. **Back** returns to the previous settings panel; **Done** closes settings and keeps play paused. Errors offer recovery appropriate to the failed action. Disabled Player 2 is collapsed, with **+ Player 2** available for multiplayer cartridges.

The browser mirrors the Rust AccessKit tree into semantic controls and sends actions back to the same widgets. Tab/Enter/Space, labelled inputs, modal focus, and **Settings → Interface size** are supported. Native uses eframe’s platform accessibility. Automated checks do not replace testing with physical assistive technology and phones.
