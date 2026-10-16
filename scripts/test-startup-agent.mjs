// Check the real deployed agent from power-on through ordinary cartridge menus.
import assert from 'node:assert/strict';
import {readFile, writeFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {resolve, dirname} from 'node:path';
import init, {NesMachine} from '../web/pkg/emulator.js';
const [romPath, modelPath, reportPath, proofPath] = process.argv.slice(2);
if (!proofPath) throw Error('usage: node scripts/test-startup-agent.mjs ROM MODEL REPORT NEW_PROOF');
const sha = value => createHash('sha256').update(value).digest('hex');
const wasm = await readFile(new URL('../web/pkg/emulator_bg.wasm', import.meta.url));
await init({module_or_path: wasm});
const rom = await readFile(romPath);
const manifestText = await readFile(resolve(modelPath, 'manifest.json'), 'utf8');
const manifest = JSON.parse(manifestText);
const weights = await readFile(resolve(modelPath, 'weights.safetensors'));
const reportBytes = await readFile(reportPath);
const report = JSON.parse(reportBytes);
assert.equal(report.kind, 'title-menu-starts');
assert.ok(['contra-jungle', 'snowbros-floor1', 'bubblebobble-round1', 'iceclimber-mountain1',
  'doubledragon2-mission1', 'tmnt2-scene1'].includes(report.task));
assert.equal(report.model.weights_sha256, sha(weights));
assert.equal(report.model.rom_sha256, sha(rom));
assert.equal(manifest.weights_sha256, sha(weights));
const profiles = report.profiles ?? ['pair', 'agent-p1', 'agent-p2'];
assert.deepEqual(profiles, report.players === 1 ? ['solo'] :
  report.mode === 'shared' ? ['agent-p1', 'agent-p2'] : ['pair', 'agent-p1', 'agent-p2']);
const cases = new Set(report.episodes.map(r => `${r.profile}:${r.delay}`));
assert.equal(cases.size, profiles.length * report.delays.length);
assert.equal(report.episodes.length, cases.size);
for (const profile of profiles) {
  for (const delay of report.delays) assert.ok(cases.has(`${profile}:${delay}`));
}
for (const row of report.episodes) {
  assert.ok(Number.isInteger(row.frames) && row.frames > 0 && row.frames <= 108000);
  assert.equal(row.replay, row.replay.split('/').at(-1));
  const replay = JSON.parse(await readFile(resolve(dirname(reportPath), row.replay)));
  assert.equal(replay.inputs.length, row.frames);
  assert.equal(sha(Uint8Array.from(replay.inputs.flat())), row.input_sha256);
  const machine = new NesMachine(rom);
  const observe = () => {
    if (report.task !== 'contra-jungle') return JSON.parse(machine.task_state(report.task));
    const state = JSON.parse(machine.contra_state());
    return {gameplay: state.gameplay, success: state.level === 1, state};
  };
  const neural = p => row.profile === 'pair' || p === Number(row.profile === 'agent-p2');
  try {
    for (const p of [0, 1]) if (neural(p)) {
      machine.load_agent(p, manifestText, weights);
      machine.set_agent_ownership(p, report.ownership ?? 243);
    }
    let played = false;
    for (let frame = 0; frame < row.frames; frame++) {
      const state = observe();
      played ||= state.gameplay;
      const expected = replay.inputs[frame];
      if (row.profile === 'solo') {
        assert.equal(expected[1], 0, 'solo P2 port must stay neutral');
        if (state.gameplay) {
          assert.equal(state.state.players[1].active, false, 'solo cartridge must have no P2');
          assert.ok(!state.state.two_players && !state.state.demo, 'actual solo game required');
        }
      }
      // Policy ports receive only human menu buttons during gameplay. The
      // runtime computes the gameplay proposal; recorded AI actions are never injected.
      machine.controllers(...(replay.human?.[frame] ?? expected.map((v, p) => neural(p) && state.gameplay ? v & 12 : v)));
      machine.frame();
      machine.audio_samples();
      const actual = JSON.parse(machine.control_trace()).map(d => d.applied);
      assert.deepEqual(actual, expected, `${row.profile}/${row.delay}, frame ${frame}`);
    }
    assert.equal(played && observe().success, row.success);
    assert.equal(sha(machine.snapshot()), row.final_state_sha256);
    console.log(`${row.profile}, delay ${row.delay}: ${row.frames} frames, clear=${row.success}, exact startup parity`);
  } finally {
    machine.free();
  }
}
await writeFile(proofPath, JSON.stringify({version: 1, result: 'exact',
  native_report_sha256: sha(reportBytes), weights_sha256: sha(weights), rom_sha256: sha(rom),
  wasm_sha256: sha(wasm), episodes: report.episodes.map(({profile, delay, frames, success, input_sha256, final_state_sha256}) =>
    ({profile, delay, frames, success, input_sha256, final_state_sha256}))}, null, 2) + '\n', {flag: 'wx'});
