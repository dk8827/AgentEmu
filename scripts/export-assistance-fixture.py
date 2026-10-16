"""Generate a synthetic assistance actor for shape and runtime parity tests.

Random weights exercise the 268-feature contract; they are not a trained agent.
"""

import hashlib
import json
from pathlib import Path
import torch
from safetensors.torch import save_file
from contra_training.model import ActorNet
from contra_training.contracts import ASSIST_CONTRACTS
from contra_training.env import buttons

root = Path(__file__).resolve().parents[1] / "fixtures/assistance-policy"
root.mkdir(exist_ok=True)
torch.manual_seed(913)
torch.set_num_threads(1)
actor = ActorNet(268)
weights = root / "weights.safetensors"
save_file(actor.state_dict(), weights)
architecture, observation, rom = ASSIST_CONTRACTS["contra-jungle"]
manifest = dict(
    version=1,
    name="Synthetic assistance parity fixture",
    architecture=architecture,
    observation=observation,
    action="nes-factorized-3-3-2-2-v1",
    rom_sha256=rom,
    weights_sha256=hashlib.sha256(weights.read_bytes()).hexdigest(),
    decision_frames=1,
    training={"synthetic": True},
)
(root / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
observations = torch.rand(32, 268) * 2 - 1
observations[0] = 0
observations[1] = 1
observations[2] = -1
with torch.no_grad():
    logits = actor(observations)
rows = [
    dict(observation=o.tolist(), logits=l.tolist(), buttons=buttons(actor.action(o.numpy())))
    for o, l in zip(observations, logits)
]
(root / "parity.json").write_text(json.dumps(rows, indent=2) + "\n")
