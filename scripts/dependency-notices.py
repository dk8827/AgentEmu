"""Inventory Cargo dependencies and retain their shipped license/notice texts."""

import argparse
import json
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--metadata", type=Path, help="Use an existing cargo metadata JSON file")
args = parser.parse_args()
data = json.loads(
    args.metadata.read_text()
    if args.metadata
    else subprocess.check_output(
        ["cargo", "metadata", "--locked", "--format-version", "1", "--all-features"],
        cwd=ROOT,
        text=True,
    )
)
packages = sorted(
    (p for p in data["packages"] if p["source"]), key=lambda p: (p["name"], p["version"])
)
rows, sections = [], []
upstream_dir = ROOT / "third-party/upstream"
provenance = upstream_dir / "provenance.json"
upstream = (
    {(p["name"], p["version"]): p["notices"] for p in json.loads(provenance.read_text())}
    if provenance.exists()
    else {}
)
missing = []
for p in packages:
    base = Path(p["manifest_path"]).parent
    texts = []
    for f in sorted(base.rglob("*")):
        if not f.is_file():
            continue
        # Include nested font/vendor notices, not only a crate's top-level license.
        if (
            f.name.lower().startswith(
                ("license", "licence", "copying", "notice", "ofl", "copyright", "authors")
            )
            and f.stat().st_size < 300000
        ):
            try:
                content = f.read_text(encoding="utf-8")
            except UnicodeDecodeError:
                continue
            texts.append((str(f.relative_to(base)), content))
    for notice in upstream.get((p["name"], p["version"]), []):
        content = (upstream_dir / notice["file"]).read_text()
        import hashlib

        if hashlib.sha256(content.encode()).hexdigest() != notice["sha256"]:
            raise SystemExit(f"Upstream notice digest mismatch: {notice['file']}")
        texts.append((notice["url"], content))
    if not p["license"] and not p["license_file"]:
        raise SystemExit(f"Missing license declaration: {p['name']}")
    if p["name"] in {"block", "dispatch"}:
        texts.append(
            ("third-party/legacy-mit.txt", (ROOT / "third-party/legacy-mit.txt").read_text())
        )
    url = f"https://crates.io/crates/{p['name']}/{p['version']}"
    rows.append(
        {
            "name": p["name"],
            "version": p["version"],
            "license": p["license"],
            "repository": p["repository"],
            "source": url,
            "notice_files": [n for n, _ in texts],
        }
    )
    sections.append(
        f"\n{'=' * 78}\n{p['name']} {p['version']}\nLicense: {p['license']}\nSource: {url}\n"
    )
    if not texts:
        missing.append(p["name"])
        sections.append(
            "No separate license text shipped in this crate; consult the source URL and declared terms.\n"
        )
    for name, text in texts:
        sections.append(f"\n--- {name} ---\n{text}\n")
out = ROOT / "third-party"
out.mkdir(exist_ok=True)
(out / "rust-dependencies.json").write_text(json.dumps(rows, indent=2) + "\n")
(out / "rust-notices.txt").write_text(
    "\n".join(
        line.rstrip()
        for line in (
            "Third-party Rust dependency notices — generated from locked published sources and pinned upstream notices.\n"
            + "".join(sections)
        ).splitlines()
    ).rstrip()
    + "\n"
)
print(
    f"Inventoried {len(rows)} external packages; {len(missing)} have no separate shipped notice: {', '.join(missing)}"
)
