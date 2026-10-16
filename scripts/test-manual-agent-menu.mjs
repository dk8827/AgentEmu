// Local commercial ROM is supplied explicitly and never saved in the repository.
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import init, { NesMachine } from "../web/pkg/emulator.js";
const path = process.argv[2];
if (!path) throw Error("Supply the local Contra ROM path");
await init({
  module_or_path: await readFile(
    new URL("../web/pkg/emulator_bg.wasm", import.meta.url),
  ),
});
const m = new NesMachine(await readFile(path));
try {
  m.start_contra();
  const original = JSON.parse(m.contra_state());
  assert.equal(original.gameplay, true);
  m.load_agent(
    0,
    await readFile("models/contra-jungle-v3/manifest.json", "utf8"),
    await readFile("models/contra-jungle-v3/weights.safetensors"),
  );
  m.controllers(8, 0);
  m.frame();
  m.controllers(0, 0);
  m.frame();
  assert.equal(
    JSON.parse(m.contra_state()).gameplay,
    false,
    "manual Start pauses in-game despite neural ownership",
  );
  m.controllers(8, 0);
  m.frame();
  m.controllers(0, 0);
  m.frame();
  assert.equal(
    JSON.parse(m.contra_state()).gameplay,
    true,
    "second manual Start resumes in-game",
  );
  console.log(
    "Manual Start works on the neural port without altering emulator transport.",
  );
} finally {
  m.free();
}
