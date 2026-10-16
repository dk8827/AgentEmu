// Compare documented neutral starts with native training; user supplies the ROM.
import { readFile } from "node:fs/promises";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import assert from "node:assert/strict";
import init, { NesMachine } from "../web/pkg/emulator.js";
const [rom, task, bundle] = process.argv.slice(2);
if (!rom || !task)
  throw Error(
    "usage: node scripts/test-task-start-timing.mjs ROM TASK [MODEL]",
  );
const result = spawnSync(
  process.env.PYTHON ?? "python",
  [
    "-c",
    `
import sys,json,hashlib
from pathlib import Path
from contra_training._native import TaskEpisode
m=TaskEpisode(Path(sys.argv[1]).read_bytes(),sys.argv[2],12000)
rows=[]
for n in [0,40,240]:
 m.reset(n)
 rows.append(dict(warmup=n,digest=hashlib.sha256(m.snapshot()).hexdigest()))
print(json.dumps(rows))
`,
    rom,
    task,
  ],
  { encoding: "utf8" },
);
if (result.status !== 0) throw Error(result.stderr);
await init({ module_or_path: await readFile("web/pkg/emulator_bg.wasm") });
const m = new NesMachine(await readFile(rom));
const digest = () => createHash("sha256").update(m.snapshot()).digest("hex");
try {
  if (bundle) {
    const manifest = await readFile(`${bundle}/manifest.json`, "utf8");
    const weights = await readFile(`${bundle}/weights.safetensors`);
    m.load_agent(0, manifest, weights);
    m.load_agent(1, manifest, weights);
  }
  for (const row of JSON.parse(result.stdout)) {
    m.start_task_with_warmup(task, row.warmup);
    assert.equal(
      digest(),
      row.digest,
      `neutral start ${row.warmup}; assigned agents must not act during warmup`,
    );
  }
  const before = digest();
  assert.throws(() => m.start_task_with_warmup(task, 241));
  assert.equal(digest(), before);
  assert.throws(() => m.start_task_with_warmup("invalid-task", 40));
  assert.equal(digest(), before);
  m.start_task(task);
  assert.equal(digest(), JSON.parse(result.stdout)[0].digest);
  console.log(
    `${task}: exact native/WASM starts at 0, 40 and 240 neutral frames; invalid starts leave state unchanged`,
  );
} finally {
  m.free();
}
