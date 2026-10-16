"""Export fixed PyTorch actors and varied parity fixtures. No game training yet."""

import json
from pathlib import Path
import torch
from safetensors.torch import save_file

ROOT = Path(__file__).resolve().parents[1] / "fixtures" / "policies"
ROOT.mkdir(parents=True, exist_ok=True)
torch.set_num_threads(1)
players = []
for player in range(2):
    torch.manual_seed(2026 + player)
    layers = torch.nn.ModuleList(
        [torch.nn.Linear(32, 64), torch.nn.Linear(64, 64), torch.nn.Linear(64, 10)]
    )
    weights = {
        f"layers.{i}.{key}": tensor
        for i, layer in enumerate(layers)
        for key, tensor in layer.state_dict().items()
    }
    filename = f"player-{player + 1}.safetensors"
    save_file(weights, ROOT / filename, metadata={"architecture": "prototype-mlp-32-64-64-10-v1"})
    observations = torch.cat(
        [torch.zeros(1, 32), torch.ones(1, 32), -torch.ones(1, 32), torch.randn(61, 32)]
    )
    with torch.no_grad():
        logits = layers[2](torch.relu(layers[1](torch.relu(layers[0](observations)))))
    players.append(
        {"weights": filename, "observations": observations.tolist(), "logits": logits.tolist()}
    )
    if player == 0:
        bad = {k: v.clone() for k, v in weights.items()}
        bad["layers.0.weight"][0, 0] = float("nan")
        save_file(bad, ROOT / "invalid-nan.safetensors")
        bad["layers.0.weight"] = torch.zeros(63, 32)
        save_file(bad, ROOT / "invalid-shape.safetensors")
        bad = {**weights, "unexpected": torch.zeros(1)}
        save_file(bad, ROOT / "invalid-extra.safetensors")
(ROOT / "parity.json").write_text(
    json.dumps({"torch": torch.__version__, "seed": 2026, "players": players}, indent=2) + "\n"
)
print(f"Exported two actors, 128 observations, and three invalid fixtures to {ROOT}")
