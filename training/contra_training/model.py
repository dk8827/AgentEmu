import hashlib
import json
import copy
from pathlib import Path
import numpy as np
import torch
from safetensors.torch import save_file
from ._native import Actor

from .contracts import CONTRACTS, ASSIST_CONTRACTS, POLICY_VARIANTS


class ActorNet(torch.nn.Module):
    def __init__(self, inputs=256):
        super().__init__()
        self.layers = torch.nn.ModuleList(
            [torch.nn.Linear(inputs, 64), torch.nn.Linear(64, 64), torch.nn.Linear(64, 10)]
        )

    def forward(self, x):
        for layer in self.layers[:-1]:
            x = torch.relu(layer(x))
        return self.layers[-1](x)

    @torch.no_grad()
    def action(self, observation):
        x = self(torch.as_tensor(np.asarray(observation), dtype=torch.float32))
        return np.array([int(t.argmax()) for t in x.split((3, 3, 2, 2))], dtype=np.int64)


def calibrate_action_logits(actor, scale):
    """Lower initial PPO confidence without changing the network's feature layers."""
    if not np.isfinite(scale) or not 0.001 <= scale <= 1:
        raise ValueError("initial logit scale must be finite and in [0.001, 1]")
    result = copy.deepcopy(actor)
    with torch.no_grad():
        result.layers[-1].weight.mul_(scale)
        result.layers[-1].bias.mul_(scale)
    return result


def freeze_except_jump(ppo):
    """Constrain PPO actor updates to A logits; keep the value network trainable."""
    for parameter in ppo.policy.mlp_extractor.policy_net.parameters():
        parameter.requires_grad_(False)
    for parameter in (ppo.policy.action_net.weight, ppo.policy.action_net.bias):
        mask = torch.zeros_like(parameter)
        mask[6:8] = 1
        parameter.register_hook(lambda gradient, mask=mask: gradient * mask)


def from_ppo(ppo):
    actor = ActorNet()
    layers = [
        ppo.policy.mlp_extractor.policy_net[0],
        ppo.policy.mlp_extractor.policy_net[2],
        ppo.policy.action_net,
    ]
    for dst, src in zip(actor.layers, layers):
        dst.load_state_dict(src.state_dict())
    return actor


def validate_logits(actual, expected):
    """Compare float32 implementations per action head, including exact decisions.

    Dot-product cancellation can make a near-zero logit have a large relative
    error even when its error is tiny compared with the head's other logits.
    Use a normwise tolerance independently for each head, never across heads.
    Argmax equality remains mandatory, including for near ties.
    """
    actual, expected = np.asarray(actual), np.asarray(expected)
    if actual.shape != expected.shape or actual.ndim != 2 or actual.shape[1] != 10:
        raise ValueError("expected matching batches of ten logits")
    if not len(actual) or not np.isfinite(actual).all() or not np.isfinite(expected).all():
        raise ValueError("parity logits must be nonempty and finite")
    scaled_error = 0.0
    for a, e in zip(np.split(actual, [3, 6, 8], axis=1), np.split(expected, [3, 6, 8], axis=1)):
        scale = np.maximum(1.0, np.max(np.abs(e), axis=1, keepdims=True))
        np.testing.assert_allclose(a / scale, e / scale, atol=2e-5, rtol=2e-5)
        np.testing.assert_array_equal(a.argmax(1), e.argmax(1))
        scaled_error = max(scaled_error, float(np.max(np.abs(a - e) / scale)))
    return scaled_error


def export(
    actor, output, metadata, observations, *, task="contra-jungle", assistance=False, schema=None
):
    architecture, canonical_schema, rom_hash = (ASSIST_CONTRACTS if assistance else CONTRACTS)[task]
    if schema is None or schema == canonical_schema:
        schema = canonical_schema
    else:
        variant = POLICY_VARIANTS.get(schema)
        if variant is None:
            raise ValueError(f"unknown policy schema for {task}: {schema}")
        if variant["task"] != task or variant["assistance"] != assistance:
            raise ValueError("policy variant does not match task or assistance mode")
        architecture, rom_hash = variant["architecture"], variant["rom_sha256"]
    output = Path(output)
    output.mkdir(parents=True, exist_ok=False)
    path = output / "weights.safetensors"
    save_file({k: v.detach().cpu().contiguous() for k, v in actor.state_dict().items()}, path)
    weights = path.read_bytes()
    manifest = dict(
        version=1,
        name=metadata.get("name", "Contra Jungle experimental"),
        architecture=architecture,
        observation=schema,
        action="nes-factorized-3-3-2-2-v1",
        rom_sha256=rom_hash,
        weights_sha256=hashlib.sha256(weights).hexdigest(),
        decision_frames=1,
        training=metadata,
    )
    text = json.dumps(manifest, indent=2) + "\n"
    (output / "manifest.json").write_text(text)
    runtime = Actor(text, weights)
    obs = np.asarray(observations[:256], dtype=np.float32)
    with torch.no_grad():
        expected = actor(torch.from_numpy(obs)).numpy()
    actual = np.asarray([runtime.logits(x.tolist()) for x in obs])
    error = float(np.max(np.abs(actual - expected)))
    scaled_error = validate_logits(actual, expected)
    from .env import buttons

    for x in obs:
        assert runtime.buttons(x.tolist()) == buttons(actor.action(x))
    (output / "parity.json").write_text(
        json.dumps(
            dict(
                cases=len(obs),
                max_absolute_error=error,
                max_head_scaled_error=scaled_error,
                controller_decisions="exact",
            ),
            indent=2,
        )
        + "\n"
    )
    return error
