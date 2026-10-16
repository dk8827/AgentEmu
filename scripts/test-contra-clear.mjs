// Complete neural episodes through the application's WASM integration.
// Mixed-team tests replay only the scripted partner; neural ports infer every frame.
import { readFile, writeFile } from "node:fs/promises";
import { createHash } from "node:crypto";
import assert from "node:assert/strict";
import { resolve, dirname } from "node:path";
import init, { NesMachine, ContraPolicy } from "../web/pkg/emulator.js";
const [romPath, modelPath, reportPath] = process.argv
  .slice(2)
  .filter((arg) => !arg.startsWith("--parity-report="));
if (!romPath || !modelPath || !reportPath)
  throw Error(
    "usage: node scripts/test-contra-clear.mjs ROM MODEL_DIRECTORY EVALUATION_JSON",
  );
const parityPath = process.argv
  .find((arg) => arg.startsWith("--parity-report="))
  ?.slice("--parity-report=".length);
const wasmBytes = await readFile(
  new URL("../web/pkg/emulator_bg.wasm", import.meta.url),
);
await init({
  module_or_path: wasmBytes,
});
const rom = await readFile(romPath);
const manifest = await readFile(resolve(modelPath, "manifest.json"), "utf8");
const weights = await readFile(resolve(modelPath, "weights.safetensors"));
const report = JSON.parse(await readFile(reportPath, "utf8"));
const rows = Array.isArray(report) ? report : report.episodes;
assert.ok(rows.length > 0, "report must contain episodes");
const policy = new ContraPolicy(manifest, weights);
try {
  for (const expected of rows) {
    assert.ok(
      Number.isInteger(expected.frames) &&
        expected.frames > 0 &&
        expected.frames <= 108000,
    );
    const warmup = expected.warmup ?? expected.seed % 32;
    assert.ok(Number.isInteger(warmup) && warmup >= 0 && warmup <= 240);
    const mixed = expected.profile && expected.profile !== "pair";
    const replay = mixed
      ? JSON.parse(
          await readFile(resolve(dirname(reportPath), expected.replay), "utf8"),
        )
      : null;
    if (replay) assert.equal(replay.inputs.length, expected.frames);
    const label = `warmup ${warmup} / ${expected.profile ?? "pair"} / P${(expected.slot ?? 0) + 1}`;
    const machine = new NesMachine(rom);
    try {
      machine.start_contra();
      machine.controllers(0, 0);
      for (let i = 0; i < warmup; i++) {
        machine.frame();
        machine.audio_samples();
      }
      for (let p = 0; p < 2; p++)
        if (!mixed || p === expected.slot)
          machine.load_agent(p, manifest, weights);
      const inputs = createHash("sha256");
      for (let i = 0; i < expected.frames; i++) {
        const state = JSON.parse(machine.contra_state());
        const masks = [0, 1].map((p) =>
          !state.gameplay
            ? 0
            : mixed && p !== expected.slot
              ? replay.inputs[i][p]
              : policy.buttons(machine.contra_observation(p)),
        );
        inputs.update(Uint8Array.from(masks));
        machine.controllers(
          ...masks.map((m, p) => (mixed && p !== expected.slot ? m : 0)),
        );
        machine.frame();
        machine.audio_samples();
      }
      const state = JSON.parse(machine.contra_state());
      assert.equal(state.level === 1, expected.success, `${label}: clear`);
      assert.equal(
        inputs.digest("hex"),
        expected.input_sha256,
        `${label}: controller trace`,
      );
      assert.equal(
        createHash("sha256").update(machine.snapshot()).digest("hex"),
        expected.final_state_sha256,
        `${label}: full machine state`,
      );
      console.log(
        `${label}: ${expected.frames} frames, clear=${expected.success}, exact native/WASM parity`,
      );
    } finally {
      machine.free();
    }
  }
} finally {
  policy.free();
}

if (parityPath) {
  const sha = (bytes) => createHash("sha256").update(bytes).digest("hex");
  await writeFile(
    parityPath,
    JSON.stringify(
      {
        version: 1,
        result: "exact",
        rom_sha256: sha(rom),
        weights_sha256: sha(weights),
        native_report_sha256: sha(await readFile(reportPath)),
        wasm_sha256: sha(wasmBytes),
        episodes: rows.map(
          ({
            profile,
            slot,
            warmup,
            frames,
            success,
            input_sha256,
            final_state_sha256,
          }) => ({
            profile,
            slot,
            warmup,
            frames,
            success,
            input_sha256,
            final_state_sha256,
          }),
        ),
      },
      null,
      2,
    ) + "\n",
    { flag: "wx" },
  );
}
