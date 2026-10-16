"""Assemble an unsigned native test candidate, including licenses; never publish."""

from pathlib import Path
import shutil
import sys

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "dist/native"
suffix = ".exe" if sys.platform == "win32" else ""
binary = ROOT / "target/release" / ("agentemu" + suffix)
if not binary.is_file():
    raise SystemExit("Build first: cargo build --release --locked -p frontend-native")
if OUT.exists():
    shutil.rmtree(OUT)
OUT.mkdir(parents=True)
shutil.copy2(binary, OUT / ("AgentEmu" + suffix))
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
(OUT / "README.txt").write_text(
    "AgentEmu unsigned native test build.\nRun AgentEmu (AgentEmu.exe on Windows) and choose your own ROM.\nNot signed/notarized as a public installer. See the included third-party notices before redistribution.\nLinux requires ALSA, udev, XKB and Wayland runtime libraries.\n"
)
print(f"Unsigned native test build assembled: {OUT}")
