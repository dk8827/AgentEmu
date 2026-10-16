import test from "node:test";
import assert from "node:assert/strict";
import { FrameClock } from "../tests/reference-js/frame-clock.mjs";
test("silent worker latency does not halve emulation speed", () => {
  for (const latency of [0, 20, 45]) {
    const c = new FrameClock();
    let busyUntil = 0,
      frames = 0;
    for (let now = 0; now <= 10000; now += 1000 / 60) {
      c.tick(now, true, false);
      if (now >= busyUntil) {
        const n = c.take();
        if (n) {
          frames += n;
          busyUntil = now + latency;
        }
      }
    }
    assert.ok(frames >= 595 && frames <= 601, `${latency}ms: ${frames}`);
  }
});
test("pause, audio-clock switching and long stalls do not create runaway debt", () => {
  const c = new FrameClock();
  c.tick(100000, true, false);
  assert.equal(c.take(), 4);
  c.tick(100001, false, false);
  assert.equal(c.take(), 0);
  c.tick(100020, true, true);
  assert.equal(c.take(), 0);
  c.reset(100020);
  c.tick(100040, true, false);
  assert.equal(c.take(1), 1);
});

test("PAL pacing uses the cartridge frame rate", () => {
  const clock = new FrameClock();
  clock.rate = 50.007;
  let frames = 0;
  for (let ms = 0; ms <= 10000; ms += 10) {
    clock.tick(ms, true, false);
    frames += clock.take();
  }
  assert.equal(frames, 500);
});
