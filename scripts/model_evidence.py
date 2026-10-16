"""Validate compact model outcome records; no training dependencies required.

This checks identity and internal consistency, not independent replay or quality.
A model can legitimately have zero clears. Never turn failed cases into passes.
"""

import hashlib
import json
from pathlib import Path
import re


def require(condition, message):
    if not condition:
        raise ValueError(message)


def matches_policy_contract(task, manifest):
    contracts = {
        (task["observation"], task["architecture"]),
        (task["assistance_observation"], task["assistance_architecture"]),
        *[(v["observation"], v["architecture"]) for v in task.get("policy_variants", [])],
    }
    return (
        task["rom_sha256"] == manifest["rom_sha256"]
        and (manifest["observation"], manifest["architecture"]) in contracts
    )


def validate_evidence(evidence, manifest):
    require(evidence["version"] == 1, "unknown evidence version")
    for key in ("rom_sha256", "weights_sha256"):
        require(evidence[key] == manifest[key], f"{key} differs")
        require(re.fullmatch(r"[0-9a-f]{64}", evidence[key]), "invalid digest")
    reports = evidence["reports"]
    require(
        reports and len({r["id"] for r in reports}) == len(reports), "missing/duplicate reports"
    )
    for report in reports:
        require(report["scope"] in ("prepared-gameplay", "title-menu"), "invalid start scope")
        rows = report["cases"]
        require(rows, "empty cases")
        identities = []
        summary = {}
        for row in rows:
            require(type(row["success"]) is bool, "success must be boolean")
            require(type(row["frames"]) is int and row["frames"] > 0, "invalid frame count")
            profile = row.get("profile", "pair")
            identities.append(
                (profile, row.get("slot"), row.get("warmup"), row.get("delay"), row.get("seed"))
            )
            for key in ("input_sha256", "final_state_sha256"):
                if key in row:
                    require(re.fullmatch(r"[0-9a-f]{64}", row[key]), "invalid case digest")
            if "deaths" in row:
                require(
                    len(row["deaths"]) == 2
                    and all(type(v) is int and v >= 0 for v in row["deaths"]),
                    "invalid deaths",
                )
            if "survivors" in row:
                require(
                    len(row["survivors"]) == 2 and all(type(v) is bool for v in row["survivors"]),
                    "invalid survivors",
                )
            score = summary.setdefault(profile, {"clears": 0, "cases": 0})
            score["cases"] += 1
            score["clears"] += row["success"]
        require(len(set(identities)) == len(identities), "duplicate cases")
        require(summary == report["summary"], "summary differs from complete case outcomes")


def validate_bundle(folder):
    folder = Path(folder)
    manifest = json.loads((folder / "manifest.json").read_text())
    require(
        hashlib.sha256((folder / "weights.safetensors").read_bytes()).hexdigest()
        == manifest["weights_sha256"],
        "model weights changed",
    )
    validate_evidence(json.loads((folder / "evaluation.json").read_text()), manifest)
