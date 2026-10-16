"""Local cross-runtime reference: semantic fields, actions and hashes; no full saves."""

import json, hashlib, sys
from pathlib import Path
from contra_training._native import Contra, Actor

rom = Path(sys.argv[1]).read_bytes()
model = Path(sys.argv[2])
actor = Actor((model / "manifest.json").read_text(), (model / "weights.safetensors").read_bytes())
e = Contra(rom, 7200)
results = []
for slots in [[False, False], [True, False], [False, True], [True, True]]:
    r = json.loads(e.reset(0))
    start = json.loads(bytes(e.snapshot()))["ppu"]["completed_frames"]
    checks = []
    for i in range(120):
        human = [130 if i % 2 == 0 else 128, 66 if i % 2 else 64]
        before = r["observations"]
        masks = [actor.buttons(before[p]) if slots[p] else human[p] for p in range(2)]
        r = json.loads(e.step(masks))
        if i % 20 == 19:
            checks.append(
                dict(
                    step=i + 1,
                    human=human,
                    observations=r["observations"],
                    state_sha256=hashlib.sha256(bytes(e.snapshot())).hexdigest(),
                )
            )
    results.append(dict(slots=slots, start_frame=start, checks=checks))
p = Path(".cache/contra-parity.json")
p.write_text(json.dumps(results) + "\n")
print("Recorded four controller combinations")
