// User-owned cartridges stay local. Never infer gameplay completion from a boot.
import { readFile, writeFile, mkdir, rm, readdir } from "node:fs/promises";
import { resolve, join } from "node:path";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import assert from "node:assert/strict";
import {parityEvidence} from "./compatibility-evidence.mjs";
import init, { NesMachine } from "../web/pkg/emulator.js";
const root = process.argv[2];
if (!root)
  throw Error("usage: node scripts/test-top100.mjs ROM_ROOT [--suite=top100|ign100] [--ids=1,2,...]");
const suite = process.argv.find((x) => x.startsWith("--suite="))?.slice(8) ?? "top100";
assert.ok(["top100", "ign100"].includes(suite), "known compatibility suite");
const wanted = process.argv
  .find((x) => x.startsWith("--ids="))
  ?.slice(6)
  .split(",")
  .map(Number);
const targets = JSON.parse(
  await readFile(`fixtures/compatibility/${suite}-targets.json`),
);
const sha = (b) => createHash("sha256").update(b).digest("hex");
await init({ module_or_path: await readFile("web/pkg/emulator_bg.wasm") });
await mkdir(`.cache/${suite}/evidence`, { recursive: true });
const results = [];
for (const t of targets) {
  if (wanted && !wanted.includes(t.id)) continue;
  if (!t.file) {
    results.push({ id: t.id, title: t.title, status: t.status ?? "missing-original" });
    continue;
  }
  let m;
  const output = resolve(`.cache/${suite}/evidence`, String(t.id));
  try {
    const bytes = await readFile(join(root, t.file));
    assert.equal(sha(bytes), t.sha256, "exact cartridge revision");
    const scriptPath = resolve(
      "fixtures/compatibility",
      t.gameplay_script ?? "top100-smoke.json",
    );
    const script = JSON.parse(await readFile(scriptPath));
    await mkdir(output, { recursive: true });
    // A revised script may have fewer checkpoints. Remove only our generated
    // captures so old frames cannot be mistaken for evidence from the new run.
    for (const name of await readdir(output))
      if (/^\d+\.(pixels|f32)$/.test(name)) await rm(join(output, name));
    const native = spawnSync(
      resolve(`target/release/nes-cli${process.platform === "win32" ? ".exe" : ""}`),
      ["playback", join(root, t.file), scriptPath, output],
      { encoding: "utf8" },
    );
    if (native.status !== 0) throw Error(native.stderr);
    const expected = JSON.parse(await readFile(join(output, "report.json")));
    m = new NesMachine(bytes);
    if (script.mmc3_alternate) m.set_mmc3_alternate(true);
    const checkpoints = [];
    for (const [i, seg] of script.segments.entries()) {
      m.controllers(...seg.players);
      if (seg.zapper) m.zapper(...seg.zapper);
      const audio = [];
      for (let f = 0; f < seg.frames; f++) {
        m.frame();
        audio.push(Buffer.from(m.audio_samples().buffer));
      }
      assert.deepEqual(
        JSON.parse(m.report()),
        expected.checkpoints[i].report,
        `state parity ${i}`,
      );
      assert.deepEqual(
        Buffer.concat(audio),
        await readFile(join(output, `${String(i).padStart(2, "0")}.f32`)),
        `PCM parity ${i}`,
      );
      const saved = m.snapshot();
      m.frame();
      const next = m.snapshot(),
        pcm = m.audio_samples();
      m.restore(saved);
      m.frame();
      assert.deepEqual(m.snapshot(), next);
      assert.deepEqual(m.audio_samples(), pcm);
      m.restore(saved);
      const { frames, report, audio_rms, audio_samples } =
        expected.checkpoints[i];
      checkpoints.push({
        frames,
        state_sha256: report.state_sha256,
        framebuffer_sha256: report.ppu.framebuffer_sha256,
        audio_rms,
        audio_samples,
      });
    }
    await writeFile(join(output, "final.state"), m.snapshot());
    results.push({
      id: t.id,
      title: t.title,
      sha256: t.sha256,
      status: "passed",
      evidence: parityEvidence({frames:checkpoints.at(-1).frames,scripted:!!t.gameplay_script,
        checks:{exact_revision:true,native_wasm_state:true,native_wasm_pcm:true,deterministic_restore:true}}),
      coverage: t.gameplay_script ? "scripted-input" : "boot-input-smoke",
      script_sha256: sha(await readFile(scriptPath)),
      frame_rate: m.frame_rate(),
      checkpoints,
    });
  } catch (e) {
    results.push({
      id: t.id,
      title: t.title,
      status: "failed",
      error: String(e),
    });
  } finally {
    m?.free();
    for (const name of await readdir(output).catch(() => []))
      if (name.endsWith(".f32") || (process.argv.includes("--discard-captures") && (name.endsWith(".pixels") || name.endsWith(".state")))) await rm(join(output, name));
  }
  console.log(t.id, t.title, results.at(-1).status, results.at(-1).error ?? "");
  await persistResults();
}
// Also write missing-only selections and a missing final manifest entry.
await persistResults();
async function persistResults() {
  await writeFile(
    `.cache/${suite}/${wanted ? "selected-" : ""}results.json`,
    JSON.stringify(results, null, 2) + "\n",
  );
}
if (results.some((x) => x.status === "failed")) process.exitCode = 1;
