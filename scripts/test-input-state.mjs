import { test } from "node:test";
import assert from "node:assert/strict";
import {
  TouchState,
  MenuPulses,
  directionAt,
  normalize,
} from "../tests/reference-js/input-state.mjs";
test("independent pointers preserve diagonal fire and overlapping A+B ownership", () => {
  const s = new TouchState();
  s.set(11, 144);
  s.set(22, 2);
  assert.equal(s.mask, 146);
  s.set(33, 3);
  assert.equal(s.mask, 147);
  s.release(33);
  assert.equal(s.mask, 146);
  s.release(11);
  assert.equal(s.mask, 2);
  s.clear();
  assert.equal(s.mask, 0);
});
test("eight-way direction sectors, neutral center, and slide off controller", () => {
  const r = { left: 10, top: 20, width: 140, height: 140 };
  for (const [x, y, mask] of [
    [130, 40, 144],
    [80, 30, 16],
    [20, 40, 80],
    [20, 90, 64],
    [20, 140, 96],
    [80, 150, 32],
    [140, 140, 160],
    [140, 90, 128],
    [80, 90, 0],
    [-30, 90, 0],
  ])
    assert.equal(directionAt(x, y, r), mask);
});
test("opposite directions never send contradictory NES input", () => {
  assert.equal(normalize(255), 15);
});
test("Start/Select pulses route per port, last one frame, and cancel on pause", () => {
  const p = new MenuPulses();
  p.push(0, 8);
  p.push(1, 4);
  assert.deepEqual(p.take(), [8, 4]);
  p.push(0, 8);
  assert.deepEqual(p.take(), [0, 0]);
  assert.deepEqual(p.take(), [8, 0]);
  p.push(1, 8);
  p.clear();
  assert.deepEqual(p.take(), [0, 0]);
  assert.equal(p.pending, false);
});
test("legacy Escape bindings migrate without stealing another binding", async () => {
  const { migrateReservedKeys } = await import("../tests/reference-js/input-state.mjs");
  const defaults = [
    ["KeyJ", "KeyK", "Space", "Enter", "KeyW", "KeyS", "KeyA", "KeyD"],
    [
      "Comma",
      "Period",
      "ShiftRight",
      "Backspace",
      "ArrowUp",
      "ArrowDown",
      "ArrowLeft",
      "ArrowRight",
    ],
  ];
  const saved = defaults.map((row) => [...row]);
  saved[0][1] = "Escape";
  const migrated = migrateReservedKeys(saved, defaults);
  assert.equal(migrated[0][1], "KeyK");
  assert.equal(saved[0][1], "Escape");
  assert.equal(new Set(migrated.flat()).size, 16);
});
