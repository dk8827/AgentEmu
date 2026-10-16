// Full controller replay verification against the browser's WASM core. ROM stays local.
import { readFile } from "node:fs/promises";
import assert from "node:assert/strict";
import init, { NesMachine } from "../web/pkg/emulator.js";
import { validateReplay, digest } from "../tests/reference-js/replay.mjs";
const [romPath, ...paths] = process.argv.slice(2);
const entryIndex = paths.indexOf("--entry");
let entry = null;
if (entryIndex >= 0) {
  entry = await readFile(paths[entryIndex + 1]);
  paths.splice(entryIndex, 2);
}
if (!romPath || !paths.length)
  throw Error("usage: node scripts/verify-replay.mjs ROM REPLAY_JSON ...");
await init({
  module_or_path: await readFile(
    new URL("../web/pkg/emulator_bg.wasm", import.meta.url),
  ),
});
const rom = await readFile(romPath);
for (const path of paths) {
  const m = new NesMachine(rom);
  try {
    const replay = validateReplay(
      JSON.parse(await readFile(path, "utf8")),
      m.cartridge_hash(),
    );
    assert.equal(
      await digest(new Uint8Array(replay.inputs.flat())),
      replay.result.input_sha256,
    );
    if (replay.game === "contra-base1") {
      if (!entry) throw Error("Base replay requires --entry LOCAL_STATE");
      assert.equal(await digest(entry), replay.entry_state_sha256);
      m.restore(entry);
    } else if (replay.game === "snowbros-floor1") m.start_snow();
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
      ].includes(replay.game)
    )
      m.start_task(replay.game);
    else m.start_contra();
    m.controllers(0, 0);
    for (let i = 0; i < replay.warmup; i++) {
      m.frame();
      m.audio_samples();
    }
    for (const masks of replay.inputs) {
      m.controllers(...masks);
      m.frame();
      m.audio_samples();
    }
    assert.equal(await digest(m.snapshot()), replay.result.final_state_sha256);
    console.log(
      `${path}: ${replay.inputs.length} frames, exact final-state parity`,
    );
  } finally {
    m.free();
  }
}
