import { test } from "node:test";
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import init, { ContraPolicy, NesMachine } from "../web/pkg/emulator.js";
await init({
  module_or_path: await readFile(
    new URL("../web/pkg/emulator_bg.wasm", import.meta.url),
  ),
});
const fixture = new URL("../fixtures/contra-policy/", import.meta.url);
const manifest = await readFile(new URL("manifest.json", fixture), "utf8");
const weights = await readFile(new URL("weights.safetensors", fixture));
test("Contra actor matches PyTorch and rejects incompatible model manifests", async () => {
  const actor = new ContraPolicy(manifest, weights);
  try {
    const cases = JSON.parse(await readFile(new URL("parity.json", fixture)));
    for (const c of cases) {
      const logits = actor.forward(new Float32Array(c.observation));
      logits.forEach((x, i) => assert.ok(Math.abs(x - c.logits[i]) < 2e-5));
      assert.equal(actor.buttons(new Float32Array(c.observation)), c.buttons);
    }
    const bad = JSON.parse(manifest);
    bad.observation = "different-game";
    assert.throws(() => new ContraPolicy(JSON.stringify(bad), weights));
    assert.throws(() => actor.forward(new Float32Array(256).fill(NaN)));
  } finally {
    actor.free();
  }
});
test("Other games reject Contra adapters and policy assignment without changing state", () => {
  const m = NesMachine.playground();
  try {
    const before = m.snapshot();
    assert.throws(() => m.load_agent(0, manifest, weights));
    assert.throws(() => m.contra_observation(0));
    assert.deepEqual(m.snapshot(), before);
  } finally {
    m.free();
  }
});

test("Shared 268-feature actor matches native decisions and pins the new shape", async () => {
  const model = new URL("../fixtures/assistance-policy/", import.meta.url);
  const actor = new ContraPolicy(await readFile(new URL("manifest.json", model), "utf8"),
                                 await readFile(new URL("weights.safetensors", model)));
  try {
    const cases = JSON.parse(await readFile(new URL("../fixtures/assistance-policy/parity.json", import.meta.url)));
    for (const c of cases) {
      const actual = actor.forward(new Float32Array(c.observation));
      for (const [from,to] of [[0,3],[3,6],[6,8],[8,10]]) {
        const scale = Math.max(1,...c.logits.slice(from,to).map(Math.abs));
        for (let i=from;i<to;i++) assert.ok(Math.abs(actual[i]-c.logits[i])/scale < 2e-5);
      }
      assert.equal(actor.buttons(new Float32Array(c.observation)),c.buttons);
    }
    assert.throws(()=>actor.forward(new Float32Array(256)));
  } finally { actor.free(); }
});
