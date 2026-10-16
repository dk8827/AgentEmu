"""Shared imitation-fitting primitive for task and demonstration trainers."""

import numpy as np
import torch


def fit(actor, xs, ys, epochs, optimizer, direction_weights=None, *, label_smoothing=0.0):
    if not np.isfinite(label_smoothing) or not 0 <= label_smoothing < 1:
        raise ValueError("label smoothing must be finite and in [0, 1)")
    x, y = torch.from_numpy(xs), torch.from_numpy(ys)
    # Jump edges occupy very few frames, but missing one can lose an entire run.
    weights = torch.tensor([1.0, 6.0])
    for _ in range(epochs):
        for ix in torch.randperm(len(x)).split(512):
            logits = actor(x[ix]).split((3, 3, 2, 2), dim=1)
            loss = sum(
                torch.nn.functional.cross_entropy(
                    t,
                    y[ix, i],
                    weight=weights
                    if i == 2
                    else direction_weights[i]
                    if i < 2 and direction_weights is not None
                    else None,
                    label_smoothing=label_smoothing,
                )
                for i, t in enumerate(logits)
            )
            optimizer.zero_grad()
            loss.backward()
            optimizer.step()
    with torch.no_grad():
        correct = torch.zeros(4)
        for ix in torch.arange(len(x)).split(4096):
            predictions = torch.stack([t.argmax(1) for t in actor(x[ix]).split((3, 3, 2, 2), 1)], 1)
            correct += (predictions == y[ix]).sum(0)
    return dict(loss=float(loss.detach()), accuracy=(correct / len(x)).tolist(), examples=len(x))
