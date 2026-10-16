// Local-ROM regression: title menus must be usable, task starts must activate policies.
import { readFile } from "node:fs/promises";
import { join } from "node:path";
import { spawnSync } from "node:child_process";
import assert from "node:assert/strict";
import init, { NesMachine } from "../web/pkg/emulator.js";
const [romDir] = process.argv.slice(2);
if (!romDir)
  throw Error("usage: node scripts/validate-agent-phases.mjs ROM_DIRECTORY");
const result = spawnSync(
  process.env.PYTHON ?? "python",
  [
    "-c",
    "import json; from contra_training.contracts import CONTRACTS; print(json.dumps(CONTRACTS))",
  ],
  { encoding: "utf8" },
);
if (result.status !== 0) throw Error(result.stderr);
const contracts = JSON.parse(result.stdout);
await init({ module_or_path: await readFile("web/pkg/emulator_bg.wasm") });
const targets = JSON.parse(
  await readFile("fixtures/compatibility/targets.json", "utf8"),
);
const source = JSON.parse(
  await readFile("fixtures/contra-policy/manifest.json", "utf8"),
);
const weights = await readFile("fixtures/contra-policy/weights.safetensors");
for (const [task, [architecture, observation, hash]] of Object.entries(
  contracts,
)) {
  // Base requires a Jungle-clear entry; it cannot be tested using a title-menu shortcut.
  if (task === "contra-base1") continue;
  const target = targets.find((t) => t.sha256 === hash);
  assert.ok(target, `missing ROM fixture metadata: ${task}`);
  const m = new NesMachine(await readFile(join(romDir, target.file)));
  const manifest = JSON.stringify({
    ...source,
    architecture,
    observation,
    rom_sha256: hash,
  });
  const phase = () => {
    m.load_agent(0, manifest, weights);
    const p = JSON.parse(m.agent_phases())[0];
    m.clear_agent(0);
    return p;
  };
  assert.equal(phase(), "menu", `${task}: initial boot`);
  for (let i = 0; i < 120; i++) {
    m.frame();
    m.audio_samples();
  }
  assert.equal(phase(), "menu", `${task}: title menu`);
  if (task === "contra-jungle") m.start_contra();
  else m.start_task(task);
  assert.equal(phase(), "supported-gameplay", `${task}: normal task start`);
  console.log(`${task}: boot, title, task start passed`);
  m.free();
}
