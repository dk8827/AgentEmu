// Full local-ROM episode: Gym actions and deployed WASM must finish identically.
import { readFile } from "node:fs/promises";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import assert from "node:assert/strict";
import init, { NesMachine } from "../web/pkg/emulator.js";
const [rom, model = "models/contra-jungle-v3"] = process.argv.slice(2);
if (!rom)
  throw Error("usage: node scripts/test-env-deployment.mjs ROM [MODEL]");
const py = spawnSync(
  process.env.PYTHON ?? "python",
  [
    "-c",
    `
import json,sys,hashlib
from contra_training.env import ContraEnv
from contra_training.rollout import load_actor
from contra_training.trajectory_train import factors
actor=load_actor(sys.argv[2]); env=ContraEnv(sys.argv[1],horizon=12000,teammate='idle')
o,info=env.reset(seed=0)
while True:
 mask=actor.buttons(o)
 o,reward,term,trunc,info=env.step(factors(mask))
 if term or trunc: break
print(json.dumps(dict(frames=info['frames'],success=info['success'],hash=hashlib.sha256(env.core.snapshot()).hexdigest())))
`,
    rom,
    model,
  ],
  { encoding: "utf8", maxBuffer: 2 * 1024 * 1024 },
);
if (py.status) throw Error(py.stderr);
const expected = JSON.parse(py.stdout);
await init({ module_or_path: await readFile("web/pkg/emulator_bg.wasm") });
const m = new NesMachine(await readFile(rom));
m.start_contra();
m.load_agent(
  0,
  await readFile(`${model}/manifest.json`, "utf8"),
  await readFile(`${model}/weights.safetensors`),
);
for (let i = 0; i < expected.frames; i++) {
  m.controllers(0, 0);
  m.frame();
  m.audio_samples();
}
assert.equal(
  createHash("sha256").update(m.snapshot()).digest("hex"),
  expected.hash,
);
console.log(
  JSON.stringify({
    test: "complete Gym/deployed policy episode",
    ...expected,
    exact_match: true,
  }),
);
m.free();
