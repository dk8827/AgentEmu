import {readFile} from 'node:fs/promises';
import assert from 'node:assert/strict';
import test from 'node:test';
import init, {Policy, NesMachine} from '../web/pkg/emulator.js';

await init({module_or_path: await readFile(new URL('../web/pkg/emulator_bg.wasm', import.meta.url))});
const root = new URL('../fixtures/policies/', import.meta.url);
const fixtures = JSON.parse(await readFile(new URL('parity.json', root), 'utf8'));

test('compiled Rust/WASM policies match 128 PyTorch observations', async () => {
  for (const player of fixtures.players) {
    const policy = new Policy(await readFile(new URL(player.weights, root)));
    for (let i=0; i<player.observations.length; i++) {
      const actual = policy.forward(Float32Array.from(player.observations[i]));
      actual.forEach((v,j)=>assert.ok(Math.abs(v-player.logits[i][j])<1e-5));
    }
    assert.throws(()=>policy.forward(new Float32Array(31)));
    policy.free();
  }
});

test('compiled Rust/WASM rejects invalid model tensors', async () => {
  for (const name of ['invalid-nan.safetensors','invalid-shape.safetensors','invalid-extra.safetensors']) {
    const bytes=await readFile(new URL(name,root));
    assert.throws(()=>new Policy(bytes),name);
  }
});

test('NES diagnostic and saved state match native execution', async () => {
  const expected=JSON.parse(await readFile(new URL('../fixtures/diagnostic/native.json',import.meta.url),'utf8'));
  const machine=NesMachine.diagnostic();
  machine.controllers(0xa5,0x5a);machine.run(1000);
  assert.deepEqual(JSON.parse(machine.report()),expected);
  const saved=machine.snapshot();
  machine.controllers(0x81,0x42);machine.run(999);
  const replay=machine.report();
  machine.restore(saved);machine.controllers(0x81,0x42);machine.run(999);
  assert.equal(machine.report(),replay);
  assert.throws(()=>machine.restore(new Uint8Array([1,2,3])));
  assert.throws(()=>machine.run(100001));
  machine.free();
});

test('PPU registers, DMA and NMI diagnostic matches native and replays', async () => {
  const expected=JSON.parse(await readFile(new URL('../fixtures/diagnostic/ppu-native.json',import.meta.url),'utf8'));
  const machine=NesMachine.ppu_diagnostic();
  machine.run(50000);
  assert.deepEqual(JSON.parse(machine.report()),expected);
  const saved=machine.snapshot();
  machine.run(20000);const replay=machine.report();
  machine.restore(saved);machine.run(20000);
  assert.equal(machine.report(),replay);
  const corrupted=JSON.parse(new TextDecoder().decode(saved));
  corrupted.ppu.oam=[];
  assert.throws(()=>machine.restore(new TextEncoder().encode(JSON.stringify(corrupted))));
  assert.equal(machine.report(),replay);
  machine.free();
});

test('NES background framebuffer and partial-frame replay match native', async () => {
  const {createHash} = await import('node:crypto');
  const expected=JSON.parse(await readFile(new URL('../fixtures/diagnostic/background-native.json',import.meta.url),'utf8'));
  const machine=NesMachine.background_diagnostic();
  for (let i=0;i<8;i++) machine.frame();
  assert.deepEqual(JSON.parse(machine.report()),expected);
  const codes=machine.framebuffer_codes();
  assert.equal(codes.length,256*240);
  const bytes=Buffer.alloc(codes.length*2);
  codes.forEach((code,i)=>bytes.writeUInt16LE(code,i*2));
  assert.equal(createHash('sha256').update(bytes).digest('hex'),expected.ppu.framebuffer_sha256);
  const rgba=machine.framebuffer_rgba();
  assert.equal(rgba.length,256*240*4);
  for (let i=3;i<rgba.length;i+=4) assert.equal(rgba[i],255);
  machine.run(4500); // partial frame, including fetch-latch state
  const saved=machine.snapshot();
  machine.frame();const replay=machine.report();const pixels=machine.framebuffer_codes();
  machine.restore(saved);machine.frame();
  assert.equal(machine.report(),replay);
  assert.deepEqual(machine.framebuffer_codes(),pixels);
  machine.free();
});

test('five-channel audio output and filter replay match native bytes',async()=>{
  const expected=await readFile(new URL('../fixtures/diagnostic/audio.f32',import.meta.url));
  const native=JSON.parse(await readFile(new URL('../fixtures/diagnostic/audio-native.json',import.meta.url),'utf8'));
  const m=NesMachine.audio_diagnostic();for(let i=0;i<8;i++)m.frame();
  const samples=m.audio_samples();const bytes=Buffer.alloc(samples.length*4);
  samples.forEach((value,i)=>bytes.writeFloatLE(value,i*4));assert.deepEqual(bytes,expected);
  assert.deepEqual(JSON.parse(m.report()),native);
  const saved=m.snapshot();m.frame();const next=m.audio_samples();m.restore(saved);m.frame();assert.deepEqual(m.audio_samples(),next);m.free();
});

test("battery RAM is explicit, bounded and independent from save-state snapshots", () => {
  const rom = new Uint8Array(16 + 16384 + 8192);
  rom.set([78, 69, 83, 26, 1, 1, 2]);
  const m = new NesMachine(rom),
    ram = Uint8Array.from({ length: 8192 }, (_, i) => i % 251);
  m.restore_battery_ram(ram);
  assert.deepEqual(m.battery_ram(), ram);
  const before = m.snapshot();
  assert.throws(() => m.restore_battery_ram(new Uint8Array(1)));
  assert.deepEqual(m.snapshot(), before);
  rom[6] = 0;
  const volatile = new NesMachine(rom);
  assert.equal(volatile.battery_ram().length, 0);
  assert.throws(() => volatile.restore_battery_ram(ram));
  m.free();
  volatile.free();
});

test("PAL WASM uses 50 Hz video and 48 kHz audio and rejects a region-changing snapshot", () => {
  const rom = new Uint8Array(16 + 32768);
  rom.set([78, 69, 83, 26, 2, 0, 0, 0, 0, 1]);
  rom.set([0x4c, 0, 0x80], 16);
  rom.set([0, 0x80], 16 + 32768 - 4);
  const m = new NesMachine(rom);
  try {
    assert.ok(Math.abs(m.frame_rate() - 50.007) < 0.01);
    m.frame();
    m.audio_samples();
    let samples = 0;
    for (let i = 0; i < 50; i++) {
      m.frame();
      samples += m.audio_samples().length;
    }
    assert.ok(samples >= 47990 && samples <= 48020, String(samples));
    const saved = m.snapshot();
    const bad = JSON.parse(new TextDecoder().decode(saved));
    bad.ppu.pal = false;
    assert.throws(() =>
      m.restore(new TextEncoder().encode(JSON.stringify(bad))),
    );
    assert.deepEqual(m.snapshot(), saved);
    m.frame();
    const expected = m.snapshot(),
      pcm = m.audio_samples();
    m.restore(saved);
    m.frame();
    assert.deepEqual(m.snapshot(), expected);
    assert.deepEqual(m.audio_samples(), pcm);
  } finally {
    m.free();
  }
});
