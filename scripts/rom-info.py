"""Print local cartridge identity without uploading or modifying the file."""

import argparse
import hashlib
import json
from pathlib import Path

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("rom", type=Path)
args = parser.parse_args()
data = args.rom.read_bytes()
if len(data) < 16 or data[:4] != b"NES\x1a":
    parser.error("Expected an uncompressed iNES/NES 2.0 cartridge file")
root = Path(__file__).resolve().parents[1]
sha = hashlib.sha256(data).hexdigest()
game = next(
    (g for g in json.loads((root / "catalog/games.json").read_text()) if g["sha256"] == sha), None
)
models = []
for entry in json.loads((root / "catalog/models.json").read_text()):
    manifest = json.loads((root / "models" / entry["id"] / "manifest.json").read_text())
    if manifest["rom_sha256"] == sha:
        models.append({"id": entry["id"], "bundled": entry["bundled"], "status": entry["status"]})
print(json.dumps({"sha256": sha, "recognized_game": game, "models": models}, indent=2))
