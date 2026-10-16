// Local, user-supplied ROM only. Mutated states below are isolated fault-injection tests.
import { readFile, writeFile } from "node:fs/promises";
import assert from "node:assert/strict";
import init, { NesMachine } from "../web/pkg/emulator.js";
const [rom, bundle = "models/contra-jungle-v3"] = process.argv.slice(2);
if (!rom)
  throw Error("usage: node scripts/validate-shared-control.mjs ROM [BUNDLE]");
await init({ module_or_path: await readFile("web/pkg/emulator_bg.wasm") });
const m = new NesMachine(await readFile(rom));
m.start_contra();
const baseline = m.snapshot(),
  manifest = await readFile(`${bundle}/manifest.json`, "utf8"),
  weights = await readFile(`${bundle}/weights.safetensors`);
const normal = (v) =>
  (v & 48) === 48 ? normal(v & ~48) : (v & 192) === 192 ? v & ~192 : v;
for (let slot = 0; slot < 2; slot++) {
  m.load_agent(slot, manifest, weights);
  for (let owners = 0; owners < 16; owners++) {
    const mask = [192, 48, 1, 2].reduce(
      (a, g, i) => a | (owners & (1 << i) ? g : 0),
      0,
    );
    m.set_agent_ownership(slot, mask);
    for (const human of [0, 1, 2, 3, 12, 81, 146, 255]) {
      m.restore(baseline);
      m.controllers(human, human);
      m.frame();
      m.audio_samples();
      const trace = JSON.parse(m.control_trace());
      assert.equal(
        trace[slot].applied,
        normal((human & ~mask) | (trace[slot].proposal & mask)),
      );
      assert.equal(trace[1 - slot].applied, normal(human));
      assert.equal(trace[slot].applied & 12, human & 12);
    }
  }
  for (const invalid of [4, 8, 16, 64, 255])
    assert.throws(() => m.set_agent_ownership(slot, invalid));
  // Restoring directly into a later stage must refuse even when gameplay=false.
  const unsupported = JSON.parse(new TextDecoder().decode(baseline));
  unsupported.ram[0x30] = 2;
  m.restore(new TextEncoder().encode(JSON.stringify(unsupported)));
  assert.equal(JSON.parse(m.agent_phases())[slot], "unsupported-gameplay");
  const before = m.snapshot();
  assert.throws(() => m.frame());
  assert.deepEqual(m.snapshot(), before);
  // A genuine title/menu still allows all human controls on an AI-owned port.
  const menu = JSON.parse(new TextDecoder().decode(baseline));
  menu.ram[0x18] = 0;
  m.restore(new TextEncoder().encode(JSON.stringify(menu)));
  m.set_agent_ownership(slot, 243);
  m.controllers(129, 129);
  m.frame();
  assert.equal(JSON.parse(m.control_trace())[slot].phase, "menu");
  assert.equal(JSON.parse(m.control_trace())[slot].applied, 129);
  m.clear_agent(slot);
}
m.free();
console.log(
  "Shared control: 256 runtime arbitration cases, both ports, menu ownership, invalid masks, and restored unsupported-stage refusal passed.",
);
