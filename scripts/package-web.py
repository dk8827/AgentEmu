"""Create an offline-capable static player directory; never publish or deploy it."""

import json
from pathlib import Path
import shutil

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "dist/web"
FILES = [
    "index.html",
    "play.html",
    "favicon.svg",
    "accessibility.css",
    "accessibility.mjs",
    "rust-player.mjs",
    "session-worker.mjs",
    "runtime-pcm-worklet.mjs",
    "pkg/player_ui.js",
    "pkg/player_ui_bg.wasm",
    "pkg/emulator.js",
    "pkg/emulator_bg.wasm",
]
for name in FILES:
    if not (ROOT / "web" / name).is_file():
        raise SystemExit(f"Missing web/{name}; run sh scripts/build-web.sh first")
if OUT.exists():
    shutil.rmtree(OUT)
OUT.mkdir(parents=True)
for name in FILES:
    dest = OUT / name
    dest.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(ROOT / "web" / name, dest)
# wasm-bindgen can generate imported snippets; retain them if emitted.
if (ROOT / "web/pkg/snippets").exists():
    shutil.copytree(ROOT / "web/pkg/snippets", OUT / "pkg/snippets")
for name in [
    "LICENSE",
    "THIRD_PARTY_NOTICES.md",
    "third-party/rust-notices.txt",
    "crates/player-ui/assets/OFL.txt",
    "crates/nes-video/palettes/LICENSE-MIT-0",
    "crates/player-ui/assets/gallery/NOTICE.md",
]:
    dest = OUT / "licenses" / name
    dest.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(ROOT / name, dest)
for model in (ROOT / "models").iterdir():
    dest = OUT / "licenses/models" / model.name
    dest.mkdir(parents=True, exist_ok=True)
    shutil.copy2(model / "LICENSE", dest / "LICENSE")
(OUT / "RELEASE-STATUS.txt").write_text(
    "AgentEmu browser test build. Review docs/RELEASE.md before hosting; physical-device validation is pending.\nServe over HTTPS or localhost. No ROMs included.\n"
)
print(
    f"Packaged {len(FILES)} player assets in {OUT}; developer pages and model services excluded. No deployment performed."
)
