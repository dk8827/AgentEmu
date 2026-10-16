// Complete expansion-task neural rollouts: native reference vs application's WASM integration.
import { readFile, writeFile } from "node:fs/promises";
import { resolve, dirname } from "node:path";
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import init, { NesMachine, ContraPolicy } from "../web/pkg/emulator.js";
const [romPath, modelPath, reportPath, entryPath] = process.argv
  .slice(2)
  .filter((arg) => !arg.startsWith("--parity-report="));
if (!reportPath)
  throw Error(
    "usage: node scripts/test-task-agent.mjs ROM MODEL REPORT [BASE_ENTRY_STATE]",
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
const rom = await readFile(romPath),
  manifest = await readFile(resolve(modelPath, "manifest.json"), "utf8"),
  weights = await readFile(resolve(modelPath, "weights.safetensors")),
  report = JSON.parse(await readFile(reportPath, "utf8")),
  actor = new ContraPolicy(manifest, weights);
assert.ok(report.episodes.length > 0, "report must contain episodes");
try {
  for (const expected of report.episodes) {
    assert.ok(
      Number.isInteger(expected.frames) &&
        expected.frames > 0 &&
        expected.frames <= 108000,
    );
    const m = new NesMachine(rom);
    try {
      if (report.task === "snowbros-floor1") m.start_snow();
      else if (
        [
          "bubblebobble-round1",
          "iceclimber-mountain1",
          "rivercity-moose",
          "tmnt2-scene1",
          "doubledragon2-mission1",
          "worldcup-firstmatch",
          "chipndale-zone0",
          "jackal-stage1",
          "lifeforce-stage1",
        ].includes(report.task)
      )
        m.start_task(report.task);
      else {
        if (!entryPath) throw Error("Base task requires local entry");
        m.restore(await readFile(entryPath));
      }
      m.controllers(0, 0);
      for (let i = 0; i < expected.warmup; i++) {
        m.frame();
        m.audio_samples();
      }
      const mixed = ["agent-p1", "agent-p2"].includes(expected.profile);
      const replay = mixed
        ? JSON.parse(
            await readFile(
              resolve(dirname(reportPath), expected.replay),
              "utf8",
            ),
          )
        : null;
      if (mixed) assert.equal(replay.inputs.length, expected.frames);
      for (let p = 0; p < 2; p++) {
        if (!mixed || p === expected.slot) m.load_agent(p, manifest, weights);
      }
      const inputs = createHash("sha256");
      for (let i = 0; i < expected.frames; i++) {
        const f = JSON.parse(m.task_state(report.task));
        const masks = f.observations.map((o, p) =>
          !f.gameplay
            ? 0
            : mixed && p !== expected.slot
              ? replay.inputs[i][p]
              : actor.buttons(new Float32Array(o)),
        );
        m.controllers(
          ...masks.map((mask, p) => (mixed && p !== expected.slot ? mask : 0)),
        );
        inputs.update(new Uint8Array(masks));
        m.frame();
        m.audio_samples();
      }
      const state = JSON.parse(m.task_state(report.task));
      assert.equal(state.success, expected.success);
      assert.equal(inputs.digest("hex"), expected.input_sha256);
      assert.equal(
        createHash("sha256").update(m.snapshot()).digest("hex"),
        expected.final_state_sha256,
      );
      console.log(
        `${report.task} warmup ${expected.warmup}: ${expected.frames} frames, clear=${state.success}, exact neural/native/WASM parity`,
      );
    } finally {
      m.free();
    }
  }
} finally {
  actor.free();
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
        episodes: report.episodes.map(
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
