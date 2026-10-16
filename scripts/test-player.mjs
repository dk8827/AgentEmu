import { test } from "node:test";
import assert from "node:assert/strict";
import { PcmQueue } from "../tests/reference-js/pcm-queue.mjs";
test("PCM resampler preserves duration and linear samples at 44.1/48/96 kHz", () => {
  for (const rate of [44100, 48000, 96000]) {
    const q = new PcmQueue(rate);
    q.push(Float32Array.from({ length: 48000 }, (_, i) => i / 48000));
    const out = new Float32Array(rate / 2);
    q.render(out);
    assert.ok(Math.abs(q.consumed - 24000) <= 1);
    assert.equal(q.underruns, 0);
    for (let i = 0; i < out.length; i++)
      assert.ok(Math.abs(out[i] - i / rate) < 1e-6);
  }
});
test("PCM queue bounds latency, reports silence on starvation, resets and rejects nonfinite samples", () => {
  const q = new PcmQueue(48000);
  q.push(new Float32Array(96000).fill(0.5));
  assert.equal(q.size, 48000);
  assert.equal(q.dropped, 48000);
  q.reset();
  const out = new Float32Array(256).fill(1);
  q.render(out);
  assert.ok(out.every((x) => x === 0));
  assert.equal(q.underruns, 256);
  q.push(new Float32Array([NaN, Infinity, 1, 1]));
  q.render(out);
  assert.ok(out.every(Number.isFinite));
  q.reset();
  assert.equal(q.size, 0);
  assert.equal(q.consumed, 0);
});

import { readBattery, writeBattery } from "../tests/reference-js/battery-store.mjs";
import { SOLO_GAMES, gameInfo } from "../tests/reference-js/game-info.mjs";
test("battery saves are ROM-specific and reject corrupt or unavailable storage", () => {
  const data = new Map(),
    storage = {
      getItem: (k) => data.get(k) ?? null,
      setItem: (k, v) => data.set(k, v),
    };
  const ram = Uint8Array.from({ length: 8192 }, (_, i) => i % 251);
  assert.equal(readBattery(storage, "one"), null);
  writeBattery(storage, "one", ram);
  assert.deepEqual(readBattery(storage, "one"), ram);
  assert.equal(readBattery(storage, "two"), null);
  assert.throws(() => writeBattery(storage, "one", new Uint8Array(4)));
  for (const raw of [
    "null",
    "{}",
    "[1]",
    JSON.stringify(Array(8192).fill(-1)),
  ]) {
    storage.setItem("nes-battery-v1:bad", raw);
    assert.throws(() => readBattery(storage, "bad"));
  }
  assert.throws(() =>
    writeBattery(
      {
        setItem: () => {
          throw Error("quota");
        },
      },
      "one",
      ram,
    ),
  );
});
test("solo UI metadata matches the ten exact tested cartridge identities", async () => {
  const { readFile } = await import("node:fs/promises");
  const games = JSON.parse(
    await readFile(
      new URL("../fixtures/compatibility/solo-targets.json", import.meta.url),
    ),
  );
  assert.equal(Object.keys(SOLO_GAMES).length, 10);
  for (const game of games) {
    const info = gameInfo(game.sha256);
    assert.equal(info.players, 1);
    assert.equal(info.battery, game.battery);
    assert.equal(info.actions.length, 2);
  }
  assert.equal(gameInfo("Super Mario Bros. 2.nes"), null);
});

test("MMC6 one-KiB battery data round trips without padding", () => {
  const values = new Map();
  const storage = {
    getItem: (k) => values.get(k) ?? null,
    setItem: (k, v) => values.set(k, v),
  };
  const ram = new Uint8Array(1024);
  ram[1023] = 213;
  writeBattery(storage, "mmc6", ram);
  assert.deepEqual(readBattery(storage, "mmc6"), ram);
});
test("all 100 requested titles are accounted for and available originals have UI identities", async () => {
  const { readFile } = await import("node:fs/promises");
  const targets = JSON.parse(
    await readFile(
      new URL("../fixtures/compatibility/top100-targets.json", import.meta.url),
    ),
  );
  assert.equal(targets.length, 100);
  assert.deepEqual(
    targets.map((t) => t.id),
    Array.from({ length: 100 }, (_, i) => i + 1),
  );
  for (const t of targets) {
    if (!t.file) {
      assert.equal(t.status, "missing-original");
      continue;
    }
    const info = gameInfo(t.sha256);
    assert.ok(info, t.title);
    assert.equal(info.players, t.players);
    assert.equal(info.battery, t.battery);
  }
  assert.equal(gameInfo(targets[3].sha256).play_mode, "alternating");
  assert.equal(gameInfo(targets[56].sha256).peripheral, "zapper");
});

test("IGN NES list has exact identities and matching player modes", async () => {
  const { readFile } = await import("node:fs/promises");
  const targets = JSON.parse(await readFile(new URL("../fixtures/compatibility/ign100-targets.json", import.meta.url)));
  assert.deepEqual(targets.map(t => t.id), Array.from({length:100}, (_, i) => i+1));
  for (const t of targets.filter(t => t.file)) {
    const info = gameInfo(t.sha256);
    assert.ok(info, t.title);
    assert.equal(info.players, t.players, t.title);
    assert.equal(info.play_mode ?? "", t.play_mode ?? "", t.title);
  }
});
