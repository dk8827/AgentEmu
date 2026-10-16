"""Original synthetic fixtures; no game data or trained capability implied."""

import json, hashlib
from pathlib import Path
import torch
from safetensors.torch import save_file
from contra_training.model import ActorNet
from contra_training.contracts import ROM
from contra_training.env import buttons

root = Path(__file__).resolve().parents[1] / "fixtures/contra-policy"
torch.manual_seed(912)
torch.set_num_threads(1)
actor = ActorNet()
weights = root / "weights.safetensors"
save_file(actor.state_dict(), weights)
manifest = dict(
    version=1,
    name="Synthetic parity fixture",
    architecture="contra-mlp-256-64-64-10-v1",
    observation="contra-jungle-256-v1",
    action="nes-factorized-3-3-2-2-v1",
    rom_sha256=ROM,
    weights_sha256=hashlib.sha256(weights.read_bytes()).hexdigest(),
    decision_frames=1,
    training={"synthetic": True},
)
(root / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
obs = torch.rand((32, 256)) * 2 - 1
obs[0] = 0
obs[1] = 1
obs[2] = -1
with torch.no_grad():
    outputs = actor(obs).tolist()
(root / "parity.json").write_text(
    json.dumps(
        [
            dict(observation=o.tolist(), logits=y, buttons=buttons(actor.action(o.numpy())))
            for o, y in zip(obs, outputs)
        ],
        indent=2,
    )
    + "\n"
)
