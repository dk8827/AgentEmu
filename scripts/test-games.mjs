// User-supplied ROMs only. Produces metadata in ignored .cache; no cartridge data is published.
import { readFile, mkdir, writeFile } from "node:fs/promises";
import { resolve, join } from "node:path";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import assert from "node:assert/strict";
import init, { NesMachine } from "../web/pkg/emulator.js";
const folder = process.argv[2];
if (!folder)
  throw Error(
    "usage: node scripts/test-games.mjs ROM_FOLDER [--gameplay] [--solo]",
  );
await init({
  module_or_path: await readFile(
    new URL("../web/pkg/emulator_bg.wasm", import.meta.url),
  ),
});
const solo = process.argv.includes("--solo");
const targets = JSON.parse(
  await readFile(
    new URL(
      `../fixtures/compatibility/${solo ? "solo-targets" : "targets"}.json`,
      import.meta.url,
    ),
  ),
);
const selectedGame = process.argv
  .find((arg) => arg.startsWith("--game="))
  ?.slice(7);
if (selectedGame && !targets.some((target) => target.file === selectedGame))
  throw Error(`unknown game: ${selectedGame}`);
const mode = process.argv.includes("--gameplay") ? "gameplay" : "smoke";
const results = [];
for (const [index, target] of targets.entries()) {
  if (selectedGame && target.file !== selectedGame) continue;
  try {
    const scriptPath = resolve(
      "fixtures/compatibility",
      mode === "gameplay" ? target.gameplay_script : "smoke.json",
    );
    const script = JSON.parse(await readFile(scriptPath));
    const path = join(folder, target.file);
    const bytes = await readFile(path);
    assert.equal(
      createHash("sha256").update(bytes).digest("hex"),
      target.sha256,
      "wrong ROM revision",
    );
    const output = resolve(
      ".cache/game-parity",
      solo ? "solo" : "coop",
      mode,
      String(index),
    );
    await mkdir(output, { recursive: true });
    const native = spawnSync(
      resolve(
        "target/release",
        process.platform === "win32" ? "nes-cli.exe" : "nes-cli",
      ),
      ["playback", path, scriptPath, output],
      { encoding: "utf8" },
    );
    if (native.status !== 0) throw Error(native.stderr);
    const expected = JSON.parse(await readFile(join(output, "report.json")));
    const m = new NesMachine(bytes);
    try {
      if (script.mmc3_alternate) m.set_mmc3_alternate(true);
      for (const [i, segment] of script.segments.entries()) {
        m.controllers(...segment.players);
        const chunks = [];
        for (let frame = 0; frame < segment.frames; frame++) {
          m.frame();
          chunks.push(Buffer.from(m.audio_samples().buffer));
        }
        assert.deepEqual(
          JSON.parse(m.report()),
          expected.checkpoints[i].report,
          `checkpoint ${i} state parity`,
        );
        assert.deepEqual(
          Buffer.concat(chunks),
          await readFile(join(output, `${String(i).padStart(2, "0")}.f32`)),
          `checkpoint ${i} audio parity`,
        );
        const saved = m.snapshot();
        m.frame();
        const state = m.snapshot();
        const audio = m.audio_samples();
        m.restore(saved);
        m.frame();
        assert.deepEqual(m.snapshot(), state);
        assert.deepEqual(m.audio_samples(), audio);
        m.restore(saved);
      }
      results.push({
        ...target,
        status: "passed",
        mode,
        script_sha256: createHash("sha256")
          .update(await readFile(scriptPath))
          .digest("hex"),
        frames: expected.checkpoints.at(-1).frames,
        checkpoints: expected.checkpoints.map((c) => ({
          frames: c.frames,
          state_sha256: c.report.state_sha256,
          framebuffer_sha256: c.report.ppu.framebuffer_sha256,
          audio_rms: c.audio_rms,
        })),
      });
    } finally {
      m.free();
    }
  } catch (error) {
    results.push({ ...target, status: "failed", error: String(error) });
  }
  console.log(
    `${target.file}: ${results.at(-1).status}${results.at(-1).error ?? ""}`,
  );
}
await writeFile(
  resolve(
    ".cache/game-parity",
    `${solo ? "solo-" : ""}${mode}${selectedGame ? "-single" : ""}-results.json`,
  ),
  JSON.stringify(results, null, 2) + "\n",
);
if (results.some((r) => r.status !== "passed")) process.exitCode = 1;
