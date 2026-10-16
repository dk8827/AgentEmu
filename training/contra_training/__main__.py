import argparse
import json
import time
from pathlib import Path
import numpy as np
import torch
from .env import ContraEnv, scripted, buttons
from .model import ActorNet, export, from_ppo


def evaluate(rom, actor, seeds, horizon, teammate="scripted"):
    rows = []
    for player in (0, 1):
        env = ContraEnv(rom, player=player, horizon=horizon, teammate=teammate)
        for seed in seeds:
            obs, info = env.reset(seed=seed)
            rng = np.random.default_rng(seed + 1000 * player)
            total = 0.0
            while True:
                if actor == "random":
                    action = np.array([rng.integers(n) for n in (3, 3, 2, 2)])
                elif actor == "scripted":
                    action = scripted(obs)
                else:
                    action = actor.action(obs)
                obs, reward, terminated, truncated, info = env.step(action)
                total += reward
                if terminated or truncated:
                    break
            rows.append(
                dict(
                    player=player,
                    seed=seed,
                    reward=total,
                    progress=info["max_progress"],
                    deaths=info["deaths"],
                    success=info["success"],
                    frames=info["frames"],
                )
            )
    return dict(
        episodes=rows,
        mean_progress=float(np.mean([r["progress"][r["player"]] for r in rows])),
        mean_reward=float(np.mean([r["reward"] for r in rows])),
        clears=sum(r["success"] for r in rows),
    )


def collect(rom, steps, seed, horizon):
    env = ContraEnv(rom, horizon=horizon)
    obs, _ = env.reset(seed=seed % 24)
    xs, ys = [], []
    rng = np.random.default_rng(seed)
    for i in range(steps):
        # Label both ports, including off-teacher states, from one consistent Rust frame.
        for o in env.last["observations"]:
            xs.append(o)
            ys.append(scripted(o))
        action = scripted(obs)
        if rng.random() < 0.08:
            action = np.array([rng.integers(n) for n in (3, 3, 2, 2)])
        obs, _, term, trunc, _ = env.step(action)
        if term or trunc:
            obs, _ = env.reset()
    return np.asarray(xs, dtype=np.float32), np.asarray(ys, dtype=np.int64)


def main():
    p = argparse.ArgumentParser(
        description="Train and evaluate the fixed Contra Jungle actor with local ROMs."
    )
    p.add_argument("command", choices=["train", "evaluate", "evaluate-pair"])
    p.add_argument("--rom", required=True)
    p.add_argument("--output", required=True, help="new output directory; never overwritten")
    p.add_argument("--model", help="exported model directory for evaluation")
    p.add_argument("--second-model", help="player-two model for evaluate-pair; defaults to --model")
    p.add_argument("--bc-steps", type=int, default=6000)
    p.add_argument(
        "--envs", type=int, default=4, help="independent Rust environments for PPO (1..128)"
    )
    p.add_argument("--ppo-steps", type=int, default=16384)
    p.add_argument("--horizon", type=int, default=3600)
    p.add_argument("--seed", type=int, default=7)
    p.add_argument("--eval-seeds", type=int, default=4)
    args = p.parse_args()
    if Path(args.output).exists():
        p.error("output directory already exists")
    if not 1 <= args.envs <= 128:
        p.error("envs must be 1..128")
    if args.bc_steps < 1 or args.ppo_steps < 0 or not 1 <= args.eval_seeds <= 8:
        p.error("invalid training/evaluation budget")
    torch.set_num_threads(1)
    torch.manual_seed(args.seed)
    np.random.seed(args.seed)
    torch.use_deterministic_algorithms(True)
    started = time.perf_counter()
    seeds = list(range(24, 24 + args.eval_seeds))
    if args.command == "evaluate-pair":
        if not args.model:
            p.error("evaluate-pair requires --model")
        from .rollout import evaluate_pair

        result = evaluate_pair(args.rom, args.model, seeds, args.horizon, args.second_model)
        Path(args.output).mkdir(parents=True)
        (Path(args.output) / "evaluation.json").write_text(json.dumps(result, indent=2) + "\n")
        print(f"Stage clears: {result['clears']}/{len(result['episodes'])}")
        return
    if args.command == "evaluate":
        from safetensors.torch import load_file
        from ._native import Actor

        model = Path(args.model or "")
        Actor((model / "manifest.json").read_text(), (model / "weights.safetensors").read_bytes())
        actor = ActorNet()
        actor.load_state_dict(load_file(model / "weights.safetensors"))
        result = evaluate(args.rom, actor, seeds, args.horizon)
        Path(args.output).mkdir(parents=True)
        (Path(args.output) / "evaluation.json").write_text(json.dumps(result, indent=2) + "\n")
        print(json.dumps(result))
        return
    xs, ys = collect(args.rom, args.bc_steps, args.seed, args.horizon)
    print(f"Collected {len(xs)} structured demonstrations", flush=True)
    actor = ActorNet()
    opt = torch.optim.Adam(actor.parameters(), lr=0.001)
    x, y = torch.from_numpy(xs), torch.from_numpy(ys)
    for epoch in range(24):
        for indices in torch.randperm(len(x)).split(256):
            logits = actor(x[indices]).split((3, 3, 2, 2), dim=1)
            loss = sum(
                torch.nn.functional.cross_entropy(t, y[indices, i]) for i, t in enumerate(logits)
            )
            opt.zero_grad()
            loss.backward()
            opt.step()
    print(f"Behavior cloning loss {loss.item():.4f}", flush=True)
    candidates = [("behavior-cloning", actor)]
    if args.ppo_steps:
        from stable_baselines3 import PPO
        from .vector import RustVecEnv

        env = RustVecEnv(args.rom, count=args.envs, player=args.seed % 2, horizon=args.horizon)
        ppo = PPO(
            "MlpPolicy",
            env,
            seed=args.seed,
            device="cpu",
            n_steps=512,
            batch_size=128,
            n_epochs=4,
            learning_rate=1e-4,
            ent_coef=0.005,
            policy_kwargs=dict(
                net_arch=dict(pi=[64, 64], vf=[64, 64]), activation_fn=torch.nn.ReLU
            ),
            verbose=1,
        )
        for dst, src in zip(
            [
                ppo.policy.mlp_extractor.policy_net[0],
                ppo.policy.mlp_extractor.policy_net[2],
                ppo.policy.action_net,
            ],
            actor.layers,
        ):
            dst.load_state_dict(src.state_dict())
        ppo.learn(total_timesteps=args.ppo_steps)
        env.close()
        candidates.append(("ppo", from_ppo(ppo)))
    # Selection uses training-distribution starts; held-out seeds are reported only after selection.
    selection = {
        name: evaluate(args.rom, model, [0, 8], args.horizon) for name, model in candidates
    }
    chosen = max(candidates, key=lambda c: selection[c[0]]["mean_reward"])
    print(f"Selected {chosen[0]} using validation starts 0 and 8", flush=True)
    results = {
        "random": evaluate(args.rom, "random", seeds, args.horizon),
        "scripted": evaluate(args.rom, "scripted", seeds, args.horizon),
        "trained": evaluate(args.rom, chosen[1], seeds, args.horizon),
    }
    metadata = dict(
        name="Contra Jungle experimental",
        algorithm=chosen[0],
        seed=args.seed,
        bc_steps=args.bc_steps,
        ppo_steps=args.ppo_steps,
        horizon=args.horizon,
        teammate="scripted",
        reset="two-player-jungle-v1; neutral warmup seed%32",
        reward="jungle-progress-v2",
        stage="Jungle only",
        evaluation=results,
        selection=selection,
    )
    # Keep the public manifest bounded; detailed evaluation remains alongside the model.
    compact = {k: v for k, v in metadata.items() if k not in ("evaluation", "selection")}
    error = export(chosen[1], args.output, compact, xs[:: max(1, len(xs) // 256)])
    output = Path(args.output)
    (output / "evaluation.json").write_text(
        json.dumps(
            dict(**metadata, elapsed_seconds=time.perf_counter() - started, export_max_error=error),
            indent=2,
        )
        + "\n"
    )
    # Varied local parity observations contain semantic numbers only, not RAM/ROM/assets.
    np.savez_compressed(
        output / "observations.npz", observations=xs[:: max(1, len(xs) // 256)][:256]
    )
    print(
        json.dumps(
            {
                k: {a: v[a] for a in ("mean_progress", "mean_reward", "clears")}
                for k, v in results.items()
            }
        ),
        flush=True,
    )


if __name__ == "__main__":
    main()
