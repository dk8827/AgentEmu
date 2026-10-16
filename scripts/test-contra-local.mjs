// Requires a local ROM and a trace from record-contra-parity.py; never downloads a game.
import { readFile } from "node:fs/promises";
import { createHash } from "node:crypto";
import assert from "node:assert/strict";
import { resolve } from "node:path";
import init, { NesMachine, ContraPolicy } from "../web/pkg/emulator.js";
await init({
  module_or_path: await readFile(
    new URL("../web/pkg/emulator_bg.wasm", import.meta.url),
  ),
});
const [romPath, modelPath] = process.argv.slice(2);
if (!romPath || !modelPath)
  throw Error("usage: node scripts/test-contra-local.mjs ROM MODEL_DIRECTORY");
const rom = await readFile(romPath),
  manifest = await readFile(resolve(modelPath, "manifest.json"), "utf8"),
  weights = await readFile(resolve(modelPath, "weights.safetensors"));
const runs = JSON.parse(await readFile(".cache/contra-parity.json", "utf8"));
const actor = new ContraPolicy(manifest, weights);
for (const run of runs) {
  const m = new NesMachine(rom);
  m.start_contra();
  try {
    assert.equal(Number(m.completed_frames()), run.start_frame);
    run.slots.forEach((active, p) => {
      if (active) m.load_agent(p, manifest, weights);
    });
    for (let i = 0; i < 120; i++) {
      m.controllers(i % 2 === 0 ? 130 : 128, i % 2 ? 66 : 64);
      m.frame();
      m.audio_samples();
      if (i % 20 === 19) {
        const expected = run.checks[(i + 1) / 20 - 1];
        assert.equal(
          createHash("sha256").update(m.snapshot()).digest("hex"),
          expected.state_sha256,
        );
        for (let p = 0; p < 2; p++)
          assert.deepEqual(
            [...m.contra_observation(p)],
            expected.observations[p].map(Math.fround),
          );
      }
    }
    const saved = m.snapshot();
    m.frame();
    const next = m.snapshot();
    m.restore(saved);
    m.frame();
    assert.deepEqual(m.snapshot(), next);
    console.log(`native/WASM simultaneous ports ${run.slots}: passed`);
  } finally {
    m.free();
  }
}
actor.free();
