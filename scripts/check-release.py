"""Check tracked release contents without requiring commercial ROMs or dependencies."""

import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys
from urllib.parse import unquote
from model_evidence import validate_bundle

ROOT = Path(__file__).resolve().parents[1]
paths = subprocess.check_output(["git", "ls-files", "-z"], cwd=ROOT).decode().split("\0")
errors = []
allowlist = set(json.loads((ROOT / "release-files.json").read_text()))
tracked = set(filter(None, paths))
if tracked != allowlist:
    errors.append(
        f"Release allowlist differs: unreviewed {sorted(tracked - allowlist)}, missing {sorted(allowlist - tracked)}"
    )
for name in filter(None, paths):
    p = ROOT / name
    if not p.is_file():
        errors.append(f"Missing tracked file: {name}")
        continue
    if p.is_symlink():
        errors.append(f"Tracked symlink: {name}")
    if p.suffix.lower() in {
        ".nes",
        ".sav",
        ".state",
        ".zip",
        ".7z",
        ".mp4",
        ".mov",
        ".webm",
        ".npz",
        ".pt",
        ".pth",
        ".ckpt",
        ".whl",
        ".log",
        ".sqlite",
        ".db",
    } or any(
        part
        in {
            ".cache",
            ".tools",
            "node_modules",
            "target",
            ".venv",
            "design",
            "research",
            "experiments",
        }
        for part in p.relative_to(ROOT).parts
    ):
        errors.append(f"Non-release material: {name}")
    data = p.read_bytes()
    if data.startswith(bytes([78, 69, 83, 26])):
        errors.append(f"iNES payload: {name}")
    try:
        text = data.decode("utf-8")
    except UnicodeDecodeError:
        continue
    if re.search(r"/(?:Users|home)/[\w.-]+/", text):
        errors.append(f"Personal absolute path: {name}")
    if "github.com/dk8827/" + "emulator" in text:
        errors.append(f"Old repository URL: {name}")
    if p.suffix == ".md":
        # Inline relative Markdown file links; fragment anchors are intentionally not checked.
        for link in re.findall(r'\]\(([^\s)]+)(?:\s+"[^"]*")?\)', text):
            if ":" in link or link.startswith(("#", "/")):
                continue
            dest = unquote(link.split("#")[0])
            if dest and not (p.parent / dest).exists():
                errors.append(f"Broken documentation link in {name}: {link}")
for name in (
    "LICENSE",
    "SECURITY.md",
    "CONTRIBUTING.md",
    "THIRD_PARTY_NOTICES.md",
    "docs/RELEASE.md",
    "third-party/rust-dependencies.json",
    "third-party/rust-notices.txt",
):
    if not (ROOT / name).is_file():
        errors.append(f"Missing release document: {name}")
for model in sorted((ROOT / "models").iterdir()):
    if not model.is_dir():
        continue
    if (model / "LICENSE").read_bytes() != (ROOT / "LICENSE").read_bytes():
        errors.append(f"Model license differs: {model.name}")
    manifest = json.loads((model / "manifest.json").read_text())
    if (
        hashlib.sha256((model / "weights.safetensors").read_bytes()).hexdigest()
        != manifest["weights_sha256"]
    ):
        errors.append(f"Model digest mismatch: {model.name}")
    if (model / "evaluation.json").exists():
        try:
            validate_bundle(model)
        except (ValueError, KeyError, OSError) as error:
            errors.append(f"Invalid release evidence for {model.name}: {error}")
    else:
        errors.append(f"Missing evaluation record: {model.name}")
# Ensure the inventory follows Cargo.lock, including platform-specific crates.
import tomllib

locked = tomllib.loads((ROOT / "Cargo.lock").read_text())
expected = {(p["name"], p["version"]) for p in locked["package"] if p.get("source")}
inventory = json.loads((ROOT / "third-party/rust-dependencies.json").read_text())
actual = {(p["name"], p["version"]) for p in inventory}
if expected != actual:
    errors.append(
        f"Dependency inventory stale: missing {expected - actual}, extra {actual - expected}"
    )
if errors:
    print("\n".join(errors), file=sys.stderr)
    sys.exit(1)
print(
    f"Release tree passed: {len(list(filter(None, paths)))} tracked files; model digests, licenses, links and {len(actual)} dependency entries verified."
)
print("This checks contents, not asset clearance or permission to publish. See docs/RELEASE.md.")
